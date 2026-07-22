//! Completion unit tests — split from `completion/mod.rs` at the Invariant-13 hard cap
//! (M-Decomp; behaviour-identical move).
use super::complete;

/// Extract every `"label":"…"` value from a completion response (assert on CONTENT, not just count).
fn labels(resp: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = resp;
    while let Some(i) = rest.find("\"label\":\"") {
        rest = &rest[i + 9..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    out
}

// The key regression this slice fixes: completion on an INCOMPLETE buffer (parse fails) must still
// work — before 2026-07-20 every case below returned `[]` because `symbol_at` required a parse.

#[test]
fn import_context_lists_core_modules() {
    let src = "package Main;\nimport Core.\n";
    let offset = src.find("Core.").unwrap() + "Core.".len(); // right after the dot
    let got = labels(&complete(
        src,
        offset,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        got.iter().any(|l| l == "Core.Json"),
        "want Core.Json in {got:?}"
    );
    assert!(
        got.iter().any(|l| l == "Core.Http"),
        "want Core.Http in {got:?}"
    );
    // Raw `Core.Native.*` twins are excluded (users import the friendly module).
    assert!(
        !got.iter().any(|l| l.starts_with("Core.Native.")),
        "raw twins leaked: {got:?}"
    );
}

#[test]
fn import_context_filters_by_prefix() {
    let src = "import Core.J";
    let got = labels(&complete(
        src,
        src.len(),
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        got.iter().any(|l| l == "Core.Json"),
        "want Core.Json in {got:?}"
    );
    assert!(
        got.iter().all(|l| l.starts_with("Core.J")),
        "prefix not applied: {got:?}"
    );
}

#[test]
fn member_context_lists_module_natives_on_incomplete_buffer() {
    // `Output.` with nothing after ⇒ the buffer does NOT parse; member completion must still fire.
    let src = "package Main;\nfunction main(): void {\n  Output.\n}\n";
    let offset = src.find("Output.").unwrap() + "Output.".len();
    let got = labels(&complete(
        src,
        offset,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        got.iter().any(|l| l == "printLine"),
        "want printLine in {got:?}"
    );
    assert!(!got.is_empty());
}

#[test]
fn unresolved_lowercase_receiver_emits_neither_module_members_nor_keywords() {
    // A lowercase receiver is an instance, never a Core module → must NOT emit module members. And
    // an UNRESOLVED receiver (no declared type in scope) emits nothing — member context is
    // conservative; it must NOT dump general/keyword completions after a `.`.
    let src = "function main(): void {\n  myvar.\n}\n";
    let offset = src.find("myvar.").unwrap() + "myvar.".len();
    let got = labels(&complete(
        src,
        offset,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        !got.iter().any(|l| l == "map"),
        "no module members: {got:?}"
    );
    assert!(
        !got.iter().any(|l| l == "function"),
        "member context must not fall back to keywords: {got:?}"
    );
}

// Instance/type-aware member completion (this./typed-receiver.) — works on the INCOMPLETE buffer
// via the repaired parse, resolving the receiver's declared type → the class's members + inherited.

#[test]
fn this_member_completion_includes_own_and_inherited() {
    let src = "package Main;\n\
               class Animal {\n  public string name = \"\";\n  function speak(): void {}\n}\n\
               class Dog extends Animal {\n  function bark(): void {}\n  function go(): void {\n    this.\n  }\n}\n";
    let off = src.find("this.").unwrap() + "this.".len();
    let got = labels(&complete(
        src,
        off,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(got.contains(&"bark".to_string()), "own method: {got:?}");
    assert!(
        got.contains(&"speak".to_string()),
        "inherited method: {got:?}"
    );
    assert!(
        got.contains(&"name".to_string()),
        "inherited field: {got:?}"
    );
}

#[test]
fn typed_local_member_completion() {
    // Type-first typed local `Dog d = …` (NOT `var d: Dog` — `var` is the inferred form).
    let src = "package Main;\n\
               class Animal { function speak(): void {} }\n\
               class Dog extends Animal { function bark(): void {} }\n\
               function main(): void {\n  Dog d = new Dog();\n  d.\n}\n";
    let off = src.find("  d.").unwrap() + "  d.".len();
    let got = labels(&complete(
        src,
        off,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(got.contains(&"bark".to_string()), "own: {got:?}");
    assert!(got.contains(&"speak".to_string()), "inherited: {got:?}");
}

#[test]
fn inferred_or_unknown_receiver_yields_nothing() {
    // `var x = …` has no DECLARED type (Type::Infer) → conservative gate emits nothing (never a
    // wrong member list). Also an undeclared receiver.
    let src = "package Main;\nfunction main(): void {\n  var x = 1;\n  x.\n}\n";
    let off = src.find("  x.").unwrap() + "  x.".len();
    let got = labels(&complete(
        src,
        off,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        !got.iter().any(|l| l == "bark" || l == "speak"),
        "must not invent members for an inferred receiver: {got:?}"
    );
}

#[test]
fn general_completion_includes_open_sibling_buffer_symbols() {
    // A function/class defined in ANOTHER open project buffer completes in this file's general ctx.
    let mut docs: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    docs.insert(
        "file:///lib.phg".to_string(),
        "package App;\nfunction helper(): void {}\nclass Widget {}\n".to_string(),
    );
    let src = "package Main;\nfunction main(): void {\n  \n}\n";
    let off = src.find("  \n").unwrap() + 2; // empty line inside main body → general ctx
    let got = labels(&complete(src, off, None, Some("file:///t.phg"), &docs));
    assert!(got.contains(&"helper".to_string()), "sibling fn: {got:?}");
    assert!(
        got.contains(&"Widget".to_string()),
        "sibling class: {got:?}"
    );
}

#[test]
fn general_completion_survives_the_mid_typing_parse_error() {
    // THE real-world regression (dev field report 2026-07-22): a half-typed identifier makes the
    // buffer unparseable, and completion then dropped every symbol — the user typing `Out` saw only
    // keywords, which VSCode's prefix filter turned into an EMPTY popup ("no autocomplete"). The
    // repaired parse (cursor line blanked) must keep top-level symbols, locals, and the imported
    // module qualifiers alive.
    let src = "package Main;\n\nimport Core.Output;\n\nfunction helper() -> int { return 1; }\n\n#[Entry]\nfunction main() -> void {\n    var greeting = \"hi\";\n    Out\n}\n";
    let off = src.find("    Out").unwrap() + "    Out".len();
    let got = labels(&complete(
        src,
        off,
        None, // the live buffer does NOT parse — exactly the mid-typing state
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        got.contains(&"Output".to_string()),
        "imported module qualifier missing: {got:?}"
    );
    assert!(
        got.contains(&"helper".to_string()),
        "top-level fn missing on broken buffer: {got:?}"
    );
    assert!(
        got.contains(&"greeting".to_string()),
        "local missing on broken buffer: {got:?}"
    );
    assert!(
        got.contains(&"main".to_string()),
        "enclosing fn missing on broken buffer: {got:?}"
    );
}

#[test]
fn import_completion_includes_native_only_modules() {
    // `Core.Output`/`Core.Map` live only in native::registry() (no prelude twin) — the catalog
    // used to list just the prelude CORE_MODULES, silently hiding every native-only module.
    let src = "package Main;\nimport Core.\n";
    let offset = src.find("Core.").unwrap() + "Core.".len();
    let got = labels(&complete(
        src,
        offset,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        got.iter().any(|l| l == "Core.Output"),
        "native-only module Core.Output missing: {got:?}"
    );
    assert!(
        got.iter().any(|l| l == "Core.Map"),
        "native-only module Core.Map missing: {got:?}"
    );
    // The raw twins stay excluded even from the union.
    assert!(
        !got.iter().any(|l| l.starts_with("Core.Native.")),
        "raw twins leaked: {got:?}"
    );
}

#[test]
fn general_context_offers_keywords_without_a_parse() {
    // Even a buffer that does not parse yields keywords (never a bare `[]`).
    let got = labels(&complete(
        "packag",
        6,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(
        got.iter().any(|l| l == "package"),
        "want keyword 'package' in {got:?}"
    );
}
