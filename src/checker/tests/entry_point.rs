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
    let src = "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Cli)] function main(): void { } class App { #[Entry(kind: EntryKind.Cli)] static function main(): void { } }";
    assert!(has(src, "E-DUPLICATE-ENTRY-KIND"), "{:?}", errors_of(src));
}

#[test]
fn two_class_static_entries_is_duplicate_kind() {
    let src =
        "import Core.Runtime.EntryKind; class A { #[Entry(kind: EntryKind.Cli)] static function main(): void { } } class B { #[Entry(kind: EntryKind.Cli)] static function main(): void { } }";
    assert!(has(src, "E-DUPLICATE-ENTRY-KIND"), "{:?}", errors_of(src));
}

#[test]
fn cli_and_web_entries_may_coexist() {
    // DEC-331 D1: one Cli + one Web entry in one program is legal — run/serve pick their kind.
    let src = "import Core.Runtime.EntryKind; import Core.Http.Request; import Core.Http.Response; \
               #[Entry(kind: EntryKind.Cli)] function cli(): void { } \
               #[Entry(kind: EntryKind.Web)] function web(Request r): Response { return Response.text(\"ok\"); }";
    assert!(!has(src, "E-DUPLICATE-ENTRY-KIND"), "{:?}", errors_of(src));
    assert!(!has(src, "E-ENTRY-SIG"), "{:?}", errors_of(src));
}

#[test]
fn entry_on_instance_method_is_target_error() {
    let src = "class App { #[Entry(kind: EntryKind.Cli)] function run(): void { } }";
    assert!(has(src, "E-ENTRY-TARGET"), "{:?}", errors_of(src));
}

#[test]
fn entry_with_unmatched_signature_is_sig_error() {
    let src = "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Cli)] function main(int x): void { }";
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
    let src =
        "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Banana)] function main(): void { }";
    assert!(has(src, "E-ENTRY-KIND-UNKNOWN"), "{:?}", errors_of(src));
}

#[test]
fn reserved_entry_kind_is_reserved_error() {
    for k in ["Desktop", "Mobile", "Worker", "Embedded"] {
        let src = format!(
            "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.{k})] function main(): void {{ }}"
        );
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
    let src =
        "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Web)] function main(): void { }";
    assert!(has(src, "E-ENTRY-SIG"), "{:?}", errors_of(src));
}

#[test]
fn well_formed_cli_and_web_kinds_are_clean() {
    assert!(!has(
        "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Cli)] function main(): void { }",
        "E-ENTRY-KIND-REQUIRED"
    ));
    let web = "import Core.Runtime.EntryKind; import Core.Http.Request; import Core.Http.Response; \
               #[Entry(kind: EntryKind.Web)] function h(Request r): Response { return Response.text(\"ok\"); }";
    assert!(!has(web, "E-ENTRY-SIG"), "{:?}", errors_of(web));
}

// ── DEC-337: `kind:` is an injected `EntryKind` variant — qualified + imported, never in the wind ──

#[test]
fn bare_kind_is_injected_variant_bare_error() {
    // A bare `kind: Cli` (unqualified injected variant) is rejected — write `EntryKind.Cli`.
    let src = "import Core.Runtime.EntryKind; #[Entry(kind: Cli)] function main(): void { }";
    assert!(has(src, "E-INJECTED-VARIANT-BARE"), "{:?}", errors_of(src));
}

#[test]
fn bare_enum_name_as_kind_is_missing_variant_not_bogus_self_qualify() {
    // `kind: EntryKind` — the enum NAME with no variant — reports a MISSING variant, not a bare
    // variant needing `EntryKind.EntryKind` (which would be nonsensical).
    let src = "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind)] function main(): void { }";
    assert!(has(src, "E-ENTRY-KIND-REQUIRED"), "{:?}", errors_of(src));
    assert!(!has(src, "E-INJECTED-VARIANT-BARE"), "{:?}", errors_of(src));
}

#[test]
fn qualified_kind_without_import_is_unimported_error() {
    // `EntryKind.Cli` used without `import Core.Runtime.EntryKind;` — nothing in the wind.
    let src = "#[Entry(kind: EntryKind.Cli)] function main(): void { }";
    assert!(has(src, "E-UNIMPORTED"), "{:?}", errors_of(src));
}

#[test]
fn wrong_kind_qualifier_is_unknown_error() {
    // `Foo.Cli` — a qualifier other than `EntryKind` — is not a valid entry kind.
    let src = "import Core.Runtime.EntryKind; #[Entry(kind: Foo.Cli)] function main(): void { }";
    assert!(has(src, "E-ENTRY-KIND-UNKNOWN"), "{:?}", errors_of(src));
}

#[test]
fn whole_module_runtime_import_enables_qualified_kind() {
    // `import Core.Runtime;` (whole-module) also binds `EntryKind` — no E-UNIMPORTED.
    let src = "import Core.Runtime; #[Entry(kind: EntryKind.Cli)] function main(): void { }";
    assert!(!has(src, "E-UNIMPORTED"), "{:?}", errors_of(src));
    assert!(!has(src, "E-INJECTED-VARIANT-BARE"), "{:?}", errors_of(src));
}

#[test]
fn single_class_static_main_is_not_multiple() {
    let src = "class App { static function main(): void { } }";
    assert!(!has(src, "E-MULTIPLE-MAIN"), "{:?}", errors_of(src));
}

#[test]
fn deep_dotted_kind_chain_does_not_overflow_the_stack() {
    // Guards `flatten_dotted_path` (the entry `kind:` qualifier reader) against unbounded
    // per-segment recursion. A pathological `#[Entry(kind: a.a.a.…)]` chain reaches it via the
    // attribute-arg path, which bypasses both depth guards (attr args are never `check_expr`'d, so
    // `MAX_EXPR_DEPTH` never fires; member access parses left-associatively, so the parser's
    // `MAX_NEST_DEPTH` counts the chain once). This test drives the checker `check()` path (collect +
    // check_program), where the now-iterative flatten is the walker, and asserts a 50k chain
    // classifies cleanly instead of aborting (50k not `EntryKind` → E-ENTRY-KIND-UNKNOWN). On an
    // 8 MiB thread a recursive-flatten regression aborts here (debug frames overflow well below 200k).
    // SCOPE: this does NOT cover the full `phg check`/run pipeline, whose first pass
    // `enforce_injected::walk_expr` has its own pre-existing guard-free recursion over the same chain
    // (the general deep-chain hazard in KNOWN_ISSUES.md) — closing that is out of DEC-337 scope.
    let ok = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let chain = vec!["a"; 50_000].join(".");
            let src = format!(
                "import Core.Runtime.EntryKind; #[Entry(kind: {chain})] function main(): void {{ }}"
            );
            has(&src, "E-ENTRY-KIND-UNKNOWN")
        })
        .expect("spawn checker thread")
        .join()
        .expect("deep `kind:` chain must not overflow the stack");
    assert!(
        ok,
        "deep dotted `kind:` chain should classify as E-ENTRY-KIND-UNKNOWN, not crash"
    );
}

#[test]
fn entry_diagnostics_quote_the_qualified_form_not_bare() {
    // DEC-337: even for a VALIDLY-declared entry, the E-ENTRY-SIG and E-DUPLICATE-ENTRY-KIND
    // message text must quote `#[Entry(kind: EntryKind.Cli)]` — never the now-rejected bare form
    // (the checker would otherwise suggest a spelling it itself rejects as E-INJECTED-VARIANT-BARE).
    let sig =
        "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Web)] function main(): void { }";
    let sig_msg = errors_of(sig)
        .into_iter()
        .find(|e| e.code == Some("E-ENTRY-SIG"))
        .expect("E-ENTRY-SIG fires")
        .message;
    assert!(
        sig_msg.contains("EntryKind."),
        "must quote qualified form: {sig_msg}"
    );
    assert!(
        !sig_msg.contains("kind: Web)"),
        "must not quote bare form: {sig_msg}"
    );

    let dup =
        "import Core.Runtime.EntryKind; #[Entry(kind: EntryKind.Cli)] function main(): void { } \
               class App { #[Entry(kind: EntryKind.Cli)] static function main(): void { } }";
    let dup_msg = errors_of(dup)
        .into_iter()
        .find(|e| e.code == Some("E-DUPLICATE-ENTRY-KIND"))
        .expect("E-DUPLICATE-ENTRY-KIND fires")
        .message;
    assert!(
        dup_msg.contains("EntryKind."),
        "must quote qualified form: {dup_msg}"
    );
    assert!(
        !dup_msg.contains("kind: Cli)"),
        "must not quote bare form: {dup_msg}"
    );
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
