//! R9-G-1: a non-UTF-8 command-line argument is refused with one line on stderr and exit 1 — never a
//! panic (exit 101). `argv[0]` (the name `phg` was invoked by) is only a label and is read lossily.
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
