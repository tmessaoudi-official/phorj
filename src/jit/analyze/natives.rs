//! JIT unboxed native-recognition predicates + the bridge-2 shape table (M-Decomp split
//! from `analyze.rs`, Invariant 13). New per-vertical `unboxed_native_is_*` predicates
//! land HERE (headroom for the perf campaign), not in the analyze pass.

use super::*;

/// Is native-registry entry `id` the `Core.String.length` native (the sole `CallNative` in the P-2a
/// unboxed subset)? Matched by registry identity — the compiler emitted the id from the same
/// registry, so this can never alias another native.
pub(crate) fn unboxed_native_is_str_len(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.String" && nf.name == "length" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.length` (the listappend vertical: inline for a
/// flat handle — count bits — or an ACL builder record; helper for a boxed list)?
pub(crate) fn unboxed_native_is_list_len(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "length" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.append` (the listappend vertical: at a PROVEN
/// accumulator site the consumed lhs becomes/extends an ACL builder record — in-place push,
/// php's `$xs[] =`; any other use stays on the VM)?
pub(crate) fn unboxed_native_is_list_append(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "append" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.map` (the hofpipe vertical: a STATIC-lambda map
/// lowers to a native loop — inline element loads, a direct call per element, an ACL builder
/// output — no closure object, no VM re-entry)?
pub(crate) fn unboxed_native_is_list_map(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "map" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.count` (the hofpipe vertical: a STATIC-predicate
/// count lowers to a native loop — inline element loads, a direct call per element, a running
/// register sum)?
pub(crate) fn unboxed_native_is_list_count(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "count" && nf.pure)
}

/// Is native-registry entry `id` `Core.Map.has` (the maphas vertical: the mapget inline bucket
/// probe returning a Bool `present?` instead of the value — a HIT is `true`, an empty bucket is a
/// clean `false` (NOT a fault, unlike `m[k]`); canon-0 keys / non-flat maps punt to the
/// `rt_u_map_has` helper)?
pub(crate) fn unboxed_native_is_map_has(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Map" && nf.name == "has" && nf.pure)
}

/// Is native-registry entry `id` `Core.Set.of` (the setcontains vertical: re-tag a fresh OWNED
/// flat int-list handle as an [`Kind::IntSet`] membership store — the sealed block IS the store,
/// dedup-invariant for the sole consumer `Set.contains`; Borrowed / non-int input → VM fallback)?
pub(crate) fn unboxed_native_is_set_of(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Set" && nf.name == "of" && nf.pure)
}

/// Is native-registry entry `id` `Core.Set.contains` (the setcontains vertical: an inline linear
/// scan of the flat int block — byte-identical to the interpreter's `Vec<HKey::Int>::contains` —
/// a HIT is `true`, an exhausted scan a clean `false` (never a fault); a non-flat, too-large set
/// punts to a code-5 VM redo)?
pub(crate) fn unboxed_native_is_set_contains(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Set" && nf.name == "contains" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.contains` (the listcontains vertical: an inline linear
/// scan of the flat int block — byte-identical to the interpreter's `Vec<Value::Int>` membership; a
/// HIT is `true`, an exhausted scan a clean `false`, never a fault; a non-flat (boxed) int list punts
/// to a code-5 VM redo)? Unlike `Set.contains` (a hash probe over a sealed set), this is a plain
/// linear scan — `IntList` is a flat `count<<40 | base` array, not a hash table.
pub(crate) fn unboxed_native_is_list_contains(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "contains" && nf.pure)
}

/// Is `id` one of the pure 2-arg natives routed through the GENERIC `rt_u_native2` bridge
/// (which calls the registered native itself — single-sourced semantics)? Cheap name gate for
/// the match guards; the shape table is [`unboxed_native_bridge2`].
pub(crate) fn unboxed_native_is_bridge2(id: usize) -> bool {
    crate::native::registry().get(id).is_some_and(|nf| {
        nf.pure
            && matches!(
                (nf.module, nf.name),
                ("Core.String", "join" | "contains" | "splitOnce") | ("Core.List", "drop")
            )
    })
}

/// The bridge-2 shape table: given the native and the two COMPILE-TIME operand kinds
/// (a = pushed first, b = second), return the `rt_u_native2` meta base (arg/result reprs —
/// see the helper's bit layout) and the result kind. `None` = kind mismatch (fail closed).
pub(crate) fn unboxed_native_bridge2(id: usize, a: &Kind, b: &Kind) -> Option<(i64, Kind)> {
    let nf = crate::native::registry().get(id)?;
    match ((nf.module, nf.name), a, b) {
        (("Core.String", "join"), Kind::StrList(_), Kind::Str(_)) => {
            Some((3 | (2 << 3) | (2 << 6), Kind::Str(Own::Owned)))
        }
        (("Core.String", "contains"), Kind::Str(_), Kind::Str(_)) => {
            Some((2 | (2 << 3), Kind::Bool))
        }
        (("Core.String", "splitOnce"), Kind::Str(_), Kind::Str(_)) => {
            Some((2 | (2 << 3) | (3 << 6), Kind::StrList(Own::Owned)))
        }
        (("Core.List", "drop"), Kind::StrList(_), Kind::Int) => {
            Some((3 | (3 << 6), Kind::StrList(Own::Owned)))
        }
        (("Core.List", "drop"), Kind::IntList(_), Kind::Int) => {
            Some((4 | (4 << 6), Kind::IntList(Own::Owned)))
        }
        _ => None,
    }
}

/// Is native-registry entry `id` `Core.Conversion.toString` (INT operand only in this subset —
/// routed to the same zero-alloc `rt_u_int_to_str` renderer interpolation uses, so the bytes
/// can never drift from the VM's `as_display`)?
pub(crate) fn unboxed_native_is_to_string(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Conversion" && nf.name == "toString" && nf.pure)
}

/// Is native-registry entry `id` `Core.Conversion.toFloat` (P-2c: inline `fcvt_from_sint` — the
/// kernel is `n as f64`, the same IEEE round-to-nearest widening)?
pub(crate) fn unboxed_native_is_to_float(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Conversion" && nf.name == "toFloat" && nf.pure)
}

/// Is native-registry entry `id` `Core.Conversion.truncate` (P-2c: inline trunc + range guard +
/// `fcvt_to_sint`, mirroring `value::float_to_int` exactly — out-of-range/NaN/±∞ → code 5, the
/// VM redo renders the canonical fault)?
pub(crate) fn unboxed_native_is_truncate(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Conversion" && nf.name == "truncate" && nf.pure)
}

/// Is native-registry entry `id` `Core.Math.max` (two-arg signed integer max)? The kernel is
/// `(*a).max(*b)` = `i64::max` = Cranelift `smax` = PHP `max($a,$b)` on two ints — byte-identical
/// signed max by construction, a PURE SCALAR op with no handles and no allocation. Inline-emitted
/// to eliminate the ~188ns VM→native dispatch (the mathmax perf flip).
pub(crate) fn unboxed_native_is_math_max(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Math" && nf.name == "max" && nf.pure)
}

/// Is native-registry entry `id` `Core.Math.min` (two-arg signed integer min)? The kernel is
/// `(*a).min(*b)` = `i64::min` = Cranelift `smin` = PHP `min($a,$b)` on two ints — byte-identical
/// signed min by construction, a PURE SCALAR op with no handles and no allocation. The exact mirror
/// of `unboxed_native_is_math_max` (the mathmin perf flip).
pub(crate) fn unboxed_native_is_math_min(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Math" && nf.name == "min" && nf.pure)
}

/// Is native-registry entry `id` `Core.Math.sign` (one-arg integer sign)? The kernel is
/// `i64::from(*n > 0) - i64::from(*n < 0)` = -1/0/1 = PHP `($n <=> 0)` (spaceship) — a branchless
/// PURE SCALAR op with no fault, no overflow, no handles. Materialized inline as two `icmp`s
/// (`>0`, `<0`) widened to i64 and subtracted (the mathsign perf flip).
pub(crate) fn unboxed_native_is_math_sign(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Math" && nf.name == "sign" && nf.pure)
}

/// Is native-registry entry `id` `Core.Math.abs` (one-arg integer absolute value)? The kernel is
/// `n.checked_abs()`, which FAULTS ("integer overflow in Math.abs") on `i64::MIN`. Cranelift's
/// `iabs` WRAPS `i64::MIN` to `i64::MIN` (no trap), so the arm GUARDS `n == i64::MIN` → code 5
/// (VM redo renders the canonical fault) before `iabs`, keeping interp ≡ VM ≡ JIT (the mathabs
/// perf flip — the one vertical with a fault path).
pub(crate) fn unboxed_native_is_math_abs(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Math" && nf.name == "abs" && nf.pure)
}
