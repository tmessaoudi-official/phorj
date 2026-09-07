//! PHP-lift parser tests — **closures** (split from `parser_tests.rs`, Invariant 13, 2026-09-07).
//! Both forms parse: the arrow closure (Lane R) and the block-bodied one (Lane L1b). What does not
//! parse is what phorj has RULED OUT rather than merely not built — by-reference capture (DEC-506).

use super::ast::*;
#[allow(unused_imports)]
use super::lexer::lex_php;
use super::parser_tests::{expr_of, perr, stmt_of};

#[test]
fn parses_both_closure_forms_and_refuses_what_phorj_ruled_out() {
    // Lane R (2026-09-05): the arrow form is Tier-1 — it is the closure shape real code uses.
    assert!(matches!(expr_of("fn ($x) => $x"), PhpExpr::Closure { .. }));
    // Lane L1b (2026-09-07): the block form too, when it declares its return type. Written as an
    // assignment because a bare `function (` at STATEMENT position is a declaration in PHP's
    // grammar, not an anonymous function — the parser is right to read it that way.
    assert!(format!(
        "{:?}",
        stmt_of("<?php $f = function (int $x): int { return $x; };")
    )
    .contains("BlockClosure"));
    // Without one there is nothing to write: phorj requires `: T` on a statement-bodied lambda and
    // the lifter does not infer it (DEC-166).
    assert!(perr("<?php $f = function () { return 1; };").contains("return type"));
    // And by-reference capture is a RULED rejection, named as one (DEC-506).
    assert!(
        perr("<?php $n = 0; $f = function () use (&$n): int { return $n; };")
            .contains("by reference")
    );
}
