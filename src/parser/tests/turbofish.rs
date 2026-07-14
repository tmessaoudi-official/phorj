//! Parser tests — turbofish call-site type arguments (DEC-208 slice A). The heuristic: a `<` after a
//! call head is a turbofish only when `< TypeList >` is immediately followed by `(`; otherwise it
//! backtracks cleanly and `<` is the ordinary comparison operator.

use super::support::*;
use crate::ast::{Expr, Type};

/// The turbofish type-argument list on a `Call`, or `panic` if `e` is not a call.
fn call_type_args(e: &Expr) -> &[Type] {
    match e {
        Expr::Call { type_args, .. } => type_args,
        other => panic!("expected a Call, got {other:?}"),
    }
}

#[test]
fn free_call_single_type_arg() {
    let e = expr("identity<int>(5)");
    let ta = call_type_args(&e);
    assert_eq!(ta.len(), 1, "one type argument");
    assert!(matches!(&ta[0], Type::Named { name, .. } if name == "int"));
    // The callee is the bare function name, the value argument is preserved.
    assert_eq!(sexpr(&e), "identity(5)");
}

#[test]
fn free_call_two_type_args() {
    let e = expr("firstOf<int, string>(1, \"x\")");
    let ta = call_type_args(&e);
    assert_eq!(ta.len(), 2, "two type arguments");
    assert!(matches!(&ta[0], Type::Named { name, .. } if name == "int"));
    assert!(matches!(&ta[1], Type::Named { name, .. } if name == "string"));
}

#[test]
fn nested_generic_type_arg_reuses_the_shift_split() {
    // `foo<List<int>>(x)` — the inner `List<int>` consumes ITS `>`; the outer `>` closes the
    // turbofish (the `>>` tokenization as two `Gt` is what makes this work).
    let e = expr("foo<List<int>>(x)");
    let ta = call_type_args(&e);
    assert_eq!(ta.len(), 1);
    match &ta[0] {
        Type::Named { name, args, .. } => {
            assert_eq!(name, "List");
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Type::Named { name, .. } if name == "int"));
        }
        other => panic!("expected List<int>, got {other:?}"),
    }
}

#[test]
fn method_turbofish() {
    let e = expr("obj.method<T>(a)");
    let ta = call_type_args(&e);
    assert_eq!(ta.len(), 1);
    assert!(matches!(&ta[0], Type::Named { name, .. } if name == "T"));
    match &e {
        Expr::Call { callee, .. } => {
            assert!(matches!(&**callee, Expr::Member { name, .. } if name == "method"));
        }
        other => panic!("expected a method Call, got {other:?}"),
    }
}

#[test]
fn ordinary_call_has_empty_type_args() {
    // A non-turbofish call carries an empty type-argument list (byte-identical to before).
    assert!(call_type_args(&expr("foo(5)")).is_empty());
}

#[test]
fn plain_less_than_is_not_a_turbofish() {
    // `a < b` has no following `(` after the (hypothetical) `>`, so `<` stays a comparison.
    assert_eq!(sexpr(&expr("a < b")), "(< a b)");
}

#[test]
fn chained_comparison_survives() {
    // `a < b > c` is `(a < b) > c`, NOT a turbofish (`c` is not `(`). This is the headline
    // backtrack guarantee — the safe heuristic never changes a comparison's meaning.
    assert_eq!(sexpr(&expr("a < b > c")), "(> (< a b) c)");
}

#[test]
fn integer_comparison_is_never_a_turbofish() {
    // `1 < 2 > (3)` — type arguments must be type NAMES (idents), so an int operand fails the
    // type-list parse and the whole thing backtracks to a comparison chain.
    assert_eq!(sexpr(&expr("1 < 2 > (3)")), "(> (< 1 2) 3)");
}

#[test]
fn backtrack_leaves_no_partial_consume() {
    // `a < b + c` — the type-list parses `b`, but no `>` follows, so it backtracks fully; the
    // expression is the ordinary comparison `a < (b + c)`.
    assert_eq!(sexpr(&expr("a < b + c")), "(< a (+ b c))");
}
