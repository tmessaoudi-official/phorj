//! Statement- and pattern-SHAPE predicates (M-Decomp; split out of `common.rs` when DEC-364 pushed
//! that file past Invariant 13's hard cap). Stateless like its parent — no `Checker` state — but a
//! distinct concern: these answer "what shape is this AST node?" for the totality/definite-assignment
//! engine (`checker::program::totality`) and for duplicate-arm detection.
//!
//! **Every `Stmt` match here is exhaustive on purpose** (Invariant 3, widened by DEC-356). A
//! catch-all in this file is not a style question: `breaks_this_loop` had one, and because of it a
//! `break` inside a `try` was invisible — `while (true) { try { break; } finally { … } }` looked
//! non-exiting, so a function with no `return` after it type-checked clean and then returned `unit`
//! from an `int` signature. Exhaustive matches turn the next such omission into a build failure.

/// Whether an expression is the literal `true` — the only condition an always-running loop can carry
/// for the structural termination analysis (M-RT totality cluster). Anything else (a variable, a
/// comparison) might be false, so the loop might exit and is not treated as divergent.
pub(super) fn is_true_lit(e: &crate::ast::Expr) -> bool {
    matches!(e, crate::ast::Expr::Bool(true, _))
}

/// Whether `stmts` contains a `break` bound to the *current* loop. Descends into every construct
/// that is NOT itself a loop — `if`, `block`, `try`/`catch`/`finally`, a destructure `else`, and a
/// `using` body — because a `break` written there still targets the enclosing loop. It deliberately
/// does NOT descend into nested `while`/`for`/`do` loops (their `break`s bind to them). `match` arms
/// are expressions and carry no `break`.
///
/// **This used to miss `try` (and the destructure `else`), which was a live soundness hole**, found
/// while DEC-364 made this predicate total. `function f(): int { while (true) { try { break; }
/// finally { … } } }` type-checked clean — the only exit was invisible here, so `while (true)` looked
/// non-exiting, the function looked like it could not fall through, and `E-MISSING-RETURN` never
/// fired. Both Rust legs then ran it and printed `unit` from a function whose declared return type is
/// `int` [Verified: reproduced before the fix on `run` and `run --tree-walker`].
///
/// The match is exhaustive on purpose (Invariant 3 / DEC-356): a new `Stmt` that can host a `break`
/// must break this build rather than silently read as "no break here", which is what a catch-all did.
pub(super) fn breaks_this_loop(stmts: &[crate::ast::Stmt]) -> bool {
    use crate::ast::Stmt;
    stmts.iter().any(|s| match s {
        Stmt::Break(_) => true,
        Stmt::Block(b, _) => breaks_this_loop(b),
        // Not a loop — a `break` in the guarded body binds to the enclosing loop (DEC-364).
        Stmt::Using { body, .. } => breaks_this_loop(body),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            breaks_this_loop(then_block)
                || else_block.as_ref().is_some_and(|eb| breaks_this_loop(eb))
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            breaks_this_loop(body)
                || catches.iter().any(|c| breaks_this_loop(&c.body))
                || finally_block
                    .as_ref()
                    .is_some_and(|fb| breaks_this_loop(fb))
        }
        Stmt::Destructure {
            else_block: Some(eb),
            ..
        } => breaks_this_loop(eb),
        // Nested loops own their `break`s; everything else cannot host one.
        Stmt::While { .. }
        | Stmt::CFor { .. }
        | Stmt::For { .. }
        | Stmt::Destructure {
            else_block: None, ..
        }
        | Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Return { .. }
        | Stmt::Expr(..)
        | Stmt::Discard(..)
        | Stmt::Throw { .. }
        | Stmt::Continue(_) => false,
    })
}

/// Whether `target` is an assignment to `this.<field>` (the constructor definite-assignment analysis,
/// Soundness Batch D). Matches a non-safe member access `this.field` exactly.
pub(super) fn is_this_field(target: &crate::ast::Expr, field: &str) -> bool {
    use crate::ast::Expr;
    matches!(
        target,
        Expr::Member { object, name, safe: false, .. }
            if name == field && matches!(**object, Expr::This(_))
    )
}

/// Whether a statement contains a `return` anywhere on any path (descending into blocks, `if`, loops,
/// and `try`). Used by the constructor definite-assignment check (Batch D): a `return` reached before a
/// field is assigned completes construction with the field unset, so it conservatively fails the check.
pub(super) fn stmt_has_return(s: &crate::ast::Stmt) -> bool {
    use crate::ast::Stmt;
    match s {
        Stmt::Return { .. } => true,
        Stmt::Block(b, _) => b.iter().any(stmt_has_return),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            then_block.iter().any(stmt_has_return)
                || else_block
                    .as_ref()
                    .is_some_and(|eb| eb.iter().any(stmt_has_return))
        }
        Stmt::While { body, .. }
        | Stmt::CFor { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Using { body, .. } => body.iter().any(stmt_has_return),
        Stmt::Destructure {
            else_block: Some(eb),
            ..
        } => eb.iter().any(stmt_has_return),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            body.iter().any(stmt_has_return)
                || catches.iter().any(|c| c.body.iter().any(stmt_has_return))
                || finally_block
                    .as_ref()
                    .is_some_and(|fb| fb.iter().any(stmt_has_return))
        }
        _ => false,
    }
}

/// Whether a pattern matches *every* value of its static type — it can never fall through. Only a
/// wildcard or plain binding qualifies; a literal, variant, type or struct pattern is a runtime test
/// that can fail. Drives both `match_arm_key` (a refined payload isn't a plain duplicate) and the
/// variant exhaustiveness rule in `check_match` (a refutable payload doesn't discharge coverage).
pub(super) fn is_irrefutable(pat: &crate::ast::Pattern) -> bool {
    use crate::ast::Pattern;
    matches!(pat, Pattern::Wildcard(_) | Pattern::Binding { .. })
}

/// A stable identity for a `match` pattern, for duplicate-arm detection (`W-MATCH-UNREACHABLE`).
/// `None` for patterns that should not be deduplicated: `float` (equality is fuzzy) and the
/// catch-alls (`_`/bare binding, handled separately as a catch-all).
pub(super) fn match_arm_key(p: &crate::ast::Pattern) -> Option<String> {
    use crate::ast::Pattern;
    match p {
        Pattern::Int(v, _) => Some(format!("i{v}")),
        // A decimal pattern dedups by its *numeric* value (scale-insensitive, like `==`): `1.5d` and
        // `1.50d` are the same value, so they share a key. Normalize by stripping trailing zeros from
        // the unscaled value while decrementing the scale, yielding a canonical `(unscaled, scale)`.
        Pattern::Decimal {
            unscaled, scale, ..
        } => {
            let (mut u, mut s) = (*unscaled, *scale);
            while s > 0 && u % 10 == 0 {
                u /= 10;
                s -= 1;
            }
            Some(format!("d{u}e{s}"))
        }
        Pattern::Str(s, _) => Some(format!("s{s}")),
        Pattern::Bool(b, _) => Some(format!("b{b}")),
        Pattern::Null(_) => Some("null".to_string()),
        // A variant arm is a duplicate of an earlier one only when both have an *irrefutable* payload
        // (every field a wildcard/binding) — `Some(x)` after `Some(y)` is unreachable, but `Some(0)`
        // and `Some(1)`, or `W(Circle c)` and `W(Square s)` (S5.2-T2), are distinct refinements and
        // must not be flagged. A refined payload yields no dedup key.
        Pattern::Variant { name, fields, .. } if fields.iter().all(is_irrefutable) => {
            Some(format!("v{name}"))
        }
        Pattern::Variant { .. } => None,
        // A type pattern, and a struct pattern with an all-binding payload, share the `t` keyspace:
        // `Point { x }` and `Point p` both match any `Point`, so a later one is an unreachable dup. A
        // struct pattern with a refined field (`Point { x: 0 }`) is not a plain duplicate.
        Pattern::Type { type_name, .. } => Some(format!("t{type_name}")),
        Pattern::Struct {
            type_name, fields, ..
        } if fields.iter().all(|f| is_irrefutable(&f.pat)) => Some(format!("t{type_name}")),
        Pattern::Struct { .. } => None,
        Pattern::Float(_, _) | Pattern::Wildcard(_) | Pattern::Binding { .. } => None,
    }
}
