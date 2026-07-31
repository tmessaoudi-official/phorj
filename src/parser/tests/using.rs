//! Parser tests — the `using` scope guard (DEC-364) and its contextual-keyword gate (DEC-364.1).

use super::support::*;

/// DEC-364 / DEC-364.1 — `using` is a CONTEXTUAL keyword: it must parse as a scope guard in the
/// header position and as an ordinary identifier everywhere else. This pair IS the decision's
/// regression surface (the ruling's own words), so both halves are asserted together.
#[test]
fn parses_using_scope_guard_without_reserving_the_word() {
    match stmt("using (Connection db = acquire()) { db.exec(\"x\"); }") {
        Stmt::Using {
            ty: Type::Named { name: t, .. },
            name,
            init: Expr::Call { .. },
            body,
            ..
        } => {
            assert_eq!(t, "Connection");
            assert_eq!(name, "db");
            assert_eq!(body.len(), 1);
        }
        other => panic!("got {other:?}"),
    }
    // Still a perfectly ordinary identifier — nothing is reserved.
    match stmt("int using = 1;") {
        Stmt::VarDecl { name, .. } => assert_eq!(name, "using"),
        other => panic!("`int using = 1;` must stay a declaration, got {other:?}"),
    }
    // And a CALL to a function named `using` stays a call: the gate checks the header's `Type name =`
    // shape, not merely the `(`, so `using(…)` is not swallowed by the guard.
    match stmt("using(1);") {
        Stmt::Expr(Expr::Call { .. }, _) => {}
        other => panic!("`using(1);` must stay a call, got {other:?}"),
    }
    // A `using` used as a value in other positions is likewise untouched.
    match stmt("using = 2;") {
        Stmt::Assign { .. } => {}
        other => panic!("`using = 2;` must stay an assignment, got {other:?}"),
    }
}

/// `using (var h = …)` parses (as `Type::Infer`) so the CHECKER can reject it with `E-USING-INFER`
/// and name the fix, rather than the parser emitting "unknown type `var`".
#[test]
fn using_with_var_parses_as_inferred_for_a_better_diagnostic() {
    match stmt("using (var h = acquire()) { }") {
        Stmt::Using {
            ty: Type::Infer(_),
            name,
            ..
        } => assert_eq!(name, "h"),
        other => panic!("got {other:?}"),
    }
}
