//! Scout row 4l-b2 (DEC-554) — a `preg_match` with a captures array, lifted onto `Regex.first`,
//! certified against real PCRE.
//!
//! The lift hoists `RegexMatch? m = Regex.first(…)` in front of the statement whose FIRST evaluated
//! test is the `preg_match`, turns the comparison into `m is null`, and reads `$m[k]` as
//! `m.at(k) ?? ""`. Each claim is a claim about PHP's `$m` array — a middle group that did not take
//! part is `""`, a trailing one is unset, a named group is also numbered — so the only certificate is
//! running the ORIGINAL program under PHP and the LIFTED one on every phorj leg.
//! `a_fallback_on_a_middle_group_really_differs` shows the one refusal is load-bearing.
//!
//! Gating as `lift_preg.rs`: `PHORJ_REQUIRE_PHP=1` makes a missing `php` a failure, and
//! `PHORJ_SKIP_PHP=1` (the pre-commit tier) skips loudly.

use phorj::cli::{cmd_run, cmd_treewalk};
use phorj::{cli, lift};
use std::process::Command;

fn php_or_gate(test: &str) -> Option<String> {
    if std::env::var("PHORJ_SKIP_PHP").as_deref() != Ok("1") {
        let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
        let ok = Command::new(&cand)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            return Some(cand);
        }
    }
    assert!(
        std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
        "{test}: php required (PHORJ_REQUIRE_PHP=1) but not found"
    );
    eprintln!("SKIP {test}: php not found — set PHORJ_REQUIRE_PHP=1 to make this a failure");
    None
}

fn run_php(php: &str, src: &str, label: &str) -> String {
    let path = std::env::temp_dir().join(format!("phorj_lift_preg_caps_{label}.php"));
    std::fs::write(&path, src).expect("write temp php");
    let out = Command::new(php)
        .arg("-n")
        .arg(&path)
        .output()
        .expect("spawn php");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "php failed for {label}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8 php stdout")
}

/// Four statement shapes, each fed subjects that make a group match, sit out in the MIDDLE (PHP: `""`),
/// sit out at the END (PHP: unset), and fail to match at all.
const PROGRAM: &str = r#"<?php
function guard(string $s): string
{
    if (1 !== preg_match('/^(\d+)(?:-(\d+))?(x)?$/D', $s, $m)) {
        return 'no';
    }
    return $m[0] . '|' . $m[1] . '|' . ($m[2] ?? '') . '|' . ($m[3] ?? 'X');
}
function positive(string $s): string
{
    if (preg_match('/(?<k>[a-z]+)=(?<v>[0-9]*)(!)?/', $s, $m) === 1) {
        return $m['k'] . ':' . $m['v'] . ':' . $m[2] . ':' . ($m[3] ?? '-');
    }
    return 'none';
}
function charset(string $s): string
{
    $c = preg_match('/charset="?([^";]+)"?/iu', $s, $g) === 1 ? $g[1] : 'UTF-8';
    return $c;
}
function hasB(string $s): bool
{
    return preg_match('/^a(b)?$/', $s, $g) === 1 && ($g[1] ?? '') === 'b';
}
function t(bool $b): string
{
    return $b ? 'T' : 'F';
}
foreach (['12', '12-34', '12x', '12-34x', 'ab', ''] as $s) {
    echo guard($s), "\n";
}
foreach (['k=1', 'k=', 'k=1!', 'x', 'ab=12!'] as $s) {
    echo positive($s), "\n";
}
foreach (['text/plain; charset=ISO-8859-1', 'text/plain; CHARSET="koi8"', 'text/plain'] as $s) {
    echo charset($s), "\n";
}
foreach (['ab', 'a', 'abb', ''] as $s) {
    echo t(hasB($s)), "\n";
}
"#;

#[test]
fn lifted_captures_answer_as_pcre_does() {
    let Some(php) = php_or_gate("lifted_captures_answer_as_pcre_does") else {
        return;
    };
    let phorj = lift::lifter::lift_source(PROGRAM).unwrap_or_else(|e| panic!("lift: {e}"));
    let expected = run_php(&php, PROGRAM, "orig");
    assert_eq!(expected.lines().count(), 18, "{expected}");
    let tw = cmd_treewalk(&phorj).unwrap_or_else(|e| panic!("tree-walker: {e}\n{phorj}"));
    assert_eq!(tw, expected, "tree-walker ≠ PCRE\n{phorj}");
    let vm = cmd_run(&phorj).unwrap_or_else(|e| panic!("vm: {e}"));
    assert_eq!(vm, expected, "VM ≠ PCRE");
    let back = cli::cmd_transpile(&phorj).unwrap_or_else(|e| panic!("transpile: {e}"));
    assert_eq!(
        run_php(&php, &back, "back"),
        expected,
        "transpiled back ≠ PCRE"
    );
}

/// `$m[2] ?? 'X'` on a MIDDLE group: PHP answers `""` when group 2 sat out and group 3 matched, where
/// `m.at(2) ?? "X"` answers `X`. The lifter refuses it; this shows the refusal guards a real difference.
#[test]
fn a_fallback_on_a_middle_group_really_differs() {
    let Some(php) = php_or_gate("a_fallback_on_a_middle_group_really_differs") else {
        return;
    };
    let src = "<?php\nfunction f(string $s): string {\n    \
               if (1 !== preg_match('/^(\\d+)(?:-(\\d+))?(x)?$/', $s, $m)) { return 'no'; }\n    \
               return $m[2] ?? 'X';\n}\necho f('12x');\n";
    let err = lift::lifter::lift_source(src).expect_err("a middle-group fallback must be refused");
    assert!(err.contains("preg_match"), "{err}");
    assert_eq!(run_php(&php, src, "middle"), "");
    let phg = "package Main;\nimport Core.Output;\nimport Core.Regex;\nimport Core.Runtime.Entry;\n\
               import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {\n    \
               RegexMatch? m = Regex.first(Regex.compile(r\"^([0-9]+)(?:-([0-9]+))?(x)?\\n?\\z\"), \"12x\");\n    \
               if (m is null) { return; }\n    Output.print(m.at(2) ?? \"X\");\n}\n";
    assert_eq!(cmd_run(phg).unwrap_or_else(|e| panic!("{e}\n{phg}")), "X");
}

/// Row 4l-b2's `$m[0]` with a trailing `$` (panel finding, row 4l-b3): PHP's `$` is zero-width before a
/// final newline, the lifted `\n?\z` consumes it. The lifter refuses the read; this shows the refusal
/// guards a real difference.
#[test]
fn a_whole_match_under_a_rewritten_dollar_really_differs() {
    let Some(php) = php_or_gate("a_whole_match_under_a_rewritten_dollar_really_differs") else {
        return;
    };
    let src = "<?php\nfunction f(string $s): string {\n    \
               if (preg_match('/[a-z]+$/u', $s, $m) === 1) { return '[' . $m[0] . ']'; }\n    \
               return '';\n}\necho f(\"cd\\n\");\n";
    let err = lift::lifter::lift_source(src).expect_err("must be refused");
    assert!(err.contains("final newline"), "{err}");
    assert_eq!(run_php(&php, src, "dollar"), "[cd]");
}

/// Round 3 (F4, F5): PHP's answers for the two shapes the lifter now refuses by name — a `.` without
/// `u` captures ONE BYTE of `é`, and `(a?)+b` reports the group as the empty string its last iteration
/// matched where the native engines report the last non-empty iteration.
#[test]
fn a_byte_atom_and_a_nullable_repeat_really_differ() {
    let Some(php) = php_or_gate("a_byte_atom_and_a_nullable_repeat_really_differ") else {
        return;
    };
    let cases = [
        ("/./", "é", "c3", "no `u` modifier", 0),
        ("/(a?)+b/", "aab", "", "can match the empty string", 1),
    ];
    for (i, (pat, subject, expect, why, group)) in cases.iter().enumerate() {
        let src = format!(
            "<?php\nfunction f(string $s): string {{\n    \
             if (preg_match('{pat}', $s, $m) === 1) {{ return bin2hex($m[{group}]); }}\n    return 'none';\n}}\n\
             echo f(\"{subject}\");\n"
        );
        let err = lift::lifter::lift_source(&src).expect_err("must be refused");
        assert!(err.contains(why), "{pat}: want `{why}` in: {err}");
        assert_eq!(
            run_php(&php, &src, &format!("round3_{i}")),
            *expect,
            "{pat}"
        );
    }
}
