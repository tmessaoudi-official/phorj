//! Member-modifier helpers: ctor-param promotion detection + the PHP visibility keyword. Split out
//! of `transpile/mod.rs` (M-Decomp) to keep the root under the file-size cap; re-globbed by the root
//! so the emit modules keep reaching them via `use super::*`. Pure code movement — no emit-logic
//! change.

use super::*;

/// A ctor param is promoted (becomes a field) iff it carries a visibility modifier —
/// matches the evaluator (EV-4) and the checker's `collect_class`. Single-sourced via
/// `Modifier::is_member_visibility` so `internal` (Q-B DV-3) promotes like the others.
pub(super) fn is_promoted(mods: &[Modifier]) -> bool {
    mods.iter().any(Modifier::is_member_visibility)
}

/// PHP visibility keyword for a member's modifiers (empty string = no keyword). DEC-241: an
/// asymmetric `private(set)`/`protected(set)` rides along 1:1 (PHP 8.4 syntax; the 8.5 floor makes
/// it always legal) — phorj enforces at compile time, PHP re-enforces at runtime for free.
pub(super) fn vis(mods: &[Modifier]) -> String {
    let read = if mods.iter().any(|m| matches!(m, Modifier::Private)) {
        "private"
    } else if mods.iter().any(|m| matches!(m, Modifier::Protected)) {
        "protected"
    } else if mods.iter().any(|m| matches!(m, Modifier::Public)) {
        "public"
    } else if mods.iter().any(|m| matches!(m, Modifier::Internal)) {
        // Q-B DV-3: `internal` has no PHP analog → erases to `public`. Emitting the keyword EXPLICITLY
        // (not "") is required for a PROMOTED param: `public int $x` is a promoted property, bare
        // `int $x` is just a constructor argument — the latter would drop the field (byte-identity break).
        "public"
    } else {
        ""
    };
    let set = if mods.iter().any(|m| matches!(m, Modifier::PrivateSet)) {
        " private(set)"
    } else if mods.iter().any(|m| matches!(m, Modifier::ProtectedSet)) {
        " protected(set)"
    } else {
        ""
    };
    if set.is_empty() {
        read.to_string()
    } else if read.is_empty() {
        // A bare `private(set) mutable int x;` — public read is the default; PHP needs an
        // explicit read keyword before the set one.
        format!("public{set}")
    } else {
        format!("{read}{set}")
    }
}
