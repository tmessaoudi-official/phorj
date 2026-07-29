//! The public native wrappers for `Core.DatabaseModule` (DEC-208): each wraps an [`super::ops`] /
//! [`super::rows`] inner body so a DB error becomes `DatabaseResult.Err` (a value the prelude throws
//! on), never a hard fault. Pure ops go through [`wrap`]/[`wrap_unit`]; statement-executing ops are
//! `HigherOrder` (they route through [`with_hook`] to fire the `onQuery` hook + timeout classification);
//! `dsnWithPassword` returns a plain string; `transaction` is the closure-form transactional attempt.

use super::handles::{failure, success, wrap, wrap_unit};
use super::ops::*;
use super::ops_tx::*; // DEC-340: the transaction/depth ops moved out of `ops.rs` (Inv 13)
use super::rows::*;
use crate::native::ClosureInvoker;
use crate::value::Value;

macro_rules! db_native {
    ($name:ident, $inner:ident) => {
        pub(super) fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap($inner(args)))
        }
    };
}
/// Same as [`db_native!`] but for ops whose Ok payload the prelude discards — routes through
/// [`wrap_unit`] so the hot success path is a cached-carrier Rc bump, not an allocation (DEC-292).
macro_rules! db_native_unit {
    ($name:ident, $inner:ident) => {
        pub(super) fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap_unit($inner(args)))
        }
    };
}
db_native!(db_open, open_inner);
/// `dsnWithPassword` returns a plain `string` (the authenticated DSN), NOT a `DatabaseResult` — it is a pure
/// string transform with no DB error, so it does not go through `wrap`.
pub(super) fn db_dsn_with_password(args: &[Value], _out: &mut String) -> Result<Value, String> {
    dsn_with_password_inner(args)
}
db_native!(db_prepare, prepare_inner);
db_native!(db_stream_next, stream_next_inner);
// Payload-discarded ops (prelude arm `Ok(_) => this` / `Ok(_) => Database.ok()`) — cached-carrier path.
db_native_unit!(db_bind, bind_inner);
db_native_unit!(db_bind_named, bind_named_inner);
db_native_unit!(db_bind_list, bind_list_inner);
db_native!(db_last_insert_id, last_insert_id_inner);
db_native_unit!(db_timeout, timeout_inner);
db_native_unit!(db_on_query, on_query_inner);
db_native_unit!(db_begin, begin_inner);
db_native_unit!(db_commit, commit_inner);
db_native_unit!(db_rollback, rollback_inner);
// DEC-340: `rollbackAll` can hit a driver error, so it goes through the `DatabaseResult` wrapper like
// `rollback`. `transactionDepth` deliberately does NOT: it only reads an `Rc<Cell<u32>>`, so it cannot
// fail, and wrapping it would hand phorj a `DatabaseResult` enum where the prelude declares a plain
// `int` — which is exactly the mismatch that first showed up as "cannot interpolate enum into a string".
db_native_unit!(db_rollback_all, rollback_all_inner);
pub(super) fn db_transaction_depth(args: &[Value], _out: &mut String) -> Result<Value, String> {
    transaction_depth_inner(args)
}
db_native!(db_close, close_inner);
db_native!(row_get_int, get_int_inner);
db_native!(row_get_string, get_string_inner);
db_native!(row_get_float, get_float_inner);
db_native!(row_get_bool, get_bool_inner);
db_native!(row_get_int_or_null, get_int_or_null_inner);
db_native!(row_get_string_or_null, get_string_or_null_inner);
db_native!(row_get_float_or_null, get_float_or_null_inner);
db_native!(row_get_bool_or_null, get_bool_or_null_inner);
db_native!(row_get_decimal, get_decimal_inner);
db_native!(row_get_decimal_or_null, get_decimal_or_null_inner);
db_native!(row_get_int_list, get_int_list_inner);
db_native!(row_get_string_list, get_string_list_inner);
db_native!(row_get_float_list, get_float_list_inner);
db_native!(row_get_bool_list, get_bool_list_inner);
db_native!(row_get_int_list_or_null, get_int_list_or_null_inner);
db_native!(row_get_string_list_or_null, get_string_list_or_null_inner);
db_native!(row_get_float_list_or_null, get_float_list_or_null_inner);
db_native!(row_get_bool_list_or_null, get_bool_list_or_null_inner);
db_native!(row_column_names, column_names_inner);
db_native!(row_is_null, is_null_inner);

// HigherOrder natives (DEC-208 slice D): the statement-executing paths route through `with_hook` so
// they can fire the `onQuery` closure and apply the timeout classification. `wrap` still turns a DB
// error into a catchable `DatabaseResult.Err`; the `Result<_, String>` a HigherOrder body returns is used
// ONLY for the hook-invoke propagation (a hard fault / throw sentinel), never for a DB error.
pub(super) fn db_query(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, query_inner)
}
pub(super) fn db_exec(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, exec_inner)
}
pub(super) fn db_stream(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, stream_inner)
}
pub(super) fn db_execute_many(
    args: &[Value],
    invoke: &mut ClosureInvoker,
) -> Result<Value, String> {
    with_hook(args, invoke, execute_many_inner)
}
pub(super) fn db_exec_returning_id(
    args: &[Value],
    invoke: &mut ClosureInvoker,
) -> Result<Value, String> {
    with_hook(args, invoke, exec_returning_id_inner)
}

/// `db.transaction(fn)` (DEC-208 slice C, the closure form — unblocked by DEC-222 throwing closures) —
/// ONE transactional attempt: `BEGIN`, run the closure, `COMMIT` on a normal return (returning the
/// closure's value), and auto-`ROLLBACK` + **re-propagate the ORIGINAL thrown value** on a throw. A
/// NESTED call opens a `SAVEPOINT` (via the shared `tx_depth`), so it composes into partial rollback.
/// The RETRY loop lives in the prelude, not here: retry must inspect the TYPED error to decide whether
/// it is transient (`SerializationFailureError`), and that thrown value sits in the backend's `pending_throw`
/// — invisible to a native. So this native is a single attempt; the prelude's `catch (SerializationFailureError)`
/// loop drives the retries.
///
/// **Throw preservation** (the load-bearing part): a closure `throw` reaches the invoker as
/// `Err(THROW_SENTINEL)` with the thrown `Value` stashed in the backend's `pending_throw`.
/// [`rollback_inner`] runs pure `rusqlite` SQL ([`super::ops`] `control` → `execute_batch`) and NEVER
/// re-enters the backend, so `pending_throw` stays intact; returning the SAME `Err(e)` unchanged lets the
/// outer backend arm (interpreter `call.rs` / VM `exec.rs`, both keyed on the sentinel) rebuild the
/// ORIGINAL typed `DatabaseError` — the caller catches the exact error the closure threw, never a generic one.
pub(super) fn db_transaction(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    let (db, fnv) = match args {
        [db, fnv] => (db, fnv),
        _ => return Err("Core.DatabaseModule.__transaction expects (Database, fn)".into()),
    };
    // DEC-340 — record the depth we FOUND, before opening anything. Every unwind below returns to
    // exactly this, never to 0: unwinding to 0 would roll back a caller-owned outer transaction
    // (`db.begin(); db.transaction(fn)` where `fn` throws), destroying work this call was never given
    // authority over.
    let entry_depth = match tx_depth_of(db) {
        Ok(d) => d,
        Err(msg) => return Ok(failure(msg)),
    };
    // BEGIN. A DB error opening the (save)point is a catchable `DatabaseResult.Err`, never a hard fault.
    if let Err(msg) = begin_inner(std::slice::from_ref(db)) {
        return Ok(failure(msg));
    }
    match invoke(fnv, Vec::new()) {
        // Normal return: COMMIT and hand back the closure's value. If the COMMIT itself fails, roll
        // back best-effort (to reset the shared `tx_depth`) and surface the commit error as a
        // catchable `DatabaseResult.Err` — the closure's work is not returned.
        Ok(v) => match commit_inner(std::slice::from_ref(db)) {
            Ok(_) => Ok(success(v)),
            Err(msg) => {
                // DEC-340: unwind to the ENTRY depth, not one level. A `begin()` leaked inside the
                // closure would otherwise consume the single rollback and leave this transaction's own
                // level open with its writes live.
                let _ = unwind_to_inner(db, entry_depth);
                Ok(failure(msg))
            }
        },
        // The closure threw (sentinel + `pending_throw`) or hard-faulted: roll back best-effort — a
        // rollback error must NEVER mask the original — then re-propagate the SAME `Err` unchanged, so
        // the backend reconstructs the ORIGINAL typed throw (`pending_throw` is untouched by rollback).
        Err(e) => {
            // DEC-340 — THE P1 FIX. This used to be a single `rollback_inner`, which unwinds exactly one
            // level: a `begin()` leaked anywhere in the closure (or in a helper it called) consumed it,
            // so the transaction's OWN level stayed open with its work live and a later unrelated
            // `commit()` made it permanent — after the error handler had been told it rolled back.
            // Discarding the unwind error is deliberate and unchanged: a rollback failure must never
            // mask the original throw, whose typed value is waiting in the backend's `pending_throw`.
            let _ = unwind_to_inner(db, entry_depth);
            Err(e)
        }
    }
}
