//! `Op::MakeList` / `Op::MakeMap` — the sequence CONSTRUCTORS (M-Decomp from `verticals.rs`,
//! Invariant 13): element kinds select the flavor, the seal flattens an eligible list or map into
//! consecutive arena slots so the matching `Index` runs fully inline. Bodies moved verbatim.

use super::*;

/// `Op::MakeList(n)` — element kinds select the flavor (all-`Str` → `StrList` handle pushes,
/// all-`Int` → `IntList` raw i64 pushes, P-2c; all-`StrIntMap`/`IntSet`/`IntList` → the
/// handle-word flavors, DEC-520); the seal flattens eligible lists into consecutive arena
/// slots (a FLAT handle) so `Index` runs fully inline.
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
    // `StrList` (handle pushes), all-`Int` → `IntList` (raw i64 pushes, P-2c), all-map →
    // `MapList` (the map HANDLE words ride the raw-i64 path — never freed here: a flat map
    // word is bump-pinned, a boxed one safe-leaks until run end).
    let all_str = kinds[d - n..].iter().all(|k| matches!(k, Kind::Str(_)));
    let all_int = n > 0 && kinds[d - n..].iter().all(|k| *k == Kind::Int);
    let all_map = n > 0
        && kinds[d - n..]
            .iter()
            .all(|k| matches!(k, Kind::StrIntMap(_)));
    let all_set = n > 0 && kinds[d - n..].iter().all(|k| matches!(k, Kind::IntSet(_)));
    // DEC-520: all-`IntList` → `IntListList`. Same raw-i64 path as the map/set flavors — the
    // element words are int-list HANDLES, never freed here (a flat one is bump-pinned, a boxed
    // one safe-leaks until run end). NOT recursive: an `IntListList` element is not admitted.
    let all_intlist = n > 0 && kinds[d - n..].iter().all(|k| matches!(k, Kind::IntList(_)));
    if !(all_str || all_int || all_map || all_set || all_intlist) {
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
        let pc = if all_int || all_map || all_set || all_intlist {
            b.ins().call(h.list_push_int, &[ec.ctx, list_h, ev])
        } else {
            let freev = b
                .ins()
                .iconst(types::I64, kinds[depth_j].is_owned_handle() as i64);
            b.ins().call(h.list_push, &[ec.ctx, list_h, ev, freev])
        };
        let status = b.inst_results(pc)[0];
        let bad = b.ins().icmp_imm_s(IntCC::NotEqual, status, 0);
        ec.fault_if(b, bad, 5);
    }
    // Seal: all-short strings / all ints (incl. map words) flatten into consecutive arena
    // slots (a FLAT handle) so `Index` runs fully inline; anything else keeps the boxed handle.
    let sc = b.ins().call(h.list_seal, &[ec.ctx, list_h]);
    let sealed = b.inst_results(sc)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sealed, 0);
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
        } else if all_map {
            Kind::MapList(Own::Owned)
        } else if all_set {
            Kind::SetList(Own::Owned)
        } else if all_intlist {
            Kind::IntListList(Own::Owned)
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
        let bad = b.ins().icmp_imm_s(IntCC::NotEqual, status, 0);
        ec.fault_if(b, bad, 5);
    }
    // Seal: dedup through the canonical `build_map` kernel; an all-short-key int map
    // flattens into arena slot PAIRS (a `SLOT|FLAT` handle) so lookup runs fully inline.
    let sc = b.ins().call(h.map_seal, &[ec.ctx, map_h]);
    let sealed = b.inst_results(sc)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sealed, 0);
    ec.fault_if(b, bad, 5);
    kinds.truncate(d - 2 * n);
    ub_push(b, vars, fvars, kinds, sealed, Kind::StrIntMap(Own::Owned))
}
