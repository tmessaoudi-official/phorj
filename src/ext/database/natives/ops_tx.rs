//! `Core.Database` — the TRANSACTION/depth ops: `begin`, `commit`, `rollback`, the DEC-340
//! entry-depth unwind, `rollbackAll` and `transactionDepth`.
//!
//! Split out of `ops.rs` by cohesion (Invariant 13, M-Decomp) when DEC-340's additions took that file
//! over the 500-line hard cap. These belong together: they are the only ops that read or write the
//! connection's shared `tx_depth`, and the depth arithmetic is exactly what the P1 data-loss bug was
//! about — a `rollback` that unwound one level while `db.transaction` needed to restore the depth it
//! found on entry.
//!
//! The SQL these emit (`SAVEPOINT phorj_sp_N` / `ROLLBACK TO …` / `RELEASE …`) is mirrored by the PHP
//! leg's `__phorj_db_*` helpers in `src/transpile/db_php.rs`, savepoint names included.

use super::handles::as_conn;
use super::ops::control;
use crate::value::Value;

/// `db.begin()` → open a transaction (DEC-208 slice C). At depth 0 this is a top-level `BEGIN`; nested,
/// it opens `SAVEPOINT phorj_sp_<depth>` so transactional helpers compose. Increments the depth only on
/// success. Returns the new depth (the prelude ignores the payload; it is handy for tests/debugging).
pub(super) fn begin_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Database.__begin expects (Connection)".into()),
    };
    let depth = conn.tx_depth.get();
    let sql = if depth == 0 {
        "BEGIN".to_string()
    } else {
        format!("SAVEPOINT phorj_sp_{depth}")
    };
    control(conn, &sql)?;
    let new_depth = depth + 1;
    conn.tx_depth.set(new_depth);
    Ok(Value::Int(i64::from(new_depth)))
}

/// `db.commit()` → commit the innermost open transaction level. At the outermost level (depth 1) this is
/// `COMMIT`; nested, it `RELEASE`s the matching savepoint. A commit with no open transaction (depth 0) is
/// a best-effort no-op so a secondary fault can never mask an original one. Returns the remaining depth.
pub(super) fn commit_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Database.__commit expects (Connection)".into()),
    };
    let depth = conn.tx_depth.get();
    if depth == 0 {
        return Ok(Value::Int(0));
    }
    let remaining = depth - 1;
    let sql = if remaining == 0 {
        "COMMIT".to_string()
    } else {
        format!("RELEASE phorj_sp_{remaining}")
    };
    control(conn, &sql)?;
    conn.tx_depth.set(remaining);
    Ok(Value::Int(i64::from(remaining)))
}

/// `db.rollback()` → roll back the innermost open transaction level. At the outermost level this is
/// `ROLLBACK`; nested, it `ROLLBACK`s to and `RELEASE`s the matching savepoint (so the outer transaction
/// survives an inner rollback). A rollback with no open transaction is a best-effort no-op. The depth is
/// decremented BEFORE issuing the SQL, so the counter stays consistent even if the driver rejects the
/// statement (a doomed transaction is reset by SQLite regardless). Returns the remaining depth.
pub(super) fn rollback_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Database.__rollback expects (Connection)".into()),
    };
    let depth = conn.tx_depth.get();
    if depth == 0 {
        return Ok(Value::Int(0));
    }
    let remaining = depth - 1;
    conn.tx_depth.set(remaining);
    let sql = if remaining == 0 {
        "ROLLBACK".to_string()
    } else {
        format!("ROLLBACK TO phorj_sp_{remaining}; RELEASE phorj_sp_{remaining}")
    };
    control(conn, &sql)?;
    Ok(Value::Int(i64::from(remaining)))
}

/// The connection's CURRENT transaction depth (0 = none open). DEC-340: `db.transaction` reads this on
/// ENTRY so it can restore exactly what it found, and the prelude's `transactionDepth()` surfaces it to
/// user code — before this, depth was unobservable from phorj (the native returned it and the prelude
/// discarded the payload), so the invariant could not be asserted in a test at all.
pub(super) fn tx_depth_of(db: &Value) -> Result<u32, String> {
    Ok(as_conn(db)?.tx_depth.get())
}

/// `db.transactionDepth()` → the current depth as an `int`.
pub(super) fn transaction_depth_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Database.__transactionDepth expects (Connection)".into()),
    };
    Ok(Value::Int(i64::from(conn.tx_depth.get())))
}

/// Roll back repeatedly until the depth is back down to `target` (DEC-340 — *"restore the depth I
/// found"*).
///
/// This is the P1 fix. [`rollback_inner`] unwinds exactly ONE level, so a single call could be consumed
/// by a `begin()` leaked anywhere inside a transaction's closure — including inside a helper it calls —
/// leaving the transaction's OWN level open with its writes live, for a later unrelated `commit()` to
/// make permanent after the error handler had already been told the transaction rolled back.
///
/// Unwinding to the CALLER'S entry depth rather than to 0 is the ruled behaviour and the important part:
/// unwinding to 0 would destroy a caller-owned outer transaction (`db.begin(); db.transaction(fn)` where
/// `fn` throws), trading this bug for a rarer but worse one.
///
/// Best-effort by contract: the first driver error stops the loop and is returned, but callers on a
/// throw path deliberately discard it — a rollback error must never mask the original throw. The loop
/// cannot spin: `rollback_inner` always decrements, and depth 0 returns immediately.
pub(super) fn unwind_to_inner(db: &Value, target: u32) -> Result<(), String> {
    while as_conn(db)?.tx_depth.get() > target {
        rollback_inner(std::slice::from_ref(db))?;
    }
    Ok(())
}

/// `db.rollbackAll()` → unwind every open level (to depth 0). DEC-340: for the MANUAL
/// `begin`/`commit`/`rollback` path, where the caller genuinely does own the outermost level. Auto-
/// rollback deliberately does NOT use this — it restores its entry depth instead.
pub(super) fn rollback_all_inner(args: &[Value]) -> Result<Value, String> {
    let db = match args {
        [c] => c,
        _ => return Err("Core.Database.__rollbackAll expects (Connection)".into()),
    };
    unwind_to_inner(db, 0)?;
    Ok(Value::Int(0))
}
