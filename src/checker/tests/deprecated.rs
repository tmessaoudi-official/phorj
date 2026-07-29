//! DEC-417 — the userland `#[Deprecated(message: "…")]` attribute.
//!
//! The ruling has two halves and both are pinned here: the declaration is MARKED (harvested onto its
//! `FnSig`, which is what the LSP tags), and every USE site is REPORTED (`W-DEPRECATED` carrying
//! `DiagnosticTag.Deprecated`, which is what makes an editor strike the call through).
//!
//! Also pinned: the mark does NOT spread. A function that calls a deprecated function does not itself
//! become deprecated — matching Rust/Kotlin/Swift/C#, none of which propagate (META-7 cross-language
//! scan). If that ever changes it must change here first.

use super::support::*;

/// `#[Deprecated]` needs its provider imported, like `#[Entry]`/`#[Config]` (DEC-337, nothing in the
/// wind). Every test below leans on this preamble.
const PRE: &str = "import Core.Runtime.Deprecated;\n";

fn warn_codes(src: &str) -> Vec<String> {
    warnings_of(src)
        .iter()
        .filter_map(|d| d.code.map(str::to_string))
        .collect()
}

fn deprecation_texts(src: &str) -> Vec<String> {
    warnings_of(src)
        .iter()
        .filter(|d| d.code == Some("W-DEPRECATED"))
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn calling_a_deprecated_function_warns_with_the_authors_message() {
    let src = format!(
        "{PRE}#[Deprecated(message: \"use shout\")] function yell(string s): string {{ return s; }}\n\
         function main() -> void {{ string x = yell(\"hi\"); }}"
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    let texts = deprecation_texts(&src);
    assert_eq!(
        texts.len(),
        1,
        "expected exactly one warning, got {texts:?}"
    );
    assert!(
        texts[0].contains("`yell` is deprecated") && texts[0].contains("use shout"),
        "the warning must name the symbol AND carry the author's message: {texts:?}"
    );
}

#[test]
fn a_bare_deprecated_with_no_message_is_legal_and_still_warns() {
    let src = format!(
        "{PRE}#[Deprecated] function old(): int {{ return 1; }}\n\
         function main() -> void {{ int x = old(); }}"
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    let texts = deprecation_texts(&src);
    assert_eq!(texts.len(), 1, "{texts:?}");
    assert!(
        texts[0].contains("`old` is deprecated") && !texts[0].contains(':'),
        "with no message the warning should name only the symbol: {texts:?}"
    );
}

#[test]
fn a_live_function_never_warns() {
    let src = format!(
        "{PRE}function fine(): int {{ return 1; }}\n\
         function main() -> void {{ int x = fine(); }}"
    );
    assert!(
        deprecation_texts(&src).is_empty(),
        "a non-deprecated call must be silent: {:?}",
        warn_codes(&src)
    );
}

#[test]
fn the_declaration_itself_does_not_warn_only_uses_do() {
    // The declaration is MARKED (for the LSP to tag), never warned about — otherwise an author could
    // not deprecate anything without warning in their own file.
    let src = format!("{PRE}#[Deprecated] function old(): int {{ return 1; }}");
    assert!(
        deprecation_texts(&src).is_empty(),
        "declaring a deprecation must not warn: {:?}",
        deprecation_texts(&src)
    );
}

#[test]
fn deprecation_does_not_spread_to_the_caller() {
    // DEC-417.5 scope: `wrapper` CALLS a deprecated function, so the call inside it warns — but
    // `wrapper` itself is NOT deprecated, so calling `wrapper` warns nothing further. Exactly one
    // warning in total, at the one real use site.
    let src = format!(
        "{PRE}#[Deprecated(message: \"gone soon\")] function old(): int {{ return 1; }}\n\
         function wrapper(): int {{ return old(); }}\n\
         function main() -> void {{ int x = wrapper(); }}"
    );
    let texts = deprecation_texts(&src);
    assert_eq!(
        texts.len(),
        1,
        "the mark must not propagate to callers — expected 1 warning (inside `wrapper`), got {texts:?}"
    );
    assert!(texts[0].contains("`old`"), "{texts:?}");
}

#[test]
fn an_interpolated_message_is_rejected() {
    // Compile-time-only metadata: there is no runtime to evaluate `{x}` against, so the text would be
    // silently lost. Rejected loudly instead.
    let src = format!(
        "{PRE}function other(): int {{ return 0; }}\n\
         #[Deprecated(message: \"use {{other}}\")] function old(): int {{ return 1; }}"
    );
    let errs = errors_of(&src);
    assert!(
        errs.iter().any(|d| d.code == Some("E-DEPRECATED-MESSAGE")),
        "expected E-DEPRECATED-MESSAGE, got {errs:?}"
    );
}

#[test]
fn a_positional_argument_is_rejected() {
    // The named `message:` is the spelling, matching `#[Entry(kind: …)]`.
    let src = format!("{PRE}#[Deprecated(\"use shout\")] function old(): int {{ return 1; }}");
    let errs = errors_of(&src);
    assert!(
        errs.iter().any(|d| d.code == Some("E-DEPRECATED-MESSAGE")),
        "expected E-DEPRECATED-MESSAGE, got {errs:?}"
    );
}

// NOTE: import-gating (`E-INJECTED-TYPE-BARE` on an unimported bare `#[Deprecated]`) is NOT asserted
// here. It is enforced by the `enforce_injected` pass, which runs in the CLI's `check_and_expand`
// chokepoint rather than inside `check()`, so this unit harness cannot observe it. Verified against the
// shipped binary instead — `phg check` on an unimported `#[Deprecated]` reports
// `E-INJECTED-TYPE-BARE`, exactly like `#[Entry]`/`#[Config]` — and pinned by the CLI-level test in
// `src/cli/tests/`. Asserting it from here would have produced a test that passes for the wrong reason.

#[test]
fn a_deprecated_method_warns_at_its_call_site() {
    let src = format!(
        "{PRE}class Thing {{\n\
           constructor() {{}}\n\
           #[Deprecated(message: \"use fresh\")] public function stale(): int {{ return 1; }}\n\
         }}\n\
         function main() -> void {{ Thing t = new Thing(); int x = t.stale(); }}"
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    let texts = deprecation_texts(&src);
    assert_eq!(
        texts.len(),
        1,
        "expected one method-call warning, got {texts:?}"
    );
    assert!(texts[0].contains("use fresh"), "{texts:?}");
}
