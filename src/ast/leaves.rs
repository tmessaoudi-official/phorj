//! The AST **leaf sets**, single-sourced as macros (DEC-356 — Wave 2.1).
//!
//! **Why macros and not a helper function.** Rust cannot share match *arms*, and the whole point of
//! DEC-356 is that a rewriter must be exhaustive so the compiler enumerates the blast radius when a
//! variant is added. A `fn is_leaf(&Expr) -> bool` would give a `_ =>` catch-all by the back door. A
//! macro expanding to an or-pattern keeps exhaustiveness checking fully intact — `rustc` still verifies
//! every variant is covered — while costing ONE line per site instead of eleven.
//!
//! **Why that matters here.** The 26 catch-alls DEC-356 fixes live in files that are already at or over
//! Invariant 13's caps. Spelling the leaf list out at each site would add ~900 lines to files that must
//! not grow; a macro adds ~1 line per site and puts the list in ONE reviewable place.
//!
//! **The enforcement property, stated precisely.** Adding a variant to `Expr` breaks the build at every
//! site using these macros *unless* the variant is added to the macro's list here. So the decision "is
//! this new form a leaf, or does it bear expressions?" is forced ONCE, in this file, with the compiler
//! naming every site that has to care — which is the mechanical exhaustiveness the ruling asked for,
//! and the same discipline Invariant 3 already holds for the `Op` set.
//!
//! **A leaf is a form that carries no nested `Expr` — nothing more.** Getting that wrong in the *unsafe*
//! direction (calling an expression-bearing form a leaf) silently skips a rewrite, which is exactly the
//! bug class this decision exists to close — so the sets are deliberately minimal and each entry is
//! justified. `NewColl` and `Inject` are excluded on purpose; see `expr_leaves!`.

/// Every `Expr` form that carries **no nested `Expr`**, as an or-pattern.
///
/// Use as `e @ (expr_leaves!()) => e` in a rewriter that returns the node unchanged, or
/// `expr_leaves!() => {}` in a visitor that has nothing to do for them.
///
/// | leaf | why it carries no `Expr` |
/// |---|---|
/// | `Int` `Float` `Decimal` `Bool` `Null` `Bytes` | scalar literals |
/// | `Ident` `This` | name references |
/// | `PipePlaceholder` | the bare `_` in a pipe — a marker, no payload |
///
/// **`NewColl` and `Inject` are deliberately NOT in this set**, even though they also carry no nested
/// `Expr` (only `Type`s). A site that *meaningfully handles* one of them would then get an
/// `unreachable_pattern` error from the macro's arm — and `desugar_di` handling `Expr::Inject` is not a
/// leaf case, it is the entire point of that pass. Sites treating them as pass-throughs spell them in
/// their own or-pattern next to this macro; the two extra words are worth not having to reach for
/// `#[allow(unreachable_patterns)]`, which would mask real mistakes too.
///
/// Everything else bears at least one `Expr` and MUST be recursed into. The five that are easiest to
/// forget — because they read as "small" — are `Tuple`, `NamedArg`, `InstanceOf`, `Cast` and `Pipe`;
/// every one of them was being swallowed by a `leaf => leaf` arm before DEC-356.
#[macro_export]
macro_rules! expr_leaves {
    () => {
        $crate::ast::Expr::Int(..)
            | $crate::ast::Expr::Float(..)
            | $crate::ast::Expr::Decimal { .. }
            | $crate::ast::Expr::Bool(..)
            | $crate::ast::Expr::Null(..)
            | $crate::ast::Expr::Bytes(..)
            | $crate::ast::Expr::Ident(..)
            | $crate::ast::Expr::This(..)
            | $crate::ast::Expr::PipePlaceholder(..)
    };
}

/// Every `Stmt` form that carries **no nested `Expr` and no nested `Stmt`**: `break` and `continue`.
///
/// Deliberately tiny. `Stmt` has 15 variants and 13 of them bear something, so a `leaf => leaf` arm here
/// was hiding twelve possible misses behind two real ones.
#[macro_export]
macro_rules! stmt_leaves {
    () => {
        $crate::ast::Stmt::Break(..) | $crate::ast::Stmt::Continue(..)
    };
}

/// Every `Pattern` form that carries **no nested `Pattern` and no binding name**.
///
/// `Type { binding: None, .. }` is NOT included: whether it binds depends on a field, not on the
/// variant, so folding it in would make the set lie. Sites that care match it explicitly — see
/// `ast::walk::collect_pattern_bindings`.
#[macro_export]
macro_rules! pattern_leaves {
    () => {
        $crate::ast::Pattern::Wildcard(..)
            | $crate::ast::Pattern::Int(..)
            | $crate::ast::Pattern::Float(..)
            | $crate::ast::Pattern::Decimal { .. }
            | $crate::ast::Pattern::Str(..)
            | $crate::ast::Pattern::Bool(..)
            | $crate::ast::Pattern::Null(..)
    };
}

/// Every `Item` form that carries **no nested `Expr` anywhere in its payload**.
///
/// | leaf | why it carries no `Expr` |
/// |---|---|
/// | `Import` | `path`/`alias`/`except` are strings; `wildcard` is a flag |
/// | `TypeAlias` | `name` + a `Type` — and a `Type` is not an `Expr` |
///
/// **That is the whole set — two of eight.** Every other `Item` bears expressions, and three of them
/// bear them in places that read as "declaration, not code":
///
/// * `Trait(TraitDecl)` — `members: Vec<ClassMember>` is FULL method, constructor and hook bodies.
///   `rewrite_html.rs` named `Item::Trait(..)` in a hand-written "no expression-bearing body" leaf set,
///   which shipped a crash: `html"…"` inside a trait method reached the backends unresolved, so
///   `phg check` printed `OK (type-checks clean)` and both engines then hit
///   `unreachable!("html literal not resolved before …")`. A trait's bodies reach both backends —
///   they flatten into the using class (`checker/collect/inherit.rs`) — so a pre-check rewrite that
///   skips traits is skipping executable code, not a signature.
/// * `Enum(EnumDecl)` — `variants[].backing_value: Option<Box<Expr>>` (DEC-302), parsed with the full
///   `parse_expr`. No live defect is known here (a non-scalar backing is rejected downstream); it is
///   listed because it IS an `Expr` and this file's definition of "leaf" admits nothing else.
/// * `Interface(InterfaceDecl)` — its `methods` are `FunctionDecl`s with empty bodies, so it reads
///   inert. It is not: `Param.default: Option<Box<Expr>>` and `Attribute.args: Vec<Expr>` mean a
///   signature carries expressions. Sites that genuinely have nothing to do for it write an explicit
///   `Item::Interface(i) => Item::Interface(i)` pass-through rather than claiming leaf status.
///
/// **What this macro does NOT assert.** It says these two variants carry no `Expr`. It does not say
/// the sites using it are total over expression *positions*: no item-level pass in the tree walks
/// param defaults or attribute arguments today — not for `Function` or `Class` either — so a rewrite
/// needed inside `function f(int n = <expr>)` is missed uniformly across every pass. That gap is
/// recorded (CD-31), not closed here, and must not be described as covered.
///
/// Use as `it @ (item_leaves!()) => it` in a rewriter returning the node unchanged.
#[macro_export]
macro_rules! item_leaves {
    () => {
        $crate::ast::Item::Import { .. } | $crate::ast::Item::TypeAlias { .. }
    };
}

#[cfg(test)]
mod tests {
    /// DEC-356's gate (C), as a source-scan ratchet.
    ///
    /// The ruling asked for "a never-constructed probe variant whose addition must break the build in
    /// every rewriter that should care", and noted the honest limitation itself: **a match that still
    /// carries a catch-all keeps compiling**, so a probe variant only proves anything about matches D has
    /// already made total. A `#[cfg(test)]` variant on `Expr` cannot express that either — it would have
    /// to be added to the real enum, breaking the non-test build.
    ///
    /// So the gate is inverted into something checkable at every commit: assert that the rewriters D
    /// fixed have NOT regrown a catch-all. Combined with the compiler's own exhaustiveness checking (a
    /// new `Expr`/`Stmt`/`Pattern` variant already breaks every total match — verified by hand during
    /// the build by temporarily adding one, which produced `non-exhaustive patterns:
    /// Expr::ProbeVariant(_, _) not covered` at each fixed site), the two together give what the ruling
    /// wanted: a new variant is *considered* everywhere, and nowhere can quietly opt out again.
    #[test]
    fn no_fixed_rewriter_regrows_a_catch_all() {
        // Files D made total. `rewrite_ufcs_walk.rs` is present because only its `apply_repl` is exempt
        // (CD-27) — the exemption is asserted by name below, so a SECOND catch-all there still fails.
        const FIXED: &[&str] = &[
            "src/checker/rewrite_html.rs",
            "src/checker/rewrite_generics_walk.rs",
            "src/checker/resolve_variant_imports_walk.rs",
            "src/checker/rewrite_ufcs_walk.rs",
            "src/checker/desugar_router_walk.rs",
            "src/checker/desugar_di/walk.rs",
            "src/ast/walk.rs",
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut offenders = Vec::new();
        for rel in FIXED {
            let text = std::fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("read {rel}: {e} — did the file move? update FIXED"));
            for (i, line) in text.lines().enumerate() {
                let t = line.trim();
                if t.starts_with("//") {
                    continue;
                }
                // A named catch-all is WORSE than `_`: it compiles cleanly, reads as deliberate, and
                // greps as a handled case. Both forms are rejected.
                // Only INERT catch-alls are the defect: one that RECURSES
                // (`other => Box::new(rexpr(other, m))`) is total behaviour, not a swallow — it is the
                // opposite of the bug. Flag pass-through and no-op forms only.
                let inert = [
                    "other => other",
                    "leaf => leaf",
                    "_ => {}",
                    "_ => false",
                    "_ => true",
                ]
                .iter()
                .any(|p| t.starts_with(p));
                if inert {
                    // CD-27: `apply_repl`'s domain is checker-constructed replacements, not user AST.
                    let exempt = *rel == "src/checker/rewrite_ufcs_walk.rs"
                        && t.starts_with("other => other.clone(),");
                    if !exempt {
                        offenders.push(format!("{rel}:{}: {t}", i + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "DEC-356 regression — {} catch-all(s) reappeared in a rewriter that was made total. \
             Add explicit arms (or `crate::expr_leaves!()` for genuinely inert forms); if the site truly \
             needs an exemption, record it as a CD row first:\n  {}",
            offenders.len(),
            offenders.join("\n  ")
        );
    }
}
