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
fn a_nested_commit_releases_the_savepoint_and_keeps_its_work() {
    // The `RELEASE SAVEPOINT` branch (commit with levels still open), which no test reached before
    // DEC-351's D5 fold-in — every prior case committed at depth 1, i.e. the real `commit()`. It is
    // exactly the branch that carried the bare `RELEASE` spelling, so it is the one that would have
    // failed on a MySQL PDO handle.
    let Some(out) = run_with_helpers(
        "nested_commit",
        &format!(
            r#"{SETUP}
echo __phorj_db_begin($pdo), " ";      // 1
upd($pdo, 400);
echo __phorj_db_begin($pdo), " ";      // 2
upd($pdo, 500);
echo __phorj_db_commit($pdo), " ";     // 1 — RELEASE SAVEPOINT phorj_sp_1: 500 KEPT, outer still open
echo bal($pdo), " ";
echo __phorj_db_rollback($pdo), " ";   // 0 — the outer level rolls back to before 400
echo bal($pdo), "\n";
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "1 2 1 500 0 100", "got {out:?}");
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
fn savepoint_names_and_portable_forms_match_the_rust_leg_exactly() {
    // Load-bearing twice over. The NAME `phorj_sp_{remaining}` is the same one
    // `src/ext/database/natives/savepoint.rs` emits, so a database inspected mid-transaction looks
    // identical whichever leg ran the program. And the FORMS are the portable ones (DEC-351's D5 fix):
    // `RELEASE SAVEPOINT` / `ROLLBACK TO SAVEPOINT`, spelled in full because the keyword is optional in
    // SQLite/Postgres but MANDATORY in MySQL — the bare spellings this file used to assert would have
    // failed on a MySQL PDO handle. Asserted against the helper SOURCE, so a drift on either side breaks
    // this test; the Rust-side twin of this ratchet is `savepoint.rs`'s own source scan.
    let src = phorj::transpile::db_php::db_helper_source();
    assert!(
        src.contains("'SAVEPOINT phorj_sp_' . $depth"),
        "savepoint name drifted from the Rust leg: {src}"
    );
    assert!(
        src.contains("'RELEASE SAVEPOINT phorj_sp_' . $remaining"),
        "release form/name drifted: {src}"
    );
    assert!(
        src.contains("'ROLLBACK TO SAVEPOINT phorj_sp_' . $remaining"),
        "rollback-to form/name drifted: {src}"
    );
}

// ── The case-1 groundwork: the SQLSTATE → kind classifier (DEC-340 follow-on) ─────────────────────
//
// Lifting `Core.Database` to Ladder case 1 needs the PHP leg to produce the SAME `<<Kind>>`-tagged
// messages the Rust drivers do, because the phorj prelude parses that marker to decide which typed error
// to throw. Without it `catch (UniqueViolationError e)` never matches on the PHP leg and
// `db.transaction(fn, retries)` — which retries ONLY the transient class — silently never retries.
//
// These drive REAL PDO exceptions rather than synthesising codes, so the classifier is verified against
// what the driver actually reports.

#[test]
fn a_unique_violation_is_classified_from_a_real_pdo_exception() {
    let Some(out) = run_with_helpers(
        "unique",
        r#"$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE t(id INTEGER PRIMARY KEY)');
$pdo->exec('INSERT INTO t(id) VALUES (1)');
try { $pdo->exec('INSERT INTO t(id) VALUES (1)'); }
catch (PDOException $e) { echo __phorj_db_classify($e), "\n"; }
"#,
    ) else {
        return;
    };
    assert!(
        out.starts_with("<<UniqueViolationError>>"),
        "a duplicate PK must classify as UniqueViolationError, got {out:?}"
    );
}

#[test]
fn a_syntax_error_is_classified_from_a_real_pdo_exception() {
    let Some(out) = run_with_helpers(
        "syntax",
        r#"$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
try { $pdo->exec('SELCT oops FROM nowhere'); }
catch (PDOException $e) { echo __phorj_db_classify($e), "\n"; }
"#,
    ) else {
        return;
    };
    assert!(
        out.starts_with("<<SyntaxError>>"),
        "a mis-typed statement must classify as SyntaxError, got {out:?}"
    );
}

#[test]
fn a_not_null_violation_is_a_constraint_not_a_unique_violation() {
    // The discriminator matters: SQLite reports both through the same generic integrity class, and
    // mis-classifying would make a handler catch the wrong type.
    let Some(out) = run_with_helpers(
        "notnull",
        r#"$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT NOT NULL)');
try { $pdo->exec('INSERT INTO t(id, name) VALUES (1, NULL)'); }
catch (PDOException $e) { echo __phorj_db_classify($e), "\n"; }
"#,
    ) else {
        return;
    };
    assert!(
        out.starts_with("<<ConstraintViolationError>>"),
        "a NOT NULL violation must be ConstraintViolationError, not Unique: got {out:?}"
    );
}

#[test]
fn an_unclassifiable_error_stays_untagged() {
    // Deliberate: the prelude maps an untagged message to the base `DatabaseError`, exactly as an
    // unmatched Rust-side error does. Inventing a kind here would be worse than staying generic.
    let Some(out) = run_with_helpers(
        "untagged",
        r#"$e = new PDOException('something the drivers never emit');
echo __phorj_db_classify($e), "\n";
"#,
    ) else {
        return;
    };
    assert_eq!(
        out.trim(),
        "something the drivers never emit",
        "got {out:?}"
    );
}

#[test]
fn every_kind_the_rust_side_tags_is_reachable_in_the_php_classifier() {
    // Drift guard: the PHP classifier must be able to produce every kind the phorj prelude knows how to
    // throw. If a kind is added on the Rust side and not here, the PHP leg would silently downgrade it to
    // the base DatabaseError — which is exactly the failure mode that keeps this module at Ladder case 2.
    let src = phorj::transpile::db_php::db_helper_source();
    for kind in [
        "UniqueViolationError",
        "ConstraintViolationError",
        "SerializationFailureError",
        "SyntaxError",
        "ConnectionError",
        "TimeoutError",
    ] {
        assert!(
            src.contains(kind),
            "the PHP classifier cannot produce `{kind}` — it would silently degrade to DatabaseError"
        );
    }
}

// ── Case-1 step 2: the DatabaseResult protocol on the PHP leg ─────────────────────────────────────
//
// Every `Core.Native.Database` native returns `DatabaseResult.Ok(v)` / `Err(msg)`, which the phorj
// prelude MATCHES on to decide between returning a value and throwing a typed `DatabaseError`. The
// prelude is phorj source, so it already runs on this leg — producing the right variant is the whole
// contract. `DatabaseResult<T>` erases to DEC-329.3's `DatabaseResult_Ok`/`_Err` (generics are erased
// before any backend, Invariant 5), so the tests below declare those classes exactly as the transpiler
// emits them.

/// The two variant classes as the transpiler emits them for `enum DatabaseResult<T>`.
const RESULT_CLASSES: &str = r#"abstract class DatabaseResult {}
final class DatabaseResult_Ok extends DatabaseResult { public function __construct(public $value) {} }
final class DatabaseResult_Err extends DatabaseResult { public function __construct(public string $message) {} }
"#;

#[test]
fn db_try_wraps_a_success_as_databaseresult_ok() {
    let Some(out) = run_with_helpers(
        "try_ok",
        &format!(
            r#"{RESULT_CLASSES}
$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE t(id INTEGER PRIMARY KEY)');
$r = __phorj_db_try(fn() => $pdo->exec('INSERT INTO t(id) VALUES (1)'));
echo get_class($r), " ", var_export($r->value, true), "\n";
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "DatabaseResult_Ok 1", "got {out:?}");
}

#[test]
fn db_try_wraps_a_pdo_error_as_databaseresult_err_with_the_kind_tag() {
    // The join between step 1 and step 2: the Err payload must carry the `<<Kind>>` marker, because that
    // is what the prelude parses to pick WHICH typed error to throw.
    let Some(out) = run_with_helpers(
        "try_err",
        &format!(
            r#"{RESULT_CLASSES}
$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE t(id INTEGER PRIMARY KEY)');
$pdo->exec('INSERT INTO t(id) VALUES (1)');
$r = __phorj_db_try(fn() => $pdo->exec('INSERT INTO t(id) VALUES (1)'));
echo get_class($r), " ", (str_starts_with($r->message, '<<UniqueViolationError>>') ? 'tagged' : $r->message), "\n";
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "DatabaseResult_Err tagged", "got {out:?}");
}

#[test]
fn db_try_unit_still_returns_ok_for_a_discarded_payload() {
    let Some(out) = run_with_helpers(
        "try_unit",
        &format!(
            r#"{RESULT_CLASSES}
$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$r = __phorj_db_try_unit(fn() => $pdo->exec('CREATE TABLE t(id INTEGER)'));
echo get_class($r), " ", var_export($r->value, true), "\n";
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "DatabaseResult_Ok 0", "got {out:?}");
}

#[test]
fn db_try_does_not_launder_a_non_database_error_into_a_result() {
    // Deliberate: only PDOException is caught. A TypeError or a bug in the emitted expression is NOT a
    // database error, and turning it into `DatabaseResult.Err` would let a real defect be caught by
    // `catch (DatabaseError e)` and reported as a database problem. It must stay a hard fault, exactly as
    // a Rust-side panic does.
    let Some(out) = run_with_helpers(
        "try_passthrough",
        &format!(
            r#"{RESULT_CLASSES}
try {{
    __phorj_db_try(fn() => throw new RuntimeException('a real bug'));
    echo "WRONG: swallowed\n";
}} catch (RuntimeException $e) {{
    echo "propagated: ", $e->getMessage(), "\n";
}}
"#
        ),
    ) else {
        return;
    };
    assert_eq!(out.trim(), "propagated: a real bug", "got {out:?}");
}

// ── CD-14: the `decimal` PARITY facts, pinned rather than ruled ───────────────────────────────────
//
// I initially recommended the developer RULE on how DB decimals map to the PHP leg (exact TEXT round-trip
// vs disclosed float). Measuring it showed there is nothing to rule, so these tests pin the facts instead
// — and will fail if any of them stops being true, which is what would reopen the question.
//
// The three facts: `bind` does not accept `decimal` at all, so the write path is already text-based on
// BOTH legs; a TEXT column round-trips exactly; and a `NUMERIC` column loses precision inside SQLite's
// type affinity, BEFORE either leg sees the value — so it is a property of the schema, not a divergence.

#[test]
fn cd14_a_numeric_column_loses_precision_before_either_leg_sees_it() {
    // The load-bearing fact. If this ever starts round-tripping exactly, the storage layer changed and the
    // decimal mapping is worth revisiting. Note `CAST(... AS TEXT)` cannot recover it: the damage is done
    // at INSERT, by column affinity.
    let Some(out) = run_with_helpers(
        "cd14_numeric",
        r#"$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE m(amt NUMERIC)');
$pdo->exec("INSERT INTO m(amt) VALUES ('12345678901234567.89')");
echo (string) $pdo->query('SELECT CAST(amt AS TEXT) AS t FROM m')->fetchColumn(), "\n";
"#,
    ) else {
        return;
    };
    assert_ne!(
        out.trim(),
        "12345678901234567.89",
        "a NUMERIC column preserved full precision — the storage assumption behind CD-14 changed, so the \
         decimal mapping is worth revisiting"
    );
}

#[test]
fn cd14_a_text_column_round_trips_a_decimal_exactly_on_the_php_leg() {
    // The exact path, which is the one phorj's own API steers you to (`bind` takes a string, not a
    // decimal). The Rust leg was verified to produce the same value by hand:
    //   TEXT column + string bind -> getDecimal -> 12345678901234567.89
    let Some(out) = run_with_helpers(
        "cd14_text",
        r#"$pdo = new PDO('sqlite::memory:', null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$pdo->exec('CREATE TABLE m(amt TEXT)');
$s = $pdo->prepare('INSERT INTO m(amt) VALUES (?)');
$s->execute(['12345678901234567.89']);
echo (string) $pdo->query('SELECT amt FROM m')->fetchColumn(), "\n";
"#,
    ) else {
        return;
    };
    assert_eq!(
        out.trim(),
        "12345678901234567.89",
        "the exact TEXT path must round-trip on the PHP leg too — this is what makes CD-14 safe"
    );
}
