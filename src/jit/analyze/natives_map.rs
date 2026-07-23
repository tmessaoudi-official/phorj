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
