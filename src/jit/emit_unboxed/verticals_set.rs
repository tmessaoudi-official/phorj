//! Set-OP emit arms (the DEC-332 setdifference/setunion flips): `Set.union` / `Set.difference`
//! probe the set-op memo lines INLINE (a sealed flat set is immutable + bump-pinned, so a
//! memoized result is exact) and only enter the `rt_u_set_*` helper on a miss / non-flat
//! operand; `Set.size` reads the flat handle's count bits inline; `Index` over a
//! [`Kind::SetList`] is the flat int-list load plus a FLAT_SET tag guard on the loaded word.
//! Siblings of `verticals_map.rs` (Invariant 13).

use super::*;

/// `Op::CallNative(Core.Set.{difference,union}, 2)` — FLAT × FLAT probes the op's direct-mapped
/// memo lines (entries 24..32 difference / 32..40 union: `{a, b, handle}`, Fibonacci-mixed pair
/// index — MUST mirror `sets_ext::memo_setop_install` bit-for-bit) and calls the helper on a
/// miss. A helper `-1` (boxed operand / arena guard) → code 5, the byte-identical VM redo.
pub(super) fn arm_set_op(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    is_union: bool,
) -> Result<(), JitError> {
    let (bv, bk) = ub_pop(b, vars, fvars, kinds)?;
    let (av, ak) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(ak, Kind::IntSet(_)) || !matches!(bk, Kind::IntSet(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Set op operand kinds ({ak:?}, {bk:?})"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let chk1 = b.create_block();
    let chk2 = b.create_block();
    let slow = b.create_block();
    let memo = b.ins().load(types::I64, ec.stable, ec.ctx, 48);
    let bs = b.ins().ishl_imm_s(bv, 1);
    let x = b.ins().bxor(av, bs);
    let mixed = b.ins().imul_imm_s(x, UB_SET_HASH_MULT);
    let ei = b.ins().ushr_imm_s(mixed, 61);
    let eoff = b.ins().imul_imm_s(ei, 24);
    let ebase = b
        .ins()
        .iadd_imm_s(eoff, if is_union { 32 * 24 } else { 24 * 24 });
    let eaddr = b.ins().iadd(memo, ebase);
    let m0 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 0);
    let aeq = b.ins().icmp(IntCC::Equal, m0, av);
    b.ins().brif(aeq, chk1, &[], slow, &[]);
    b.switch_to_block(chk1);
    let m1 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 8);
    let beq = b.ins().icmp(IntCC::Equal, m1, bv);
    b.ins().brif(beq, chk2, &[], slow, &[]);
    b.switch_to_block(chk2);
    let hh = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 16);
    b.ins().brif(hh, merge, &[hh.into()], slow, &[]);
    b.switch_to_block(slow);
    // Flat operands' releases no-op; boxed operands never reach the sealed result leg (the
    // helper returns -1 → VM redo), so the mask is symmetry-only.
    let mask = (ak.is_owned_handle() as i64) | ((bk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let helper = if is_union { h.set_union } else { h.set_diff };
    let call = b.ins().call(helper, &[ec.ctx, av, bv, maskv]);
    let r = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[r.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::IntSet(Own::Owned))
}

/// `Op::CallNative(Core.Set.size, 1)` — FLAT_SET: the handle's 12-bit count field (two ops);
/// anything else → code 5 (the VM redo renders the boxed count).
pub(super) fn arm_set_size(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (sv, sk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(sk, Kind::IntSet(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Set.size receiver kind {sk:?}"
        )));
    }
    let tag = b.ins().band_imm_s(sv, UB_TAG_FLAT_SET);
    let not_flat = b.ins().icmp_imm_s(IntCC::NotEqual, tag, UB_TAG_FLAT_SET);
    ec.fault_if(b, not_flat, 5);
    let cnt_raw = b.ins().ushr_imm_s(sv, UB_MAP_CNT_SHIFT);
    let cnt = b.ins().band_imm_s(cnt_raw, 0xFFF);
    // The receiver was a QUERY; a flat set's release is a no-op (kind gate mirrors siblings).
    if sk.is_owned_handle() {
        emit_release(b, ec, h, sv);
    }
    ub_push(b, vars, fvars, kinds, cnt, Kind::Int)
}

/// `Op::Index` over a [`Kind::SetList`] — the flat int-list load (bounds check + one load of
/// the SET handle word) plus a FLAT_SET tag guard on the loaded word: a boxed set element
/// (possible only via the seal's fallback) is code 5, the byte-identical VM redo — which keeps
/// the OWNED push aliasing-safe (flat set releases no-op). Mirrors `arm_index_map_list`.
pub(super) fn arm_index_set_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    proven: bool,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int || !matches!(lk, Kind::SetList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({lk:?}[{ik:?}])"
        )));
    }
    let flat_bit = b.ins().band_imm_s(lv, UB_TAG_FLAT);
    let not_flat = b.ins().icmp_imm_s(IntCC::Equal, flat_bit, 0);
    ec.fault_if(b, not_flat, 5); // boxed set-list scratch → VM redo
    if !proven {
        let cnt_raw = b.ins().ushr_imm_s(lv, 40);
        let cnt = b.ins().band_imm_s(cnt_raw, 0xFFFFF);
        let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
        ec.fault_if(b, oob, 5);
    }
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let slot = b.ins().iadd(base, iv);
    let soff = b.ins().ishl_imm_s(slot, 6);
    let addr = b.ins().iadd(buf, soff);
    let word = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    let wtag = b.ins().band_imm_s(word, UB_TAG_FLAT_SET);
    let wbad = b.ins().icmp_imm_s(IntCC::NotEqual, wtag, UB_TAG_FLAT_SET);
    ec.fault_if(b, wbad, 5);
    ub_push(b, vars, fvars, kinds, word, Kind::IntSet(Own::Owned))
}

/// `Core.Set.of(List<int>)` — the FORK-D setcontains vertical's producer. SEALS a FRESH OWNED flat
/// int-list into an int-keyed packed OPEN-ADDRESSED hash table (via [`rt_u_set_seal`]) so
/// `Set.contains` probes in O(1) — the prior linear-scan vertical could not beat php's C `in_array`.
/// Returns a [`UB_TAG_FLAT_SET`] handle (base → the bucket table). Owned-ONLY input is the
/// double-free gate (a Borrowed copy would alias its source local); `analyze` enforces the same, so
/// this reject is defensive. A `-1` from the helper (non-int element, too-large, arena exhaustion) →
/// code 5, the WHOLE call redoes on the VM (which builds a real `Value::Set` — byte-identical).
pub(super) fn arm_set_of(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (v, k) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(k, Kind::IntList(Own::Owned)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Set.of operand kind {k:?} (only a fresh Owned IntList)"
        )));
    }
    let call = b.ins().call(h.set_seal, &[ec.ctx, v]);
    let sealed = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sealed, 0);
    ec.fault_if(b, bad, 5);
    ub_push(b, vars, fvars, kinds, sealed, Kind::IntSet(Own::Owned))
}

/// `Core.Set.contains(Set<int>, int)` — the FORK-D setcontains vertical: an INLINE O(1) probe of the
/// int-keyed packed hash table built by [`rt_u_set_seal`]. Mirrors [`arm_maphas`] but keyed by a raw
/// `i64` with a SEPARATE occupancy word (an int key of 0 is a valid member, unlike a map's never-0
/// canon, so the probe tests OCCUPANCY FIRST — an empty bucket's zero key-word must never false-hit a
/// needle of 0). A HIT pushes `1`; an empty bucket pushes `0` (a CLEAN false — a membership test never
/// faults). The set is a QUERY — NOT consumed (mirrors `Map.has`); the needle is a raw scalar, so
/// there is nothing to free (simpler than the map probe). A non-flat (boxed / seal-failed) set is
/// undecidable here → code 5, the whole call redoes on the VM byte-identically.
///
/// SAFETY: the flat handle's table lives in `[base, base + tslots)` arena slots, written whole by the
/// seal and never mutated after; the probe reads only `{occupied, key}` at `tbase + t·16` for
/// `t < tsize`, in bounds by the same cap check that bounded the seal's writes — no new `unsafe` here
/// (the arena-buffer base load is the identical `ec.stable` pattern the map probe uses).
pub(super) fn arm_setcontains(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (needle, nk) = ub_pop(b, vars, fvars, kinds)?;
    let (sv, sk) = ub_pop(b, vars, fvars, kinds)?;
    if nk != Kind::Int || !matches!(sk, Kind::IntSet(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Set.contains operand kinds ({sk:?}, {nk:?})"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64); // 1 = present, 0 = absent
    let flat_blk = b.create_block();
    let slow_blk = b.create_block();
    // FLAT-sealed set → inline probe; a boxed (seal-failed) set → helper-less code-5 redo.
    let flat_bit = b.ins().band_imm_s(sv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
    // FAST: O(1) packed-bucket probe. `base` (low 40) is the table start; `log2` (bits 52..57) sizes
    // it. fibonacci high bits pick the first bucket; the walk terminates on an empty bucket (load
    // factor ≤ 1/2 guarantees one exists). Each 16-byte bucket is `{occupied: u64, key: i64}`.
    b.switch_to_block(flat_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm_s(sv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm_s(base, 6);
    let tbase = b.ins().iadd(buf, boff);
    let lg_raw = b.ins().ushr_imm_s(sv, UB_MAP_LOG_SHIFT);
    let lg = b.ins().band_imm_s(lg_raw, 0x1F);
    let one = b.ins().iconst(types::I64, 1);
    let tsize = b.ins().ishl(one, lg);
    let mask = b.ins().iadd_imm_s(tsize, -1);
    // fibonacci high bits: t0 = (needle · MULT) >>u (64 − log2). Identical bits to the seal's build
    // (i64 imul low-64 == u64 wrapping_mul; ushr == logical shift).
    let hprod = b.ins().imul_imm_s(needle, UB_SET_HASH_MULT);
    let c64 = b.ins().iconst(types::I64, 64);
    let sh = b.ins().isub(c64, lg); // 64 − log2
    let t0 = b.ins().ushr(hprod, sh);
    let head = b.create_block();
    b.append_block_param(head, types::I64); // bucket index
    let cont = b.create_block();
    let step = b.create_block();
    b.ins().jump(head, &[t0.into()]);
    b.switch_to_block(head);
    let t = b.block_params(head)[0];
    let btoff = b.ins().ishl_imm_s(t, 4);
    let baddr = b.ins().iadd(tbase, btoff);
    // OCCUPANCY FIRST: an empty bucket (occupied 0) is a genuine ABSENT (clean false, never a fault),
    // and its key word is 0 — testing occupancy before the key is what stops a needle of 0
    // false-hitting an empty bucket.
    let occ = b.ins().load(types::I64, MemFlagsData::new(), baddr, 0);
    let zerov = b.ins().iconst(types::I64, 0);
    b.ins().brif(occ, cont, &[], merge, &[zerov.into()]);
    b.switch_to_block(cont);
    let key = b.ins().load(types::I64, MemFlagsData::new(), baddr, 8);
    let eq = b.ins().icmp(IntCC::Equal, key, needle);
    let onev = b.ins().iconst(types::I64, 1);
    b.ins().brif(eq, merge, &[onev.into()], step, &[]);
    b.switch_to_block(step);
    let t1 = b.ins().iadd_imm_s(t, 1);
    let tw = b.ins().band(t1, mask);
    b.ins().jump(head, &[tw.into()]);
    // SLOW: UNREACHABLE BY CONSTRUCTION — kept as a defensive terminator. `rt_u_set_seal` returns
    // either a FLAT-tagged handle or `-1` (which faults at `Set.of`, so the seal never yields a boxed
    // set), and `Kind::IntSet` is produced ONLY by `arm_set_of` (borrowed copies keep the same word),
    // so every live IntSet handle has the FLAT bit and `flat_bit` always takes `flat_blk`. If a future
    // change ever routed a non-flat set here, this code-5 redoes the whole call on the VM byte-identically.
    b.switch_to_block(slow_blk);
    let always = b.ins().iconst(types::I64, 1);
    ec.fault_if(b, always, 5);
    let unreach = b.ins().iconst(types::I64, 0);
    b.ins().jump(merge, &[unreach.into()]); // unreachable terminator (fault_if diverges)
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Bool)
}
