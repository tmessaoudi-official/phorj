//! Set-OP native predicates + admissions (the DEC-332 setdifference/setunion flips) — the
//! `Core.Set.{union,difference,size}` verticals and the narrow [`Kind::SetList`] (a
//! `List<Set<int>>` built by `MakeList`, indexed by the rotating-operand bench shape
//! `bs[i % 4]`). Lives here (natives headroom) so the grandfathered `analyze/mod.rs` arms stay
//! one-liners (Invariant 13).

use super::*;

/// Is native-registry entry `id` `Core.Set.union` (FLAT × FLAT builds a fresh sealed flat set,
/// memoized per `(a, b)` handle pair — see `handles/sets_ext.rs`)?
pub(crate) fn unboxed_native_is_set_union(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Set" && nf.name == "union" && nf.pure)
}

/// Is native-registry entry `id` `Core.Set.difference` (same memoized flat×flat build — `a`'s
/// keys not in `b`)?
pub(crate) fn unboxed_native_is_set_difference(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Set" && nf.name == "difference" && nf.pure)
}

/// Is native-registry entry `id` `Core.Set.size` (FLAT → the handle's count bits inline; a
/// non-flat set word → code-5 VM redo)?
pub(crate) fn unboxed_native_is_set_size(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Set" && nf.name == "size" && nf.pure)
}

/// Admit `Set.union(a, b)` / `Set.difference(a, b)`: pop two `IntSet` operands, push the
/// result `IntSet` (Owned — a memoized sealed flat set at runtime, whose release is a no-op,
/// so Owned is sound for every leg). The RESULT is a hash table with NO insertion order —
/// sound precisely because every admitted `IntSet` consumer (`Set.size`, `Set.contains`,
/// these two ops) is order-insensitive, and set kinds can never escape the unboxed graph
/// (not a param / call-arg / return kind); any order-observing path runs on the VM.
pub(crate) fn admit_set_op(kinds: &mut Vec<Kind>) -> Result<(), JitError> {
    for side in ["second", "first"] {
        match kinds.pop() {
            Some(Kind::IntSet(_)) => {}
            other => {
                return Err(JitError::Unsupported(format!(
                    "unboxed Set op {side} operand kind {other:?}"
                )))
            }
        }
    }
    kinds.push(Kind::IntSet(Own::Owned));
    Ok(())
}

/// Admit `Set.size(s)`: pop the `IntSet` receiver (a QUERY), push `Int`.
pub(crate) fn admit_set_size(kinds: &mut Vec<Kind>) -> Result<(), JitError> {
    match kinds.pop() {
        Some(Kind::IntSet(_)) => {}
        other => {
            return Err(JitError::Unsupported(format!(
                "unboxed Set.size receiver kind {other:?}"
            )))
        }
    }
    kinds.push(Kind::Int);
    Ok(())
}
