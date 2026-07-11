//! UNBOXED handle-op arms — the P-2a/P-2b/P-2c inline fast paths over the `UbCtx` arena:
//! list/map construction + sealing, the flat index reads, the map bucket probe, inline concat,
//! `String.length`, and the owned-handle `Pop` release. Bodies moved verbatim from the
//! pre-decomposition `emit_unboxed.rs` (M-Decomp); shared emit state arrives via [`Ec`].

use super::*;

/// `Op::MakeList(n)` — element kinds select the flavor (all-`Str` → `StrList` handle pushes,
/// all-`Int` → `IntList` raw i64 pushes, P-2c); the seal flattens eligible lists into
/// consecutive arena slots (a FLAT handle) so `Index` runs fully inline.
pub(super) fn arm_make_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    n: usize,
) -> Result<(), JitError> {
    let d = kinds.len();
    if n > d {
        return Err(JitError::Codegen("unboxed MakeList underflow".to_string()));
    }
    // Element kinds select the flavor (mirrors the analyze arm): all-`Str` →
    // `StrList` (handle pushes), all-`Int` → `IntList` (raw i64 pushes, P-2c).
    let all_str = kinds[d - n..].iter().all(|k| matches!(k, Kind::Str(_)));
    let all_int = n > 0 && kinds[d - n..].iter().all(|k| *k == Kind::Int);
    if !(all_str || all_int) {
        return Err(JitError::Unsupported(format!(
            "unboxed MakeList element kinds {:?}",
            &kinds[d - n..]
        )));
    }
    let capv = b.ins().iconst(types::I64, n as i64);
    let call = b.ins().call(h.list_new, &[ec.ctx, capv]);
    let list_h = b.inst_results(call)[0];
    // Push elements bottom-up straight from their depth-indexed Variables (no pops —
    // the kind stack is truncated once below). An OWNED element is consumed (moved).
    for j in 0..n {
        let depth_j = d - n + j;
        let ev = b.use_var(vars[depth_j]);
        let pc = if all_int {
            b.ins().call(h.list_push_int, &[ec.ctx, list_h, ev])
        } else {
            let freev = b
                .ins()
                .iconst(types::I64, kinds[depth_j].is_owned_handle() as i64);
            b.ins().call(h.list_push, &[ec.ctx, list_h, ev, freev])
        };
        let status = b.inst_results(pc)[0];
        let bad = b.ins().icmp_imm(IntCC::NotEqual, status, 0);
        ec.fault_if(b, bad, 5);
    }
    // Seal: all-short strings / all ints flatten into consecutive arena slots (a FLAT
    // handle) so `Index` runs fully inline; anything else keeps the boxed handle.
    let sc = b.ins().call(h.list_seal, &[ec.ctx, list_h]);
    let sealed = b.inst_results(sc)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sealed, 0);
    ec.fault_if(b, bad, 5);
    kinds.truncate(d - n);
    ub_push(
        b,
        vars,
        fvars,
        kinds,
        sealed,
        if all_int {
            Kind::IntList(Own::Owned)
        } else {
            Kind::StrList(Own::Owned)
        },
    )
}

/// `Op::MakeMap(n)` — validates the 2n-operand key/value tail, accumulates pairs through the
/// scratch list allocator, then seals: an all-short-key int map flattens into arena slot PAIRS
/// (a `SLOT|FLAT` handle) so lookup runs fully inline.
pub(super) fn arm_make_map(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    n: usize,
) -> Result<(), JitError> {
    let d = kinds.len();
    if 2 * n > d {
        return Err(JitError::Codegen("unboxed MakeMap underflow".to_string()));
    }
    // Validate the 2n-operand tail alternates key (Str) / value (Int) BEFORE emitting
    // (mirrors the analyze arm exactly).
    for j in 0..n {
        let (kk, vk) = (kinds[d - 2 * n + 2 * j], kinds[d - 2 * n + 2 * j + 1]);
        if !matches!(kk, Kind::Str(_)) || vk != Kind::Int {
            return Err(JitError::Unsupported(format!(
                "unboxed MakeMap pair kinds ({kk:?} => {vk:?})"
            )));
        }
    }
    // Scratch: an untagged list accumulating k1,v1,…  (reuses the list allocator).
    let capv = b.ins().iconst(types::I64, 2 * n as i64);
    let call = b.ins().call(h.list_new, &[ec.ctx, capv]);
    let map_h = b.inst_results(call)[0];
    for j in 0..n {
        let kd = d - 2 * n + 2 * j;
        let kv = b.use_var(vars[kd]);
        let vv = b.use_var(vars[kd + 1]);
        let freev = b
            .ins()
            .iconst(types::I64, kinds[kd].is_owned_handle() as i64);
        let pc = b
            .ins()
            .call(h.map_push_pair, &[ec.ctx, map_h, kv, vv, freev]);
        let status = b.inst_results(pc)[0];
        let bad = b.ins().icmp_imm(IntCC::NotEqual, status, 0);
        ec.fault_if(b, bad, 5);
    }
    // Seal: dedup through the canonical `build_map` kernel; an all-short-key int map
    // flattens into arena slot PAIRS (a `SLOT|FLAT` handle) so lookup runs fully inline.
    let sc = b.ins().call(h.map_seal, &[ec.ctx, map_h]);
    let sealed = b.inst_results(sc)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sealed, 0);
    ec.fault_if(b, bad, 5);
    kinds.truncate(d - 2 * n);
    ub_push(b, vars, fvars, kinds, sealed, Kind::StrIntMap(Own::Owned))
}

/// `Op::Index` with a `Str` key on top — P-2b string-keyed map lookup (`m[k]` → Int): the
/// inline O(1) bucket probe over a FLAT-sealed map (one canon compare decides the pair),
/// punting to the helper for canon-0 keys / non-flat maps.
pub(super) fn arm_index_map(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (mv, mk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(ik, Kind::Str(_)) || !matches!(mk, Kind::StrIntMap(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({mk:?}[{ik:?}])"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let fast_blk = b.create_block();
    let slow_blk = b.create_block();
    // INLINE iff the map sealed FLAT and the key is an arena slot; the probe needs the
    // key's CANON word, so a canon-0 slot (an inline-concat result, an unregistered
    // runtime string) also punts.
    // P-2c fused tag check — `(mv & FLAT) != 0 && (iv & SLOT) != 0` in three ops: shift the
    // map's FLAT bit (61) up onto the key's SLOT bit (62), AND the two words, mask SLOT —
    // nonzero ⇔ both tags present. (Replaces two band_imm + two icmp + a band.)
    const _: () = assert!(UB_TAG_FLAT << 1 == UB_TAG_SLOT, "fused tag shift");
    let mv_flat_up = b.ins().ishl_imm(mv, 1);
    let fused = b.ins().band(mv_flat_up, iv);
    let both = b.ins().band_imm(fused, UB_TAG_SLOT);
    b.ins().brif(both, fast_blk, &[], slow_blk, &[]);
    // INLINE: O(1) bucket walk. The key's CANON word (slot byte 32 — nonzero only when
    // assigned via the content registry, so canon equality ⇔ byte equality) indexes
    // nothing itself; the HASH picks the bucket (`hash & mask`), each bucket holds a
    // pair index (u32::MAX = empty), and ONE canon compare decides the pair. An empty
    // bucket is a genuine miss (the seal's load factor ≤ 1/2 guarantees termination) —
    // code 5, the VM redo renders the canonical `"map key not found"`. A canon-0 key
    // (inline-concat result, unregistered runtime string) punts to the helper.
    b.switch_to_block(fast_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let ki = b.ins().band_imm(iv, UB_IDX_MASK);
    let koff = b.ins().ishl_imm(ki, 6);
    let pk = b.ins().iadd(buf, koff);
    let khash = b
        .ins()
        .load(types::I64, MemFlagsData::new(), pk, UB_SLOT_HASH_OFF as i32);
    let kcanon = b.ins().load(
        types::I64,
        MemFlagsData::new(),
        pk,
        UB_SLOT_CANON_OFF as i32,
    );
    let probe_blk = b.create_block();
    b.ins().brif(kcanon, probe_blk, &[], slow_blk, &[]);
    b.switch_to_block(probe_blk);
    let cnt_raw = b.ins().ushr_imm(mv, UB_MAP_CNT_SHIFT);
    let cnt = b.ins().band_imm(cnt_raw, 0xFFF);
    let lg_raw = b.ins().ushr_imm(mv, UB_MAP_LOG_SHIFT);
    let lg = b.ins().band_imm(lg_raw, 0x1F);
    let base = b.ins().band_imm(mv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm(base, 6);
    let pbase = b.ins().iadd(buf, boff);
    let tsoff = b.ins().ishl_imm(cnt, 7); // table starts after the 2·cnt pair slots
    let tbase = b.ins().iadd(pbase, tsoff);
    let one = b.ins().iconst(types::I64, 1);
    let tsize = b.ins().ishl(one, lg);
    let mask = b.ins().iadd_imm(tsize, -1);
    let t0 = b.ins().band(khash, mask);
    let head = b.create_block();
    b.append_block_param(head, types::I64); // bucket index
    let hit = b.create_block();
    b.append_block_param(hit, types::I64); // matched pair index
    let next = b.create_block();
    b.append_block_param(next, types::I64);
    b.ins().jump(head, &[t0.into()]);
    b.switch_to_block(head);
    let t = b.block_params(head)[0];
    let btoff = b.ins().ishl_imm(t, 2);
    let baddr = b.ins().iadd(tbase, btoff);
    let e = b.ins().uload32(MemFlagsData::new(), baddr, 0);
    let empty = b.ins().icmp_imm(IntCC::Equal, e, 0xFFFF_FFFF);
    ec.fault_if(b, empty, 5); // genuine miss → canonical fault on the VM
    let poff = b.ins().ishl_imm(e, 7); // pair stride = 2 slots = 128 bytes
    let ph = b.ins().iadd(pbase, poff);
    let pcanon = b.ins().load(
        types::I64,
        MemFlagsData::new(),
        ph,
        UB_SLOT_CANON_OFF as i32,
    );
    let ceq = b.ins().icmp(IntCC::Equal, pcanon, kcanon);
    b.ins().brif(ceq, hit, &[e.into()], next, &[t.into()]);
    b.switch_to_block(hit);
    let he = b.block_params(hit)[0];
    let hoff = b.ins().ishl_imm(he, 7);
    let hph = b.ins().iadd(pbase, hoff);
    let val = b.ins().load(types::I64, MemFlagsData::new(), hph, 64);
    // Consume the key (recycle iff runtime-OWNED); the flat map is bump-pinned
    // (runtime-borrowed always) — nothing to free.
    if ik.is_owned_handle() {
        ec.slot_free_if_owned(b, iv);
    }
    b.ins().jump(merge, &[val.into()]);
    b.switch_to_block(next);
    let nt = b.block_params(next)[0];
    let t1 = b.ins().iadd_imm(nt, 1);
    let tw = b.ins().band(t1, mask);
    b.ins().jump(head, &[tw.into()]);
    // SLOW: the helper (boxed map through the canonical kernel; hash-0 / untagged keys).
    b.switch_to_block(slow_blk);
    let mask = (ik.is_owned_handle() as i64) | ((mk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let call = b.ins().call(h.map_get, &[ec.ctx, mv, iv, maskv]);
    let sval = b.inst_results(call)[0];
    let scode = b.inst_results(call)[1];
    let bad = b.ins().icmp_imm(IntCC::NotEqual, scode, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sval.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `Op::Index` with an `IntList` beneath the index — P-2c int-list element read
/// (`xs[i]` → Int, raw i64 at slot bytes 0..8): inline unsigned bounds check + ONE load
/// for a flat list; the two-return helper for a boxed one.
pub(super) fn arm_index_int_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int || !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({lk:?}[{ik:?}])"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let flat_blk = b.create_block();
    let slow_blk = b.create_block();
    let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
    // INLINE (flat int list): unsigned bounds check, then ONE load of the raw i64 at
    // `buf[(base+idx)*64]`. Out-of-range → code 5 → the canonical fault on the VM.
    b.switch_to_block(flat_blk);
    let cnt_raw = b.ins().ushr_imm(lv, 40);
    let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
    let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
    ec.fault_if(b, oob, 5);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm(lv, UB_IDX_MASK);
    let slot = b.ins().iadd(base, iv);
    let soff = b.ins().ishl_imm(slot, 6);
    let addr = b.ins().iadd(buf, soff);
    let fres = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW (boxed int list): the two-return helper (value spans the full i64 range).
    b.switch_to_block(slow_blk);
    let freev = b.ins().iconst(types::I64, lk.is_owned_handle() as i64);
    let call = b.ins().call(h.index_int, &[ec.ctx, lv, iv, freev]);
    let sval = b.inst_results(call)[0];
    let scode = b.inst_results(call)[1];
    let bad = b.ins().icmp_imm(IntCC::NotEqual, scode, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sval.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// Plain `Op::Index` — string-list element read: a flat list yields base+idx as a BORROWED
/// slot handle (zero copy, zero alloc); a boxed list goes through the helper.
pub(super) fn arm_index_str_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int || !matches!(lk, Kind::StrList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({lk:?}[{ik:?}])"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let flat_blk = b.create_block();
    let slow_blk = b.create_block();
    let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
    // INLINE (flat list): unsigned bounds check (a negative idx is a huge u64 — same
    // reject as the VM's `usize::try_from`), then base+idx is a BORROWED slot handle —
    // zero copy, zero alloc. Out-of-range → code 5 → the VM redo renders the canonical
    // "list index out of range".
    b.switch_to_block(flat_blk);
    let cnt_raw = b.ins().ushr_imm(lv, 40);
    let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
    let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
    ec.fault_if(b, oob, 5);
    let base = b.ins().band_imm(lv, UB_IDX_MASK);
    let slot = b.ins().iadd(base, iv);
    let fres = b.ins().bor_imm(slot, UB_TAG_SLOT);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW (boxed list): the helper (element clone into a slot / untagged temp).
    b.switch_to_block(slow_blk);
    let freev = b.ins().iconst(types::I64, lk.is_owned_handle() as i64);
    let call = b.ins().call(h.index, &[ec.ctx, lv, iv, freev]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Str(Own::Owned))
}

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
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
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
    let both_slot = b.ins().band_imm(both, UB_TAG_SLOT);
    b.ins().brif(both_slot, fast1, &[], slow_blk, &[]);
    // INLINE: load both lengths; a ≤22-byte result is built in a fresh slot with
    // bounded 3×8-byte over-copies (the 64-byte slot slack absorbs them — see
    // `UB_SLOT_SIZE`); the byte semantics are exactly `PhStr::concat`'s.
    b.switch_to_block(fast1);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let ia = b.ins().band_imm(av, UB_IDX_MASK);
    let aoff = b.ins().ishl_imm(ia, 6);
    let pa = b.ins().iadd(buf, aoff);
    let ib = b.ins().band_imm(bv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm(ib, 6);
    let pb = b.ins().iadd(buf, boff);
    let la = b.ins().uload8(types::I64, MemFlagsData::new(), pa, 0);
    let lb = b.ins().uload8(types::I64, MemFlagsData::new(), pb, 0);
    let tot = b.ins().iadd(la, lb);
    let big = b.ins().icmp_imm(
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
    let doff = b.ins().ishl_imm(sidx, 6);
    let pd = b.ins().iadd(buf, doff);
    b.ins().istore8(MemFlagsData::new(), tot, pd, 0);
    // Copy a → dst+1 (static offsets; over-copy is absorbed by the slot slack).
    for k in 0..3 {
        let w = b.ins().load(types::I64, MemFlagsData::new(), pa, 1 + 8 * k);
        b.ins().store(MemFlagsData::new(), w, pd, 1 + 8 * k);
    }
    // Copy b → dst+1+la (runtime offset).
    let la1 = b.ins().iadd_imm(la, 1);
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
    let fres_raw = b.ins().bor_imm(sidx, UB_TAG_SLOT);
    let fres = b.ins().bor_imm(fres_raw, UB_TAG_OWNED);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW: the helper handles every encoding + the >22-byte (heap) result.
    b.switch_to_block(slow_blk);
    let mask = (ak.is_owned_handle() as i64) | ((bk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let call = b.ins().call(h.concat, &[ec.ctx, av, bv, maskv]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
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
        let slot_bit = b.ins().band_imm(acc, UB_TAG_SLOT);
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
                let idx = b.ins().band_imm(pv, UB_IDX_MASK);
                let off = b.ins().ishl_imm(idx, 6);
                let ps = b.ins().iadd(buf, off);
                let len = b.ins().uload8(types::I64, MemFlagsData::new(), ps, 0);
                let data = b.ins().iadd_imm(ps, 1);
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
                let neg = b.ins().icmp_imm(IntCC::SignedLessThan, pv, 0);
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
                let pos1 = b.ins().iadd_imm(pos, -1);
                let ten = b.ins().iconst(types::I64, 10);
                let rem = b.ins().urem(u, ten);
                let ch = b.ins().iadd_imm(rem, b'0' as i64);
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
    let big = b.ins().icmp_imm(
        IntCC::UnsignedGreaterThan,
        tot,
        crate::phstr::INLINE_CAP as i64,
    );
    let fast2 = b.create_block();
    b.ins().brif(big, slow_blk, &[], fast2, &[]);
    b.switch_to_block(fast2);
    let sidx = ec.slot_alloc(b);
    let doff = b.ins().ishl_imm(sidx, 6);
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
    let fres_raw = b.ins().bor_imm(sidx, UB_TAG_SLOT);
    let fres = b.ins().bor_imm(fres_raw, UB_TAG_OWNED);
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
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Str(Own::Owned))
}

/// `String.length` native — INLINE for a slot operand (the length is the slot's leading
/// byte), the helper otherwise.
pub(super) fn arm_str_len(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (sv, sk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(sk, Kind::Str(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed String.length operand kind {sk:?}"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let fast_blk = b.create_block();
    let slow_blk = b.create_block();
    let slot_bit = b.ins().band_imm(sv, UB_TAG_SLOT);
    b.ins().brif(slot_bit, fast_blk, &[], slow_blk, &[]);
    // INLINE: the length is the slot's leading byte.
    b.switch_to_block(fast_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let si = b.ins().band_imm(sv, UB_IDX_MASK);
    let soff = b.ins().ishl_imm(si, 6);
    let ps = b.ins().iadd(buf, soff);
    let n = b.ins().uload8(types::I64, MemFlagsData::new(), ps, 0);
    if sk.is_owned_handle() {
        ec.slot_free_if_owned(b, sv);
    }
    b.ins().jump(merge, &[n.into()]);
    b.switch_to_block(slow_blk);
    let freev = b.ins().iconst(types::I64, sk.is_owned_handle() as i64);
    let call = b.ins().call(h.str_len, &[ec.ctx, sv, freev]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `Op::Pop` — an owned handle dies unconsumed here (a statement-expression string, a
/// loop-body temp): release it so the arena/table stay at steady state. Scalars and borrows:
/// dropping the SSA value is the whole discard. Runtime dispatch: an owned slot recycles
/// inline; an untagged temp goes through the helper; a borrowed slot or a flat list is a
/// no-op (flat handled by the helper defensively).
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_pop(
    b: &mut FunctionBuilder,
    ec: &Ec,
    ub: Option<&UbHelperRefs>,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
) -> Result<(), JitError> {
    let (v, k) = ub_pop(b, vars, fvars, kinds)?;
    if k.is_owned_handle() {
        let h = ub_ref(ub, "Pop(owned handle)")?;
        release_kinded(b, ec, h, v, k, program, info, None);
    }
    Ok(())
}

/// The FUSED runtime release dispatch for a compile-time-OWNED handle value — ONE branch on
/// the hot path: `x = v & (SLOT|OWNED)`. `x == SLOT` is a runtime-BORROWED slot (the common
/// case — a flat-list element / joined-to-Owned const dying at its release point) → nothing to
/// do. Everything else re-dispatches on the cold side: OWNED → inline recycle; untagged →
/// helper free. `OWNED ⇒ SLOT` at runtime (only arena slots carry OWNED), so the three-way is
/// behavior-identical to an owned-first two-branch ladder. Used by `Pop`, the `SetLocal`
/// handle-overwrite, and any consumer releasing a dead handle.
pub(super) fn emit_release(b: &mut FunctionBuilder, ec: &Ec, h: &UbHelperRefs, v: ClValue) {
    let x = b.ins().band_imm(v, UB_TAG_SLOT | UB_TAG_OWNED);
    let is_borrowed_slot = b.ins().icmp_imm(IntCC::Equal, x, UB_TAG_SLOT);
    let slow_blk = b.create_block();
    let cont = b.create_block();
    b.ins().brif(is_borrowed_slot, cont, &[], slow_blk, &[]);
    b.switch_to_block(slow_blk);
    let owned_bit = b.ins().band_imm(v, UB_TAG_OWNED);
    let push_blk = b.create_block();
    let helper_blk = b.create_block();
    b.ins().brif(owned_bit, push_blk, &[], helper_blk, &[]);
    b.switch_to_block(push_blk);
    ec.slot_push(b, v);
    b.ins().jump(cont, &[]);
    b.switch_to_block(helper_blk);
    b.ins().call(h.free, &[ec.ctx, v]);
    b.ins().jump(cont, &[]);
    b.switch_to_block(cont);
}
