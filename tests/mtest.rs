//! `phg test` runner integration tests (M-Test T3). Drives `cli::cmd_test` against throwaway temp
//! directories holding `.phg` test files — covering a passing test, a failing assertion, a real
//! runtime fault, a file-level type error, and the discovery/exit-code contract.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use phorj::cli;

/// A unique temp dir, removed on drop.
struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let unique = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "phorj_mtest_it_{tag}_{}_{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, contents).unwrap();
        p
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The committed self-hosted suite under `selftest/` must always pass (it doubles as the M-Test
/// showcase — `selftest/README.md`). It lives outside `examples/`, so the byte-identity differential
/// never touches it; this is its gate.
#[test]
fn the_selftest_suite_is_green() {
    let suite = Path::new(env!("CARGO_MANIFEST_DIR")).join("selftest");
    let (report, code) = cli::cmd_test(&[suite.display().to_string()]);
    assert_eq!(code, 0, "selftest/ suite must be green:\n{report}");
    assert!(report.contains("0 failed"), "{report}");
}

#[test]
fn all_passing_tests_exit_zero() {
    let d = TempDir::new("pass");
    d.write(
        "ok_test.phg",
        "package Main;\nimport Core.Test;\n\
         test \"adds\" { Test.assertEquals(2 + 2, 4); }\n\
         test \"booleans\" { Test.assertTrue(1 < 2); Test.assertFalse(2 < 1); }\n",
    );
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    assert_eq!(code, 0, "all-pass should exit 0:\n{report}");
    assert!(report.contains("2 passed, 0 failed"), "{report}");
}

#[test]
fn failing_assertion_is_reported_and_exits_one() {
    let d = TempDir::new("fail");
    d.write(
        "bad_test.phg",
        "package Main;\nimport Core.Test;\n\
         test \"good\" { Test.assertEquals(1, 1); }\n\
         test \"bad\" { Test.assertEquals(1, 2); }\n",
    );
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    assert_eq!(code, 1, "a failure should exit 1:\n{report}");
    assert!(report.contains("1 passed, 1 failed"), "{report}");
    assert!(report.contains("assertion failed"), "{report}");
    assert!(
        report.contains(":: bad"),
        "failing test name in report:\n{report}"
    );
}

#[test]
fn real_runtime_fault_fails_the_test() {
    let d = TempDir::new("fault");
    d.write(
        "oob_test.phg",
        "package Main;\n\
         test \"out of range\" { var xs = [1, 2, 3]; var bad = xs[9]; }\n",
    );
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    assert_eq!(code, 1, "{report}");
    assert!(report.contains("list index out of range"), "{report}");
}

#[test]
fn a_type_error_in_a_test_file_is_a_failure() {
    let d = TempDir::new("typeerr");
    d.write(
        "type_test.phg",
        "package Main;\ntest \"bad types\" { var y = 1 + true; }\n",
    );
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    assert_eq!(code, 1, "{report}");
    assert!(
        report.contains("<check>"),
        "file-level check failure:\n{report}"
    );
}

#[test]
fn no_test_files_found_exits_zero() {
    let d = TempDir::new("empty");
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    assert_eq!(code, 0, "{report}");
    assert!(report.contains("no test files found"), "{report}");
}

#[test]
fn assert_faults_passes_when_the_closure_faults() {
    let d = TempDir::new("faults");
    d.write(
        "faults_test.phg",
        "package Main;\nimport Core.Test;\n\
         test \"oob faults\" { Test.assertFaults(function() => [1, 2, 3][9]); }\n\
         test \"no fault is a failure\" { Test.assertFaults(function() => 1 + 1); }\n",
    );
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    assert_eq!(code, 1, "{report}");
    assert!(report.contains("1 passed, 1 failed"), "{report}");
    assert!(
        report.contains(":: oob faults ... ok"),
        "the faulting closure should pass:\n{report}"
    );
    assert!(
        report.contains("expected the closure to fault"),
        "the non-faulting closure should fail:\n{report}"
    );
}

#[test]
fn a_single_file_path_runs_just_that_file() {
    let d = TempDir::new("single");
    let f = d.write(
        "one_test.phg",
        "package Main;\nimport Core.Test;\ntest \"x\" { Test.assertTrue(true); }\n",
    );
    let (report, code) = cli::cmd_test(&[f.display().to_string()]);
    assert_eq!(code, 0, "{report}");
    assert!(report.contains("1 passed, 0 failed"), "{report}");
}

/// KNOWN_ISSUES §TEST-RAW-CHECKER — `phg test` used to run the RAW checker, so every symbol that
/// exists only in an injected prelude was reported as the user's own type error.
///
/// `phg check` and `phg run` both accept the program below; `phg test` rejected it with a wall of
/// `E-UNKNOWN-IDENT` and three cascading consequences, none of which is a real defect in the file.
/// This is precisely the hole DEC-252 closed for the LSP and left open here: the LSP was routed
/// through the shared front end, `phg test` was not, so `phg check` ≡ LSP held while
/// `phg check` ≢ `phg test`.
///
/// The blast radius is every injected module — `Core.Http`, `Core.Database`, `Core.Secret`,
/// `Core.DI`'s `inject<T>()`, `Core.Runtime.EntryKind` — so a whole class of programs simply could
/// not be `phg test`ed, and the failure blamed the user.
#[test]
fn a_test_file_using_an_injected_prelude_type_is_checked_through_the_front_end() {
    let d = TempDir::new("prelude");
    d.write(
        "http_test.phg",
        "package Main;\nimport Core.Test;\nimport Core.Http;\nimport Core.Http.ServeConfig;\n\
         test \"the injected ServeConfig is constructible\" {\n\
         \x20   ServeConfig cfg = new ServeConfig(port: 3000);\n\
         \x20   Test.assertEquals(cfg.port, 3000);\n\
         }\n",
    );
    let (report, code) = cli::cmd_test(&[d.path().display().to_string()]);
    // The point of the test is the ABSENCE of the spurious diagnostic, so assert on that first —
    // a bare exit-code check would also pass if the runner started failing for some new reason.
    assert!(
        !report.contains("E-UNKNOWN-IDENT"),
        "an injected-prelude symbol must not be reported as an unknown identifier:\n{report}"
    );
    assert!(
        !report.contains(":: <check> ... FAILED"),
        "the file-level check must not reject an injected-prelude program:\n{report}"
    );
    assert_eq!(code, 0, "the test file should pass:\n{report}");
    assert!(report.contains("1 passed, 0 failed"), "{report}");
}
