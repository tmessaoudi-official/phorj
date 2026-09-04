//! Property-hook diagnostics (M-mut.7b, PHP 8.4 hooks). Four codes, none asserted before.
use super::support::*;

fn has(src: &str, code: &str) {
    let e = errors_of(src);
    assert!(
        e.iter().any(|d| d.code == Some(code)),
        "expected {code}, got {e:?}"
    );
}

#[test]
fn a_get_must_yield_the_hook_type() {
    has(
        "class C { constructor() {} int n { get => \"s\"; } } function main() -> void { }",
        "E-HOOK-TYPE",
    );
}

#[test]
fn a_hook_is_declared_once() {
    has(
        "class C { constructor() {} int n { get => 1; } int n { get => 2; } } function main() -> void { }",
        "E-HOOK-DUP",
    );
}

#[test]
fn reading_a_write_only_hook_is_rejected() {
    has(
        "class C { constructor(public mutable int raw) {} int n { set(int v) { this.raw = v; } } } function main() -> void { C c = new C(1); int x = c.n; }",
        "E-HOOK-NO-GET",
    );
}

#[test]
fn writing_a_read_only_hook_is_rejected() {
    has(
        "class C { constructor(public mutable int raw) {} int n { get => this.raw; } } function main() -> void { C c = new C(1); c.n = 2; }",
        "E-HOOK-NO-SET",
    );
}
