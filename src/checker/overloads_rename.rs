//! Return-overload definition renaming (M-RT Slice C1 / S2.2) — the item-level walk that applies the
//! mangled names `overloads` resolved.
//!
//! Split out of `overloads.rs` (CD-31): adding the trait and test arms pushed that file past its
//! Invariant-13 size baseline, and this walk is a cohesive unit with one job — it consumes the
//! `renames` map and touches nothing else in the resolver.

use crate::ast::{ClassMember, Item, Program};
use std::collections::HashMap;

/// Rename each return-overload member's *definition* to its mangled name (M-RT Slice C1), keyed by the
/// `FunctionDecl`'s span. The resolved call sites were already rewritten to the same mangled names by
/// [`super::rewrite_ufcs`]; renaming the definitions makes the backends see distinct, single-overload
/// functions (so no ambiguous identical-`ParamKind` dispatch table is ever built, and the transpiler
/// emits each as a plain PHP function). A no-op when `renames` is empty — so a program with no
/// return-overloading is byte-for-byte the pre-Slice-C AST.
///
/// Free functions, class methods (S2.2) AND trait methods (CD-31) are all renamed: a trait's members
/// flatten into the using class, and `rewrite_ufcs` has already rewritten their call sites to the
/// mangled name, so leaving a trait's declaration unmangled would break dispatch. An earlier version
/// of this comment said "Free functions only … class members are returned untouched", which had been
/// false since S2.2.
pub fn rename_overload_defs(program: Program, renames: &HashMap<usize, String>) -> Program {
    if renames.is_empty() {
        return program;
    }
    /// Apply return-overload mangling across a member list — shared by the class and trait arms,
    /// which carry the identical `Vec<ClassMember>` (CD-31).
    fn rename_methods(
        members: &mut [ClassMember],
        renames: &std::collections::HashMap<usize, String>,
    ) {
        for member in members {
            if let ClassMember::Method(f) = member {
                if let Some(mangled) = renames.get(&f.span.start) {
                    f.name = mangled.clone();
                }
            }
        }
    }

    let items = program
        .items
        .into_iter()
        .map(|item| match item {
            Item::Function(mut f) => {
                if let Some(mangled) = renames.get(&f.span.start) {
                    f.name = mangled.clone();
                }
                Item::Function(f)
            }
            // M-RT S2.2: a class may hold return-overloaded *methods*; rename each method member whose
            // declaration span is in the map to its mangled name (`m__ret_int`). The resolved selector
            // call sites were rewritten to the same mangled names by `rewrite_ufcs`, so dispatch on the
            // mangled `(class, name)` stays consistent across all backends. A no-op for a class with no
            // return-overloaded method.
            Item::Class(mut c) => {
                rename_methods(&mut c.members, renames);
                Item::Class(c)
            }
            // CD-31: a trait's members are the same `Vec<ClassMember>` and flatten into the using
            // class, so a return-overloaded method declared in a trait must be mangled here too —
            // `rewrite_ufcs` already rewrote its call sites to the mangled name, and leaving the
            // declaration unmangled would break dispatch. The old comment asserted traits were
            // "returned untouched" as though that were a decision; it was the catch-all.
            Item::Trait(mut t) => {
                rename_methods(&mut t.members, renames);
                Item::Trait(t)
            }
            // A `test "…" { … }` body declares no items, so there is nothing to rename inside one —
            // but it is named rather than swept into a catch-all (CD-31).
            it @ Item::Test { .. } => it,
            // Enums and interfaces declare no method BODIES to rename against; named rather than
            // folded into `item_leaves!()`, which asserts freedom from `Expr` and would be false here.
            it @ (Item::Enum(..) | Item::Interface(..)) => it,
            it @ (crate::item_leaves!()) => it,
        })
        .collect();
    Program {
        package: program.package,
        items,
        span: program.span,
    }
}
