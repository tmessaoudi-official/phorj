//! PHP transpiler — the `__phorj_db_*` SAVEPOINT helpers (DEC-340), gated by `uses_db`.
//!
//! **Why these exist at all.** PDO's `beginTransaction()` does NOT nest — a second call throws. Phorj's
//! `begin()` does nest (it opens a `SAVEPOINT`), and DEC-340 gives `db.transaction` depth semantics that
//! depend on that nesting: auto-rollback unwinds to the depth it found on ENTRY. Mapping `begin()`
//! straight onto `beginTransaction()` therefore could not express the language's own semantics, and the
//! closure form was not implemented on this leg at all — the emitter was a literal placeholder comment.
//!
//! Shipping that placeholder is what Invariant 14 forbids (a silent semantic downgrade), and the
//! developer ruled the fix: emit a `__phorj_*` helper. Invariant 16 admits that explicitly as a
//! legitimate tool when the trade is surfaced rather than self-decided; the rejected alternative was to
//! keep the leg behind a hard `E-TRANSPILE-*` quarantine.
//!
//! **The model.** One depth counter per PDO handle, held in a `SplObjectStorage` keyed by the handle, so
//! two phorj bindings of the same connection share it exactly as the Rust side shares an
//! `Rc<Cell<u32>>`. Depth 0 → 1 is a real `beginTransaction()`; deeper levels are `SAVEPOINT phorj_sp_N`,
//! matching the SQL the Rust legs emit (`ops.rs`), so the two legs' savepoint names agree.

use super::*;

/// The `__phorj_db_*` helper source, exposed so `tests/db_savepoints.rs` can execute it under a REAL
/// `php` + PDO and prove the savepoint arithmetic composes there (DEC-340 item 3's actual intent).
/// That test is what makes the helpers verified rather than merely written, while `Core.Database` stays
/// Ladder case 2 — lifting that quarantine needs a separate developer ruling.
#[must_use]
pub fn db_helper_source() -> &'static str {
    DB_HELPERS
}

impl Transpiler {
    pub(super) fn emit_db_helpers(&mut self) {
        if !self.gates.uses_db {
            return;
        }
        for line in DB_HELPERS.lines() {
            self.line(line);
        }
    }
}

/// The helper bodies as literal PHP. Kept literal (no interpolation) for the same reason as the FS
/// helpers: it is easier to keep in sync with the Rust bodies in `src/ext/database/natives/ops.rs`.
///
/// The savepoint NAMES are load-bearing: `phorj_sp_{remaining}` matches `ops.rs` exactly, so a database
/// inspected mid-transaction looks the same whichever leg ran the program.
pub const DB_HELPERS: &str = r#"function &__phorj_db_depths() {
    static $depths = null;
    if ($depths === null) { $depths = new SplObjectStorage(); }
    return $depths;
}
function __phorj_db_tx_depth($pdo) {
    $d = &__phorj_db_depths();
    // `offsetExists`, NOT `contains`: SplObjectStorage::contains() is DEPRECATED as of PHP 8.5, which is
    // the transpile floor — it printed a deprecation notice onto stdout on every depth read, which would
    // have broken byte-identity outright. Caught by running these helpers under the real oracle
    // (`tests/db_savepoints.rs`) rather than by reading them.
    return $d->offsetExists($pdo) ? $d[$pdo] : 0;
}
function __phorj_db_set_depth($pdo, $n) {
    $d = &__phorj_db_depths();
    $d[$pdo] = $n;
}
function __phorj_db_begin($pdo) {
    $depth = __phorj_db_tx_depth($pdo);
    if ($depth === 0) { $pdo->beginTransaction(); }
    else { $pdo->exec('SAVEPOINT phorj_sp_' . $depth); }
    __phorj_db_set_depth($pdo, $depth + 1);
    return $depth + 1;
}
function __phorj_db_commit($pdo) {
    $depth = __phorj_db_tx_depth($pdo);
    if ($depth === 0) { return 0; }
    $remaining = $depth - 1;
    if ($remaining === 0) { $pdo->commit(); }
    else { $pdo->exec('RELEASE phorj_sp_' . $remaining); }
    __phorj_db_set_depth($pdo, $remaining);
    return $remaining;
}
function __phorj_db_rollback($pdo) {
    $depth = __phorj_db_tx_depth($pdo);
    if ($depth === 0) { return 0; }
    $remaining = $depth - 1;
    __phorj_db_set_depth($pdo, $remaining);
    if ($remaining === 0) { $pdo->rollBack(); }
    else { $pdo->exec('ROLLBACK TO phorj_sp_' . $remaining . '; RELEASE phorj_sp_' . $remaining); }
    return $remaining;
}
function __phorj_db_unwind_to($pdo, $target) {
    while (__phorj_db_tx_depth($pdo) > $target) { __phorj_db_rollback($pdo); }
    return __phorj_db_tx_depth($pdo);
}
function __phorj_db_rollback_all($pdo) {
    return __phorj_db_unwind_to($pdo, 0);
}
"#;
