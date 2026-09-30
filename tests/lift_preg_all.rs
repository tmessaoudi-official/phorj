//! Scout row 4l-b3 (DEC-554) — `preg_match_all`, lifted onto `Regex.all`, certified against real PCRE.
//!
//! The lift hoists `List<RegexMatch> m = Regex.all(…)`, turns the count into `m.length()`, and reads
//! `$m[k]` as the COLUMN of group `k` — `m.map(r => r.at(k) ?? "")`, or with `PREG_OFFSET_CAPTURE`
//! `(text, byte offset)` pairs. Each claim is a claim about PHP's PATTERN_ORDER array — a group that
//! sat out is `""` in every column, and `-1` as an offset, and offsets are BYTES (the subject below
//! carries a two-byte `é` before a match) — so the certificate is running the ORIGINAL program under
//! PHP and the LIFTED one on every phorj leg.
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
    let path = std::env::temp_dir().join(format!("phorj_lift_preg_all_{label}.php"));
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

/// Columns of a pattern whose groups sit out in the middle and at the end; offset pairs after a
/// two-byte `é`; a count guard that finds nothing.
const PROGRAM: &str = r#"<?php
function cols(string $s): string
{
    if (preg_match_all('/([a-z])(-)?([0-9])?/u', $s, $m) === 0) {
        return 'none';
    }
    $out = '';
    foreach ($m[0] as $x) {
        $out = $out . $x . ';';
    }
    $out = $out . '|';
    foreach ($m[2] as $x) {
        $out = $out . '[' . $x . ']';
    }
    $out = $out . '|';
    foreach ($m[3] as $x) {
        $out = $out . '[' . $x . ']';
    }
    return $out;
}
function offsets(string $s): string
{
    if (preg_match_all('/x(?<d>[0-9])?/u', $s, $m, PREG_OFFSET_CAPTURE) < 1) {
        return 'none';
    }
    $out = '';
    foreach ($m[0] as [$t, $at]) {
        $out = $out . $t . '@' . $at . ' ';
    }
    foreach ($m['d'] as [$t, $at]) {
        $out = $out . '<' . $t . '@' . $at . '>';
    }
    return $out;
}
foreach (['a-1b', 'a-b1', 'ab', '', '9'] as $s) {
    echo cols($s), "\n";
}
foreach (['x1 x', 'éx2éx', 'é', 'xx'] as $s) {
    echo offsets($s), "\n";
}
"#;

#[test]
fn lifted_columns_answer_as_pcre_does() {
    let Some(php) = php_or_gate("lifted_columns_answer_as_pcre_does") else {
        return;
    };
    let phorj = lift::lifter::lift_source(PROGRAM).unwrap_or_else(|e| panic!("lift: {e}"));
    let expected = run_php(&php, PROGRAM, "orig");
    assert_eq!(expected.lines().count(), 9, "{expected}");
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
