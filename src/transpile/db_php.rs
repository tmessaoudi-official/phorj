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
function __phorj_db_classify($e) {
    // Map a PDOException onto the SAME 7-kind taxonomy the Rust drivers produce, tagged with the same
    // `<<Kind>>` marker the phorj prelude parses (`DatabaseError.fail`). Because the prelude IS phorj
    // source, it already runs on this leg — so tagging here is the whole of what makes
    // `catch (UniqueViolationError e)` work in transpiled PHP, and what makes
    // `db.transaction(fn, retries)` retry the transient class instead of silently never retrying.
    //
    // Driver-agnostic SQLSTATE first (Postgres/MySQL agree on these), then the SQLite driver code, which
    // is what `sqlite.rs` keys on. An unmatched error stays UNTAGGED on purpose — the prelude maps that
    // to the base `DatabaseError`, exactly as an unmatched Rust error does.
    $state = $e->getCode();
    $msg = $e->getMessage();
    $info = $e->errorInfo ?? [];
    $driverCode = isset($info[1]) ? (int) $info[1] : 0;
    $kind = null;
    if ($state === '23505' || $state === '23000') {
        // 23505 = Postgres unique_violation. 23000 is the generic integrity class shared by MySQL and
        // SQLite, so it needs a discriminator. MySQL's 1062 IS unique-specific. SQLite's is NOT: it
        // reports code 19 (`SQLITE_CONSTRAINT`) for a duplicate AND for NOT NULL/foreign-key/check, and
        // the EXTENDED codes the Rust driver keys on (2067 `_UNIQUE`, 1555 `_PRIMARYKEY`) are not exposed
        // through PDO's `errorInfo`. So the message is the only discriminator there — verified against a
        // real driver: keying on 19 mis-classified a NOT NULL violation as a unique violation.
        $kind = ($driverCode === 1062 || __phorj_db_msg_is_unique($msg))
            ? 'UniqueViolationError'
            : 'ConstraintViolationError';
    } elseif ($state === '23503' || $state === '23502' || $state === '23514') {
        $kind = 'ConstraintViolationError';   // foreign key / not-null / check
    } elseif ($state === '40001' || $state === '40P01') {
        $kind = 'SerializationFailureError'; // serialization failure / deadlock — the retry target
    } elseif ($state === '42601' || $state === '42000' || $state === '42S02' || $state === '42S22') {
        $kind = 'SyntaxError';
    } elseif ($state === '08006' || $state === '08001' || $state === '08003' || $state === '08004') {
        $kind = 'ConnectionError';
    } elseif ($state === 'HYT00' || $state === 'HYT01') {
        $kind = 'TimeoutError';
    } elseif ($driverCode === 5 || $driverCode === 6) {
        $kind = 'SerializationFailureError'; // SQLITE_BUSY / SQLITE_LOCKED
    } elseif ($driverCode === 1) {
        $kind = 'SyntaxError';               // SQLITE_ERROR at prepare time
    }
    return $kind === null ? $msg : '<<' . $kind . '>>' . $msg;
}
function __phorj_db_try($fn) {
    // The `DatabaseResult` protocol on the PHP leg (case-1 step 2). Every `Core.Native.Database` native
    // returns `DatabaseResult.Ok(v)` / `Err(msg)`, which the phorj prelude MATCHES on to decide whether to
    // return a value or throw a typed `DatabaseError`. Since the prelude is phorj source it already runs
    // here, so producing the right variant is the whole of the contract.
    //
    // `DatabaseResult<T>` erases to the DEC-329.3 enum-scoped classes `DatabaseResult_Ok` (field `value`)
    // and `DatabaseResult_Err` (field `message`) — generics are erased before any backend (Invariant 5).
    //
    // Only PDOException is caught. A TypeError or a bug in the emitted expression is NOT a database error
    // and must not be laundered into one — it stays a hard fault, exactly as a Rust-side panic would.
    try {
        return new DatabaseResult_Ok($fn());
    } catch (PDOException $e) {
        return new DatabaseResult_Err(__phorj_db_classify($e));
    }
}
function __phorj_db_try_unit($fn) {
    // For natives whose Ok payload the prelude discards (`wrap_unit` on the Rust side): still a real
    // `DatabaseResult_Ok`, carrying 0 so the shape matches, because the prelude matches the VARIANT.
    try {
        $fn();
        return new DatabaseResult_Ok(0);
    } catch (PDOException $e) {
        return new DatabaseResult_Err(__phorj_db_classify($e));
    }
}
function __phorj_db_msg_is_unique($msg) {
    // SQLite reports UNIQUE/PRIMARY KEY violations through the same generic constraint code, so the
    // message is the only discriminator — mirroring `sqlite.rs`, which inspects the extended code and
    // falls back to the same textual markers.
    return str_contains($msg, 'UNIQUE constraint failed')
        || str_contains($msg, 'PRIMARY KEY must be unique')
        || str_contains($msg, 'Duplicate entry');
}
"#;
