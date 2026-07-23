//! Map MATERIALIZATION emit arms (the DEC-332 mapkeys/mapvalues/mapmerge/mapsize flips):
//! `Map.keys` / `Map.values` / `Map.merge` probe the JIT-visible memo table INLINE (a sealed
//! flat map is immutable + bump-pinned, so a memoized result is exact) and only enter the
//! `rt_u_map_*` helper on a miss / non-flat receiver; `Map.size` reads the flat handle's count
//! bits (or the builder record's count word) inline; `Index` over a [`Kind::MapList`] is the
//! flat int-list load plus a FLAT-map tag guard on the loaded word. Siblings of `verticals.rs`
//! (Invariant 13).

use super::*;

/// Inline direct-mapped memo probe for a 1-arg materialization (`Map.keys` result word at entry
/// offset 8, `Map.values` at 16 — entries 0..8 of the memo table are `{map, keys_h, values_h}`).
/// FLAT receiver + entry hit → the memoized handle (~10 ops, no call); miss / non-flat →
/// `helper(ctx, map, free_owned)`, `-1` → code 5 (VM redo).
fn emit_map_memo1(
    b: &mut FunctionBuilder,
    ec: &Ec,
    helper: FuncRef,
    mv: ClValue,
    mk: Kind,
    off: i32,
) -> ClValue {
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let probe = b.create_block();
    let chk = b.create_block();
    let slow = b.create_block();
    let tag = b.ins().band_imm_s(mv, UB_TAG_FLAT_MAP);
    let is_flat = b.ins().icmp_imm_s(IntCC::Equal, tag, UB_TAG_FLAT_MAP);
    b.ins().brif(is_flat, probe, &[], slow, &[]);
    b.switch_to_block(probe);
    let memo = b.ins().load(types::I64, ec.stable, ec.ctx, 48);
    // Fibonacci top-3-bits index — MUST mirror `maps_ext::memo_slot` bit-for-bit.
    let mixed = b.ins().imul_imm_s(mv, UB_SET_HASH_MULT);
    let ei = b.ins().ushr_imm_s(mixed, 61);
    let eoff = b.ins().imul_imm_s(ei, 24);
    let eaddr = b.ins().iadd(memo, eoff);
    let m0 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 0);
    let meq = b.ins().icmp(IntCC::Equal, m0, mv);
    b.ins().brif(meq, chk, &[], slow, &[]);
    b.switch_to_block(chk);
    let hh = b.ins().load(types::I64, MemFlagsData::new(), eaddr, off);
    // A zero result word = "not built yet" (or evicted twin) — build through the helper.
    b.ins().brif(hh, merge, &[hh.into()], slow, &[]);
    b.switch_to_block(slow);
    // The helper frees an OWNED receiver only on ITS legs; the memo-hit leg is flat-only,
    // where a release is a no-op by construction — skipping it is exact.
    let freev = b.ins().iconst(types::I64, mk.is_owned_handle() as i64);
    let call = b.ins().call(helper, &[ec.ctx, mv, freev]);
    let r = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[r.into()]);
    b.switch_to_block(merge);
    b.block_params(merge)[0]
}

/// `Op::CallNative(Core.Map.keys, 1)` — a `StrList` of the map's keys (memoized SHARED record
/// of borrowed key-slot handles on the flat path; canonical boxed clone otherwise).
pub(super) fn arm_map_keys(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (mv, mk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(mk, Kind::StrIntMap(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Map.keys receiver kind {mk:?}"
        )));
    }
    let res = emit_map_memo1(b, ec, h.map_keys, mv, mk, 8);
    ub_push(b, vars, fvars, kinds, res, Kind::StrList(Own::Owned))
}

/// `Op::CallNative(Core.Map.values, 1)` — the int twin: an `IntList` of the map's values.
pub(super) fn arm_map_values(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (mv, mk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(mk, Kind::StrIntMap(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Map.values receiver kind {mk:?}"
        )));
    }
    let res = emit_map_memo1(b, ec, h.map_values, mv, mk, 16);
    ub_push(b, vars, fvars, kinds, res, Kind::IntList(Own::Owned))
}

/// `Op::CallNative(Core.Map.merge, 2)` — FLAT × FLAT probes the pair memo (entries 8..16:
/// `{a, b, merged_h}`) inline; a miss / boxed operand builds through `rt_u_map_merge` (the
/// canonical kernel order: `a`'s positions, `b`'s values win, new keys append).
pub(super) fn arm_map_merge(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (bv, bk) = ub_pop(b, vars, fvars, kinds)?;
    let (av, ak) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(ak, Kind::StrIntMap(_)) || !matches!(bk, Kind::StrIntMap(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Map.merge operand kinds ({ak:?}, {bk:?})"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let chk_b = b.create_block();
    let probe = b.create_block();
    let chk_h = b.create_block();
    let slow = b.create_block();
    let ta = b.ins().band_imm_s(av, UB_TAG_FLAT_MAP);
    let fa = b.ins().icmp_imm_s(IntCC::Equal, ta, UB_TAG_FLAT_MAP);
    b.ins().brif(fa, chk_b, &[], slow, &[]);
    b.switch_to_block(chk_b);
    let tb = b.ins().band_imm_s(bv, UB_TAG_FLAT_MAP);
    let fb = b.ins().icmp_imm_s(IntCC::Equal, tb, UB_TAG_FLAT_MAP);
    b.ins().brif(fb, probe, &[], slow, &[]);
    b.switch_to_block(probe);
    let memo = b.ins().load(types::I64, ec.stable, ec.ctx, 48);
    // Fibonacci top-3-bits pair index — MUST mirror `maps_ext::merge_slot` bit-for-bit.
    let bsh = b.ins().ishl_imm_s(bv, 1);
    let x = b.ins().bxor(av, bsh);
    let mixed = b.ins().imul_imm_s(x, UB_SET_HASH_MULT);
    let ei = b.ins().ushr_imm_s(mixed, 61);
    let eoff = b.ins().imul_imm_s(ei, 24);
    let ebase = b.ins().iadd_imm_s(eoff, 192); // entries 8..16
    let eaddr = b.ins().iadd(memo, ebase);
    let m0 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 0);
    let aeq = b.ins().icmp(IntCC::Equal, m0, av);
    let chk_b2 = b.create_block();
    b.ins().brif(aeq, chk_b2, &[], slow, &[]);
    b.switch_to_block(chk_b2);
    let m1 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 8);
    let beq = b.ins().icmp(IntCC::Equal, m1, bv);
    b.ins().brif(beq, chk_h, &[], slow, &[]);
    b.switch_to_block(chk_h);
    let hh = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 16);
    b.ins().brif(hh, merge, &[hh.into()], slow, &[]);
    b.switch_to_block(slow);
    let mask = (ak.is_owned_handle() as i64) | ((bk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let call = b.ins().call(h.map_merge, &[ec.ctx, av, bv, maskv]);
    let r = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[r.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::StrIntMap(Own::Owned))
}

/// Which Map higher-order op [`arm_map_hof`] is emitting: `Map` transforms every value (all
/// entries survive), `Filter` keeps entries whose 0/1 predicate result is nonzero.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum MapHof {
    Map,
    Filter,
}

/// `Map.map` / `Map.filter` with a STATIC lambda (the mapmap/mapfilter flips): an inline walk
/// over the FLAT receiver's pair slots — one value load + one DIRECT call per entry — building
/// an AMB record (`rt_u_map_ext_new`/`_push`: canon+hash read straight off the parent's
/// bump-pinned key slots, insertion order preserved). The result is a fresh RECYCLABLE record
/// (released by its consumer — a hot loop never seals, so the arena cannot exhaust) that every
/// AMB consumer already understands (`Map.size` inline, `m[k]` probe, `m[k] = v` builder-set,
/// `Map.values` rank walk). A non-FLAT receiver (boxed / AMB / long-key seal fallback) is
/// code 5 BEFORE any calls — the byte-identical VM redo runs the canonical higher-order native
/// (sound unconditionally: the unboxed graph admits pure ops only).
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_map_hof(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    fn_refs: &[Option<FuncRef>],
    ctx: ClValue,
    depth: ClValue,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    info: &UbGraphInfo,
    hof: MapHof,
) -> Result<(), JitError> {
    let (fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    let (f, has_cap) = match fk {
        Kind::Fn(f) => (f, false),
        Kind::FnCap1(f) => (f, true),
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed Map HOF callee kind {other:?}"
            )))
        }
    };
    let (mv, mk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(mk, Kind::StrIntMap(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Map HOF receiver kind {mk:?}"
        )));
    }
    // FLAT-only, checked BEFORE any lambda call (a mid-loop deopt would be sound — pure graph —
    // but a pre-loop check keeps the fallback a single branch).
    let tag = b.ins().band_imm_s(mv, UB_TAG_FLAT_MAP);
    let not_flat = b.ins().icmp_imm_s(IntCC::NotEqual, tag, UB_TAG_FLAT_MAP);
    ec.fault_if(b, not_flat, 5);
    let cnt_raw = b.ins().ushr_imm_s(mv, UB_MAP_CNT_SHIFT);
    let count = b.ins().band_imm_s(cnt_raw, 0xFFF);
    let base = b.ins().band_imm_s(mv, UB_IDX_MASK);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let call = b.ins().call(h.map_ext_new, &[ec.ctx, count]);
    let amb = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, amb, 0);
    ec.fault_if(b, bad, 5);
    // The loop: pair index `i` walks 0..count; the AMB handle is loop-invariant (the record
    // buffer may grow inside `_push` — the ptr refresh lives in the record table, not here).
    let header = b.create_block();
    b.append_block_param(header, types::I64); // i
    let bodyb = b.create_block();
    let exitb = b.create_block();
    let zero = b.ins().iconst(types::I64, 0);
    b.ins().jump(header, &[zero.into()]);
    b.switch_to_block(header);
    let i = b.block_params(header)[0];
    let more = b.ins().icmp(IntCC::UnsignedLessThan, i, count);
    b.ins().brif(more, bodyb, &[], exitb, &[]);
    b.switch_to_block(bodyb);
    // kslot = base + 2i; value = raw i64 at slot (kslot + 1), bytes 0..8.
    let i2 = b.ins().ishl_imm_s(i, 1);
    let kslot = b.ins().iadd(base, i2);
    let vslot = b.ins().iadd_imm_s(kslot, 1);
    let voff = b.ins().ishl_imm_s(vslot, 6);
    let vaddr = b.ins().iadd(buf, voff);
    let val = b.ins().load(types::I64, MemFlagsData::new(), vaddr, 0);
    let cargs = if has_cap { vec![fv, val] } else { vec![val] };
    emit_call_to(
        b,
        ec,
        fn_refs,
        ctx,
        depth,
        vars,
        fvars,
        kinds,
        f,
        cargs,
        info.ret_of(f),
        None,
    )?;
    let (rv, _rk) = ub_pop(b, vars, fvars, kinds)?;
    let next = b.create_block();
    match hof {
        MapHof::Map => {
            // Every entry survives with the transformed value.
            let pc = b.ins().call(h.map_ext_push, &[ec.ctx, amb, kslot, rv]);
            let pr = b.inst_results(pc)[0];
            let pbad = b.ins().icmp_imm_s(IntCC::SignedLessThan, pr, 0);
            ec.fault_if(b, pbad, 5);
            b.ins().jump(next, &[]);
        }
        MapHof::Filter => {
            // Keep the ORIGINAL value iff the 0/1 predicate word is nonzero.
            let keep_blk = b.create_block();
            b.ins().brif(rv, keep_blk, &[], next, &[]);
            b.switch_to_block(keep_blk);
            let pc = b.ins().call(h.map_ext_push, &[ec.ctx, amb, kslot, val]);
            let pr = b.inst_results(pc)[0];
            let pbad = b.ins().icmp_imm_s(IntCC::SignedLessThan, pr, 0);
            ec.fault_if(b, pbad, 5);
            b.ins().jump(next, &[]);
        }
    }
    b.switch_to_block(next);
    let i1 = b.ins().iadd_imm_s(i, 1);
    b.ins().jump(header, &[i1.into()]);
    b.switch_to_block(exitb);
    // The receiver was a QUERY; a flat map's release is a no-op, but the kind gate mirrors the
    // sibling arms (a future Owned boxed receiver would recycle correctly).
    if mk.is_owned_handle() {
        emit_release(b, ec, h, mv);
    }
    ub_push(b, vars, fvars, kinds, amb, Kind::StrIntMap(Own::Owned))
}

/// `Op::CallNative(Core.Map.size, 1)` — FLAT: the handle's 12-bit count field (two ops); AMB:
/// the builder record's count word; boxed: the helper. Never a fault on a valid map.
pub(super) fn arm_map_size(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (mv, mk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(mk, Kind::StrIntMap(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Map.size receiver kind {mk:?}"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let flat_blk = b.create_block();
    let chk_amb = b.create_block();
    let amb_blk = b.create_block();
    let slow = b.create_block();
    let tag = b.ins().band_imm_s(mv, UB_TAG_FLAT_MAP);
    let is_flat = b.ins().icmp_imm_s(IntCC::Equal, tag, UB_TAG_FLAT_MAP);
    b.ins().brif(is_flat, flat_blk, &[], chk_amb, &[]);
    b.switch_to_block(flat_blk);
    let cnt_raw = b.ins().ushr_imm_s(mv, UB_MAP_CNT_SHIFT);
    let cnt = b.ins().band_imm_s(cnt_raw, 0xFFF);
    b.ins().jump(merge, &[cnt.into()]);
    b.switch_to_block(chk_amb);
    let amb_bit = b.ins().band_imm_s(mv, UB_TAG_AMB);
    b.ins().brif(amb_bit, amb_blk, &[], slow, &[]);
    // AMB builder: count lives in the record BUFFER at bytes 8..16 (after log2).
    b.switch_to_block(amb_blk);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let aidx = b.ins().band_imm_s(mv, UB_IDX_MASK);
    let aroff = b.ins().imul_imm_s(aidx, 24);
    let aprec = b.ins().iadd(abase, aroff);
    let aptr = b.ins().load(types::I64, MemFlagsData::new(), aprec, 0);
    let acnt = b.ins().load(types::I64, MemFlagsData::new(), aptr, 8);
    b.ins().jump(merge, &[acnt.into()]);
    b.switch_to_block(slow);
    let zero = b.ins().iconst(types::I64, 0);
    let call = b.ins().call(h.map_size, &[ec.ctx, mv, zero]);
    let r = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[r.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    // The receiver was a QUERY; release an OWNED one after the count is in hand (runtime-bit-
    // gated — flat words no-op, an untagged boxed map takes the helper free).
    if mk.is_owned_handle() {
        emit_release(b, ec, h, mv);
    }
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `Op::Index` over a [`Kind::MapList`] — the flat int-list load (bounds check + one load of
/// the map HANDLE word) plus a FLAT-map tag guard on the loaded word: a boxed map element
/// (possible only via the seal's arena-exhaustion fallback) is code 5, the byte-identical VM
/// redo — which is what keeps the OWNED push aliasing-safe (flat map releases no-op).
pub(super) fn arm_index_map_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    proven: bool,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int || !matches!(lk, Kind::MapList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({lk:?}[{ik:?}])"
        )));
    }
    let flat_bit = b.ins().band_imm_s(lv, UB_TAG_FLAT);
    let not_flat = b.ins().icmp_imm_s(IntCC::Equal, flat_bit, 0);
    ec.fault_if(b, not_flat, 5); // boxed map-list scratch → VM redo
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
    // The loaded element must be a SEALED FLAT map — everything downstream (release no-op,
    // COW-copy conversion on mutation) relies on it.
    let wtag = b.ins().band_imm_s(word, UB_TAG_FLAT_MAP);
    let wbad = b.ins().icmp_imm_s(IntCC::NotEqual, wtag, UB_TAG_FLAT_MAP);
    ec.fault_if(b, wbad, 5);
    ub_push(b, vars, fvars, kinds, word, Kind::StrIntMap(Own::Owned))
}
