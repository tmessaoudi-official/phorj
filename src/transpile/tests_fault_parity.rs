//! DEC-560/561 — fault-parity emission (split out of `tests.rs`, Invariant 13).

use super::emit;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn php(src: &str) -> String {
    let tokens = lex(src).expect("lex");
    let prog = Parser::new(tokens).parse_program().expect("parse");
    emit(&prog).expect("emit")
}

/// DEC-560: where the native legs FAULT, the PHP leg must fault too (KNOWN_ISSUES "where Phorj faults
/// PHP must fault"). Three shapes were silent successes in PHP: `i64::MIN % -1` (PHP gives 0),
/// `List.product` overflow (PHP promotes to float) and `String.substring` splitting a multibyte char
/// (PHP's `substr` returns the broken bytes).
#[test]
fn dec560_fault_shapes_are_guarded_on_the_php_leg() {
    // `%` by a non-literal could be `-1`: checked helper. By a literal other than -1 it cannot
    // overflow, so the hot-loop `i % 2` keeps the bare operator (Invariant 11: no perf regression).
    let var = php("function f(int a, int b) -> int { return a % b; }");
    assert!(var.contains("__phorj_checked_rem($a, $b)"), "{var}");
    assert!(var.contains("function __phorj_checked_rem("), "{var}");
    let lit = php("function f(int a) -> int { return a % 2; }");
    assert!(
        lit.contains("$a % 2") && !lit.contains("__phorj_checked_rem"),
        "{lit}"
    );
    let neg = php("function f(int a) -> int { return a % -1; }");
    assert!(
        neg.contains("__phorj_checked_rem("),
        "a literal -1 is the overflowing divisor: {neg}"
    );

    let prod =
        php("import Core.List; function f(List<int> xs) -> int { return List.product(xs); }");
    assert!(
        prod.contains("__phorj_checked_int(array_product($xs))"),
        "{prod}"
    );

    let sub = php(
        "import Core.String; function f(string s) -> string { return String.substring(s, 0, 2); }",
    );
    assert!(sub.contains("__phorj_substring($s, 0, 2)"), "{sub}");
    assert!(sub.contains("function __phorj_substring("), "{sub}");
}

/// DEC-561: `Cryptography.verifyPassword` guards a bcrypt hash on the PHP leg too (PHP's own
/// `password_verify` WOULD verify it, where phorj has no bcrypt and faults).
#[test]
fn dec561_verify_password_is_guarded_against_bcrypt_hashes() {
    let s = php(
        "import Core.Cryptography; function f(string p, string h) -> bool { return Cryptography.verifyPassword(p, h); }",
    );
    assert!(s.contains("__phorj_verify_password($p, $h)"), "{s}");
    assert!(s.contains("function __phorj_verify_password("), "{s}");
    assert!(s.contains("password_verify"), "{s}");
}
