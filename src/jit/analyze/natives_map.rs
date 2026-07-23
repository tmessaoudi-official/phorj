//! Map-MATERIALIZATION native predicates + admissions (the DEC-332 mapkeys/mapvalues/mapmerge
//! flips) — the `Core.Map.{keys,values,merge,size}` verticals and the narrow [`Kind::MapList`]
//! (a `List<Map<string,int>>` built by `MakeList`, indexed by the rotating-operand bench shape
//! `maps[i % 3]`). Lives here (natives headroom) so the grandfathered `analyze/mod.rs` arms
//! stay one-liners (Invariant 13).

use super::*;

/// Is native-registry entry `id` `Core.Map.keys` (a FLAT receiver materializes as a SHARED
/// builder record of borrowed key-slot handles, memoized per map handle — see
/// `handles/maps_ext.rs`; boxed → canonical clone; AMB → code-5 VM redo)?
pub(crate) fn unboxed_native_is_map_keys(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "keys" && nf.pure)
}

/// Is native-registry entry `id` `Core.Map.values` (the int twin of `keys` — a SHARED record
/// of the raw value words, memoized in the same entry)?
pub(crate) fn unboxed_native_is_map_values(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "values" && nf.pure)
}

/// Is native-registry entry `id` `Core.Map.merge` (FLAT × FLAT builds a fresh sealed flat map,
/// memoized per `(a, b)` handle pair — the canonical `map_merge` kernel order)?
pub(crate) fn unboxed_native_is_map_merge(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "merge" && nf.pure)
}

/// Is native-registry entry `id` `Core.Map.size` (FLAT → count bits inline, AMB → the builder
/// record's count word inline, boxed → helper — never a fault on a valid map)?
pub(crate) fn unboxed_native_is_map_size(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "size" && nf.pure)
}

/// Is native-registry entry `id` `Core.Map.map` (the mapmap flip: a STATIC-lambda value
/// transform over a FLAT receiver → an inline pair walk, one direct call per entry, an AMB
/// record result — keys preserved, insertion order preserved)?
pub(crate) fn unboxed_native_is_map_map(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "map" && nf.pure)
}

/// Is native-registry entry `id` `Core.Map.filter` (the mapfilter flip: the same walk with a
/// CONDITIONAL push — an entry survives iff the 0/1 predicate on its VALUE is nonzero)?
pub(crate) fn unboxed_native_is_map_filter(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "filter" && nf.pure)
}

/// Admit a `Map.map`/`Map.filter` (arity-2 `CallNative`) into the unboxed subset: pop the static
/// `Fn`/`FnCap1` callee (1 declared param over the VALUE; return `Int` for map, `Bool` for
/// filter — fail closed otherwise), pop the `StrIntMap` receiver, push the `StrIntMap(Owned)`
/// result (an AMB record / boxed map at runtime). A throwing graph stays on the VM (mirrors the
/// List HOF rule — no thrown payload out of the inline loop).
pub(crate) fn admit_map_hof(
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    kinds: &mut Vec<Kind>,
    want_bool: bool,
    what: &str,
) -> Result<(), JitError> {
    if info.thrown_class.is_some() {
        return Err(JitError::Unsupported(format!(
            "unboxed: {what} in a throwing graph (deferred)"
        )));
    }
    let f = match kinds.pop() {
        Some(Kind::Fn(f)) | Some(Kind::FnCap1(f)) => f,
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed {what} callee kind {other:?} (deferred)"
            )))
        }
    };
    if program.functions[f].arity - program.functions[f].n_captures != 1 {
        return Err(JitError::Unsupported(format!(
            "unboxed: {what} lambda arity != 1 (VM renders any fault)"
        )));
    }
    let rk = info.ret_of(f);
    if rk != if want_bool { Kind::Bool } else { Kind::Int } {
        return Err(JitError::Unsupported(format!(
            "unboxed: {what} lambda return kind {rk:?} (deferred)"
        )));
    }
    match kinds.pop() {
        Some(Kind::StrIntMap(_)) => {}
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed {what} receiver kind {other:?}"
            )))
        }
    }
    kinds.push(Kind::StrIntMap(Own::Owned));
    Ok(())
}

/// Admit a 1-arg map-consuming native (`keys` / `values` / `size`): pop the `StrIntMap`
/// receiver (a QUERY — mirrors `Map.has`), push `out`.
pub(crate) fn admit_map_query1(
    kinds: &mut Vec<Kind>,
    out: Kind,
    what: &str,
) -> Result<(), JitError> {
    match kinds.pop() {
        Some(Kind::StrIntMap(_)) => {}
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed {what} receiver kind {other:?}"
            )))
        }
    }
    kinds.push(out);
    Ok(())
}

/// Admit `Map.merge(a, b)`: pop two `StrIntMap` operands, push the merged `StrIntMap` (Owned —
/// a fresh sealed flat map / untagged boxed map at runtime; a memoized flat result's release is
/// a no-op, so Owned is sound for every leg).
pub(crate) fn admit_map_merge(kinds: &mut Vec<Kind>) -> Result<(), JitError> {
    for side in ["second", "first"] {
        match kinds.pop() {
            Some(Kind::StrIntMap(_)) => {}
            other => {
                return Err(JitError::Unsupported(format!(
                    "unboxed Map.merge {side} operand kind {other:?}"
                )))
            }
        }
    }
    kinds.push(Kind::StrIntMap(Own::Owned));
    Ok(())
}
