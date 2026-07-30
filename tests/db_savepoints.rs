//! DEC-340 item 3 — the `__phorj_db_*` savepoint helpers, executed under a REAL `php` + PDO.
//!
//! **Why this file exists in this shape.** The spec asked for "the savepoint helper, with a test that
//! nested begin/rollback composes under PDO". Building it surfaced that `Core.Database` is deliberately
//! QUARANTINED by `E-TRANSPILE-DB` (Ladder case 2), so the helpers are not reachable through
//! `phg transpile` at all — the placeholder emitter they replace was equally unreachable. Lifting that
//! quarantine is a case-2 → case-1 move for the whole module and needs a developer ruling (the blocker
//! is not savepoints, it is porting `DatabaseResult` + the 7-kind error taxonomy to PHP: without it
//! `catch (UniqueViolationError)` never matches and `db.transaction(fn, retries)` silently never
//! retries).
//!
//! So this proves the helpers CORRECT against the real thing, which is what item 3 was really for, while
//! the Ladder decision stays open. When the quarantine is lifted these become the leg's regression tests
//! unchanged.
//!
//! PHP gating mirrors `tests/conformance.rs`: `PHORJ_PHP` overrides the binary, `PHORJ_SKIP_PHP=1`
//! forces a skip, `PHORJ_REQUIRE_PHP=1` turns a missing `php` into a failure rather than a skip.

use std::process::Command;

fn php_bin() -> Option<String> {
    if std::env::var("PHORJ_SKIP_PHP").as_deref() == Ok("1") {
        return None;
    }
    let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
    let ok = Command::new(&cand)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some(cand)
}

fn php_or_gate(label: &str) -> Option<String> {
    if let Some(p) = php_bin() {
        return Some(p);
    }
    assert!(
        std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
        "{label}: php required (PHORJ_REQUIRE_PHP=1) but not found on PATH or $PHORJ_PHP"
    );
    eprintln!("[skip] {label}: no php available");
    None
}

/// Run `script` with the real helper source prepended, and return its stdout.
fn run_with_helpers(label: &str, script: &str) -> Option<String> {
    let php = php_or_gate(label)?;
    // The helper source is taken from the transpiler itself — never re-typed here, so this test cannot
    // pass against a stale copy of the helpers.
    let src = format!(
        "<?php\n{}\n{}\n",
        phorj::transpile::db_php::db_helper_source(),
        script
    );
    let path = std::env::temp_dir().join(format!("phorj_sp_{label}_{}.php", std::process::id()));
    std::fs::write(&path, src).expect("write php");
    let out = Command::new(&php).arg(&path).output().expect("run php");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "{label}: php exited non-zero\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

const SETUP: &str = r#"$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE acct(id INTEGER PRIMARY KEY, bal INTEGER)');
$pdo->exec('INSERT INTO acct(id, bal) VALUES (1, 100)');
function bal($pdo) { return (int) $pdo->query('SELECT bal FROM acct WHERE id = 1')->fetchColumn(); }
function upd($pdo, $n) { $pdo->exec('UPDATE acct SET bal = ' . $n . ' WHERE id = 1'); }
"#;

#[test]
fn nested_begin_and_rollback_compose_under_pdo() {
    // The property PDO alone cannot give: `beginTransaction()` does NOT nest (a second call throws), so
    // phorj's nesting `begin()` is only expressible through these SAVEPOINT helpers.
    let Some(out) = run_with_helpers(
        "nested",
        &format!(
            r#"{SETUP}
echo __phorj_db_begin($pdo), " ";      // 1 — real beginTransaction
upd($pdo, 200);
echo __phorj_db_begin($pdo), " ";      // 2 — SAVEPOINT phorj_sp_1
upd($pdo, 300);
echo __phorj_db_rollback($pdo), " ";   // 1 — back to the savepoint: 200 survives
echo bal($pdo), " ";
echo __phorj_db_commit($pdo), " ";     // 0 — real commit
echo bal($pdo), "\n";
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "1 2 1 200 0 200", "got {out:?}");
}

#[test]
fn unwind_to_entry_depth_matches_the_rust_semantics() {
    // The DEC-340 rule itself, on the PHP side: unwinding to the depth found on ENTRY leaves a
    // caller-owned outer transaction intact, and discards everything opened above it.
    let Some(out) = run_with_helpers(
        "unwind",
        &format!(
            r#"{SETUP}
__phorj_db_begin($pdo);                // caller's own level, depth 1
upd($pdo, 555);
$entry = __phorj_db_tx_depth($pdo);
__phorj_db_begin($pdo);                // the transaction's level
upd($pdo, 999);
__phorj_db_begin($pdo);                // a LEAKED begin
upd($pdo, 777);
echo __phorj_db_unwind_to($pdo, $entry), " ";  // 1 — restore the depth we found
echo bal($pdo), " ";                            // 555 — the caller's work survived
__phorj_db_commit($pdo);
echo bal($pdo), "\n";                           // 555 — and committed
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "1 555 555", "got {out:?}");
}

#[test]
fn rollback_all_unwinds_every_level_under_pdo() {
    let Some(out) = run_with_helpers(
        "rollback_all",
        &format!(
            r#"{SETUP}
__phorj_db_begin($pdo); upd($pdo, 1);
__phorj_db_begin($pdo); upd($pdo, 2);
__phorj_db_begin($pdo); upd($pdo, 3);
echo __phorj_db_tx_depth($pdo), " ";
echo __phorj_db_rollback_all($pdo), " ";
echo bal($pdo), "\n";
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "3 0 100", "got {out:?}");
}

#[test]
fn the_depth_counter_is_shared_per_handle_not_global() {
    // The Rust side shares one `Rc<Cell<u32>>` across every binding of a connection. The PHP twin keys a
    // `SplObjectStorage` on the handle, so two connections must NOT see each other's depth — otherwise a
    // second connection would mis-nest.
    let Some(out) = run_with_helpers(
        "per_handle",
        r#"$a = new PDO('sqlite::memory:');
$b = new PDO('sqlite::memory:');
__phorj_db_begin($a);
__phorj_db_begin($a);
echo __phorj_db_tx_depth($a), " ", __phorj_db_tx_depth($b), "\n";
"#,
    ) else {
        return;
    };
    assert_eq!(out.trim(), "2 0", "got {out:?}");
}

#[test]
fn savepoint_names_match_the_rust_leg_exactly() {
    // Load-bearing: `phorj_sp_{remaining}` is the same name `src/ext/database/natives/ops_tx.rs` emits,
    // so a database inspected mid-transaction looks identical whichever leg ran the program. Asserted
    // against the helper SOURCE so a rename on either side breaks this test.
    let src = phorj::transpile::db_php::db_helper_source();
    assert!(
        src.contains("'SAVEPOINT phorj_sp_' . $depth"),
        "savepoint name drifted from the Rust leg: {src}"
    );
    assert!(
        src.contains("'RELEASE phorj_sp_' . $remaining"),
        "release name drifted: {src}"
    );
    assert!(
        src.contains("'ROLLBACK TO phorj_sp_' . $remaining"),
        "rollback-to name drifted: {src}"
    );
}
