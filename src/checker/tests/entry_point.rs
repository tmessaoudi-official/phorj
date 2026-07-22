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
fn top_level_and_class_static_entry_is_multiple() {
    // DEC-191: multiplicity is per attributed ROLE, not per name — two CLI entries collide.
    let src = "#[Entry] function main(): void { } class App { #[Entry] static function main(): void { } }";
    assert!(has(src, "E-MULTIPLE-ENTRY"), "{:?}", errors_of(src));
}

#[test]
fn two_class_static_entries_is_multiple() {
    let src =
        "class A { #[Entry] static function main(): void { } } class B { #[Entry] static function main(): void { } }";
    assert!(has(src, "E-MULTIPLE-ENTRY"), "{:?}", errors_of(src));
}

#[test]
fn cli_and_web_entries_may_coexist() {
    // DEC-191: one CLI + one web entry in one program is legal — run/serve pick their role.
    let src = "import Core.Http.Request; import Core.Http.Response; \
               #[Entry] function cli(): void { } \
               #[Entry] function web(Request r): Response { return Response.text(\"ok\"); }";
    assert!(!has(src, "E-MULTIPLE-ENTRY"), "{:?}", errors_of(src));
}

#[test]
fn entry_on_instance_method_is_target_error() {
    let src = "class App { #[Entry] function run(): void { } }";
    assert!(has(src, "E-ENTRY-TARGET"), "{:?}", errors_of(src));
}

#[test]
fn entry_with_unmatched_signature_is_sig_error() {
    let src = "#[Entry] function main(int x): void { }";
    assert!(has(src, "E-ENTRY-SIG"), "{:?}", errors_of(src));
}

#[test]
fn single_class_static_main_is_not_multiple() {
    let src = "class App { static function main(): void { } }";
    assert!(!has(src, "E-MULTIPLE-MAIN"), "{:?}", errors_of(src));
}

// ── DEC-329.3 commit A: variant-use resolution ───────────────────────────────────────────────────

#[test]
fn bare_variant_shared_by_two_enums_is_ambiguous() {
    let src = "enum A { Dup(int x) }\nenum B { Dup(string y) }\n\
               function f(): void { discard new Dup(1); }";
    assert!(has(src, "E-VARIANT-AMBIGUOUS"), "must be ambiguous");
    // Qualified constructions of BOTH stay clean.
    let q = "enum A { Dup(int x) }\nenum B { Dup(string y) }\n\
             function f(): void { discard new A.Dup(1); discard new B.Dup(\"s\"); }";
    assert!(!has(q, "E-VARIANT-AMBIGUOUS"), "qualified is unambiguous");
}

#[test]
fn variant_resolutions_side_table_maps_uses_to_owning_enums() {
    let src = "package Main;\nenum Shape { Circle(float r) }\n\
               function f(Shape s): float {\n  return match (s) { Circle(r) => r };\n}\n\
               function g(): Shape { return new Circle(1.0); }\n";
    let toks = crate::tokenizer::lex(src).expect("lex");
    let prog = crate::parser::Parser::new(toks)
        .parse_program()
        .expect("parse");
    let (.., table) = crate::checker::check_resolutions(&prog).expect("checks clean");
    assert!(
        table.values().any(|e| e == "Shape"),
        "construction + pattern resolutions recorded: {table:?}"
    );
    assert!(table.len() >= 2, "both use-sites recorded: {table:?}");
}
