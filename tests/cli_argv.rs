//! R9-G-1: a non-UTF-8 command-line argument is refused with one line on stderr and exit 1 — never a
//! panic (exit 101). `argv[0]` (the name `phg` was invoked by) is only a label and is read lossily.
//! R9-G-2: a non-UTF-8 environment variable (key or value) is left out of `Environment.all()`, as
//! `Environment.get` already answers `null` for it, on the three native legs. The PHP leg passes the raw bytes through
//! (`getenv`), a disclosed split (KNOWN_ISSUES "NONUTF8-PROCESS-INPUT"); DEC-590 (PENDING) decides it.
#![cfg(unix)]

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_phg");

fn bytes(b: &[u8]) -> &OsStr {
    OsStr::from_bytes(b)
}

fn refused(out: &Output, what: &str) {
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{what}: {err}");
    assert!(out.stdout.is_empty(), "{what}: stdout {:?}", out.stdout);
    assert!(!err.contains("panicked"), "{what}: {err}");
    assert!(err.contains("not valid UTF-8"), "{what}: {err}");
}

#[test]
fn a_non_utf8_argument_is_refused_not_a_panic() {
    let trailing = Command::new(BIN)
        .arg("check")
        .arg("examples/hello.phg")
        .arg(bytes(b"\xff"))
        .output()
        .unwrap();
    refused(&trailing, "trailing argument");
    let err = String::from_utf8_lossy(&trailing.stderr);
    assert!(
        err.contains("argument 3") && err.contains(r#""\xFF""#),
        "{err}"
    );

    let file = Command::new(BIN)
        .arg("check")
        .arg(bytes(b"a\xff.phg"))
        .output()
        .unwrap();
    refused(&file, "file name");

    let program_arg = Command::new(BIN)
        .args(["run", "examples/process/main-args.phg", "--", "ok"])
        .arg(bytes(b"\xfe\xff"))
        .output()
        .unwrap();
    refused(&program_arg, "program argument after --");
}

#[test]
fn valid_non_ascii_arguments_and_a_non_utf8_program_name_still_run() {
    let out = Command::new(BIN)
        .arg0(bytes(b"ph\xffg"))
        .args(["run", "examples/process/main-args.phg", "--", "é", "🙂"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "argc = 2\narg: é\narg: 🙂\nmatches Process.arguments() = true\n"
    );
}

const ENV_PROBE: &str = r#"package Main;

import Core.Output;
import Core.Map;
import Core.Environment;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;

#[Entry(kind: EntryKind.Cli)]
function main(): void {
    var env = Environment.all();
    Output.printLine("good = {Map.get(env, \"PHORJ_GOOD\") ?? \"<none>\"}");
    Output.printLine("bad in all = {Map.has(env, \"PHORJ_BAD\")}");
    Output.printLine("bad get = {Environment.get(\"PHORJ_BAD\") ?? \"<unset>\"}");
    Output.printLine("count = {Map.size(env)}");
}
"#;

#[test]
fn a_non_utf8_environment_variable_is_left_out_of_all_on_the_native_legs() {
    let dir = std::env::temp_dir().join(format!("phorj_env_utf8_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("envprobe.phg");
    std::fs::write(&file, ENV_PROBE).unwrap();
    for leg in [&[][..], &["--no-jit"][..], &["--tree-walker"][..]] {
        let out = Command::new(BIN)
            .arg("run")
            .args(leg)
            .arg(&file)
            .env_clear() // only the three variables below: `count` pins that the bad KEY is left out too
            .env("PHORJ_GOOD", "ok")
            .env("PHORJ_BAD", bytes(b"\xff\xfe"))
            .env(bytes(b"PHORJ_\xffKEY"), "v")
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{leg:?}: {err}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "good = ok\nbad in all = false\nbad get = <unset>\ncount = 1\n",
            "{leg:?}: {err}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The php binary, or `None` with a loud SKIP — a FAILURE under `PHORJ_REQUIRE_PHP=1` (skip-loud).
fn php_or_gate(label: &str) -> Option<String> {
    let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
    let ok = Command::new(&cand)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if ok {
        return Some(cand);
    }
    assert!(
        std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
        "{label}: php required (PHORJ_REQUIRE_PHP=1) but not found on PATH or $PHORJ_PHP"
    );
    eprintln!("SKIP {label}: php not found — set PHORJ_REQUIRE_PHP=1 to make this a failure");
    None
}

const ARGS_PROBE: &str = r#"package Main;

import Core.Output;
import Core.List;
import Core.Process;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;

function listed(): void {
    for (string s in Process.arguments()) {
        Output.printLine("helper [{s}]");
    }
}

#[Entry(kind: EntryKind.Cli)]
function main(List<string> args): int {
    listed();
    var f = function(): int { return List.length(Process.arguments()); };
    Output.printLine("lambda {f()}");
    Output.printLine("main {List.length(args)}");
    return List.length(args);
}
"#;

/// PROCESS-ARGS-PHP-SCOPE: `Process.arguments()` read from a helper function and from a lambda gives the same
/// arguments on the PHP leg as natively. Its PHP twin used `$argv`, which a PHP function does not see, so it
/// returned `[]` there. `Process` programs are quarantined from the differential (impure), so this is where the
/// PHP leg is compared.
#[test]
fn process_arguments_inside_a_function_match_on_the_php_leg() {
    let Some(php) = php_or_gate("process_arguments_inside_a_function_match_on_the_php_leg") else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("phorj_args_php_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("argsprobe.phg");
    std::fs::write(&src, ARGS_PROBE).unwrap();
    let args = ["alpha", "b c"];
    let native = Command::new(BIN)
        .arg("run")
        .arg(&src)
        .arg("--")
        .args(args)
        .output()
        .unwrap();
    let transpiled = Command::new(BIN)
        .arg("transpile")
        .arg(&src)
        .output()
        .unwrap();
    assert!(
        transpiled.status.success(),
        "{}",
        String::from_utf8_lossy(&transpiled.stderr)
    );
    let php_file = dir.join("argsprobe.php");
    std::fs::write(&php_file, &transpiled.stdout).unwrap();
    let php_out = Command::new(&php)
        .arg("-n")
        .arg(&php_file)
        .args(args)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let want = "helper [alpha]\nhelper [b c]\nlambda 2\nmain 2\n";
    assert_eq!(String::from_utf8_lossy(&native.stdout), want);
    assert_eq!(
        String::from_utf8_lossy(&php_out.stdout),
        want,
        "{}",
        String::from_utf8_lossy(&php_out.stderr)
    );
    assert!(
        php_out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&php_out.stderr)
    );
    assert_eq!(native.status.code(), Some(2));
    assert_eq!(php_out.status.code(), Some(2));
}
