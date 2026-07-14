//! `Core.Db` — the enhanced-PDO database primitive (DEC-208), backed by `rusqlite` (bundled SQLite).
//!
//! Feature-gated (`db`) and native-only. This module owns the RUNTIME layer: the opaque connection /
//! statement handles ([`DbConn`] / [`DbStmt`], carried by [`Value::Db`] via the [`DbObject`] trait) and
//! the internal `Core.DbSys` native bodies for connect / prepare / bind / bindNamed / query / exec and
//! the Row accessors. The public `Core.Db` SURFACE (`Db`/`Statement`/`Row` + `new Db(dsn)` +
//! `db.prepare(sql).bind(v).query()`) is the phorj-source `DB_PRELUDE` (`src/cli/preludes.rs`) on top of
//! these — the natives live under the `DbSys` qualifier so a prelude `class Db` never collides with them.
//!
//! **Error mechanism (DEC-208 = prelude-wrapper).** phorj's native ABI has no throws channel: a native's
//! `Err(String)` is an uncatchable HARD fault (`vm/exec.rs`), so it cannot express the ruled catchable
//! `throws DbError` (Q6). Instead every native here returns a `DbResult<T>` VALUE (`DbResult.Ok(payload)`
//! on success, `DbResult.Err(message)` on any DB error — it NEVER faults on a DB error); the phorj-source
//! prelude `match`es it and `throw`s a catchable `DbError` (a real `Op::Throw`). `DbResult` is a
//! prelude-LOCAL enum (not `Core.Result`, whose injection sits earlier in the module chain and so is not
//! pulled in by `Core.Db`'s transitive import). Only a checker-unreachable arity/shape bug returns `Err`.
//!
//! **Spine treatment.** Every native is `pure: false`, so `uses_impure_native` auto-excludes any
//! `import Core.Db` program from the byte-identity differential (live DB I/O can't be byte-identical
//! across rusqlite and PHP PDO). Correctness: the in-module unit tests + the `tests/db.rs` fixture.
//! `run ≡ runvm` holds unconditionally (both backends call these one shared `eval` bodies). The `php`
//! emitters (faithful PDO, DEC-208 LADDER case 1) are finalized in the DEC-208 transpile slice.

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{DbObject, EnumVal, HKey, Value};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Wrap a success payload as `DbResult.Ok(v)`. The natives here NEVER fault on a DB error (a native
/// `Err(String)` is an uncatchable hard fault — `vm/exec.rs`); instead they return this `DbResult<T>`
/// VALUE, and the phorj-source `Core.Db` prelude `match`es it and `throw`s a catchable `DbError`
/// (DEC-208 error-mechanism = prelude-wrapper). `DbResult` is a PRELUDE-LOCAL enum (defined in
/// DB_PRELUDE, injected with it) — NOT `Core.Result`, whose injection sits earlier in the module chain
/// and so is not pulled in by `Core.Db`'s transitive import (importer-after-imported doesn't inject).
fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "DbResult".into(),
        variant: "Ok".into(),
        payload: vec![v],
    }))
}

/// Wrap a DB error message as `DbResult.Err(msg)` — the prelude turns this into `throw DbError`.
fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "DbResult".into(),
        variant: "Err".into(),
        payload: vec![Value::Str(msg.into())],
    }))
}

/// Map an inner body's `Result<payload, db-error-message>` onto the returned `Result<T, string>` VALUE:
/// `Ok(v) → Success(v)`, `Err(msg) → Failure(msg)`. A DB error thus becomes a value the prelude throws
/// on, never an uncatchable native fault. (An arity/shape bug the checker forbids stays an `Err` — a
/// hard fault — because it is a program-construction error, not a recoverable DB error.)
fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

/// A live SQLite connection handle (`Value::Db` payload). Shared-mutable: cloning the `Value::Db`
/// shares this `Rc`, so all bindings name the same connection.
///
/// The connection is wrapped in an `Option` so `close()` (DEC-208 slice C) can deterministically drop
/// it — every derived `DbStmt` shares the same `Rc<RefCell<Option<…>>>`, so closing invalidates all of
/// them (a later op then faults with `<<ConnectionError>>`). `tx_depth` is the transaction / savepoint
/// nesting level (0 = no open transaction), shared across every binding of the connection: `begin`
/// opens `BEGIN` at depth 0 and a `SAVEPOINT` deeper, `commit`/`rollback` `RELEASE`/`ROLLBACK TO` the
/// innermost level — so transactional helpers compose (an inner rollback never aborts the outer).
#[derive(Debug)]
struct DbConn {
    conn: Rc<RefCell<Option<rusqlite::Connection>>>,
    tx_depth: Cell<u32>,
}

impl DbObject for DbConn {
    fn kind(&self) -> &'static str {
        "db-connection"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Accumulated bind parameters for a prepared statement. Positional and named are mutually exclusive
/// per statement (the surface's contract) — mixing is a DB error (`Failure`, catchable).
#[derive(Debug, Default, Clone)]
enum Binds {
    #[default]
    None,
    Positional(Vec<Value>),
    Named(Vec<(String, Value)>),
}

/// A lazily-executed prepared statement handle. rusqlite's `Statement` borrows its `Connection`, so
/// storing a live one in a `Value` would leak a lifetime; instead the handle keeps the connection
/// `Rc`, the SQL text, and the accumulated binds, and prepares+binds+executes eagerly at `query`/
/// `exec` (fetch-all semantics, like PDO). `binds` is interior-mutable so a chained `.bind(v)` mutates
/// in place and returns the same shared handle.
#[derive(Debug)]
struct DbStmt {
    conn: Rc<RefCell<Option<rusqlite::Connection>>>,
    sql: String,
    binds: RefCell<Binds>,
}

impl DbObject for DbStmt {
    fn kind(&self) -> &'static str {
        "db-statement"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Downcast a `Value::Db` handle to a concrete resource, or a clean fault (checker-unreachable once the
/// surface enforces the receiver types, but the natives stay total).
fn as_conn(v: &Value) -> Result<&DbConn, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<DbConn>()
            .ok_or_else(|| "Core.Db: expected a connection".to_string()),
        other => Err(format!(
            "Core.Db: expected a connection, got {}",
            other.type_name()
        )),
    }
}

fn as_stmt(v: &Value) -> Result<&DbStmt, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<DbStmt>()
            .ok_or_else(|| "Core.Db: expected a statement".to_string()),
        other => Err(format!(
            "Core.Db: expected a statement, got {}",
            other.type_name()
        )),
    }
}

/// Phorj value → SQLite storage value (bind side). Only the storable scalars are bindable; anything
/// else (list/map/instance/…) is a clean DB error, never a silent coercion (DEC-208 "no silent coercion").
fn to_sql(v: &Value) -> Result<rusqlite::types::Value, String> {
    use rusqlite::types::Value as S;
    Ok(match v {
        Value::Int(n) => S::Integer(*n),
        Value::Float(f) => S::Real(*f),
        Value::Str(s) => S::Text(s.as_str().to_string()),
        // SQLite has no boolean storage class — PDO/rusqlite both store it as an integer 0/1.
        Value::Bool(b) => S::Integer(i64::from(*b)),
        Value::Null => S::Null,
        Value::Bytes(b) => S::Blob((**b).clone()),
        other => {
            return Err(format!(
                "Core.Db: cannot bind a {} value",
                other.type_name()
            ))
        }
    })
}

/// SQLite storage value → phorj value (fetch side). The dynamic `query()` path returns each column's
/// natural type; `Row.getInt`/etc. then assert the expected type at the accessor (no silent coercion).
fn from_sql(v: rusqlite::types::Value) -> Value {
    use rusqlite::types::Value as S;
    match v {
        S::Null => Value::Null,
        S::Integer(n) => Value::Int(n),
        S::Real(f) => Value::Float(f),
        S::Text(s) => Value::Str(s.into()),
        S::Blob(b) => Value::Bytes(Rc::new(b)),
    }
}

/// Classify a `rusqlite` error into the taxonomy marker the prelude's `DbError.fail` reads (DEC-208
/// slice C, spec §6). The mapping keys off SQLite's (extended) result codes: `SQLITE_CONSTRAINT_UNIQUE`
/// / `_PRIMARYKEY` → `UniqueViolation`, generic `SQLITE_CONSTRAINT` → `ConstraintViolation`,
/// `SQLITE_BUSY`/`SQLITE_LOCKED` → `SerializationFailure` (the transient class retry targets — the
/// spec's `Deadlock` under one name), `SQLITE_CANTOPEN`/`SQLITE_NOTADB` → `ConnectionError`, generic
/// `SQLITE_ERROR` (a mis-typed statement at prepare time) → `SyntaxError`. Anything else stays generic
/// (no marker → the base `DbError`). `Timeout` has no SQLite source yet (it arrives with query
/// `.timeout(ms)`, slice D); the subtype exists in the taxonomy and the classifier already reads its
/// marker, so wiring it later is emit-only.
fn err_kind(e: &rusqlite::Error) -> Option<&'static str> {
    // Both a runtime failure (`SqliteFailure`) and a prepare-time SQL error (`SqlInputError`, which
    // carries the byte offset) wrap an `ffi::Error` with the result codes we classify on.
    let err = match e {
        rusqlite::Error::SqliteFailure(err, _) => err,
        rusqlite::Error::SqlInputError { error, .. } => error,
        _ => return None,
    };
    let ext = err.extended_code;
    if ext == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
        || ext == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
    {
        return Some("UniqueViolation");
    }
    // The primary result code is the low byte of the extended code.
    match ext & 0xff {
        rusqlite::ffi::SQLITE_CONSTRAINT => Some("ConstraintViolation"),
        rusqlite::ffi::SQLITE_BUSY | rusqlite::ffi::SQLITE_LOCKED => Some("SerializationFailure"),
        rusqlite::ffi::SQLITE_CANTOPEN | rusqlite::ffi::SQLITE_NOTADB => Some("ConnectionError"),
        rusqlite::ffi::SQLITE_ERROR => Some("SyntaxError"),
        _ => None,
    }
}

/// Render a `rusqlite` error as the `DbResult.Err` message the prelude throws on, PREFIXED with a
/// `<<Kind>>` marker when the error classifies into the typed taxonomy (see [`err_kind`]). The prelude's
/// single `DbError.fail` classification point strips the marker and throws the matching subtype.
fn sql_err(e: rusqlite::Error) -> String {
    let kind = err_kind(&e);
    let base = format!("Core.Db: {e}");
    match kind {
        Some(tag) => format!("<<{tag}>>{base}"),
        None => base,
    }
}

/// The catchable message for using a connection (or a statement derived from it) after `close()`.
/// Tagged `ConnectionError` so `catch (ConnectionError e)` is precise.
fn conn_closed() -> String {
    "<<ConnectionError>>Core.Db: the connection is closed".to_string()
}

// --- Internal bodies: `Ok(payload)` on success, `Err(db-error-message)` on a DB error. `wrap` maps
// these onto the `Result<T, string>` VALUE the public `__`-natives return (Success | Failure). ---

/// `new Db(dsn)` → open a connection. `dsn` is `"sqlite:PATH"` or `"sqlite::memory:"` (the PDO DSN
/// shape); a bare path is also accepted.
fn open_inner(args: &[Value]) -> Result<Value, String> {
    let dsn = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.Db.__open expects (string dsn)".into()),
    };
    let conn = if dsn == "sqlite::memory:" || dsn == ":memory:" {
        rusqlite::Connection::open_in_memory()
    } else {
        let path = dsn.strip_prefix("sqlite:").unwrap_or(dsn);
        rusqlite::Connection::open(path)
    }
    .map_err(sql_err)?;
    Ok(Value::Db(Rc::new(DbConn {
        conn: Rc::new(RefCell::new(Some(conn))),
        tx_depth: Cell::new(0),
    })))
}

/// `db.prepare(sql)` → a lazily-executed statement handle carrying the connection + SQL.
fn prepare_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, sql) = match args {
        [c, Value::Str(s)] => (as_conn(c)?, s.as_str().to_string()),
        _ => return Err("Core.Db.__prepare expects (Db, string sql)".into()),
    };
    // Reject preparing on a closed connection eagerly (the statement would otherwise fault only at
    // query/exec time).
    if conn.conn.borrow().is_none() {
        return Err(conn_closed());
    }
    Ok(Value::Db(Rc::new(DbStmt {
        conn: Rc::clone(&conn.conn),
        sql,
        binds: RefCell::new(Binds::None),
    })))
}

/// `stmt.bind(value)` → append a positional bind; returns the same shared handle (chainable).
fn bind_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, val) = match args {
        [s, v] => (as_stmt(s)?, v),
        _ => return Err("Core.Db.__bind expects (Statement, value)".into()),
    };
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Positional(vec![val.clone()]),
        Binds::Positional(v) => v.push(val.clone()),
        Binds::Named(_) => {
            return Err("Core.Db: cannot mix positional bind() with named bindNamed()".into())
        }
    }
    drop(binds);
    Ok(args[0].clone())
}

/// `stmt.bindNamed(name, value)` → append a named bind; returns the same shared handle (chainable).
fn bind_named_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, name, val) = match args {
        [s, Value::Str(n), v] => (as_stmt(s)?, n.as_str().to_string(), v),
        _ => return Err("Core.Db.__bindNamed expects (Statement, string name, value)".into()),
    };
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Named(vec![(name, val.clone())]),
        Binds::Named(v) => v.push((name, val.clone())),
        Binds::Positional(_) => {
            return Err("Core.Db: cannot mix named bindNamed() with positional bind()".into())
        }
    }
    drop(binds);
    Ok(args[0].clone())
}

/// Materialize a rusqlite `Rows` cursor into a `List<Row>` (each `Row` is a column-name→value `Map`).
fn collect_rows(rows: &mut rusqlite::Rows, cols: &[String]) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(sql_err)? {
        let mut pairs = Vec::with_capacity(cols.len());
        for (i, name) in cols.iter().enumerate() {
            let cell: rusqlite::types::Value = row.get(i).map_err(sql_err)?;
            pairs.push((HKey::Str(name.as_str().into()), from_sql(cell)));
        }
        out.push(Value::Map(Rc::new(pairs)));
    }
    Ok(out)
}

/// `stmt.query()` → run the prepared+bound statement and return `List<Row>` (fetch-all).
fn query_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.Db.__query expects (Statement)".into()),
    };
    let guard = stmt.conn.borrow();
    let conn = guard.as_ref().ok_or_else(conn_closed)?;
    let mut prepared = conn.prepare(&stmt.sql).map_err(sql_err)?;
    let cols: Vec<String> = prepared
        .column_names()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let binds = stmt.binds.borrow();
    let rows = match &*binds {
        Binds::None => {
            let mut r = prepared.query([]).map_err(sql_err)?;
            collect_rows(&mut r, &cols)?
        }
        Binds::Positional(vs) => {
            let sv: Vec<rusqlite::types::Value> =
                vs.iter().map(to_sql).collect::<Result<_, _>>()?;
            let mut r = prepared
                .query(rusqlite::params_from_iter(sv.iter()))
                .map_err(sql_err)?;
            collect_rows(&mut r, &cols)?
        }
        Binds::Named(pairs) => {
            let sv: Vec<(String, rusqlite::types::Value)> = pairs
                .iter()
                .map(|(k, v)| Ok((format!(":{k}"), to_sql(v)?)))
                .collect::<Result<_, String>>()?;
            let refs: Vec<(&str, &dyn rusqlite::ToSql)> = sv
                .iter()
                .map(|(k, v)| (k.as_str(), v as &dyn rusqlite::ToSql))
                .collect();
            let mut r = prepared.query(refs.as_slice()).map_err(sql_err)?;
            collect_rows(&mut r, &cols)?
        }
    };
    Ok(Value::List(Rc::new(rows)))
}

/// `stmt.exec()` → run a write (INSERT/UPDATE/DELETE/DDL) and return the affected-row count.
fn exec_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.Db.__exec expects (Statement)".into()),
    };
    let guard = stmt.conn.borrow();
    let conn = guard.as_ref().ok_or_else(conn_closed)?;
    let mut prepared = conn.prepare(&stmt.sql).map_err(sql_err)?;
    let binds = stmt.binds.borrow();
    let n = match &*binds {
        Binds::None => prepared.execute([]).map_err(sql_err)?,
        Binds::Positional(vs) => {
            let sv: Vec<rusqlite::types::Value> =
                vs.iter().map(to_sql).collect::<Result<_, _>>()?;
            prepared
                .execute(rusqlite::params_from_iter(sv.iter()))
                .map_err(sql_err)?
        }
        Binds::Named(pairs) => {
            let sv: Vec<(String, rusqlite::types::Value)> = pairs
                .iter()
                .map(|(k, v)| Ok((format!(":{k}"), to_sql(v)?)))
                .collect::<Result<_, String>>()?;
            let refs: Vec<(&str, &dyn rusqlite::ToSql)> = sv
                .iter()
                .map(|(k, v)| (k.as_str(), v as &dyn rusqlite::ToSql))
                .collect();
            prepared.execute(refs.as_slice()).map_err(sql_err)?
        }
    };
    Ok(Value::Int(n as i64))
}

/// Run one SQL control statement (`BEGIN`/`COMMIT`/`SAVEPOINT`/…) on the live connection, or a clean
/// `<<ConnectionError>>` if the connection was closed.
fn control(conn: &DbConn, sql: &str) -> Result<(), String> {
    let guard = conn.conn.borrow();
    let live = guard.as_ref().ok_or_else(conn_closed)?;
    live.execute_batch(sql).map_err(sql_err)
}

/// `db.begin()` → open a transaction (DEC-208 slice C). At depth 0 this is a top-level `BEGIN`; nested,
/// it opens `SAVEPOINT phorj_sp_<depth>` so transactional helpers compose. Increments the depth only on
/// success. Returns the new depth (the prelude ignores the payload; it is handy for tests/debugging).
fn begin_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Db.__begin expects (Db)".into()),
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
fn commit_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Db.__commit expects (Db)".into()),
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
fn rollback_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Db.__rollback expects (Db)".into()),
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
fn close_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.Db.__close expects (Db)".into()),
    };
    *conn.conn.borrow_mut() = None;
    conn.tx_depth.set(0);
    Ok(Value::Int(0))
}

/// Look up a column in a `Row` (a `Map`), or a DB error if the column is absent.
fn row_cell<'a>(args: &'a [Value], who: &str) -> Result<(&'a Value, &'a str), String> {
    match args {
        [Value::Map(pairs), Value::Str(key)] => {
            let k = key.as_str();
            pairs
                .iter()
                .find(|(hk, _)| matches!(hk, HKey::Str(s) if s.as_str() == k))
                .map(|(_, v)| (v, k))
                .ok_or_else(|| format!("Core.Db.{who}: no column `{k}` in this row"))
        }
        _ => Err(format!("Core.Db.{who} expects (Row, string column)")),
    }
}

fn get_int_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getInt")?;
    match v {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Null => Err(format!("Core.Db.getInt: column `{k}` is NULL (use int?)")),
        other => Err(format!(
            "Core.Db.getInt: column `{k}` is {}, not int",
            other.type_name()
        )),
    }
}

fn get_string_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getString")?;
    match v {
        Value::Str(s) => Ok(Value::Str(s.clone())),
        Value::Null => Err(format!(
            "Core.Db.getString: column `{k}` is NULL (use string?)"
        )),
        other => Err(format!(
            "Core.Db.getString: column `{k}` is {}, not string",
            other.type_name()
        )),
    }
}

fn get_float_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getFloat")?;
    match v {
        Value::Float(f) => Ok(Value::Float(*f)),
        // SQLite stores an integral REAL as INTEGER; widen int→float for a float column, matching PDO.
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Null => Err(format!(
            "Core.Db.getFloat: column `{k}` is NULL (use float?)"
        )),
        other => Err(format!(
            "Core.Db.getFloat: column `{k}` is {}, not float",
            other.type_name()
        )),
    }
}

fn get_bool_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getBool")?;
    match v {
        // SQLite has no bool: it round-trips as 0/1 integer (matching the `to_sql` bind side).
        Value::Int(0) => Ok(Value::Bool(false)),
        Value::Int(_) => Ok(Value::Bool(true)),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Null => Err(format!("Core.Db.getBool: column `{k}` is NULL (use bool?)")),
        other => Err(format!(
            "Core.Db.getBool: column `{k}` is {}, not bool",
            other.type_name()
        )),
    }
}

// --- Nullable Row accessors (DEC-208 S2): a `T?`-typed hydration field admits a SQL NULL, so these
// return `null` for a NULL column instead of faulting. A wrong non-null storage type is still a DB
// error, and a missing column is still a DB error (`row_cell`). Shared by the dynamic path and the
// generic hydration desugar. ---

fn get_int_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getIntOrNull")?;
    match v {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.Db.getIntOrNull: column `{k}` is {}, not int",
            other.type_name()
        )),
    }
}

fn get_string_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getStringOrNull")?;
    match v {
        Value::Str(s) => Ok(Value::Str(s.clone())),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.Db.getStringOrNull: column `{k}` is {}, not string",
            other.type_name()
        )),
    }
}

fn get_float_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getFloatOrNull")?;
    match v {
        Value::Float(f) => Ok(Value::Float(*f)),
        // SQLite stores an integral REAL as INTEGER; widen int→float, matching the non-nullable accessor.
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.Db.getFloatOrNull: column `{k}` is {}, not float",
            other.type_name()
        )),
    }
}

fn get_bool_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getBoolOrNull")?;
    match v {
        // SQLite has no bool: it round-trips as 0/1 integer (matching the `to_sql` bind side).
        Value::Int(0) => Ok(Value::Bool(false)),
        Value::Int(_) => Ok(Value::Bool(true)),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.Db.getBoolOrNull: column `{k}` is {}, not bool",
            other.type_name()
        )),
    }
}

// --- Column introspection (DEC-208 slice B): two capabilities the desugared `queryScalar` /
// `queryMap` / nested-hydration helpers need, routed through the SAME `DbResult`/`wrap` protocol as
// the accessors (NOT a duplication of `getX` — genuinely new operations). `columnNames` gives the
// ORDERED column names of a row (the row is an insertion-ordered `Map`, so selection order is
// preserved) — `queryScalar` reads the sole column whose name is unpredictable (`COUNT(*)`), and
// `queryMap` keys on the first / reads the second. `isNull` reports whether a column is SQL NULL
// (type-agnostic) — the nested-optional-entity hydration tests "all this entity's columns are NULL"
// (a LEFT JOIN miss → the whole entity is `null`); it cannot use `== null` (phorj rejects a
// cross-type `T? == null` comparison), so this boolean primitive is required. ---

/// `row.columnNames()` → the ordered `List<string>` of this row's column names (selection order).
fn column_names_inner(args: &[Value]) -> Result<Value, String> {
    match args {
        [Value::Map(pairs)] => {
            let names: Vec<Value> = pairs
                .iter()
                // Column names are always text from SQL; the non-Str arms are unreachable in practice
                // but kept total (a row is a general `Map`).
                .map(|(k, _)| match k {
                    HKey::Str(s) => Value::Str(s.clone()),
                    HKey::Int(n) => Value::Str(n.to_string().into()),
                    HKey::Bool(b) => Value::Str(b.to_string().into()),
                })
                .collect();
            Ok(Value::List(Rc::new(names)))
        }
        _ => Err("Core.Db.columnNames expects (Row)".into()),
    }
}

/// `row.isNull(column)` → `true` iff the column is SQL NULL; a DB error if the column is absent
/// (reusing `row_cell`, so a missing nested column is a strict error exactly like the accessors).
fn is_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, _k) = row_cell(args, "isNull")?;
    Ok(Value::Bool(matches!(v, Value::Null)))
}

// --- Public natives: each wraps its inner body so a DB error becomes `Result.Failure` (a value the
// prelude throws on), never a hard fault. `_out` (the stdout buffer) is unused — DB ops have no stdout. ---

macro_rules! db_native {
    ($name:ident, $inner:ident) => {
        fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap($inner(args)))
        }
    };
}
db_native!(db_open, open_inner);
db_native!(db_prepare, prepare_inner);
db_native!(db_bind, bind_inner);
db_native!(db_bind_named, bind_named_inner);
db_native!(db_query, query_inner);
db_native!(db_exec, exec_inner);
db_native!(db_begin, begin_inner);
db_native!(db_commit, commit_inner);
db_native!(db_rollback, rollback_inner);
db_native!(db_close, close_inner);
db_native!(row_get_int, get_int_inner);
db_native!(row_get_string, get_string_inner);
db_native!(row_get_float, get_float_inner);
db_native!(row_get_bool, get_bool_inner);
db_native!(row_get_int_or_null, get_int_or_null_inner);
db_native!(row_get_string_or_null, get_string_or_null_inner);
db_native!(row_get_float_or_null, get_float_or_null_inner);
db_native!(row_get_bool_or_null, get_bool_or_null_inner);
db_native!(row_column_names, column_names_inner);
db_native!(row_is_null, is_null_inner);

/// The `Core.DbSys` registry entries — the INTERNAL natives the phorj-source `Core.Db` prelude wraps.
/// They live under the `DbSys` qualifier (NOT `Db`) so a prelude `class Db` calling `DbSys.open(..)`
/// never collides with the class. Every opaque connection / statement / row handle is typed `DbHandle`
/// (a reserved opaque type backed by `Value::Db`/`Value::Map` — the prelude threads it, never inspects
/// it). Every native is `pure: false` (opens/uses a real DB resource) so any `import Core.Db` program is
/// auto-quarantined from the byte-identity differential, and every native returns `Result<T, string>`
/// (Success | Failure) — never a hard fault on a DB error (the prelude throws a catchable `DbError`).
/// The `php` emitters map to PDO (DEC-208 LADDER case 1); finalized in the transpile slice.
pub fn db_natives() -> Vec<NativeFn> {
    let handle = || Ty::Named("DbHandle".into(), vec![]);
    let res = |t: Ty| Ty::Named("DbResult".into(), vec![t]);
    // A bindable scalar — the same union the old Core.Sql `Query.bind` used.
    let bindable = || Ty::Union(vec![Ty::String, Ty::Int, Ty::Float, Ty::Bool]);
    vec![
        NativeFn {
            module: "Core.DbSys",
            name: "connect",
            params: vec![Ty::String],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_open),
            php: |a| format!("new \\PDO({})", a.first().map_or("''", |s| s)),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "prepare",
            params: vec![handle(), Ty::String],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_prepare),
            php: |a| format!("{}->prepare({})", a[0], a[1]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "bind",
            params: vec![handle(), bindable()],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_bind),
            // Positional binds are collected and passed to execute() in the transpile slice; the
            // receiver PHP is threaded through for now (finalized there).
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "bindNamed",
            params: vec![handle(), Ty::String, bindable()],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_bind_named),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "query",
            params: vec![handle()],
            ret: res(Ty::List(Box::new(handle()))),
            pure: false,
            eval: NativeEval::Pure(db_query),
            php: |a| {
                format!(
                    "{}->execute() /* fetchAll finalized in transpile slice */",
                    a[0]
                )
            },
        },
        NativeFn {
            module: "Core.DbSys",
            name: "exec",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_exec),
            php: |a| format!("{}->execute()", a[0]),
        },
        // --- Transaction control (DEC-208 slice C). Savepoint-aware via the connection's depth counter
        // (managed in the native, shared across handles). The `php` emitters map to PDO's transaction
        // methods (LADDER case 1); nested-savepoint PDO emission is finalized in the transpile slice. ---
        NativeFn {
            module: "Core.DbSys",
            name: "begin",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_begin),
            php: |a| format!("{}->beginTransaction()", a[0]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "commit",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_commit),
            php: |a| format!("{}->commit()", a[0]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "rollback",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_rollback),
            php: |a| format!("{}->rollBack()", a[0]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "close",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_close),
            // PDO closes when the last reference is unset; there is no explicit close() method.
            php: |_a| "null".to_string(),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getInt",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(row_get_int),
            php: |a| format!("(int) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getString",
            params: vec![handle(), Ty::String],
            ret: res(Ty::String),
            pure: false,
            eval: NativeEval::Pure(row_get_string),
            php: |a| format!("(string) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getFloat",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Float),
            pure: false,
            eval: NativeEval::Pure(row_get_float),
            php: |a| format!("(float) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getBool",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Bool),
            pure: false,
            eval: NativeEval::Pure(row_get_bool),
            php: |a| format!("(bool) {}[{}]", a[0], a[1]),
        },
        // Nullable accessors (DEC-208 S2): a NULL column yields `null`; a wrong non-null type is still
        // a DB error. `ret` is `DbResult<T?>` so the prelude method types as `T?`.
        NativeFn {
            module: "Core.DbSys",
            name: "getIntOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Int))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_or_null),
            php: |a| format!("(({0}[{1}] === null) ? null : (int) {0}[{1}])", a[0], a[1]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getStringOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_or_null),
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : (string) {0}[{1}])",
                    a[0], a[1]
                )
            },
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getFloatOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Float))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_or_null),
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : (float) {0}[{1}])",
                    a[0], a[1]
                )
            },
        },
        NativeFn {
            module: "Core.DbSys",
            name: "getBoolOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Bool))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_or_null),
            php: |a| format!("(({0}[{1}] === null) ? null : (bool) {0}[{1}])", a[0], a[1]),
        },
        // Column introspection (DEC-208 slice B). `columnNames` → ordered `List<string>`; `isNull` →
        // `bool`. Used by the `queryScalar`/`queryMap`/nested-hydration desugar; PHP emitters are
        // placeholders (Core.Db is spine-quarantined, transpile finalized in a later slice).
        NativeFn {
            module: "Core.DbSys",
            name: "columnNames",
            params: vec![handle()],
            ret: res(Ty::List(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_column_names),
            php: |a| format!("array_keys({})", a[0]),
        },
        NativeFn {
            module: "Core.DbSys",
            name: "isNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Bool),
            pure: false,
            eval: NativeEval::Pure(row_is_null),
            php: |a| format!("({0}[{1}] === null)", a[0], a[1]),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the payload of a `Result.Success(v)` value the natives now return; panic on `Failure`.
    fn ok_of(v: Value) -> Value {
        match v {
            Value::Enum(e) if e.variant.as_ref() == "Ok" => e.payload[0].clone(),
            other => panic!("expected DbResult.Ok, got {other:?}"),
        }
    }

    /// Extract the message of a `Result.Failure(msg)` value; panic on `Success`.
    fn err_of(v: Value) -> String {
        match v {
            Value::Enum(e) if e.variant.as_ref() == "Err" => match &e.payload[0] {
                Value::Str(s) => s.as_str().to_string(),
                other => panic!("Failure payload not a string: {other:?}"),
            },
            other => panic!("expected DbResult.Err, got {other:?}"),
        }
    }

    /// End-to-end runtime round-trip (in-process): open in-memory → DDL → insert (positional + named
    /// binds) → query back → Row accessors. Proves the rusqlite integration through the `Value` model
    /// and the `Result`-returning protocol, independent of the language surface (which lands next slice).
    #[test]
    fn db_runtime_round_trip() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str("sqlite::memory:".into())], &mut out).unwrap());

        // CREATE TABLE (no binds) via exec.
        let stmt = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("CREATE TABLE users(name TEXT, age INTEGER)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        assert!(ok_of(db_exec(&[stmt], &mut out).unwrap()).eq_val(&Value::Int(0)));

        // INSERT with positional binds.
        let ins = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO users(name, age) VALUES(?, ?)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let ins = ok_of(db_bind(&[ins, Value::Str("Ada".into())], &mut out).unwrap());
        let ins = ok_of(db_bind(&[ins, Value::Int(36)], &mut out).unwrap());
        assert!(ok_of(db_exec(&[ins], &mut out).unwrap()).eq_val(&Value::Int(1)));

        // INSERT with named binds.
        let ins2 = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO users(name, age) VALUES(:n, :a)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let ins2 = ok_of(
            db_bind_named(
                &[ins2, Value::Str("n".into()), Value::Str("Grace".into())],
                &mut out,
            )
            .unwrap(),
        );
        let ins2 = ok_of(
            db_bind_named(&[ins2, Value::Str("a".into()), Value::Int(45)], &mut out).unwrap(),
        );
        assert!(ok_of(db_exec(&[ins2], &mut out).unwrap()).eq_val(&Value::Int(1)));

        // Query back, ordered, and read via Row accessors.
        let sel = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("SELECT name, age FROM users WHERE age > ? ORDER BY age".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let sel = ok_of(db_bind(&[sel, Value::Int(30)], &mut out).unwrap());
        let rows = ok_of(db_query(&[sel], &mut out).unwrap());
        let Value::List(rows) = rows else {
            panic!("query must return a list")
        };
        assert_eq!(rows.len(), 2);

        // Row 0 = Ada / 36.
        assert!(ok_of(
            row_get_string(&[rows[0].clone(), Value::Str("name".into())], &mut out).unwrap()
        )
        .eq_val(&Value::Str("Ada".into())));
        assert!(ok_of(
            row_get_int(&[rows[0].clone(), Value::Str("age".into())], &mut out).unwrap()
        )
        .eq_val(&Value::Int(36)));
        // Row 1 = Grace / 45.
        assert!(ok_of(
            row_get_string(&[rows[1].clone(), Value::Str("name".into())], &mut out).unwrap()
        )
        .eq_val(&Value::Str("Grace".into())));
    }

    #[test]
    fn mixing_bind_styles_is_a_failure() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(db_prepare(&[db, Value::Str("SELECT ?, :x".into())], &mut out).unwrap());
        let s = ok_of(db_bind(&[s, Value::Int(1)], &mut out).unwrap());
        // A DB usage error is a catchable Result.Failure, NOT a hard fault.
        let msg =
            err_of(db_bind_named(&[s, Value::Str("x".into()), Value::Int(2)], &mut out).unwrap());
        assert!(msg.contains("cannot mix"), "got: {msg}");
    }

    #[test]
    fn get_int_on_null_is_a_failure() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(db_prepare(&[db, Value::Str("SELECT NULL AS x".into())], &mut out).unwrap());
        let rows = ok_of(db_query(&[s], &mut out).unwrap());
        let Value::List(rows) = rows else { panic!() };
        let msg =
            err_of(row_get_int(&[rows[0].clone(), Value::Str("x".into())], &mut out).unwrap());
        assert!(msg.contains("NULL"), "got: {msg}");
    }

    // ── DEC-208 slice C: transactions, savepoints, taxonomy, close ─────────────────────────────

    /// Open an in-memory DB and run one `exec` statement, panicking on any failure.
    fn exec1(db: &Value, sql: &str, out: &mut String) {
        let s = ok_of(db_prepare(&[db.clone(), Value::Str(sql.into())], out).unwrap());
        ok_of(db_exec(&[s], out).unwrap());
    }

    /// Read a single-int scalar from `sql`.
    fn scalar(db: &Value, sql: &str, col: &str, out: &mut String) -> i64 {
        let s = ok_of(db_prepare(&[db.clone(), Value::Str(sql.into())], out).unwrap());
        let rows = ok_of(db_query(&[s], out).unwrap());
        let Value::List(rows) = rows else {
            panic!("query returns a list")
        };
        let v = ok_of(row_get_int(&[rows[0].clone(), Value::Str(col.into())], out).unwrap());
        match v {
            Value::Int(n) => n,
            other => panic!("expected int, got {other:?}"),
        }
    }

    /// A UNIQUE / PRIMARY KEY collision maps (via the extended result code) to the `UniqueViolation`
    /// marker the prelude classifier reads.
    #[test]
    fn unique_violation_is_tagged() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(id INTEGER PRIMARY KEY)", &mut out);
        exec1(&db, "INSERT INTO t(id) VALUES(1)", &mut out);
        let s = ok_of(
            db_prepare(
                &[db, Value::Str("INSERT INTO t(id) VALUES(1)".into())],
                &mut out,
            )
            .unwrap(),
        );
        let msg = err_of(db_exec(&[s], &mut out).unwrap());
        assert!(msg.starts_with("<<UniqueViolation>>"), "got: {msg}");
    }

    /// A malformed statement maps to the `SyntaxError` marker.
    #[test]
    fn syntax_error_is_tagged() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(db_prepare(&[db, Value::Str("SELCT oops".into())], &mut out).unwrap());
        let msg = err_of(db_query(&[s], &mut out).unwrap());
        assert!(msg.starts_with("<<SyntaxError>>"), "got: {msg}");
    }

    /// A concurrent write lock (`SQLITE_BUSY`) maps to `SerializationFailure` — the transient class
    /// `retry` would target. Provoked deterministically with two file connections and no busy handler.
    #[test]
    fn busy_maps_to_serialization_failure() {
        let path = std::env::temp_dir().join(format!(
            "phorj_db_busy_{}_{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let c1 = rusqlite::Connection::open(&path).unwrap();
        c1.execute_batch("CREATE TABLE t(x)").unwrap();
        // c1 takes a write lock and holds it.
        c1.execute_batch("BEGIN IMMEDIATE").unwrap();
        let c2 = rusqlite::Connection::open(&path).unwrap();
        // c2's write attempt cannot acquire the lock → SQLITE_BUSY (no busy timeout set).
        let err = c2.execute_batch("BEGIN IMMEDIATE").unwrap_err();
        let msg = sql_err(err);
        let _ = std::fs::remove_file(&path);
        assert!(msg.starts_with("<<SerializationFailure>>"), "got: {msg}");
    }

    /// A committed transaction persists; a rolled-back one is discarded.
    #[test]
    fn commit_persists_rollback_discards() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);

        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap());
        exec1(&db, "INSERT INTO t(n) VALUES(1)", &mut out);
        ok_of(db_commit(std::slice::from_ref(&db), &mut out).unwrap());
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);

        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap());
        exec1(&db, "INSERT INTO t(n) VALUES(2)", &mut out);
        ok_of(db_rollback(std::slice::from_ref(&db), &mut out).unwrap());
        // Still one row — the second insert was rolled back.
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);
    }

    /// A nested `begin` is a SAVEPOINT: rolling it back leaves the outer transaction's work intact.
    #[test]
    fn savepoint_partial_rollback() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);

        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap()); // outer
        exec1(&db, "INSERT INTO t(n) VALUES(1)", &mut out);
        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap()); // savepoint
        exec1(&db, "INSERT INTO t(n) VALUES(2)", &mut out);
        ok_of(db_rollback(std::slice::from_ref(&db), &mut out).unwrap()); // roll back savepoint only
        ok_of(db_commit(std::slice::from_ref(&db), &mut out).unwrap()); // commit outer
                                                                        // Only the outer insert (n=1) survives.
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);
        assert_eq!(scalar(&db, "SELECT n FROM t", "n", &mut out), 1);
    }

    /// `close` is idempotent and invalidates every derived handle; a later op faults with the
    /// `ConnectionError` marker.
    #[test]
    fn close_invalidates_connection() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);
        ok_of(db_close(std::slice::from_ref(&db), &mut out).unwrap());
        // Idempotent: a second close is still Ok.
        ok_of(db_close(std::slice::from_ref(&db), &mut out).unwrap());
        let msg = err_of(db_prepare(&[db, Value::Str("SELECT 1".into())], &mut out).unwrap());
        assert!(msg.starts_with("<<ConnectionError>>"), "got: {msg}");
    }

    #[test]
    fn column_names_are_selection_ordered() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(
            db_prepare(
                &[db, Value::Str("SELECT 1 AS a, 2 AS b, 3 AS c".into())],
                &mut out,
            )
            .unwrap(),
        );
        let rows = ok_of(db_query(&[s], &mut out).unwrap());
        let Value::List(rows) = rows else { panic!() };
        let cols = ok_of(row_column_names(&[rows[0].clone()], &mut out).unwrap());
        let Value::List(cols) = cols else {
            panic!("columnNames must return a list")
        };
        let got: Vec<&str> = cols
            .iter()
            .map(|v| match v {
                Value::Str(s) => s.as_str(),
                _ => panic!("column name not a string"),
            })
            .collect();
        assert_eq!(got, vec!["a", "b", "c"]);
    }

    #[test]
    fn is_null_reports_null_and_present() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(
            db_prepare(
                &[db, Value::Str("SELECT NULL AS a, 7 AS b".into())],
                &mut out,
            )
            .unwrap(),
        );
        let rows = ok_of(db_query(&[s], &mut out).unwrap());
        let Value::List(rows) = rows else { panic!() };
        assert!(
            ok_of(row_is_null(&[rows[0].clone(), Value::Str("a".into())], &mut out).unwrap())
                .eq_val(&Value::Bool(true))
        );
        assert!(
            ok_of(row_is_null(&[rows[0].clone(), Value::Str("b".into())], &mut out).unwrap())
                .eq_val(&Value::Bool(false))
        );
        // A missing column is a strict DB error (reuses `row_cell`).
        let msg =
            err_of(row_is_null(&[rows[0].clone(), Value::Str("zzz".into())], &mut out).unwrap());
        assert!(msg.contains("no column"), "got: {msg}");
    }
}
