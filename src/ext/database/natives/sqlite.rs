//! The SQLite [`DriverConn`] backend for `Core.DatabaseModule` (DEC-208), over bundled `rusqlite`.
//!
//! This is the driver behind a `sqlite:…` / `:memory:` DSN — the ORIGINAL, shipped `Core.DatabaseModule` runtime,
//! moved BEHIND the [`DriverConn`] trait unchanged (DEC-208 slice I, multi-driver refactor). Every value
//! mapping, placeholder-expansion, error-classification and transaction rule here is byte-identical to
//! the pre-refactor single-file implementation, so all shipped `database` tests pass unchanged — the refactor
//! is a pure extraction. Postgres ([`super::postgres`]) plugs into the same trait.
//!
//! The generic layer ([`super`]) owns the opaque handles, the bind accumulator ([`Binds`]), the natives,
//! the `DatabaseResult` protocol, the row accessors, and the (portable) transaction-control SQL; this module
//! owns only what is genuinely SQLite-specific: the `rusqlite` connection, the storage-class value
//! conversions, the `?`-placeholder expansion, and the extended-result-code taxonomy mapping.

use super::driver::DriverConn;
use super::handles::{Binds, PosBind};
use crate::value::{HKey, Value};
use std::rc::Rc;

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
                "Core.DatabaseModule: cannot bind a {} value",
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

/// Classify a `rusqlite` error into the taxonomy marker the prelude's `DatabaseError.fail` reads (DEC-208
/// slice C, spec §6). The mapping keys off SQLite's (extended) result codes: `SQLITE_CONSTRAINT_UNIQUE`
/// / `_PRIMARYKEY` → `UniqueViolationError`, generic `SQLITE_CONSTRAINT` → `ConstraintViolationError`,
/// `SQLITE_BUSY`/`SQLITE_LOCKED` → `SerializationFailureError` (the transient class retry targets — the
/// spec's `Deadlock` under one name), `SQLITE_CANTOPEN`/`SQLITE_NOTADB` → `ConnectionError`, generic
/// `SQLITE_ERROR` (a mis-typed statement at prepare time) → `SyntaxError`. Anything else stays generic
/// (no marker → the base `DatabaseError`). `TimeoutError` has no SQLite source yet (it arrives with query
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
        return Some("UniqueViolationError");
    }
    // The primary result code is the low byte of the extended code.
    match ext & 0xff {
        rusqlite::ffi::SQLITE_CONSTRAINT => Some("ConstraintViolationError"),
        rusqlite::ffi::SQLITE_BUSY | rusqlite::ffi::SQLITE_LOCKED => {
            Some("SerializationFailureError")
        }
        rusqlite::ffi::SQLITE_CANTOPEN | rusqlite::ffi::SQLITE_NOTADB => Some("ConnectionError"),
        rusqlite::ffi::SQLITE_ERROR => Some("SyntaxError"),
        _ => None,
    }
}

/// Render a `rusqlite` error as the `DatabaseResult.Err` message the prelude throws on, PREFIXED with a
/// `<<Kind>>` marker when the error classifies into the typed taxonomy (see [`err_kind`]). The prelude's
/// single `DatabaseError.fail` classification point strips the marker and throws the matching subtype.
pub(super) fn sql_err(e: rusqlite::Error) -> String {
    let kind = err_kind(&e);
    let base = format!("Core.DatabaseModule: {e}");
    match kind {
        Some(tag) => format!("<<{tag}>>{base}"),
        None => base,
    }
}

/// Render a positional-bind statement: walk the SQL, and for each bare `?` placeholder (bare `?` only —
/// numbered `?NNN` and named `:name` are NOT rewritten; `?` inside `'single'`/`"double"` quotes is
/// skipped) substitute the matching [`PosBind`] left-to-right — a [`One`](PosBind::One) keeps the `?`
/// and binds one value; a [`List`](PosBind::List) expands to `(?,?,…)` (or `(NULL)` when empty) and
/// binds each element. Returns the effective SQL + the flattened parameter vector. A `?`/bind count
/// mismatch is a clean DB error (catchable), never a silent misbind.
pub(super) fn expand_placeholders(
    sql: &str,
    binds: &[PosBind],
) -> Result<(String, Vec<rusqlite::types::Value>), String> {
    let mut out = String::with_capacity(sql.len());
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let mut idx = 0usize;
    let mut in_squote = false;
    let mut in_dquote = false;
    for c in sql.chars() {
        match c {
            '\'' if !in_dquote => {
                in_squote = !in_squote;
                out.push(c);
            }
            '"' if !in_squote => {
                in_dquote = !in_dquote;
                out.push(c);
            }
            '?' if !in_squote && !in_dquote => {
                let b = binds.get(idx).ok_or_else(|| {
                    "Core.DatabaseModule: more ? placeholders than bound values".to_string()
                })?;
                idx += 1;
                match b {
                    PosBind::One(v) => {
                        out.push('?');
                        params.push(to_sql(v)?);
                    }
                    // A list expands the SINGLE `?` in place to a comma list of placeholders, reusing the
                    // user's surrounding parens (`… IN (?)` → `… IN (?,?,?)`). An EMPTY list becomes the
                    // literal `NULL` (`… IN (NULL)` — a never-true membership, the sane empty-IN default).
                    PosBind::List(vs) if vs.is_empty() => out.push_str("NULL"),
                    PosBind::List(vs) => {
                        for (i, v) in vs.iter().enumerate() {
                            if i > 0 {
                                out.push(',');
                            }
                            out.push('?');
                            params.push(to_sql(v)?);
                        }
                    }
                }
            }
            _ => out.push(c),
        }
    }
    if idx != binds.len() {
        return Err(format!(
            "Core.DatabaseModule: {} bound value(s) but {} ? placeholder(s) in the SQL",
            binds.len(),
            idx
        ));
    }
    Ok((out, params))
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

/// The selection-ordered column names of a prepared statement.
fn col_names(prepared: &rusqlite::Statement) -> Vec<String> {
    prepared
        .column_names()
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// Prepare `sql`, bind `binds`, and execute a write on `conn`, returning the affected-row count.
/// Shared by `exec`, `exec_returning_id`, and the per-row loop of `execute_many`.
///
/// Uses rusqlite's `prepare_cached` (an LRU statement cache keyed by SQL text) rather than a fresh
/// `prepare`: a naive request handler re-prepares the SAME statement per row (`db.prepare("INSERT
/// …")` in a loop), and caching skips SQLite's re-compile of identical SQL on every hit — the DEC-266
/// perf lever for `dbwork`. PDO does NOT cache prepares, so this is a genuine language-side advantage
/// on identical code. Byte-identical: the cached statement is reset + re-bound per execute (rusqlite
/// resets on return-to-cache), so results are unchanged; validated by `tests/database.rs` on both backends.
fn exec_bound(conn: &rusqlite::Connection, sql: &str, binds: &Binds) -> Result<usize, String> {
    match binds {
        Binds::None => {
            let mut prepared = conn.prepare_cached(sql).map_err(sql_err)?;
            prepared.execute([]).map_err(sql_err)
        }
        Binds::Positional(pbs) => {
            let (esql, sv) = expand_placeholders(sql, pbs)?;
            let mut prepared = conn.prepare_cached(&esql).map_err(sql_err)?;
            prepared
                .execute(rusqlite::params_from_iter(sv.iter()))
                .map_err(sql_err)
        }
        Binds::Named(pairs) => {
            let mut prepared = conn.prepare_cached(sql).map_err(sql_err)?;
            let sv: Vec<(String, rusqlite::types::Value)> = pairs
                .iter()
                .map(|(k, v)| Ok((format!(":{k}"), to_sql(v)?)))
                .collect::<Result<_, String>>()?;
            let refs: Vec<(&str, &dyn rusqlite::ToSql)> = sv
                .iter()
                .map(|(k, v)| (k.as_str(), v as &dyn rusqlite::ToSql))
                .collect();
            prepared.execute(refs.as_slice()).map_err(sql_err)
        }
    }
}

/// A live SQLite connection, wrapped as the `sqlite:` [`DriverConn`]. rusqlite's `Connection` methods
/// take `&self`, so no interior mutability is needed here — the generic layer already guards liveness
/// (via the shared `Option<Box<dyn DriverConn>>` set to `None` on `close()`).
#[derive(Debug)]
pub(super) struct SqliteConn {
    conn: rusqlite::Connection,
}

/// Open a `sqlite:` / `:memory:` connection (DEC-208). `dsn` is `"sqlite:PATH"` / `"sqlite::memory:"`
/// (the PDO DSN shape); a bare path (no scheme) is also accepted — unchanged from the shipped runtime.
pub(super) fn open(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    let conn = if dsn == "sqlite::memory:" || dsn == ":memory:" {
        rusqlite::Connection::open_in_memory()
    } else {
        let path = dsn.strip_prefix("sqlite:").unwrap_or(dsn);
        rusqlite::Connection::open(path)
    }
    .map_err(sql_err)?;
    Ok(Box::new(SqliteConn { conn }))
}

impl DriverConn for SqliteConn {
    fn query(&self, sql: &str, binds: &Binds) -> Result<Value, String> {
        let rows = match binds {
            Binds::None => {
                let mut prepared = self.conn.prepare_cached(sql).map_err(sql_err)?;
                let cols = col_names(&prepared);
                let mut r = prepared.query([]).map_err(sql_err)?;
                collect_rows(&mut r, &cols)?
            }
            Binds::Positional(pbs) => {
                let (esql, sv) = expand_placeholders(sql, pbs)?;
                let mut prepared = self.conn.prepare_cached(&esql).map_err(sql_err)?;
                let cols = col_names(&prepared);
                let mut r = prepared
                    .query(rusqlite::params_from_iter(sv.iter()))
                    .map_err(sql_err)?;
                collect_rows(&mut r, &cols)?
            }
            Binds::Named(pairs) => {
                let mut prepared = self.conn.prepare_cached(sql).map_err(sql_err)?;
                let cols = col_names(&prepared);
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

    fn exec(&self, sql: &str, binds: &Binds) -> Result<i64, String> {
        exec_bound(&self.conn, sql, binds).map(|n| n as i64)
    }

    fn exec_returning_id(&self, sql: &str, binds: &Binds) -> Result<i64, String> {
        exec_bound(&self.conn, sql, binds)?;
        Ok(self.conn.last_insert_rowid())
    }

    fn last_insert_id(&self) -> Result<i64, String> {
        Ok(self.conn.last_insert_rowid())
    }

    fn execute_many(
        &self,
        sql: &str,
        rows: &[Value],
        _in_transaction: bool,
    ) -> Result<i64, String> {
        // SQLite auto-opens a transaction for a standalone `SAVEPOINT`, so the bulk savepoint is issued
        // unconditionally here (byte-identical to the shipped runtime; `_in_transaction` is only used by
        // Postgres, which rejects a savepoint outside a transaction block).
        self.conn
            .execute_batch("SAVEPOINT phorj_bulk")
            .map_err(sql_err)?;
        let run = || -> Result<i64, String> {
            let mut prepared = self.conn.prepare_cached(sql).map_err(sql_err)?;
            let mut total = 0i64;
            for row in rows.iter() {
                let vals = match row {
                    Value::List(v) => v,
                    other => {
                        return Err(format!(
                            "Core.DatabaseModule.executeMany: each row must be a list, got {}",
                            other.type_name()
                        ))
                    }
                };
                let sv: Vec<rusqlite::types::Value> =
                    vals.iter().map(to_sql).collect::<Result<_, _>>()?;
                let n = prepared
                    .execute(rusqlite::params_from_iter(sv.iter()))
                    .map_err(sql_err)?;
                total += n as i64;
            }
            Ok(total)
        };
        match run() {
            Ok(total) => {
                self.conn
                    .execute_batch("RELEASE phorj_bulk")
                    .map_err(sql_err)?;
                Ok(total)
            }
            Err(e) => {
                // Best-effort unwind of the whole batch; return the ORIGINAL error (a rollback failure
                // must not mask it). Same defer-don't-fail discipline as `rollback`'s no-op guard.
                let _ = self
                    .conn
                    .execute_batch("ROLLBACK TO phorj_bulk; RELEASE phorj_bulk");
                Err(e)
            }
        }
    }

    fn control(&self, sql: &str) -> Result<(), String> {
        self.conn.execute_batch(sql).map_err(sql_err)
    }

    fn set_timeout(&self, ms: i64) -> Result<(), String> {
        // SQLite: `busy_timeout(ms)` bounds how long a statement waits on a held lock before failing.
        // A genuine statement-runtime cap would need a progress-handler/interrupt (not wired); the
        // busy-wait cap is what SQLite supports cleanly. A negative `ms` is clamped by the caller.
        self.conn
            .busy_timeout(std::time::Duration::from_millis(ms.max(0) as u64))
            .map_err(sql_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `bindList` expands the single `?` in place (reusing the caller's parens) to a comma list, and to
    /// `NULL` when empty; a `?`/bind mismatch is a clean error. (SQLite-specific `?` placeholder form.)
    #[test]
    fn bind_list_placeholder_expansion() {
        let one = PosBind::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let (sql, params) = expand_placeholders("SELECT * FROM t WHERE id IN (?)", &[one]).unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE id IN (?,?,?)");
        assert_eq!(params.len(), 3);
        // Empty list → `IN (NULL)`, no params.
        let empty = PosBind::List(vec![]);
        let (sql, params) =
            expand_placeholders("SELECT * FROM t WHERE id IN (?)", &[empty]).unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE id IN (NULL)");
        assert!(params.is_empty());
        // Mixed with a positional One, left-to-right; a `?` inside a string literal is NOT a placeholder.
        let (sql, params) = expand_placeholders(
            "SELECT '?' , x WHERE a = ? AND b IN (?)",
            &[
                PosBind::One(Value::Str("hi".into())),
                PosBind::List(vec![Value::Int(9)]),
            ],
        )
        .unwrap();
        assert_eq!(sql, "SELECT '?' , x WHERE a = ? AND b IN (?)");
        assert_eq!(params.len(), 2);
        // Too few binds for the ? count is a clean error.
        assert!(expand_placeholders("a = ?", &[]).is_err());
    }

    /// A concurrent write lock (`SQLITE_BUSY`) maps to `SerializationFailureError` — the transient class
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
        assert!(
            msg.starts_with("<<SerializationFailureError>>"),
            "got: {msg}"
        );
    }
}
