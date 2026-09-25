//! DEC-535 (scout row 5r): optional and type narrowing flow into the right operand of `&&` / `||`
//! and into if-expression arms — not only into statement blocks. `a && b` narrows `b` by `a` true;
//! `a || b` narrows `b` by `a` false; an if-expression's arms narrow like an if-statement's.

use super::support::*;

fn clean(src: &str) {
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

fn refused(src: &str) {
    assert!(!errors_of(src).is_empty(), "expected a type error");
}

const OPT: &str = "function f((tenure: string, bp: int)? seen) -> bool";

#[test]
fn or_narrows_its_right_operand_by_a_null_test() {
    clean(&format!("{OPT} {{ return seen instanceof null || seen.tenure == \"x\"; }} function main() -> void {{}}"));
}

#[test]
fn and_narrows_its_right_operand_by_a_non_null_test() {
    clean(&format!(
        "{OPT} {{ return !(seen instanceof null) && seen.bp > 3; }} function main() -> void {{}}"
    ));
}

#[test]
fn an_if_expression_narrows_both_arms() {
    clean(
        "function f((tenure: string, bp: int)? seen) -> string { \
         return if (!(seen instanceof null)) { seen.tenure } else { \"\" }; } \
         function g((tenure: string, bp: int)? seen) -> string { \
         return if (seen instanceof null) { \"\" } else { seen.tenure }; } function main() -> void {}",
    );
}

#[test]
fn narrowing_nests_through_and_or_and_negation() {
    clean(&format!(
        "{OPT} {{ return !(seen instanceof null) && (seen.bp > 3 || seen.tenure == \"x\"); }} function main() -> void {{}}"
    ));
    clean(&format!(
        "{OPT} {{ return !(seen instanceof null || seen.bp < 0) && seen.tenure == \"x\"; }} function main() -> void {{}}"
    ));
}

#[test]
fn narrowing_does_not_leak_past_its_operator() {
    // The WRONG side of each operator: `||`'s right sees `a` FALSE, so `a` being a non-null test
    // narrows nothing there; `&&` gives its right operand only `a` true.
    refused(&format!(
        "{OPT} {{ return !(seen instanceof null) || seen.bp > 3; }} function main() -> void {{}}"
    ));
    refused(&format!(
        "{OPT} {{ return seen instanceof null && seen.bp > 3; }} function main() -> void {{}}"
    ));
    // Nor past the whole expression.
    refused(&format!(
        "{OPT} {{ bool b = !(seen instanceof null) && seen.bp > 3; return seen.bp > 3; }} function main() -> void {{}}"
    ));
    // No union complement: `x is int || x + 1 > 2` sees `x` NOT an int on the right.
    refused("function f(int | string x) -> bool { return x is int || x + 1 > 2; } function main() -> void {}");
}

#[test]
fn a_primitive_type_test_narrows_operator_operands() {
    clean("function f(int | string x) -> bool { return x is int && x + 1 > 2; } function main() -> void {}");
    clean("function f(int | string x) -> bool { return !(x is int) || x + 1 > 2; } function main() -> void {}");
    clean("function f(int | string x) -> int { return if (x is int) { x + 1 } else { 0 }; } function main() -> void {}");
}

#[test]
fn an_if_expression_has_the_type_of_both_arms_not_the_first() {
    // `then` narrows to `int` but `else` is still `int | string`: the expression is the WIDER type,
    // so arithmetic on it is refused.
    refused(
        "function f(int | string x) -> int { var y = if (x is int) { x } else { x }; return y + 1; } \
         function main() -> void {}",
    );
}
