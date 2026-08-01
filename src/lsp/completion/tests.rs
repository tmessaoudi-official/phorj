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

/// Extract each item's `textEdit.newText` (or None if the item has no textEdit).
fn text_edits(resp: &str) -> Vec<(String, String)> {
    // Pair each label with its textEdit newText by scanning item objects.
    let mut out = Vec::new();
    for item in resp.split("{\"label\":\"").skip(1) {
        let label = item.split('"').next().unwrap_or("").to_string();
        if let Some(i) = item.find("\"newText\":\"") {
            let nt = item[i + "\"newText\":\"".len()..]
                .split('"')
                .next()
                .unwrap_or("")
                .to_string();
            out.push((label, nt));
        }
    }
    out
}

#[test]
fn import_completion_replaces_typed_path_not_appends() {
    // Regression (user-reported): typing `import Core.` then accepting `Core.Output` must yield
    // `Core.Output`, NOT `Core.Core.Output`. Each import item carries a textEdit that REPLACES the
    // already-typed `Core.` — verify the range spans `Core.` and newText is the full label.
    let src = "package Main;\nimport Core.\n";
    let offset = src.find("Core.").unwrap() + "Core.".len();
    let resp = complete(src, offset, None, None, &std::collections::HashMap::new());
    // Every import item newText equals its label (the full path — the client replaces the typed span).
    let edits = text_edits(&resp);
    assert!(!edits.is_empty(), "expected textEdit items, got: {resp}");
    for (label, new_text) in &edits {
        assert_eq!(
            label, new_text,
            "newText must be the full label (no dup prefix)"
        );
    }
    // The replace range must start at the `C` of `Core.` on line 1 (char 7 = after `import `), end at
    // the cursor (char 12 = after `Core.`) — so `Core.` is replaced, not appended after.
    assert!(
        resp.contains("\"start\":{\"line\":1,\"character\":7}")
            && resp.contains("\"end\":{\"line\":1,\"character\":12}"),
        "import textEdit range must span the typed `Core.` (line 1, chars 7..12): {resp}"
    );
}

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

/// A MEMBER-GATED module can only be used through a member import, so `import Core.ErrorModule.` had
/// to complete its six types or DEC-421's taxonomy was unreachable from the editor — Invariant 17's
/// 100% rule. Before this, `import Core.` offered module PATHS only and a trailing `.` produced an
/// empty list for every module, which is the same class of hole `withLock` fell through.
#[test]
fn a_module_path_plus_a_dot_completes_its_member_imports() {
    let cases = [
        // (typed prefix, a member that must be offered)
        ("import Core.ErrorModule.", "Core.ErrorModule.RuntimeError"),
        (
            "import Core.FileSystemModule.",
            "Core.FileSystemModule.FileSystem",
        ),
        // A registry-only module: its members are natives, not injected types.
        ("import Core.Output.", "Core.Output.printLine"),
    ];
    for (src, want) in cases {
        let got = labels(&complete(
            src,
            src.len(),
            None,
            None,
            &std::collections::HashMap::new(),
        ));
        assert!(got.iter().any(|l| l == want), "want {want} in {got:?}");
    }
    // Filtering still applies to the member segment, and the six are complete.
    let src = "import Core.ErrorModule.";
    let got = labels(&complete(
        src,
        src.len(),
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    for want in [
        "RuntimeError",
        "LogicError",
        "MathError",
        "TypeMismatchError",
        "InvalidValueError",
        "IoError",
    ] {
        assert!(
            got.iter().any(|l| l == &format!("Core.ErrorModule.{want}")),
            "want Core.ErrorModule.{want} in {got:?}"
        );
    }
    let partial = "import Core.ErrorModule.Math";
    let got = labels(&complete(
        partial,
        partial.len(),
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert_eq!(
        got,
        vec!["Core.ErrorModule.MathError"],
        "prefix not applied"
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

/// Invariant 17's 100% rule, as a ratchet: a prelude STATIC must surface in completion the moment it
/// exists, with no LSP-side edit. `FileSystem.tryWithLock` (DEC-348) is the live case — both lock
/// entry points must appear, so a future prelude addition that the LSP silently fails to enumerate
/// fails here instead of shipping as "the compiler knows it but the editor doesn't".
/// DEC-419: a documented declaration carries its `/** … */` text as `documentation` in the completion
/// item, so the picker's detail pane shows it. Asserts on the raw JSON because `documentation` is a
/// field the `labels` helper discards.
#[test]
fn completion_items_carry_the_doc_comment_as_documentation() {
    // `plain` is deliberately UNdocumented and declared on its own line: the repaired parse blanks the
    // CURSOR's line, so a decl sharing that line would vanish from the item list entirely.
    let src = "package Main;\n/** Doubles `n`. */\nfunction helper(int n): int { return n; }\nfunction plain(): void {}\nfunction main(): void { hel }\n";
    let offset = src.find("hel }").unwrap() + 3;
    let resp = complete(src, offset, None, None, &std::collections::HashMap::new());
    // Split the response into per-item chunks so "which item carries the doc" is actually asserted —
    // a substring search over the whole payload would pass if ANY item had it.
    let items: Vec<&str> = resp.split("{\"label\":\"").collect();
    let helper = items
        .iter()
        .find(|i| i.starts_with("helper\""))
        .unwrap_or_else(|| panic!("no `helper` item in {resp}"));
    assert!(
        helper.contains("documentation") && helper.contains("Doubles"),
        "doc missing from completion item: {helper}"
    );
    // An UNdocumented declaration must not gain an empty `documentation` (a blank detail pane).
    let plain = items
        .iter()
        .find(|i| i.starts_with("plain\""))
        .unwrap_or_else(|| panic!("no `plain` item in {resp}"));
    assert!(
        !plain.contains("documentation"),
        "undocumented decl gained an empty documentation: {plain}"
    );
}

#[test]
fn prelude_statics_surface_in_member_completion_without_an_lsp_edit() {
    let src =
        "package Main;\nimport Core.FileSystemModule;\nfunction main(): void {\n  FileSystem.\n}\n";
    let offset = src.find("FileSystem.").unwrap() + "FileSystem.".len();
    let got = labels(&complete(
        src,
        offset,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    for want in ["withLock", "tryWithLock", "lines", "forEachLine"] {
        assert!(got.iter().any(|l| l == want), "want {want} in {got:?}");
    }
    // `acquireLock` is `private` — an internal `using` subject, not user-facing surface. Offering it
    // would advertise exactly the leak-prone shape the DEC-348 ruling rejected. Same for the raw
    // `readLinesChunk` native behind `lines` (DEC-347): it is `Core.Native.FileSystem`, not user surface.
    for hidden in ["acquireLock", "readLinesChunk"] {
        assert!(
            !got.iter().any(|l| l == hidden),
            "internal `{hidden}` must NOT be offered: {got:?}"
        );
    }
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
    let src = "package Main;\n\nimport Core.Output;\n\nfunction helper() -> int { return 1; }\n\n#[Entry(kind: EntryKind.Cli)]\nfunction main() -> void {\n    var greeting = \"hi\";\n    Out\n}\n";
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
