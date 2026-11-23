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

/// DEC-594: PHP 8.6 deprecates a FUNCTION named `let`, `is` or `readonly` (namespaced too; a METHOD of
/// that name is not deprecated, probed on 8.6.0 2026-11-23), so the user function is renamed in the
/// output exactly like `clamp`, and its calls follow.
#[test]
fn a_user_function_named_after_an_8_6_deprecated_word_is_mangled() {
    for name in ["let", "is", "readonly"] {
        let php = transpile(&format!(
            "function {name}(int x): int {{ return x; }}\nfunction use{name}(): int {{ return {name}(1); }}"
        ))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            !php.contains(&format!("function {name}("))
                && php.contains(&format!("function {name}_(")),
            "`{name}` is deprecated as a PHP function name on 8.6, so it must be mangled:\n{php}"
        );
        assert!(
            php.contains(&format!("{name}_(1)")),
            "the call must follow the mangle:\n{php}"
        );
    }
}

/// DEC-594: PHP 8.6 deprecates `Is` and `Let` as a class, enum, interface or trait name (probed on
/// 8.6.0 for all four kinds, 2026-11-23), so they are `E-RESERVED-NAME` on every leg, like
/// `SortDirection` — never renamed, since a class name can surface in output.
#[test]
fn a_user_type_named_after_an_8_6_deprecated_word_is_rejected() {
    for name in ["Is", "Let", "IS", "let"] {
        for decl in [
            format!("class {name} {{}}"),
            format!("enum {name} {{ A, B }}"),
            format!("interface {name} {{}}"),
            format!("trait {name} {{}}"),
        ] {
            let err = transpile(&decl).expect_err(&format!("`{decl}` must be reserved"));
            assert!(err.contains("E-RESERVED-NAME"), "{decl}: {err}");
            // The word is a parse error on no PHP: the message must say why it is refused.
            assert!(
                err.contains("PHP 8.6"),
                "{decl}: the message must name 8.6: {err}"
            );
        }
    }
}

#[test]
fn trim_ascii_emits_the_explicit_character_list() {
    // DEC-588 (ruled 2026-11-23): `trimAscii` follows PHP 8.6's default list, `\f` included, and the
    // list stays explicit so the PHP 8.5 leg (whose default lacks `\f`) agrees too.
    let php = transpile("function t(string s): string { return String.trimAscii(s); }").unwrap();
    assert!(php.contains(r#"trim($s, " \t\n\r\0\x0B\x0C")"#), "{php}");
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
