//! Checker tests — `main` entry-point signature (Batch-1 B).
//!
//! `main` is the program entry point: it accepts **zero or one** parameters (the one allowed param is
//! `List<string>`, the program argv), and returns `void` or `int` (the process exit code). Any other
//! shape is `E-MAIN-SIGNATURE`. Only the entry `main` is constrained — a library/user function named
//! `main` is mangled away by the loader, so this never bites ordinary code.

use super::support::*;

fn has(src: &str, code: &str) -> bool {
    errors_of(src).iter().any(|e| e.code == Some(code))
}

#[test]
fn main_void_no_args_ok() {
    assert!(!has("function main(): void { }", "E-MAIN-SIGNATURE"));
}

#[test]
fn main_int_no_args_ok() {
    assert!(!has(
        "function main(): int { return 0; }",
        "E-MAIN-SIGNATURE"
    ));
}

#[test]
fn main_argv_void_ok() {
    assert!(!has(
        "function main(List<string> args): void { }",
        "E-MAIN-SIGNATURE"
    ));
}

#[test]
fn main_argv_int_ok() {
    assert!(!has(
        "function main(List<string> args): int { return 0; }",
        "E-MAIN-SIGNATURE"
    ));
}

#[test]
fn main_non_list_param_rejected() {
    let src = "function main(int x): void { }";
    assert!(has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn main_wrong_list_elem_rejected() {
    let src = "function main(List<int> a): void { }";
    assert!(has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn main_extra_param_rejected() {
    let src = "function main(List<string> a, int b): int { return 0; }";
    assert!(has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn main_string_return_rejected() {
    let src = "function main(): string { return \"\"; }";
    assert!(has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn non_main_function_is_unconstrained() {
    // An ordinary function may take any params / return any type — only `main` is gated.
    let src = "function helper(int x): string { return \"\"; } function main(): void { }";
    assert!(!has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

// --- Batch-1 D: class-static entry points ------------------------------------------------------

#[test]
fn class_static_main_ok() {
    let src = "class App { static function main(): int { return 0; } }";
    assert!(!has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn class_static_main_argv_ok() {
    let src = "class App { static function main(List<string> a): int { return 0; } }";
    assert!(!has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn class_static_main_bad_signature_rejected() {
    // A static entry `main` is constrained exactly like a top-level one.
    let src = "class App { static function main(int x): void { } }";
    assert!(has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn instance_method_named_main_is_not_an_entry() {
    // An *instance* method named `main` is an ordinary method — any signature, not gated.
    let src = "class App { constructor() {} function main(int x, int y): string { return \"\"; } }";
    assert!(!has(src, "E-MAIN-SIGNATURE"), "{:?}", errors_of(src));
}

#[test]
fn top_level_and_class_static_main_is_multiple() {
    let src = "function main(): void { } class App { static function main(): void { } }";
    assert!(has(src, "E-MULTIPLE-MAIN"), "{:?}", errors_of(src));
}

#[test]
fn two_class_static_mains_is_multiple() {
    let src =
        "class A { static function main(): void { } } class B { static function main(): void { } }";
    assert!(has(src, "E-MULTIPLE-MAIN"), "{:?}", errors_of(src));
}

#[test]
fn single_class_static_main_is_not_multiple() {
    let src = "class App { static function main(): void { } }";
    assert!(!has(src, "E-MULTIPLE-MAIN"), "{:?}", errors_of(src));
}
