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
fn top_level_and_class_static_entry_is_duplicate_kind() {
    // DEC-331 D1: multiplicity is per declared KIND — two `Cli` entries collide.
    let src = "#[Entry(kind: Cli)] function main(): void { } class App { #[Entry(kind: Cli)] static function main(): void { } }";
    assert!(has(src, "E-DUPLICATE-ENTRY-KIND"), "{:?}", errors_of(src));
}

#[test]
fn two_class_static_entries_is_duplicate_kind() {
    let src =
        "class A { #[Entry(kind: Cli)] static function main(): void { } } class B { #[Entry(kind: Cli)] static function main(): void { } }";
    assert!(has(src, "E-DUPLICATE-ENTRY-KIND"), "{:?}", errors_of(src));
}

#[test]
fn cli_and_web_entries_may_coexist() {
    // DEC-331 D1: one Cli + one Web entry in one program is legal — run/serve pick their kind.
    let src = "import Core.Http.Request; import Core.Http.Response; \
               #[Entry(kind: Cli)] function cli(): void { } \
               #[Entry(kind: Web)] function web(Request r): Response { return Response.text(\"ok\"); }";
    assert!(!has(src, "E-DUPLICATE-ENTRY-KIND"), "{:?}", errors_of(src));
    assert!(!has(src, "E-ENTRY-SIG"), "{:?}", errors_of(src));
}

#[test]
fn entry_on_instance_method_is_target_error() {
    let src = "class App { #[Entry(kind: Cli)] function run(): void { } }";
    assert!(has(src, "E-ENTRY-TARGET"), "{:?}", errors_of(src));
}

#[test]
fn entry_with_unmatched_signature_is_sig_error() {
    let src = "#[Entry(kind: Cli)] function main(int x): void { }";
    assert!(has(src, "E-ENTRY-SIG"), "{:?}", errors_of(src));
}

// ── DEC-331 D1: `#[Entry(kind:)]` is required; kind must be recognized & match the signature ──

#[test]
fn bare_entry_without_kind_is_required_error() {
    let src = "#[Entry] function main(): void { }";
    assert!(has(src, "E-ENTRY-KIND-REQUIRED"), "{:?}", errors_of(src));
}

#[test]
fn unknown_entry_kind_is_unknown_error() {
    let src = "#[Entry(kind: Banana)] function main(): void { }";
    assert!(has(src, "E-ENTRY-KIND-UNKNOWN"), "{:?}", errors_of(src));
}

#[test]
fn reserved_entry_kind_is_reserved_error() {
    for k in ["Desktop", "Mobile", "Worker", "Embedded"] {
        let src = format!("#[Entry(kind: {k})] function main(): void {{ }}");
        assert!(
            has(&src, "E-ENTRY-KIND-RESERVED"),
            "{k}: {:?}",
            errors_of(&src)
        );
    }
}

#[test]
fn web_kind_on_cli_signature_is_sig_error() {
    // Declared `kind: Web` but the signature is CLI-shaped — the kind↔signature disagreement.
    let src = "#[Entry(kind: Web)] function main(): void { }";
    assert!(has(src, "E-ENTRY-SIG"), "{:?}", errors_of(src));
}

#[test]
fn well_formed_cli_and_web_kinds_are_clean() {
    assert!(!has(
        "#[Entry(kind: Cli)] function main(): void { }",
        "E-ENTRY-KIND-REQUIRED"
    ));
    let web = "import Core.Http.Request; import Core.Http.Response; \
               #[Entry(kind: Web)] function h(Request r): Response { return Response.text(\"ok\"); }";
    assert!(!has(web, "E-ENTRY-SIG"), "{:?}", errors_of(web));
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
    let (.., table, _) = crate::checker::check_resolutions(&prog).expect("checks clean");
    assert!(
        table.values().any(|e| e == "Shape"),
        "construction + pattern resolutions recorded: {table:?}"
    );
    assert!(table.len() >= 2, "both use-sites recorded: {table:?}");
}
