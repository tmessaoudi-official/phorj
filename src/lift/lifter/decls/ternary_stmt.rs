//! twes-in row T1 (2026-09-27) — a PHP ternary in STATEMENT position, `$c ? $a->m() : $b->m();`.
//!
//! Its value is discarded, so it is exactly `if ($c) { $a->m(); } else { $b->m(); }` — the condition
//! first, then one arm, nothing else. Lifted through the expression path it became an `if`-EXPRESSION
//! with value arms, `if (c) { a.m() } else { b.m() };`, which phorj parses as an `if` STATEMENT whose
//! arms lack their `;` — a draft that did not parse. Rewriting to PHP's own `if` statement first lets
//! the existing `if` lifting do the rest: a nested ternary in an arm is itself a statement ternary and
//! nests the same way, and an assignment arm lifts as any assigned statement does.
//!
//! The elvis form (`$a ?: $b`, no middle operand) is not rewritten: it has no `if` spelling without
//! evaluating `$a` twice, and the expression path refuses it by name.

use super::*;

impl Lifter {
    /// The statement-position ternary as the `if` statement it means (see the module doc).
    pub(super) fn lift_ternary_stmt(
        &mut self,
        cond: &php::PhpExpr,
        then: &php::PhpExpr,
        els: &php::PhpExpr,
        declared: &mut HashSet<String>,
    ) -> Result<Vec<Stmt>, String> {
        let as_if = php::PhpStmt::If {
            cond: cond.clone(),
            then: vec![php::PhpStmt::Expr(then.clone())],
            elifs: Vec::new(),
            els: Some(vec![php::PhpStmt::Expr(els.clone())]),
        };
        self.lift_stmt(&as_if, declared)
    }
}
