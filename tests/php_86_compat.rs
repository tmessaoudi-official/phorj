//! PHP 8.6/8.7 forward-compat of the TRANSPILED output (parity review 2026-10-01, finding D-4.1–4.3).
//!
//! The 8.5 oracle that gates every commit cannot see these: each is clean on 8.5 and breaks on 8.6+
//! (a name PHP took, and a default `trim()` character list that grew `\f`). They are asserted on the
//! emitted PHP text, which is version-independent, and each case is sabotaged (drop the reserve /
//! restore the bare `trim`) so a regression reddens it.

use phorj::cli;

fn transpile(body: &str) -> Result<String, String> {
    let src = format!(
        "package Main;\nimport Core.Output;\nimport Core.String;\nimport Core.Ini;\nimport Core.Json;\n\
         import Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n{body}\n\
         #[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ Output.printLine(\"x\"); }}\n"
    );
    cli::cmd_transpile(&src)
}

#[test]
fn a_user_function_named_after_an_8_6_builtin_is_mangled() {
    {
        let name = "clamp";
        let php = transpile(&format!("function {name}(int x): int {{ return x; }}"))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            !php.contains(&format!("function {name}("))
                && php.contains(&format!("function {name}_(")),
            "`{name}` is a PHP builtin on 8.5+/8.6+, so the user function must be mangled:\n{php}"
        );
    }
}

#[test]
fn a_user_type_named_after_an_8_6_builtin_class_is_rejected() {
    for name in [
        "SortDirection",
        "StreamError",
        "StreamException",
        "StreamErrorStore",
        "StreamErrorMode",
        "StreamErrorCode",
        "StreamPollHandle",
    ] {
        let err = transpile(&format!("enum {name} {{ Asc, Desc }}"))
            .expect_err(&format!("`{name}` must be reserved"));
        assert!(err.contains("E-RESERVED-NAME"), "{name}: {err}");
    }
}

#[test]
fn trim_ascii_emits_the_explicit_character_list() {
    // PHP 8.6 added `\f` to `trim()`'s DEFAULT list (rfc/trim_form_feed); Phorj's set never had it.
    let php = transpile("function t(string s): string { return String.trimAscii(s); }").unwrap();
    assert!(php.contains(r#"trim($s, " \t\n\r\0\x0B")"#), "{php}");
}

#[test]
fn the_ini_and_json_line_helpers_never_use_the_default_trim_list() {
    let php = transpile(
        "function f(string s): void { var a = Ini.parse(s); var b = Json.parseLines(s); \
         Output.printLine(\"y\"); }",
    )
    .unwrap();
    for bare in [
        "trim($line)",
        "trim(substr($t, 0, $eq))",
        "trim(substr($t, $eq + 1))",
        "trim(substr($t, 1, -1))",
    ] {
        assert!(
            !php.contains(bare),
            "default-list `{bare}` survived:\n{php}"
        );
    }
}
