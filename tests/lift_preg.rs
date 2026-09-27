//! Scout row 4l-a (DEC-540) — the `preg_match` translation certified against real PCRE.
//!
//! The lifter's claims are claims about PCRE: that PHP's `$` is `\n?\z` at the end of an alternative,
//! that a pattern without `u` reads `\d` `\w` `\s` as ASCII, and that a byte atom lifts only where
//! byte mode and character mode give the same existence answer. A unit test of the translation cannot
//! certify any of that; running the ORIGINAL pattern under PHP and the LIFTED one on every phorj leg
//! can. `the_refused_shapes_really_differ` is the other half: each refusal rule is shown to guard a
//! real difference, so none of them is caution that could be dropped.
//!
//! Gating as `lift_roundtrip.rs`: `PHORJ_REQUIRE_PHP=1` makes a missing `php` a failure, and
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

/// Run `src` under `php -n` (no ini — `preg_*` is core, nothing else is needed).
fn run_php(php: &str, src: &str, label: &str) -> String {
    let path = std::env::temp_dir().join(format!("phorj_lift_preg_{label}.php"));
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

/// Subjects chosen to split byte mode from character mode, `$` from `\z`, and ASCII from Unicode.
const SUBJECTS: &str = r#"['', 'abc', "abc\n", "abc\n\n", 'ABC', 'é', "é\n", 'aéb', "a\nb", 'K', '12', '١٢', 'x y', 'b', 'ab', 'rdc', 'ardc', 'x/y', 'a~b']"#;

/// Every accepted shape of `lifter/preg/tests.rs`, as a PHP existence test.
const PATTERNS: &[&str] = &[
    r"'/^abc$/'",
    r"'/^abc\Z/'",
    r"'/^abc$/D'",
    r"'/(abc$)/'",
    r"'/a$|^b/'",
    r"'/^\d+$/'",
    r"'/^[\d.]+$/'",
    r"'/\w/'",
    r"'/\s/'",
    r"'/^[^0-9]+$/'",
    r"'/[^a-z]/'",
    r"'/^a.*b$/s'",
    r"'/^\w+$/u'",
    r"'/abc/iu'",
    r"'/k/iu'",
    r"'/(?<![a-z])rdc/u'",
    r"'#^x/y$#'",
    r"'~a\~b~'",
];

#[test]
fn lifted_patterns_answer_as_pcre_does() {
    let Some(php) = php_or_gate("lifted_patterns_answer_as_pcre_does") else {
        return;
    };
    let mut body = String::new();
    for p in PATTERNS {
        body.push_str(&format!(
            "foreach ({SUBJECTS} as $s) {{ echo t(1 === preg_match({p}, $s)); }}\necho \"\\n\";\n"
        ));
    }
    // A helper, not `echo $c ? 'T' : 'F'` — an echoed ternary is LIFT-ECHO-TERNARY (KNOWN_ISSUES).
    let src = format!("<?php\nfunction t(bool $b): string {{ return $b ? 'T' : 'F'; }}\n{body}");
    let phorj = lift::lifter::lift_source(&src).unwrap_or_else(|e| panic!("lift: {e}"));
    let expected = run_php(&php, &src, "orig");
    assert_eq!(expected.lines().count(), PATTERNS.len(), "{expected}");
    let tw = cmd_treewalk(&phorj).unwrap_or_else(|e| panic!("tree-walker: {e}\n{phorj}"));
    for (k, (want, got)) in expected.lines().zip(tw.lines()).enumerate() {
        assert_eq!(got, want, "tree-walker ≠ PCRE for {}\n{phorj}", PATTERNS[k]);
    }
    assert_eq!(tw, expected);
    let vm = cmd_run(&phorj).unwrap_or_else(|e| panic!("vm: {e}"));
    assert_eq!(vm, expected, "VM ≠ PCRE");
    let back = cli::cmd_transpile(&phorj).unwrap_or_else(|e| panic!("transpile: {e}"));
    assert_eq!(
        run_php(&php, &back, "back"),
        expected,
        "transpiled back ≠ PCRE"
    );
}

/// `(original PHP pattern, what a naive lift would compile, subject spelled in PHP and in phorj)`.
/// Each must be REFUSED by the lifter and must really answer differently — the refusal is load-bearing.
const REFUSED: &[(&str, &str, &str, &str)] = &[
    // A byte atom whose count matters: one byte in PCRE, one character in phorj.
    (r"/^[^0-9]$/", r"^[^0-9]\n?\z", "'é'", "\"é\""),
    // `i` without `u`: PCRE folds ASCII only.
    (r"/k/i", r"(?i)k", "'\u{212A}'", "\"\u{212A}\""), // U+212A KELVIN SIGN
    // `\b` without `u`: `é` is not a word character to PCRE's byte mode.
    (r"/\bab/", r"\bab", "'éab'", "\"éab\""),
    // `m`: PCRE's `^` does not match after a newline that ends the subject.
    (r"/^$/m", r"(?m)^$", "\"a\\n\"", "\"a\\n\""),
    // `\x` above 0x7F names a byte, not U+0080–U+00FF.
    (r"/[\x80-\xff]/", r"[\x80-\xff]", "'ā'", "\"ā\""),
];

#[test]
fn the_refused_shapes_really_differ() {
    let Some(php) = php_or_gate("the_refused_shapes_really_differ") else {
        return;
    };
    for (k, (pattern, naive, php_subject, phg_subject)) in REFUSED.iter().enumerate() {
        let src = format!(
            "<?php\nfunction f(string $s): bool {{ return 1 === preg_match('{pattern}', $s); }}\n\
             echo f({php_subject}) ? 'T' : 'F';\n"
        );
        let err = lift::lifter::lift_source(&src).expect_err(pattern);
        assert!(err.contains("preg_match"), "{pattern}: {err}");
        let pcre = run_php(&php, &src, &format!("refused{k}"));
        let lit: String = naive
            .chars()
            .flat_map(|c| match c {
                '\\' => vec!['\\', '\\'],
                '{' => vec!['\\', '{'],
                '}' => vec!['\\', '}'],
                c => vec![c],
            })
            .collect();
        let phg = format!(
            "package Main;\nimport Core.Output;\nimport Core.Regex;\nimport Core.Runtime.Entry;\n\
             import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{\n    \
             if (Regex.matches(Regex.compile(\"{lit}\"), {phg_subject})) {{ Output.print(\"T\"); }} \
             else {{ Output.print(\"F\"); }}\n}}\n"
        );
        let native = cmd_run(&phg).unwrap_or_else(|e| panic!("{pattern}: {e}\n{phg}"));
        assert_ne!(
            native, pcre,
            "{pattern}: PCRE and the naive lift agree on {php_subject}, so this refusal guards nothing"
        );
    }
}
