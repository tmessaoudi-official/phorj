//! Concat emit arms — the string-concatenation inline fast paths: the `Op::Concat(2)`
//! both-slot SSO merge ([`concat_pair`]), the fused mixed-interpolation build
//! ([`concat_mix_n`], the webish vertical), and the accumulator in-place append
//! ([`concat_acc`], the strbuild vertical). Split from `verticals.rs` (M-Decomp).

use super::*;

/// `Op::Concat(2)` — the P-2a-inline SSO fast path: both-slot operands with a ≤22-byte total
/// build the result in a fresh arena slot with bounded 3×8-byte over-copies (then ZERO the
/// hash+canon metadata words the over-copies trashed — a stale canon would FALSE-MATCH in the
/// map probe, a byte-identity break); everything else goes through the helper.
pub(super) fn arm_concat(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    n: usize,
) -> Result<(), JitError> {
    // Pop the n parts (top = last), restore source order.
    let mut parts: Vec<(ClValue, Kind)> = Vec::with_capacity(n);
    for _ in 0..n {
        parts.push(ub_pop(b, vars, fvars, kinds)?);
    }
    parts.reverse();
    // The dominant pure `a + b` shape keeps the fully-INLINE fast path (the stringconcat WIN).
    if n == 2 && matches!(parts[0].1, Kind::Str(_)) && matches!(parts[1].1, Kind::Str(_)) {
        let (av, ak) = parts[0];
        let (bv, bk) = parts[1];
        let res = concat_pair(b, ec, h, av, ak, bv, bk)?;
        return ub_push(b, vars, fvars, kinds, res, Kind::Str(Own::Owned));
    }
    // Mixed interpolation / wide concat (n ≤ 6) — the webish vertical: the HOT shape (every
    // Str part slot-tagged, total ≤ 22 bytes) is built FULLY INLINE in IR; everything else
    // takes the ONE fused `rt_u_concat_mix` helper call (heap parts, >22-byte results).
    if n <= 6 {
        return concat_mix_n(b, ec, h, vars, fvars, kinds, &parts);
    }
    // n > 6 (rare): render `Int` parts via `rt_u_int_to_str`, then fold pairwise (string
    // concat is associative — byte-identical to the VM's single walk); each merge consumes
    // its operands per the ownership mask, so intermediates recycle.
    for part in parts.iter_mut() {
        match part.1 {
            Kind::Str(_) => {}
            Kind::Int => {
                let call = b.ins().call(h.int_to_str, &[ec.ctx, part.0]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sres, 0);
                ec.fault_if(b, bad, 5);
                *part = (sres, Kind::Str(Own::Owned));
            }
            other => {
                return Err(JitError::Unsupported(format!(
                    "unboxed Concat operand kind {other:?}"
                )));
            }
        }
    }
    let (mut av, mut ak) = parts[0];
    for &(bv, bk) in &parts[1..] {
        av = concat_pair(b, ec, h, av, ak, bv, bk)?;
        ak = Kind::Str(Own::Owned);
    }
    ub_push(b, vars, fvars, kinds, av, ak)
}

/// Emit ONE pairwise concat merge (the P-2a-inline fast path + helper slow path), returning
/// the merged value (an OWNED slot / helper handle). Both operands are consumed according to
/// their compile-time ownership.
pub(super) fn concat_pair(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    av: ClValue,
    ak: Kind,
    bv: ClValue,
    bk: Kind,
) -> Result<ClValue, JitError> {
    if !matches!(ak, Kind::Str(_)) || !matches!(bk, Kind::Str(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Concat operand kinds ({ak:?}, {bk:?})"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let fast1 = b.create_block();
    let slow_blk = b.create_block();
    // Fast iff BOTH operands are arena slots (a pinned short const, a flat element, or
    // an owned temp) — one AND + one branch.
    let both = b.ins().band(av, bv);
    let both_slot = b.ins().band_imm_s(both, UB_TAG_SLOT);
    b.ins().brif(both_slot, fast1, &[], slow_blk, &[]);
    // INLINE: load both lengths; a ≤22-byte result is built in a fresh slot with
    // bounded 3×8-byte over-copies (the 64-byte slot slack absorbs them — see
    // `UB_SLOT_SIZE`); the byte semantics are exactly `PhStr::concat`'s.
    b.switch_to_block(fast1);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let ia = b.ins().band_imm_s(av, UB_IDX_MASK);
    let aoff = b.ins().ishl_imm_s(ia, 6);
    let pa = b.ins().iadd(buf, aoff);
    let ib = b.ins().band_imm_s(bv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm_s(ib, 6);
    let pb = b.ins().iadd(buf, boff);
    let la = b.ins().uload8(types::I64, MemFlagsData::new(), pa, 0);
    let lb = b.ins().uload8(types::I64, MemFlagsData::new(), pb, 0);
    let tot = b.ins().iadd(la, lb);
    let big = b.ins().icmp_imm_s(
        IntCC::UnsignedGreaterThan,
        tot,
        crate::phstr::INLINE_CAP as i64,
    );
    let fast2 = b.create_block();
    b.ins().brif(big, slow_blk, &[], fast2, &[]);
    // Allocate the result slot (the shared inline free-stack-or-bump ladder — `Ec::slot_alloc`;
    // full → code 5, redo on VM — exhaustion is a fallback, never a user-visible fault).
    b.switch_to_block(fast2);
    let sidx = ec.slot_alloc(b);
    let doff = b.ins().ishl_imm_s(sidx, 6);
    let pd = b.ins().iadd(buf, doff);
    b.ins().istore8(MemFlagsData::new(), tot, pd, 0);
    // Copy a → dst+1 (static offsets; over-copy is absorbed by the slot slack).
    for k in 0..3 {
        let w = b.ins().load(types::I64, MemFlagsData::new(), pa, 1 + 8 * k);
        b.ins().store(MemFlagsData::new(), w, pd, 1 + 8 * k);
    }
    // Copy b → dst+1+la (runtime offset).
    let la1 = b.ins().iadd_imm_s(la, 1);
    let pdb = b.ins().iadd(pd, la1);
    for k in 0..3 {
        let w = b.ins().load(types::I64, MemFlagsData::new(), pb, 1 + 8 * k);
        b.ins().store(MemFlagsData::new(), w, pdb, 8 * k);
    }
    // Zero the metadata words the over-copies just trashed: hash 0 + canon 0 = "punt
    // to the helper" — without this, stale/garbage canon could FALSE-MATCH in the
    // inline map probe (a byte-identity break, not just a slow path).
    let zmeta = b.ins().iconst(types::I64, 0);
    b.ins()
        .store(MemFlagsData::new(), zmeta, pd, UB_SLOT_HASH_OFF as i32);
    b.ins()
        .store(MemFlagsData::new(), zmeta, pd, UB_SLOT_CANON_OFF as i32);
    // Consume compile-time-OWNED operands: recycle iff the runtime OWNED bit is set
    // (a flat element / pinned const is compile-Owned but runtime-borrowed → no-op).
    for (v, k) in [(av, ak), (bv, bk)] {
        if k.is_owned_handle() {
            ec.slot_free_if_owned(b, v);
        }
    }
    let fres_raw = b.ins().bor_imm_s(sidx, UB_TAG_SLOT);
    let fres = b.ins().bor_imm_s(fres_raw, UB_TAG_OWNED);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW: the helper handles every encoding + the >22-byte (heap) result.
    b.switch_to_block(slow_blk);
    let mask = (ak.is_owned_handle() as i64) | ((bk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let call = b.ins().call(h.concat, &[ec.ctx, av, bv, maskv]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    Ok(b.block_params(merge)[0])
}

/// Mixed interpolation / wide concat (`Concat(n)`, 2 ≤ n ≤ 6, Int|Str parts) — the webish
/// vertical: the hot shape (every `Str` part slot-tagged, total ≤ 22 bytes) is built FULLY
/// INLINE. Each `Int` part renders backward into a private 48-byte stack scratch — the exact
/// `as_display` decimal bytes, branchless sign (the '-' is ALWAYS stored at the byte before
/// the digits; it lands inside the piece only when the start steps back over it) — then all
/// parts join into a fresh arena slot with bounded 3×8-byte over-copies at a running cursor
/// (the slot slack absorbs them; hash+canon are zeroed after — the same "punt to the helper"
/// marker the fused helper writes, so the bytes AND the metadata are identical). Str pieces
/// over-read ≤ 24 bytes from byte 1 of their 64-byte slot; Int pieces over-read within their
/// 48-byte scratch (digits at [1..21], sign can reach 0, max read offset 44). Any untagged
/// (heap) part or a >22-byte total falls to the ONE fused `rt_u_concat_mix` call — the prior
/// path, byte-identical by construction.
pub(super) fn concat_mix_n(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    parts: &[(ClValue, Kind)],
) -> Result<(), JitError> {
    let n = parts.len();
    debug_assert!((2..=6).contains(&n));
    let mut kmask: i64 = 0;
    let mut fmask: i64 = 0;
    for (j, (_, k)) in parts.iter().enumerate() {
        match k {
            Kind::Int => kmask |= 1 << j,
            Kind::Str(_) => {
                if k.is_owned_handle() {
                    fmask |= 1 << j;
                }
            }
            other => {
                return Err(JitError::Unsupported(format!(
                    "unboxed Concat operand kind {other:?}"
                )));
            }
        }
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let fast0 = b.create_block();
    let slow_blk = b.create_block();
    // Fast iff every Str part is an arena slot: AND the handles (the SLOT bit survives iff
    // all have it), one branch. An all-Int mix has no handle to test — unconditionally fast.
    let str_vals: Vec<ClValue> = parts
        .iter()
        .filter(|(_, k)| matches!(k, Kind::Str(_)))
        .map(|(v, _)| *v)
        .collect();
    if let Some((&first, rest)) = str_vals.split_first() {
        let mut acc = first;
        for &v in rest {
            acc = b.ins().band(acc, v);
        }
        let slot_bit = b.ins().band_imm_s(acc, UB_TAG_SLOT);
        b.ins().brif(slot_bit, fast0, &[], slow_blk, &[]);
    } else {
        b.ins().jump(fast0, &[]);
    }
    b.switch_to_block(fast0);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    // Per part: (piece_ptr, piece_len) as runtime values, source order.
    let mut pieces: Vec<(ClValue, ClValue)> = Vec::with_capacity(n);
    for &(pv, pk) in parts {
        match pk {
            Kind::Str(_) => {
                let idx = b.ins().band_imm_s(pv, UB_IDX_MASK);
                let off = b.ins().ishl_imm_s(idx, 6);
                let ps = b.ins().iadd(buf, off);
                let len = b.ins().uload8(types::I64, MemFlagsData::new(), ps, 0);
                let data = b.ins().iadd_imm_s(ps, 1);
                pieces.push((data, len));
            }
            Kind::Int => {
                let ss = b.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    48,
                    3,
                ));
                let base = b.ins().stack_addr(types::I64, ss, 0);
                // |v| as unsigned (`ineg(i64::MIN)` == i64::MIN == 2^63 unsigned — correct).
                let neg = b.ins().icmp_imm_s(IntCC::SignedLessThan, pv, 0);
                let nv = b.ins().ineg(pv);
                let u0 = b.ins().select(neg, nv, pv);
                let loop_hdr = b.create_block();
                b.append_block_param(loop_hdr, types::I64); // u (remaining digits)
                b.append_block_param(loop_hdr, types::I64); // pos (next write + 1)
                let after = b.create_block();
                b.append_block_param(after, types::I64); // pos of the first digit
                let pos0 = b.ins().iconst(types::I64, 21);
                b.ins().jump(loop_hdr, &[u0.into(), pos0.into()]);
                b.switch_to_block(loop_hdr);
                let u = b.block_params(loop_hdr)[0];
                let pos = b.block_params(loop_hdr)[1];
                let pos1 = b.ins().iadd_imm_s(pos, -1);
                let ten = b.ins().iconst(types::I64, 10);
                let rem = b.ins().urem(u, ten);
                let ch = b.ins().iadd_imm_s(rem, b'0' as i64);
                let addr = b.ins().iadd(base, pos1);
                b.ins().istore8(MemFlagsData::new(), ch, addr, 0);
                let u1 = b.ins().udiv(u, ten);
                b.ins().brif(
                    u1,
                    loop_hdr,
                    &[u1.into(), pos1.into()],
                    after,
                    &[pos1.into()],
                );
                b.switch_to_block(after);
                let posd = b.block_params(after)[0];
                // Branchless sign: ALWAYS store '-' at posd-1 (a scratch slack byte when
                // non-negative), then step the piece start back by neg (0/1).
                let minus = b.ins().iconst(types::I64, b'-' as i64);
                let addr_m = b.ins().iadd(base, posd);
                b.ins().istore8(MemFlagsData::new(), minus, addr_m, -1);
                let negw = b.ins().uextend(types::I64, neg);
                let pos2 = b.ins().isub(posd, negw);
                let ptr = b.ins().iadd(base, pos2);
                let cap21 = b.ins().iconst(types::I64, 21);
                let len = b.ins().isub(cap21, pos2);
                pieces.push((ptr, len));
            }
            _ => unreachable!("validated above"),
        }
    }
    let mut tot = pieces[0].1;
    for &(_, l) in &pieces[1..] {
        tot = b.ins().iadd(tot, l);
    }
    let big = b.ins().icmp_imm_s(
        IntCC::UnsignedGreaterThan,
        tot,
        crate::phstr::INLINE_CAP as i64,
    );
    let fast2 = b.create_block();
    b.ins().brif(big, slow_blk, &[], fast2, &[]);
    b.switch_to_block(fast2);
    let sidx = ec.slot_alloc(b);
    let doff = b.ins().ishl_imm_s(sidx, 6);
    let pd = b.ins().iadd(buf, doff);
    b.ins().istore8(MemFlagsData::new(), tot, pd, 0);
    // Bounded copies at a running cursor (dst byte 1 onward; each piece ≤ 22 → 3×8 covers it).
    let mut cur = b.ins().iconst(types::I64, 1);
    for &(ptr, len) in &pieces {
        let dst = b.ins().iadd(pd, cur);
        for k in 0..3 {
            let w = b.ins().load(types::I64, MemFlagsData::new(), ptr, 8 * k);
            b.ins().store(MemFlagsData::new(), w, dst, 8 * k);
        }
        cur = b.ins().iadd(cur, len);
    }
    // Zero the metadata words the over-copies trashed: hash 0 + canon 0 = "punt to the
    // helper" — identical to the fused helper's `alloc_slot_bytes(joined, 0, 0)`.
    let zmeta = b.ins().iconst(types::I64, 0);
    b.ins()
        .store(MemFlagsData::new(), zmeta, pd, UB_SLOT_HASH_OFF as i32);
    b.ins()
        .store(MemFlagsData::new(), zmeta, pd, UB_SLOT_CANON_OFF as i32);
    // Consume compile-time-OWNED Str parts (runtime OWNED bit gates the recycle).
    for &(v, k) in parts {
        if matches!(k, Kind::Str(_)) && k.is_owned_handle() {
            ec.slot_free_if_owned(b, v);
        }
    }
    let fres_raw = b.ins().bor_imm_s(sidx, UB_TAG_SLOT);
    let fres = b.ins().bor_imm_s(fres_raw, UB_TAG_OWNED);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW: the fused helper — heap parts, >22-byte results (byte-identical join).
    b.switch_to_block(slow_blk);
    let nv = b.ins().iconst(types::I64, n as i64);
    let kv = b.ins().iconst(types::I64, kmask);
    let fv = b.ins().iconst(types::I64, fmask);
    let zero = b.ins().iconst(types::I64, 0);
    let mut args: Vec<ClValue> = vec![ec.ctx, nv, kv, fv];
    args.extend(parts.iter().map(|(v, _)| *v));
    args.extend(std::iter::repeat_n(zero, 6 - n));
    let call = b.ins().call(h.concat_mix, &args);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Str(Own::Owned))
}

/// FUSED-ACCUMULATOR concat (`s = s + x`, the strbuild vertical) — emitted ONLY at a proven
/// `accumulator_site` (lhs = the untouched borrow of the slot the following `SetLocal`
/// rewrites, treated as CONSUMED). The hot shape — lhs already an ACC record, rhs an arena
/// slot piece — appends FULLY INLINE: load `{ptr,len,cap}` from the record (at
/// `acc_base + idx·24`, header offset 40), cap-check, one bounded 3×8-byte copy at `ptr+len`
/// (the buffer's `UB_ACC_SLACK` absorbs the over-write), store the new len — no call, php's
/// smart_str append. Everything else (first append after entry/reset, capacity growth,
/// non-slot rhs) takes the ONE `rt_u_acc_append` helper call, which converts/grows and
/// returns the ACC handle the inline path then carries. Returns the result handle (the lhs
/// ACC handle on the fast path).
pub(super) fn concat_acc(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    av: ClValue,
    bv: ClValue,
    bk: Kind,
) -> Result<ClValue, JitError> {
    if !matches!(bk, Kind::Str(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed accumulator rhs kind {bk:?}"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let chk_rhs = b.create_block();
    let fast0 = b.create_block();
    let slow_blk = b.create_block();
    let acc_bit = b.ins().band_imm_s(av, UB_TAG_ACC);
    b.ins().brif(acc_bit, chk_rhs, &[], slow_blk, &[]);
    b.switch_to_block(chk_rhs);
    let slot_bit = b.ins().band_imm_s(bv, UB_TAG_SLOT);
    b.ins().brif(slot_bit, fast0, &[], slow_blk, &[]);
    // INLINE: cap-checked in-place append of a slot piece (≤ 22 bytes, one 3×8 copy).
    b.switch_to_block(fast0);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let idx = b.ins().band_imm_s(av, UB_IDX_MASK);
    let roff = b.ins().imul_imm_s(idx, 24);
    let prec = b.ins().iadd(abase, roff);
    let len = b.ins().load(types::I64, MemFlagsData::new(), prec, 8);
    let cap = b.ins().load(types::I64, MemFlagsData::new(), prec, 16);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let bidx = b.ins().band_imm_s(bv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm_s(bidx, 6);
    let pb = b.ins().iadd(buf, boff);
    let rl = b.ins().uload8(types::I64, MemFlagsData::new(), pb, 0);
    let nl = b.ins().iadd(len, rl);
    let too_big = b.ins().icmp(IntCC::UnsignedGreaterThan, nl, cap);
    let fast2 = b.create_block();
    b.ins().brif(too_big, slow_blk, &[], fast2, &[]);
    b.switch_to_block(fast2);
    let ptr = b.ins().load(types::I64, MemFlagsData::new(), prec, 0);
    let dst = b.ins().iadd(ptr, len);
    for k in 0..3 {
        let w = b.ins().load(types::I64, MemFlagsData::new(), pb, 1 + 8 * k);
        b.ins().store(MemFlagsData::new(), w, dst, 8 * k);
    }
    b.ins().store(MemFlagsData::new(), nl, prec, 8);
    // Consume a compile-time-OWNED rhs (runtime OWNED bit gates the recycle).
    if bk.is_owned_handle() {
        ec.slot_free_if_owned(b, bv);
    }
    b.ins().jump(merge, &[av.into()]);
    // SLOW: convert/grow through the helper (lhs ALWAYS consumed at an accumulator site).
    b.switch_to_block(slow_blk);
    let mask = 1_i64 | ((bk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let call = b.ins().call(h.acc_append, &[ec.ctx, av, bv, maskv]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    Ok(b.block_params(merge)[0])
}
