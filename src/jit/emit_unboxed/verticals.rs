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
    let chk_amb = b.create_block();
    b.ins().brif(both, fast_blk, &[], chk_amb, &[]);
    // AMB read leg (the mapinsert vertical): a MUTABLE builder record with a canonized slot
    // key probes ITS packed table inline — same walk, table at `record.ptr + 16` instead of
    // the sealed pairs' tail. The `m["alpha"]`/`m["theta"]` reads in a builder loop would
    // otherwise pay a helper call (bytes Vec + registry HashMap) each.
    b.switch_to_block(chk_amb);
    let amb_bit = b.ins().band_imm(mv, UB_TAG_AMB);
    let chk_amb_key = b.create_block();
    b.ins().brif(amb_bit, chk_amb_key, &[], slow_blk, &[]);
    b.switch_to_block(chk_amb_key);
    let kslot_bit = b.ins().band_imm(iv, UB_TAG_SLOT);
    let amb_key = b.create_block();
    b.ins().brif(kslot_bit, amb_key, &[], slow_blk, &[]);
    b.switch_to_block(amb_key);
    let abuf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let aki = b.ins().band_imm(iv, UB_IDX_MASK);
    let akoff = b.ins().ishl_imm(aki, 6);
    let apk = b.ins().iadd(abuf, akoff);
    let akhash = b.ins().load(
        types::I64,
        MemFlagsData::new(),
        apk,
        UB_SLOT_HASH_OFF as i32,
    );
    let akcanon = b.ins().load(
        types::I64,
        MemFlagsData::new(),
        apk,
        UB_SLOT_CANON_OFF as i32,
    );
    let amb_probe = b.create_block();
    b.ins().brif(akcanon, amb_probe, &[], slow_blk, &[]);
    b.switch_to_block(amb_probe);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let aridx = b.ins().band_imm(mv, UB_IDX_MASK);
    let aroff = b.ins().imul_imm(aridx, 24);
    let aprec = b.ins().iadd(abase, aroff);
    let aptr = b.ins().load(types::I64, MemFlagsData::new(), aprec, 0);
    let alg = b.ins().load(types::I64, MemFlagsData::new(), aptr, 0);
    let aone = b.ins().iconst(types::I64, 1);
    let atsize = b.ins().ishl(aone, alg);
    let amask = b.ins().iadd_imm(atsize, -1);
    let at0 = b.ins().band(akhash, amask);
    let ahead = b.create_block();
    b.append_block_param(ahead, types::I64);
    let ahit = b.create_block();
    let anext = b.create_block();
    b.ins().jump(ahead, &[at0.into()]);
    b.switch_to_block(ahead);
    let at = b.block_params(ahead)[0];
    let abtoff = b.ins().ishl_imm(at, 4);
    let aebase = b.ins().iadd(aptr, abtoff);
    let aecanon = b.ins().load(types::I64, MemFlagsData::new(), aebase, 16);
    let aceq = b.ins().icmp(IntCC::Equal, aecanon, akcanon);
    b.ins().brif(aceq, ahit, &[], anext, &[]);
    b.switch_to_block(ahit);
    let aval = b.ins().load(types::I64, MemFlagsData::new(), aebase, 24);
    if ik.is_owned_handle() {
        ec.slot_free_if_owned(b, iv);
    }
    b.ins().jump(merge, &[aval.into()]);
    b.switch_to_block(anext);
    let aempty = b.ins().icmp_imm(IntCC::Equal, aecanon, 0);
    ec.fault_if(b, aempty, 5); // genuine miss → canonical fault on the VM redo
    let at1 = b.ins().iadd_imm(at, 1);
    let atw = b.ins().band(at1, amask);
    b.ins().jump(ahead, &[atw.into()]);
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
    // PACKED bucket walk (the mapget vertical): each 16-byte bucket is `{canon, value}` —
    // a HIT is the canon compare plus ONE adjacent value load (one cache line, no pair
    // indirection); canon 0 = empty bucket = genuine miss (code 5, the VM redo renders the
    // canonical `"map key not found"`).
    let head = b.create_block();
    b.append_block_param(head, types::I64); // bucket index
    let hit = b.create_block();
    let next = b.create_block();
    b.ins().jump(head, &[t0.into()]);
    b.switch_to_block(head);
    let t = b.block_params(head)[0];
    let btoff = b.ins().ishl_imm(t, 4);
    let baddr = b.ins().iadd(tbase, btoff);
    let bcanon = b.ins().load(types::I64, MemFlagsData::new(), baddr, 0);
    let ceq = b.ins().icmp(IntCC::Equal, bcanon, kcanon);
    b.ins().brif(ceq, hit, &[], next, &[]);
    b.switch_to_block(hit);
    let val = b.ins().load(types::I64, MemFlagsData::new(), baddr, 8);
    // Consume the key (recycle iff runtime-OWNED); the flat map is bump-pinned
    // (runtime-borrowed always) — nothing to free.
    if ik.is_owned_handle() {
        ec.slot_free_if_owned(b, iv);
    }
    b.ins().jump(merge, &[val.into()]);
    b.switch_to_block(next);
    let empty = b.ins().icmp_imm(IntCC::Equal, bcanon, 0);
    ec.fault_if(b, empty, 5); // genuine miss → canonical fault on the VM
    let t1 = b.ins().iadd_imm(t, 1);
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

/// `Op::CallNative(Core.Map.has, 2)` — the maphas vertical: the SAME inline packed-bucket probe as
/// [`arm_index_map`], but it pushes a `Bool` (key present?) instead of the value, and — crucially —
/// a genuine miss is a CLEAN `false` (0), NOT a fault (`m[k]` faults on a missing key; `Map.has`
/// returns `false`). FLAT-sealed map + canon-nonzero key runs fully inline; a canon-0 key, a
/// non-flat map, or a mutable builder punts to the `rt_u_map_has` helper (which also answers a clean
/// miss with `false`, and only DEFENSIVELY returns code 5 → VM redo).
pub(super) fn arm_maphas(
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
            "unboxed Map.has operand kinds ({mk:?}, {ik:?})"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64); // 1 = present, 0 = absent
    let fast_blk = b.create_block();
    let slow_blk = b.create_block();
    // INLINE iff the map sealed FLAT and the key is an arena slot — the SAME fused tag check as the
    // mapget vertical: `(mv & FLAT) != 0 && (iv & SLOT) != 0` in three ops (shift the map's FLAT bit
    // up onto the key's SLOT bit, AND, mask SLOT). Anything else (AMB builder, boxed map) → helper.
    const _: () = assert!(UB_TAG_FLAT << 1 == UB_TAG_SLOT, "fused tag shift");
    let mv_flat_up = b.ins().ishl_imm(mv, 1);
    let fused = b.ins().band(mv_flat_up, iv);
    let both = b.ins().band_imm(fused, UB_TAG_SLOT);
    b.ins().brif(both, fast_blk, &[], slow_blk, &[]);
    // FAST: O(1) packed-bucket walk. The key's CANON word (slot byte 32 — nonzero only when
    // content-registered) picks the pair; the HASH picks the bucket, ONE canon compare decides. A
    // canon-0 key (inline-concat result, unregistered runtime string) punts to the helper.
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
    let next = b.create_block();
    b.ins().jump(head, &[t0.into()]);
    b.switch_to_block(head);
    let t = b.block_params(head)[0];
    let btoff = b.ins().ishl_imm(t, 4);
    let baddr = b.ins().iadd(tbase, btoff);
    let bcanon = b.ins().load(types::I64, MemFlagsData::new(), baddr, 0);
    let ceq = b.ins().icmp(IntCC::Equal, bcanon, kcanon);
    b.ins().brif(ceq, hit, &[], next, &[]);
    // HIT — present. Consume the key iff runtime-OWNED (the flat map is bump-pinned — nothing to
    // free); push 1.
    b.switch_to_block(hit);
    if ik.is_owned_handle() {
        ec.slot_free_if_owned(b, iv);
    }
    let onev = b.ins().iconst(types::I64, 1);
    b.ins().jump(merge, &[onev.into()]);
    // An empty bucket is a genuine ABSENT (the seal's load factor ≤ 1/2 guarantees one exists, so
    // the walk always terminates) — a CLEAN `false`, NOT a fault. Consume the key, push 0.
    b.switch_to_block(next);
    let empty = b.ins().icmp_imm(IntCC::Equal, bcanon, 0);
    let absent_blk = b.create_block();
    let step_blk = b.create_block();
    b.ins().brif(empty, absent_blk, &[], step_blk, &[]);
    b.switch_to_block(absent_blk);
    if ik.is_owned_handle() {
        ec.slot_free_if_owned(b, iv);
    }
    let zerov = b.ins().iconst(types::I64, 0);
    b.ins().jump(merge, &[zerov.into()]);
    b.switch_to_block(step_blk);
    let t1 = b.ins().iadd_imm(t, 1);
    let tw = b.ins().band(t1, mask);
    b.ins().jump(head, &[tw.into()]);
    // SLOW: the helper (boxed map / canon-0 key / mutable builder). A clean answer returns
    // present∈{0,1} with code 0; code 5 is a defensive VM redo (never a genuine miss).
    b.switch_to_block(slow_blk);
    let mask_free = (ik.is_owned_handle() as i64) | ((mk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask_free);
    let call = b.ins().call(h.map_has, &[ec.ctx, mv, iv, maskv]);
    let spresent = b.inst_results(call)[0];
    let scode = b.inst_results(call)[1];
    let bad = b.ins().icmp_imm(IntCC::NotEqual, scode, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[spresent.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Bool)
}

/// `Core.Set.of(List<int>)` — the setcontains vertical's producer. Re-tags a FRESH OWNED flat
/// int-list handle as an [`Kind::IntSet`] membership store: the sealed arena block
/// (`UB_TAG_FLAT | count<<40 | base`, raw `i64` per slot) IS the store, so this is a pure
/// COMPILE-TIME kind change — ZERO Cranelift IR, no helper, no new `unsafe`. Dedup is not applied
/// (irrelevant to the only consumer, `Set.contains` — membership is dedup-invariant, and the
/// narrow gating means nothing else observes the block). Owned-ONLY input is the double-free gate
/// (a Borrowed copy would alias its source local); `analyze` enforces the same, so this reject is
/// defensive.
pub(super) fn arm_set_of(
    b: &mut FunctionBuilder,
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
    ub_push(b, vars, fvars, kinds, v, Kind::IntSet(Own::Owned))
}

/// `Core.Set.contains(Set<int>, int)` — the setcontains vertical. An INLINE linear scan of the
/// flat int block (raw `i64` per 64-byte slot at bytes 0..8) comparing each element to the needle:
/// byte-identical to the interpreter's own `Vec<HKey::Int>::contains` (`HKey::Int` equality ⇔ i64
/// equality). A HIT pushes `1` (true); an exhausted scan pushes `0` (a CLEAN false — a membership
/// test never faults). The set is a QUERY — NOT consumed (mirrors `Map.has`); the needle is a raw
/// scalar. A non-flat set (a too-large / arena-exhausted seal that stayed boxed) is undecidable
/// here → code 5, and the WHOLE call redoes on the VM, which recomputes `Set.contains`
/// byte-identically. SAFETY: the flat handle's `[base, base+count)` slots are arena-resident by
/// construction of the seal and unchanged by the `Set.of` re-tag, so every inline load is in-bounds
/// — the same envelope as [`arm_index_int_list`]; this arm adds NO new `unsafe`.
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
    let flat_bit = b.ins().band_imm(sv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
    // FAST: linear scan of the flat int block. `count` rides bits 40..60 of the handle; `base` the
    // low 40. Walk (addr, remaining); a HIT short-circuits to `merge(1)`, exhaustion to `merge(0)`.
    b.switch_to_block(flat_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let cnt_raw = b.ins().ushr_imm(sv, 40);
    let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
    let base = b.ins().band_imm(sv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm(base, 6);
    let addr0 = b.ins().iadd(buf, boff);
    let header = b.create_block();
    b.append_block_param(header, types::I64); // element address
    b.append_block_param(header, types::I64); // remaining count
    let body = b.create_block();
    let step = b.create_block();
    b.ins().jump(header, &[addr0.into(), cnt.into()]);
    b.switch_to_block(header);
    let addr = b.block_params(header)[0];
    let rem = b.block_params(header)[1];
    // remaining == 0 → absent (a clean 0, NOT a fault).
    let zerov = b.ins().iconst(types::I64, 0);
    b.ins().brif(rem, body, &[], merge, &[zerov.into()]);
    b.switch_to_block(body);
    let elem = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    let eq = b.ins().icmp(IntCC::Equal, elem, needle);
    let onev = b.ins().iconst(types::I64, 1);
    b.ins().brif(eq, merge, &[onev.into()], step, &[]);
    b.switch_to_block(step);
    let addr1 = b.ins().iadd_imm(addr, UB_SLOT_SIZE as i64); // next 64-byte slot
    let rem1 = b.ins().iadd_imm(rem, -1);
    b.ins().jump(header, &[addr1.into(), rem1.into()]);
    // SLOW: a non-flat (boxed) set is undecidable inline → code 5, the whole call redoes on the VM
    // (byte-identical). Only reachable for a too-large / arena-exhausted seal — never a genuine miss.
    b.switch_to_block(slow_blk);
    let always = b.ins().iconst(types::I64, 1);
    ec.fault_if(b, always, 5);
    let unreach = b.ins().iconst(types::I64, 0);
    b.ins().jump(merge, &[unreach.into()]); // unreachable terminator (fault_if diverges)
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Bool)
}

/// `Op::Index` with an `IntList` beneath the index — P-2c int-list element read
/// (`xs[i]` → Int, raw i64 at slot bytes 0..8): inline unsigned bounds check + ONE load
/// for a flat list; the two-return helper for a boxed one.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_index_int_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    proven: bool,
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
    // Task-9 v2: a range-PROVEN in-bounds index (interval ⊆ [0, len)) drops the bounds branch.
    if !proven {
        let cnt_raw = b.ins().ushr_imm(lv, 40);
        let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
        let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
        ec.fault_if(b, oob, 5);
    }
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
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_index_str_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    proven: bool,
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
    // Task-9 v2: a range-PROVEN in-bounds index (interval ⊆ [0, len)) drops the bounds branch.
    if !proven {
        let cnt_raw = b.ins().ushr_imm(lv, 40);
        let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
        let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
        ec.fault_if(b, oob, 5);
    }
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

/// `Op::IterElems` — the for-in normalization (B1): a BORROWED flat-able Str/Int list handle
/// IS its element snapshot (sealed lists are immutable within this subset), so the arm is an
/// identity re-push — ZERO instructions. Analyze admits only the borrowed list kinds here.
pub(super) fn arm_iter_elems(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (v, k) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(
        k,
        Kind::StrList(Own::Borrowed) | Kind::IntList(Own::Borrowed)
    ) {
        return Err(JitError::Unsupported(format!(
            "unboxed IterElems operand kind {k:?}"
        )));
    }
    ub_push(b, vars, fvars, kinds, v, k)
}

/// Lever-3 POINTER-WALK init (the for-in desugar's `IterElems; Const(0)` site): a runtime
/// FLAT int list becomes (end, cursor) raw pointers into the arena — the harness ops then
/// strength-reduce per-op (`Len` identity, `Lt` one unsigned compare, `Index` one load,
/// `j+1` → `+64`). The mutation guard (collect walk) proved the iterated slot is never
/// written in-function, so the flat buffer is a stable snapshot for the whole loop; a
/// runtime NON-flat list (boxed — an ACL is unreachable under the guard) is code 5, redo
/// on the VM (disclosed: boxed lists don't take this vertical).
pub(super) fn arm_iter_ptr_init(
    b: &mut FunctionBuilder,
    ec: &Ec,
    _h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if lk != Kind::IntList(Own::Borrowed) {
        return Err(JitError::Unsupported(format!(
            "unboxed iter-ptr init operand kind {lk:?}"
        )));
    }
    let setup = b.create_block();
    b.append_block_param(setup, types::I64); // start
    b.append_block_param(setup, types::I64); // end
    let flat_blk = b.create_block();
    let bad_blk = b.create_block();
    let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], bad_blk, &[]);
    b.switch_to_block(flat_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm(lv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm(base, 6);
    let start = b.ins().iadd(buf, boff);
    let cnt_raw = b.ins().ushr_imm(lv, 40);
    let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
    let cbytes = b.ins().ishl_imm(cnt, 6);
    let end = b.ins().iadd(start, cbytes);
    b.ins().jump(setup, &[start.into(), end.into()]);
    b.switch_to_block(bad_blk);
    let always = b.ins().iconst(types::I64, 1);
    ec.fault_if(b, always, 5);
    let z = b.ins().iconst(types::I64, 0);
    b.ins().jump(setup, &[z.into(), z.into()]); // unreachable terminator
    b.switch_to_block(setup);
    let start_v = b.block_params(setup)[0];
    let end_v = b.block_params(setup)[1];
    ub_push(b, vars, fvars, kinds, end_v, Kind::IterEnd)?;
    ub_push(b, vars, fvars, kinds, start_v, Kind::IterPtr)
}

/// `Op::Len` — list length (the for-in inner-loop bound): INLINE for a FLAT handle (the
/// count rides bits 40..60 of the handle — two ops, no load), the helper for a boxed list.
pub(super) fn arm_list_len(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(lk, Kind::StrList(_) | Kind::IntList(_) | Kind::DynList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Len operand kind {lk:?}"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let fast_blk = b.create_block();
    let slow_blk = b.create_block();
    let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
    // A non-flat operand may be an ACL builder record (the listappend vertical's
    // `List.length(xs) >= 256` reset probe runs EVERY iteration) — read its len word
    // inline, count = len/8 (elements are raw i64s).
    let chk_acl = b.create_block();
    let fast_acl = b.create_block();
    b.ins().brif(flat_bit, fast_blk, &[], chk_acl, &[]);
    b.switch_to_block(chk_acl);
    let acl_bit = b.ins().band_imm(lv, UB_TAG_ACL);
    b.ins().brif(acl_bit, fast_acl, &[], slow_blk, &[]);
    b.switch_to_block(fast_acl);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let aidx = b.ins().band_imm(lv, UB_IDX_MASK);
    let aroff = b.ins().imul_imm(aidx, 24);
    let aprec = b.ins().iadd(abase, aroff);
    let alen = b.ins().load(types::I64, MemFlagsData::new(), aprec, 8);
    let acnt = b.ins().ushr_imm(alen, 3);
    b.ins().jump(merge, &[acnt.into()]);
    b.switch_to_block(fast_blk);
    let cnt_raw = b.ins().ushr_imm(lv, 40);
    let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
    b.ins().jump(merge, &[cnt.into()]);
    b.switch_to_block(slow_blk);
    let call = b.ins().call(h.list_len, &[ec.ctx, lv]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    // An OWNED operand dies at the measurement (`List.length(q.params())` — length of a
    // fresh clone): release AFTER the count is in hand (runtime-bit-gated — flat/const
    // words no-op; a boxed word takes the helper free).
    if lk.is_owned_handle() {
        emit_release(b, ec, h, lv);
    }
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// FUSED list-builder append (`xs = List.append(xs, v)`, the listappend vertical) — emitted
/// ONLY at a proven `accumulator_site` (the lhs is the dying borrow of the very slot the
/// following `SetLocal` rewrites, treated as CONSUMED — the pure-append clone is
/// unobservable). The hot shape — lhs already an ACL builder record — pushes FULLY INLINE:
/// load `{ptr,len,cap}` from the record (at `acc_base + idx·24`), cap-check, ONE 8-byte
/// store at `ptr+len`, bump len — php's `$xs[] =`. Everything else (first append on a
/// flat/boxed list, capacity growth) takes the ONE `rt_u_list_acc_append` helper call,
/// which converts/grows and returns the ACL handle the inline path then carries.
pub(super) fn list_append_acc(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    av: ClValue,
    vv: ClValue,
) -> Result<ClValue, JitError> {
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let fast0 = b.create_block();
    let slow_blk = b.create_block();
    let acl_bit = b.ins().band_imm(av, UB_TAG_ACL);
    b.ins().brif(acl_bit, fast0, &[], slow_blk, &[]);
    // INLINE: cap-checked in-place push of one raw i64 element.
    b.switch_to_block(fast0);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let idx = b.ins().band_imm(av, UB_IDX_MASK);
    let roff = b.ins().imul_imm(idx, 24);
    let prec = b.ins().iadd(abase, roff);
    let len = b.ins().load(types::I64, MemFlagsData::new(), prec, 8);
    let cap = b.ins().load(types::I64, MemFlagsData::new(), prec, 16);
    let nl = b.ins().iadd_imm(len, 8);
    let too_big = b.ins().icmp(IntCC::UnsignedGreaterThan, nl, cap);
    let fast2 = b.create_block();
    b.ins().brif(too_big, slow_blk, &[], fast2, &[]);
    b.switch_to_block(fast2);
    let ptr = b.ins().load(types::I64, MemFlagsData::new(), prec, 0);
    let dst = b.ins().iadd(ptr, len);
    b.ins().store(MemFlagsData::new(), vv, dst, 0);
    b.ins().store(MemFlagsData::new(), nl, prec, 8);
    b.ins().jump(merge, &[av.into()]);
    // SLOW: convert/grow through the helper (lhs ALWAYS consumed at an accumulator site);
    // a table-exhaustion `-1` is code 5 — the call redoes on the VM, correct, unspecialized.
    b.switch_to_block(slow_blk);
    let call = b.ins().call(h.list_acc_append, &[ec.ctx, av, vv]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    Ok(b.block_params(merge)[0])
}

/// `List.map` / `List.count` with a STATIC lambda (the hofpipe vertical): ONE native loop —
/// a uniform `(addr, stride)` walk over the input (flat list: 64-byte slots, raw i64 at
/// bytes 0..8; ACL builder: packed 8-byte i64s; a boxed list → code 5, redo on VM — the
/// disclosed v1 gap), a DIRECT call per element (`Fn`, or `FnCap1` with the capture word
/// PREPENDED — the VM's `[caps.., args..]` lambda frame), and: map → an ACL builder output
/// (inline cap-checked pushes via `list_append_acc`); count → a register sum of the 0/1
/// predicate results. No closure object, no VM re-entry, no per-element allocation.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_list_hof(
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
    is_map: bool,
) -> Result<(), JitError> {
    let (fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    let (f, has_cap) = match fk {
        Kind::Fn(f) => (f, false),
        Kind::FnCap1(f) => (f, true),
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed List HOF callee kind {other:?}"
            )))
        }
    };
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed List HOF receiver kind {lk:?}"
        )));
    }
    // Representation dispatch → uniform (addr0, count, stride).
    let setup = b.create_block();
    b.append_block_param(setup, types::I64); // addr0
    b.append_block_param(setup, types::I64); // count
    b.append_block_param(setup, types::I64); // stride
    let flat_blk = b.create_block();
    let chk_acl = b.create_block();
    let acl_blk = b.create_block();
    let bad_blk = b.create_block();
    let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], chk_acl, &[]);
    b.switch_to_block(flat_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm(lv, UB_IDX_MASK);
    let boff = b.ins().ishl_imm(base, 6);
    let faddr = b.ins().iadd(buf, boff);
    let fcnt_raw = b.ins().ushr_imm(lv, 40);
    let fcnt = b.ins().band_imm(fcnt_raw, 0xFFFFF);
    let s64 = b.ins().iconst(types::I64, 64);
    b.ins()
        .jump(setup, &[faddr.into(), fcnt.into(), s64.into()]);
    b.switch_to_block(chk_acl);
    let acl_bit = b.ins().band_imm(lv, UB_TAG_ACL);
    b.ins().brif(acl_bit, acl_blk, &[], bad_blk, &[]);
    b.switch_to_block(acl_blk);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let ridx = b.ins().band_imm(lv, UB_IDX_MASK);
    let roff = b.ins().imul_imm(ridx, 24);
    let prec = b.ins().iadd(abase, roff);
    let aptr = b.ins().load(types::I64, MemFlagsData::new(), prec, 0);
    let alenb = b.ins().load(types::I64, MemFlagsData::new(), prec, 8);
    let acnt = b.ins().ushr_imm(alenb, 3);
    let s8 = b.ins().iconst(types::I64, 8);
    b.ins().jump(setup, &[aptr.into(), acnt.into(), s8.into()]);
    // Boxed int list: code 5 — the VM redo runs the canonical higher-order native.
    b.switch_to_block(bad_blk);
    let always = b.ins().iconst(types::I64, 1);
    ec.fault_if(b, always, 5);
    let z = b.ins().iconst(types::I64, 0);
    b.ins().jump(setup, &[z.into(), z.into(), z.into()]); // unreachable terminator
    b.switch_to_block(setup);
    let addr0 = b.block_params(setup)[0];
    let count = b.block_params(setup)[1];
    let stride = b.block_params(setup)[2];
    // Output seed: map → a fresh ACL builder; count → a zero register.
    let acc0 = if is_map {
        let call = b.ins().call(h.list_builder_new, &[ec.ctx]);
        let oh = b.inst_results(call)[0];
        let bad = b.ins().icmp_imm(IntCC::SignedLessThan, oh, 0);
        ec.fault_if(b, bad, 5);
        oh
    } else {
        b.ins().iconst(types::I64, 0)
    };
    // The loop: header checks the remaining count (a 0-element list skips straight out).
    let header = b.create_block();
    b.append_block_param(header, types::I64); // addr
    b.append_block_param(header, types::I64); // remaining
    b.append_block_param(header, types::I64); // acc (out handle / running sum)
    let bodyb = b.create_block();
    let exitb = b.create_block();
    b.append_block_param(exitb, types::I64);
    b.ins()
        .jump(header, &[addr0.into(), count.into(), acc0.into()]);
    b.switch_to_block(header);
    let addr = b.block_params(header)[0];
    let rem = b.block_params(header)[1];
    let acc = b.block_params(header)[2];
    b.ins().brif(rem, bodyb, &[], exitb, &[acc.into()]);
    b.switch_to_block(bodyb);
    let elem = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    let cargs = if has_cap { vec![fv, elem] } else { vec![elem] };
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
    let acc1 = if is_map {
        list_append_acc(b, ec, h, acc, rv)?
    } else {
        b.ins().iadd(acc, rv) // Bool results are 0/1 i64 words — the sum IS the count
    };
    let addr1 = b.ins().iadd(addr, stride);
    let rem1 = b.ins().iadd_imm(rem, -1);
    b.ins()
        .jump(header, &[addr1.into(), rem1.into(), acc1.into()]);
    b.switch_to_block(exitb);
    let res = b.block_params(exitb)[0];
    // A consumed OWNED input dies here (an ACL from a preceding map recycles its record).
    if lk.is_owned_handle() {
        emit_release(b, ec, h, lv);
    }
    ub_push(
        b,
        vars,
        fvars,
        kinds,
        res,
        if is_map {
            Kind::IntList(Own::Owned)
        } else {
            Kind::Int
        },
    )
}

/// `Op::SetIndexLocal` on a `Map<string,int>` local — the mapinsert vertical (`m[k] = v`,
/// php's `$m[$k] = $v`): the map local is uniquely owned (analyze admits only
/// `StrIntMap(Owned)` slots — aliasing is impossible in the subset, so in-place mutation is
/// unobservable, exactly the VM's `Rc::make_mut` refcount-1 path). INLINE when the map is
/// already an AMB builder record AND the key is a canonized arena slot: packed-table probe
/// walk (same shape as the mapget probe — one canon compare per bucket), a HIT overwrites the
/// entry's value word in place (ONE store — rank/order unchanged, PHP semantics). Everything
/// else — first write on a sealed map (conversion), a genuine INSERT (probe hit a hole),
/// canon-0 keys, growth — takes the ONE `rt_u_map_builder_set` helper. The result handle is
/// stored straight back into the slot (`def_var`), which is what makes the conversion's new
/// AMB handle stick.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_set_index_map_local(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    slot: usize,
) -> Result<(), JitError> {
    let (vv, vk) = ub_pop(b, vars, fvars, kinds)?;
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    if vk != Kind::Int || !matches!(ik, Kind::Str(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed SetIndexLocal operand kinds ({ik:?}, {vk:?})"
        )));
    }
    if !matches!(kinds.get(slot), Some(Kind::StrIntMap(Own::Owned))) {
        return Err(JitError::Unsupported(format!(
            "unboxed SetIndexLocal map slot kind {:?}",
            kinds.get(slot)
        )));
    }
    let mv = b.use_var(vars[slot]);
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let chk_key = b.create_block();
    let probe0 = b.create_block();
    let slow_blk = b.create_block();
    // INLINE iff the map is an AMB record and the key is a canonized arena slot.
    let amb_bit = b.ins().band_imm(mv, UB_TAG_AMB);
    b.ins().brif(amb_bit, chk_key, &[], slow_blk, &[]);
    b.switch_to_block(chk_key);
    let slot_bit = b.ins().band_imm(iv, UB_TAG_SLOT);
    b.ins().brif(slot_bit, probe0, &[], slow_blk, &[]);
    b.switch_to_block(probe0);
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
    let walk_blk = b.create_block();
    b.ins().brif(kcanon, walk_blk, &[], slow_blk, &[]);
    b.switch_to_block(walk_blk);
    // Record `{ptr}` at `acc_base + idx·24`; the table starts at `ptr + 16`.
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let ridx = b.ins().band_imm(mv, UB_IDX_MASK);
    let roff = b.ins().imul_imm(ridx, 24);
    let prec = b.ins().iadd(abase, roff);
    let ptr = b.ins().load(types::I64, MemFlagsData::new(), prec, 0);
    let lg = b.ins().load(types::I64, MemFlagsData::new(), ptr, 0);
    let one = b.ins().iconst(types::I64, 1);
    let tsize = b.ins().ishl(one, lg);
    let mask = b.ins().iadd_imm(tsize, -1);
    let t0 = b.ins().band(khash, mask);
    // Packed bucket walk (the mapget probe shape): HIT = overwrite the value word in place;
    // an EMPTY bucket is a genuine INSERT → the helper (it appends rank + entry + count).
    let head = b.create_block();
    b.append_block_param(head, types::I64);
    let hit = b.create_block();
    let next = b.create_block();
    b.ins().jump(head, &[t0.into()]);
    b.switch_to_block(head);
    let t = b.block_params(head)[0];
    let btoff = b.ins().ishl_imm(t, 4);
    let ebase = b.ins().iadd(ptr, btoff);
    let ecanon = b.ins().load(types::I64, MemFlagsData::new(), ebase, 16);
    let ceq = b.ins().icmp(IntCC::Equal, ecanon, kcanon);
    b.ins().brif(ceq, hit, &[], next, &[]);
    b.switch_to_block(hit);
    b.ins().store(MemFlagsData::new(), vv, ebase, 24);
    b.ins().jump(merge, &[mv.into()]);
    b.switch_to_block(next);
    let empty = b.ins().icmp_imm(IntCC::Equal, ecanon, 0);
    let ins_blk = b.create_block();
    let step = b.create_block();
    b.ins().brif(empty, ins_blk, &[], step, &[]);
    // INLINE INSERT (an empty bucket at load ≤ 1/2 with rank capacity): entry + rank canon +
    // count++, four stores — php's zend-hash add. Growth (load would cross 1/2) or a full
    // rank region punts to the helper (which rebuilds / grows the record buffer).
    b.switch_to_block(ins_blk);
    let count = b.ins().load(types::I64, MemFlagsData::new(), ptr, 8);
    let cap = b.ins().load(types::I64, MemFlagsData::new(), prec, 16);
    let count1 = b.ins().iadd_imm(count, 1);
    let lf = b.ins().ishl_imm(count1, 1);
    let lf_bad = b.ins().icmp(IntCC::SignedGreaterThan, lf, tsize);
    let chk_cap = b.create_block();
    b.ins().brif(lf_bad, slow_blk, &[], chk_cap, &[]);
    b.switch_to_block(chk_cap);
    // need = 16 + tsize·16 + (count+1)·8 bytes ≤ cap, else the helper grows the buffer.
    let tbytes = b.ins().ishl_imm(tsize, 4);
    let rbytes = b.ins().ishl_imm(count1, 3);
    let need0 = b.ins().iadd(tbytes, rbytes);
    let need = b.ins().iadd_imm(need0, 16);
    let cap_bad = b.ins().icmp(IntCC::SignedGreaterThan, need, cap);
    let do_ins = b.create_block();
    b.ins().brif(cap_bad, slow_blk, &[], do_ins, &[]);
    b.switch_to_block(do_ins);
    b.ins().store(MemFlagsData::new(), kcanon, ebase, 16);
    b.ins().store(MemFlagsData::new(), vv, ebase, 24);
    let roff2 = b.ins().ishl_imm(count, 3);
    let rank0 = b.ins().iadd(ptr, tbytes);
    let rank1 = b.ins().iadd(rank0, roff2);
    b.ins().store(MemFlagsData::new(), kcanon, rank1, 16);
    b.ins().store(MemFlagsData::new(), count1, ptr, 8);
    b.ins().jump(merge, &[mv.into()]);
    b.switch_to_block(step);
    let t1 = b.ins().iadd_imm(t, 1);
    let tw = b.ins().band(t1, mask);
    b.ins().jump(head, &[tw.into()]);
    // SLOW: conversion / insert / canon-0 / growth through the helper.
    b.switch_to_block(slow_blk);
    let call = b.ins().call(h.map_builder_set, &[ec.ctx, mv, iv, vv]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    b.def_var(vars[slot], res);
    // Slot kind stays `StrIntMap(Owned)`. A compile-time-OWNED key dies here (single-use
    // GetLocal move / inline temp) — full ladder, runtime-bit-gated (a flat element or
    // pinned const is runtime-borrowed, so the free is a no-op; the canon registry never
    // points at a recyclable slot, so recycling the key cannot dangle a canon).
    if ik.is_owned_handle() {
        emit_release(b, ec, h, iv);
    }
    Ok(())
}

/// `String.length` native — INLINE for a slot operand (the length is the slot's leading
/// byte) and for a BORROWED accumulator record (the length is the record's len word — the
/// `String.length(s) > 512` reset probe in the strbuild pattern); the helper otherwise
/// (an OWNED ACC temp also goes to the helper — its free is the helper's job).
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
    if sk.is_owned_handle() {
        b.ins().brif(slot_bit, fast_blk, &[], slow_blk, &[]);
    } else {
        // Borrowed: a non-slot operand may be an ACC record — read its len word inline.
        let chk_acc = b.create_block();
        let fast_acc = b.create_block();
        b.ins().brif(slot_bit, fast_blk, &[], chk_acc, &[]);
        b.switch_to_block(chk_acc);
        let acc_bit = b.ins().band_imm(sv, UB_TAG_ACC);
        b.ins().brif(acc_bit, fast_acc, &[], slow_blk, &[]);
        b.switch_to_block(fast_acc);
        let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
        let aidx = b.ins().band_imm(sv, UB_IDX_MASK);
        let aroff = b.ins().imul_imm(aidx, 24);
        let aprec = b.ins().iadd(abase, aroff);
        let alen = b.ins().load(types::I64, MemFlagsData::new(), aprec, 8);
        b.ins().jump(merge, &[alen.into()]);
    }
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
    // W7: analyze declines Pop-of-Dyn (its payload would need a tag-gated release) — a Dyn
    // reaching here is analyze/emit drift, not a supported shape.
    if k == Kind::Dyn {
        return Err(JitError::Codegen(
            "unboxed: Pop of a Dyn value past analyze".to_string(),
        ));
    }
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
