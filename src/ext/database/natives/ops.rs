//! The connection / statement / transaction operation bodies for `Core.DatabaseModule` (DEC-208): each
//! returns `Ok(payload)` on success and `Err(db-error-message)` on a DB error. The public natives
//! ([`super::wrappers`]) `wrap` these onto the `DatabaseResult<T>` VALUE the prelude throws on.

use super::driver::{inject_pg_password, open_driver, DriverConn};
use super::handles::{
    as_conn, as_cursor, as_stmt, conn_closed, wrap, Binds, DbConn, DbCursor, DbStmt, PosBind,
};
use crate::native::ClosureInvoker;
use crate::value::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `new Database(dsn)` → open a connection, dispatching on the DSN scheme onto the right backend driver
/// ([`open_driver`]): `sqlite:PATH` / `sqlite::memory:` / a bare path → SQLite; `postgres://…` →
/// Postgres. The driver behind [`Value::Db`] is opaque to the generic layer.
pub(super) fn open_inner(args: &[Value]) -> Result<Value, String> {
    let dsn = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.DatabaseModule.__open expects (string dsn)".into()),
    };
    let driver = open_driver(dsn)?;
    Ok(Value::Db(Rc::new(DbConn {
        driver: Rc::new(RefCell::new(Some(driver))),
        tx_depth: Rc::new(Cell::new(0)),
        hook: Rc::new(RefCell::new(None)),
        timeout_ms: Rc::new(Cell::new(0)),
    })))
}

/// `DbSys.dsnWithPassword(dsn, password)` → the DSN with `password` injected as its credential (DEC-208
/// slice G, the `Database.withPassword` factory). A pure string transform ([`inject_pg_password`]); the result
/// is consumed immediately by `new Database(...)` and never retained in plaintext (the driver parses the
/// password out and stores only a redacted DSN). A non-postgres DSN is returned unchanged.
pub(super) fn dsn_with_password_inner(args: &[Value]) -> Result<Value, String> {
    let (dsn, pw) = match args {
        [Value::Str(d), Value::Str(p)] => (d.as_str(), p.as_str()),
        _ => {
            return Err(
                "Core.DatabaseModule.__dsnWithPassword expects (string dsn, string password)"
                    .into(),
            )
        }
    };
    Ok(Value::Str(inject_pg_password(dsn, pw).into()))
}

/// `db.prepare(sql)` → a lazily-executed statement handle carrying the connection driver + SQL.
pub(super) fn prepare_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, sql) = match args {
        [c, Value::Str(s)] => (as_conn(c)?, s.clone()),
        _ => return Err("Core.DatabaseModule.__prepare expects (Database, string sql)".into()),
    };
    // Reject preparing on a closed connection eagerly (the statement would otherwise fault only at
    // query/exec time).
    if conn.driver.borrow().is_none() {
        return Err(conn_closed());
    }
    Ok(Value::Db(Rc::new(DbStmt {
        driver: Rc::clone(&conn.driver),
        sql,
        binds: RefCell::new(Binds::None),
        hook: Rc::clone(&conn.hook),
        timeout_ms: Rc::clone(&conn.timeout_ms),
        tx_depth: Rc::clone(&conn.tx_depth),
    })))
}

/// `stmt.bind(value)` → append a positional bind; returns the same shared handle (chainable).
pub(super) fn bind_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, val) = match args {
        [s, v] => (as_stmt(s)?, v),
        _ => return Err("Core.DatabaseModule.__bind expects (Statement, value)".into()),
    };
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Positional(vec![PosBind::One(val.clone())]),
        Binds::Positional(v) => v.push(PosBind::One(val.clone())),
        Binds::Named(_) => {
            return Err(
                "Core.DatabaseModule: cannot mix positional bind() with named bindNamed()".into(),
            )
        }
    }
    drop(binds);
    // The prelude discards this payload (`Ok(_) => this`), so return a cheap unit rather than cloning
    // the handle — routed through `wrap_unit` to the cached carrier (DEC-292 dbwork alloc lever).
    Ok(Value::Null)
}

/// `stmt.bindList(values)` → record a list-valued positional bind (DEC-208 slice D, spec §2). It
/// occupies ONE positional `?` slot (left-to-right with `bind()`); at execute time that `?` expands to
/// `(?,?,…)` — one placeholder per value — so `… WHERE id IN (?)` binds the whole list, strictly safer
/// than PDO (which cannot bind an array to `IN`). An EMPTY list expands to `(NULL)`: `x IN (NULL)` is
/// never true, so an empty `IN` matches nothing (documented, sane default). Mixing with `bindNamed()`
/// is an error, exactly like `bind()`. Returns the same shared handle (chainable).
pub(super) fn bind_list_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, vals) = match args {
        [s, Value::List(vs)] => (as_stmt(s)?, vs),
        _ => return Err("Core.DatabaseModule.__bindList expects (Statement, List<value>)".into()),
    };
    let entry = PosBind::List(vals.iter().cloned().collect());
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Positional(vec![entry]),
        Binds::Positional(v) => v.push(entry),
        Binds::Named(_) => {
            return Err(
                "Core.DatabaseModule: cannot mix positional bindList() with named bindNamed()"
                    .into(),
            )
        }
    }
    drop(binds);
    Ok(Value::Null) // payload discarded by the prelude (see bind_inner)
}

/// `stmt.bindNamed(name, value)` → append a named bind; returns the same shared handle (chainable).
pub(super) fn bind_named_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, name, val) = match args {
        [s, Value::Str(n), v] => (as_stmt(s)?, n.as_str().to_string(), v),
        _ => {
            return Err(
                "Core.DatabaseModule.__bindNamed expects (Statement, string name, value)".into(),
            )
        }
    };
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Named(vec![(name, val.clone())]),
        Binds::Named(v) => v.push((name, val.clone())),
        Binds::Positional(_) => {
            return Err(
                "Core.DatabaseModule: cannot mix named bindNamed() with positional bind()".into(),
            )
        }
    }
    drop(binds);
    Ok(Value::Null) // payload discarded by the prelude (see bind_inner)
}

/// Borrow the live driver behind a statement, or a clean `<<ConnectionError>>` if the connection was
/// closed. The returned guard keeps the driver borrowed for the caller's operation.
fn stmt_driver(stmt: &DbStmt) -> Result<std::cell::Ref<'_, Option<Box<dyn DriverConn>>>, String> {
    let guard = stmt.driver.borrow();
    if guard.is_none() {
        return Err(conn_closed());
    }
    Ok(guard)
}

/// `stmt.query()` → run the prepared+bound statement and return `List<Row>` (fetch-all), delegating the
/// dialect-specific placeholder + value handling to the connection's driver.
pub(super) fn query_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__query expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    driver.query(&stmt.sql, &binds)
}

/// `stmt.stream()` → run the prepared+bound statement and wrap the result set in a [`DbCursor`]
/// (DEC-208 item H). Runs the SAME driver query as `stmt.query()` (so the `onQuery` hook + timeout
/// classification apply identically); the difference is delivery — rows are pulled one at a time via
/// `streamNext`, and a typed `streamInto<T>` stream hydrates each row only when pulled.
pub(super) fn stream_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__stream expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    let rows = match driver.query(&stmt.sql, &binds)? {
        Value::List(rc) => Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone()),
        other => {
            return Err(format!(
                "Core.DatabaseModule.__stream: driver returned {}, not a row list",
                other.type_name()
            ))
        }
    };
    Ok(Value::Db(Rc::new(DbCursor {
        rows: RefCell::new(rows.into_iter()),
    })))
}

/// `cursor.streamNext()` → the next row handle, or `null` when the result set is exhausted.
pub(super) fn stream_next_inner(args: &[Value]) -> Result<Value, String> {
    let cursor = match args {
        [c] => as_cursor(c)?,
        _ => return Err("Core.DatabaseModule.__streamNext expects (cursor)".into()),
    };
    let next = cursor.rows.borrow_mut().next();
    Ok(next.unwrap_or(Value::Null))
}

/// `stmt.exec()` → run a write (INSERT/UPDATE/DELETE/DDL) and return the affected-row count.
pub(super) fn exec_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__exec expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    driver.exec(&stmt.sql, &binds).map(Value::Int)
}

/// `stmt.execReturningId()` → run an INSERT and return the auto-generated rowid / PK (DEC-208 slice D,
/// spec §4). Backend-specific: SQLite `last_insert_rowid()`; Postgres `RETURNING`/`lastval()`.
pub(super) fn exec_returning_id_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__execReturningId expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    driver.exec_returning_id(&stmt.sql, &binds).map(Value::Int)
}

/// `stmt.executeMany(rows)` → prepare ONCE and execute the statement for each row of binds (DEC-208
/// slice D, spec §4) — far faster than a per-row `prepare`+`exec` loop. `rows` is a `List<List<value>>`
/// (each inner list = one positional bind-set, matching the `?` count). The whole batch runs inside a
/// dedicated SAVEPOINT (`phorj_bulk`) for atomicity + speed: it commits (`RELEASE`) on success and
/// rolls back the entire batch on ANY row's failure. A savepoint composes with an outer `begin()`
/// transaction and never touches the `begin()`/`rollback()` depth counter. Returns the TOTAL affected
/// rows. `executeMany` carries all its binds via `rows`; a statement that also has accumulated
/// `bind()`/`bindNamed()` binds is a usage error (ambiguous).
pub(super) fn execute_many_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, rows) = match args {
        [s, Value::List(rows)] => (as_stmt(s)?, rows),
        _ => {
            return Err(
                "Core.DatabaseModule.__executeMany expects (Statement, List<List<value>>)".into(),
            )
        }
    };
    if !matches!(&*stmt.binds.borrow(), Binds::None) {
        return Err(
            "Core.DatabaseModule.executeMany: pass all values via the rows argument, not bind()/bindNamed()"
                .into(),
        );
    }
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    // The driver's bulk savepoint needs to know whether a caller transaction is already open (Postgres
    // opens its own `BEGIN` at depth 0; SQLite auto-txns a standalone savepoint and ignores the flag).
    let in_transaction = stmt.tx_depth.get() > 0;
    driver
        .execute_many(&stmt.sql, rows.as_slice(), in_transaction)
        .map(Value::Int)
}

/// `db.lastInsertId()` → the auto-generated rowid / PK of the most recent INSERT on this connection
/// (DEC-208 slice D, spec §4). Backend-specific: SQLite `last_insert_rowid()`; Postgres `lastval()`.
pub(super) fn last_insert_id_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__lastInsertId expects (Database)".into()),
    };
    let guard = conn.driver.borrow();
    let driver = guard.as_ref().ok_or_else(conn_closed)?;
    driver.last_insert_id().map(Value::Int)
}

/// `db.timeout(ms)` → arm the connection's query timeout (DEC-208 slice D, spec §7). SQLite:
/// `busy_timeout(ms)` bounds how long a statement waits on a held lock before failing — a genuine
/// statement-runtime cap needs a progress-handler/interrupt (not wired here; the busy-wait cap is what
/// SQLite supports cleanly). Storing `timeout_ms > 0` makes a subsequent `busy`/`locked` failure
/// reclassify to `TimeoutError` ([`remap_timeout`]). A negative `ms` clamps to 0 (unset). Idempotent.
pub(super) fn timeout_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, ms) = match args {
        [c, Value::Int(ms)] => (as_conn(c)?, *ms),
        _ => return Err("Core.DatabaseModule.__timeout expects (Database, int ms)".into()),
    };
    let clamped = ms.max(0);
    {
        let guard = conn.driver.borrow();
        let driver = guard.as_ref().ok_or_else(conn_closed)?;
        driver.set_timeout(clamped)?;
    }
    conn.timeout_ms.set(clamped);
    Ok(Value::Int(clamped))
}

/// `db.onQuery(hook)` → register the observability hook (DEC-208 slice D, spec §7). Stores the
/// `(string, int) => void` closure in the shared cell every derived statement reads; `query`/`exec`
/// then invoke it after each op with `(sql, elapsed_ms)`. Stored eagerly (an `Rc` bump); a re-register
/// replaces the previous hook. Never a DB error.
pub(super) fn on_query_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, hook) = match args {
        [c, h] => (as_conn(c)?, h.clone()),
        _ => return Err("Core.DatabaseModule.__onQuery expects (Database, hook)".into()),
    };
    *conn.hook.borrow_mut() = Some(hook);
    Ok(Value::Int(0))
}

/// When a query timeout is active (`db.timeout(ms)`), reclassify a transient `SerializationFailureError`
/// (SQLite `busy`/`locked`) as `TimeoutError`: `busy_timeout` bounded the lock-wait, so reaching it means
/// the wait was exceeded. CONSEQUENCE: with a timeout set you no longer observe `SerializationFailureError`
/// (the class a future closure-`retry` would target) — acceptable while retry is deferred, documented
/// in `KNOWN_ISSUES.md` + the spec.
pub(super) fn remap_timeout(res: Result<Value, String>, active: bool) -> Result<Value, String> {
    if active {
        if let Err(msg) = &res {
            if let Some(rest) = msg.strip_prefix("<<SerializationFailureError>>") {
                return Err(format!("<<TimeoutError>>{rest}"));
            }
        }
    }
    res
}

/// Run a statement-executing inner body, then (a) reclassify a busy failure as `TimeoutError` when a
/// timeout is active and (b) fire the connection's `onQuery` hook with `(sql, elapsed_ms)`. This is why
/// `query`/`exec`/`executeMany`/`execReturningId` are `HigherOrder` natives: they must call BACK into
/// the calling backend to invoke the stored `Value::Closure` (the same re-entrant `invoke` the
/// interpreter/VM hand to `List.map`). A well-typed `(string, int) => void` hook cannot raise a checked
/// exception, so its error (reachable only via a hard fault / the throw sentinel) is PROPAGATED, never
/// swallowed — swallowing would strand the backend's throw sentinel. `elapsed_ms` is wall-clock and
/// thus NON-deterministic across the two backends: examples/tests must never print it raw, or
/// `run ≡ runvm` breaks. When no hook is set and no timeout is armed, this is byte-identical to the old
/// `Ok(wrap(inner(args)))`.
pub(super) fn with_hook(
    args: &[Value],
    invoke: &mut ClosureInvoker,
    inner: fn(&[Value]) -> Result<Value, String>,
) -> Result<Value, String> {
    let stmt = args.first().and_then(|v| as_stmt(v).ok());
    // Fast path: with no `onQuery` hook and no armed timeout there is nothing to instrument, so skip
    // the two `Instant::now()` clock reads (the elapsed ms only ever feeds the hook) and the
    // take/restore plumbing. dbwork runs ~20k execs/queries with neither set — this is the common
    // case (DEC-292 dbwork lever).
    let instrument = stmt.is_some_and(|s| s.timeout_ms.get() > 0 || s.hook.borrow().is_some());
    if !instrument {
        return Ok(wrap(inner(args)));
    }
    let start = std::time::Instant::now();
    let mut result = inner(args);
    if let Some(s) = stmt {
        result = remap_timeout(result, s.timeout_ms.get() > 0);
        let ms = i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX);
        // TAKE the hook out of the shared cell for the duration of its own call: a hook that itself
        // issues a query/exec on the same connection would otherwise re-enter here and recurse without
        // bound (stack overflow). With it removed, the nested op sees no hook and runs normally; the
        // hook is restored afterward (even if it faulted, so the error still propagates).
        let hook = s.hook.borrow_mut().take();
        if let Some(h) = hook {
            let fired = invoke(&h, vec![Value::Str(s.sql.as_str().into()), Value::Int(ms)]);
            *s.hook.borrow_mut() = Some(h);
            fired?;
        }
    }
    Ok(wrap(result))
}

/// Run one portable SQL control statement (`BEGIN`/`COMMIT`/`SAVEPOINT`/`RELEASE`/`ROLLBACK[ TO]`) on
/// the live connection's driver, or a clean `<<ConnectionError>>` if the connection was closed. These
/// forms are accepted identically by SQLite and Postgres, so transaction management stays generic.
fn control(conn: &DbConn, sql: &str) -> Result<(), String> {
    let guard = conn.driver.borrow();
    let driver = guard.as_ref().ok_or_else(conn_closed)?;
    driver.control(sql)
}

/// `db.begin()` → open a transaction (DEC-208 slice C). At depth 0 this is a top-level `BEGIN`; nested,
/// it opens `SAVEPOINT phorj_sp_<depth>` so transactional helpers compose. Increments the depth only on
/// success. Returns the new depth (the prelude ignores the payload; it is handy for tests/debugging).
pub(super) fn begin_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__begin expects (Database)".into()),
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
        _ => return Err("Core.DatabaseModule.__commit expects (Database)".into()),
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
        _ => return Err("Core.DatabaseModule.__rollback expects (Database)".into()),
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

/// `db.close()` → deterministically drop the connection (DEC-208 slice C, spec §1). Idempotent and
/// never a DB error: every `Value::Db`/`DbStmt` shares the same `Rc<RefCell<Option<…>>>`, so setting it
/// to `None` invalidates all of them — a later op faults with `<<ConnectionError>>`. Resets the tx depth.
pub(super) fn close_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__close expects (Database)".into()),
    };
    *conn.driver.borrow_mut() = None;
    conn.tx_depth.set(0);
    Ok(Value::Int(0))
}
