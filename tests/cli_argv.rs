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
