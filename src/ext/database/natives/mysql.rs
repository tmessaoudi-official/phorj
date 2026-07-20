//! The MySQL/MariaDB [`DriverConn`] backend for `Core.DatabaseModule` (DEC-208 slice J), over the SYNC `mysql`
//! crate (`minimal-rust`: pure-Rust wire protocol, no libmysqlclient, no TLS/compression extras).
//!
//! This is the driver behind a `mysql://…` / `mariadb://…` DSN. It plugs into the SAME
//! [`DriverConn`] trait as the SQLite and Postgres drivers — the generic layer (handles, binds,
//! natives, row accessors, portable transaction-control SQL) stays backend-agnostic and this module
//! owns only the genuinely MySQL-specific pieces:
//!
//! - **placeholder handling** — MySQL's wire protocol takes `?` natively, so positional binds pass
//!   through with only `bindList` `IN (?)` expansion ([`expand_positional`]); named `:name` binds are
//!   rewritten to `?` in first-appearance order ([`translate_named`], quote-aware — MySQL has no
//!   native named parameters);
//! - **value mapping** — phorj [`Value`] ↔ [`mysql::Value`] ([`my_param`] / [`my_cell`]): ints and
//!   floats map natively; a `bool` binds as `0`/`1` (MySQL `BOOL` IS `TINYINT(1)` — reading one back
//!   yields an `int`, exactly like SQLite's storage classes, so `getBool` keeps working); `DECIMAL`
//!   columns arrive as their exact decimal TEXT (feeding `Row.getDecimal` losslessly); `TEXT`-family
//!   blobs map to `string` unless the column is genuinely binary (`BINARY_FLAG` on a blob type →
//!   `bytes`); temporal types return a clear DatabaseError steering to `CAST(col AS CHAR)` — matching the
//!   Postgres driver's `::text` guidance;
//! - **error classification** — MySQL error codes → the DEC-208 taxonomy ([`my_err_kind`]);
//! - **credential redaction** (slice G) — the DSN password is parsed into [`Opts`] at connect and
//!   NEVER retained; only a [`redact_dsn_password`]-scrubbed DSN is stored.
//!
//! Divergences from the siblings that would be silently wrong if copied (Invariant 14, explicit):
//! MySQL has no `RETURNING` (id via the connection's `last_insert_id()`, like SQLite and unlike
//! Postgres); `SAVEPOINT` needs an open transaction (bulk insert opens its own `BEGIN` at depth 0,
//! like Postgres and unlike SQLite); and the query timeout is `max_execution_time` in ms (MySQL,
//! SELECT-only) with a `max_statement_time` (seconds) MariaDB fallback.

use super::driver::{redact_dsn_password, DriverConn};
use super::handles::Binds;
use super::mysql_sql::{my_param, my_row_to_map, translate};
use crate::value::Value;
use mysql::prelude::Queryable;
use mysql::{Conn, Opts, Params, Row};
use std::cell::RefCell;
use std::rc::Rc;

/// Classify a `mysql` error into the DEC-208 taxonomy marker (spec §6), keyed off the server error
/// code: `1062`/`1586` duplicate entry → `UniqueViolationError`; `1213` deadlock → `SerializationFailureError`
/// (the transient class retry targets); `1205` lock-wait timeout / `3024` max_execution_time exceeded
/// / `1969` MariaDB max_statement_time → `TimeoutError`; FK/NOT-NULL/CHECK violations →
/// `ConstraintViolationError`; `1064` parse error → `SyntaxError`; access/handshake failures →
/// `ConnectionError`. Client-side transport errors (Io/Driver/Url) are `ConnectionError`.
fn my_err_kind(e: &mysql::Error) -> Option<&'static str> {
    match e {
        mysql::Error::MySqlError(se) => Some(match se.code {
            1062 | 1586 => "UniqueViolationError",
            1213 => "SerializationFailureError",
            1205 | 3024 | 1969 => "TimeoutError",
            1048 | 1216 | 1217 | 1364 | 1451 | 1452 | 3819 => "ConstraintViolationError",
            1064 => "SyntaxError",
            1044 | 1045 | 1049 | 1130 => "ConnectionError",
            _ => return None,
        }),
        mysql::Error::IoError(_) | mysql::Error::DriverError(_) | mysql::Error::UrlError(_) => {
            Some("ConnectionError")
        }
        _ => None,
    }
}

/// Render a `mysql` error as the `DatabaseResult.Err` message the prelude throws on, prefixed with the
/// `<<Kind>>` taxonomy marker. The crate's `Display` is the server/client message — it never contains
/// the DSN password (redacted at connect), so it is safe verbatim.
fn my_sql_err(e: mysql::Error) -> String {
    let kind = my_err_kind(&e);
    let base = format!("Core.DatabaseModule: {e}");
    match kind {
        Some(tag) => format!("<<{tag}>>{base}"),
        None => base,
    }
}

/// A live MySQL/MariaDB connection, wrapped as the `mysql://` [`DriverConn`]. `mysql::Conn` methods
/// take `&mut self`, so it lives behind a `RefCell` (the SQLite/Postgres pattern). `redacted_dsn` is
/// the password-scrubbed DSN, held ONLY for diagnostics — the plaintext password is never stored.
pub(super) struct MyConn {
    conn: RefCell<Conn>,
    redacted_dsn: String,
}

impl std::fmt::Debug for MyConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MyConn")
            .field("dsn", &self.redacted_dsn)
            .finish_non_exhaustive()
    }
}

/// Open a `mysql://` / `mariadb://` connection (DEC-208 slice J). A `mariadb://` scheme is normalized
/// to `mysql://` (same wire protocol) before [`Opts`] parsing. Any inline password (hand-written or
/// injected by the `Database.withPassword` factory, slice G) is parsed into [`Opts`] and never retained —
/// only the redacted DSN is stored, so a connect error prints the host but never the password.
pub(super) fn open(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    let normalized = match dsn.strip_prefix("mariadb://") {
        Some(rest) => format!("mysql://{rest}"),
        None => dsn.to_string(),
    };
    let redacted = redact_dsn_password(dsn);
    let opts = Opts::from_url(&normalized).map_err(|e| {
        format!("<<ConnectionError>>Core.DatabaseModule: invalid mysql DSN `{redacted}`: {e}")
    })?;
    let conn = Conn::new(opts).map_err(|e| {
        let base = format!("Core.DatabaseModule: cannot connect to `{redacted}`: {e}");
        match my_err_kind(&e) {
            Some(tag) => format!("<<{tag}>>{base}"),
            None => format!("<<ConnectionError>>{base}"),
        }
    })?;
    Ok(Box::new(MyConn {
        conn: RefCell::new(conn),
        redacted_dsn: redacted,
    }))
}

impl DriverConn for MyConn {
    fn query(&self, sql: &str, binds: &Binds) -> Result<Value, String> {
        let (tsql, params) = translate(sql, binds)?;
        let mut conn = self.conn.borrow_mut();
        let rows: Vec<Row> = conn.exec(&tsql, Params::from(params)).map_err(my_sql_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(my_row_to_map(row)?);
        }
        Ok(Value::List(Rc::new(out)))
    }

    fn exec(&self, sql: &str, binds: &Binds) -> Result<i64, String> {
        let (tsql, params) = translate(sql, binds)?;
        let mut conn = self.conn.borrow_mut();
        conn.exec_drop(&tsql, Params::from(params))
            .map_err(my_sql_err)?;
        Ok(i64::try_from(conn.affected_rows()).unwrap_or(i64::MAX))
    }

    fn exec_returning_id(&self, sql: &str, binds: &Binds) -> Result<i64, String> {
        // MySQL has no RETURNING clause on the prepared protocol path — run the write and read the
        // connection's last_insert_id, exactly like the SQLite driver.
        self.exec(sql, binds)?;
        self.last_insert_id()
    }

    fn last_insert_id(&self) -> Result<i64, String> {
        let id = self.conn.borrow_mut().last_insert_id();
        Ok(i64::try_from(id).unwrap_or(i64::MAX))
    }

    fn execute_many(&self, sql: &str, rows: &[Value], in_transaction: bool) -> Result<i64, String> {
        // MySQL rejects a standalone SAVEPOINT under autocommit, so open our OWN BEGIN at depth 0;
        // inside a caller transaction use a SAVEPOINT (composable partial rollback) — the Postgres
        // pattern, NOT the SQLite one (Invariant 14: divergences handled explicitly).
        let (open, ok_sql, undo_sql) = if in_transaction {
            (
                "SAVEPOINT phorj_bulk",
                "RELEASE SAVEPOINT phorj_bulk",
                "ROLLBACK TO SAVEPOINT phorj_bulk",
            )
        } else {
            ("BEGIN", "COMMIT", "ROLLBACK")
        };
        self.control(open)?;
        let run = || -> Result<i64, String> {
            let mut conn = self.conn.borrow_mut();
            // Prepare ONCE; each row is a plain positional bind-set against the same statement.
            let stmt = conn.prep(sql).map_err(my_sql_err)?;
            let mut total = 0i64;
            for row in rows {
                let vals = match row {
                    Value::List(v) => v,
                    other => {
                        return Err(format!(
                            "Core.DatabaseModule.executeMany: each row must be a list, got {}",
                            other.type_name()
                        ))
                    }
                };
                let params: Vec<mysql::Value> =
                    vals.iter().map(my_param).collect::<Result<_, _>>()?;
                conn.exec_drop(&stmt, Params::from(params))
                    .map_err(my_sql_err)?;
                total += i64::try_from(conn.affected_rows()).unwrap_or(i64::MAX);
            }
            Ok(total)
        };
        match run() {
            Ok(total) => {
                self.control(ok_sql)?;
                Ok(total)
            }
            Err(e) => {
                // Best-effort unwind; return the ORIGINAL error (a rollback failure must not mask it).
                let _ = self.control(undo_sql);
                Err(e)
            }
        }
    }

    fn control(&self, sql: &str) -> Result<(), String> {
        self.conn.borrow_mut().query_drop(sql).map_err(my_sql_err)
    }

    fn set_timeout(&self, ms: i64) -> Result<(), String> {
        // MySQL 5.7.8+: `max_execution_time` in ms (SELECT-only; exceeding it → error 3024 → the
        // `TimeoutError` taxonomy). MariaDB has no such variable — fall back to its `max_statement_time`
        // (SECONDS, fractional allowed; exceeding it → 1969 → `TimeoutError`). `0` disables either.
        let ms = ms.max(0);
        let mysql_form = format!("SET SESSION max_execution_time = {ms}");
        if self.control(&mysql_form).is_ok() {
            return Ok(());
        }
        #[allow(clippy::cast_precision_loss)]
        let seconds = ms as f64 / 1000.0;
        self.control(&format!("SET SESSION max_statement_time = {seconds}"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::handles::PosBind;
    use super::super::mysql_sql::{expand_positional, my_cell, translate_named};
    use super::*;
    use mysql::consts::{ColumnFlags, ColumnType};

    // ── Placeholder handling (server-free, deterministic) ───────────────────────────────────────

    #[test]
    fn positional_passes_qmarks_through_and_expands_lists() {
        let (sql, params) = expand_positional(
            "SELECT * FROM t WHERE id IN (?) AND k = ?",
            &[
                PosBind::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
                PosBind::One(Value::Int(9)),
            ],
        )
        .unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE id IN (?,?,?) AND k = ?");
        assert_eq!(params.len(), 4);
        // Empty list → NULL, no param; a `?` inside a string literal is not a placeholder.
        let (sql, params) =
            expand_positional("SELECT '?', x WHERE id IN (?)", &[PosBind::List(vec![])]).unwrap();
        assert_eq!(sql, "SELECT '?', x WHERE id IN (NULL)");
        assert!(params.is_empty());
    }

    #[test]
    fn positional_count_mismatch_is_clean() {
        assert!(expand_positional("SELECT ?", &[]).is_err());
        assert!(expand_positional("SELECT 1", &[PosBind::One(Value::Int(1))]).is_err());
    }

    #[test]
    fn named_translates_to_qmarks_in_order() {
        let (sql, params) = translate_named(
            "UPDATE t SET a = :a WHERE b = :b AND a2 = :a",
            &[
                ("a".into(), Value::Int(1)),
                ("b".into(), Value::Str("x".into())),
            ],
        )
        .unwrap();
        assert_eq!(sql, "UPDATE t SET a = ? WHERE b = ? AND a2 = ?");
        // A repeated :name re-pushes its value — every `?` gets its own slot.
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn named_unbound_and_unused_are_clean_errors() {
        assert!(translate_named("SELECT :x", &[]).is_err());
        assert!(translate_named("SELECT 1", &[("x".into(), Value::Int(1))]).is_err());
        // A quoted `:x` is not a parameter.
        assert!(translate_named("SELECT ':x'", &[("x".into(), Value::Int(1))]).is_err());
    }

    // ── Error classification ─────────────────────────────────────────────────────────────────────

    #[test]
    fn err_kind_maps_the_taxonomy() {
        let mk = |code: u16| {
            mysql::Error::MySqlError(mysql::error::MySqlError {
                state: String::new(),
                message: String::new(),
                code,
            })
        };
        assert_eq!(my_err_kind(&mk(1062)), Some("UniqueViolationError"));
        assert_eq!(my_err_kind(&mk(1213)), Some("SerializationFailureError"));
        assert_eq!(my_err_kind(&mk(1205)), Some("TimeoutError"));
        assert_eq!(my_err_kind(&mk(3024)), Some("TimeoutError"));
        assert_eq!(my_err_kind(&mk(1452)), Some("ConstraintViolationError"));
        assert_eq!(my_err_kind(&mk(1064)), Some("SyntaxError"));
        assert_eq!(my_err_kind(&mk(1045)), Some("ConnectionError"));
        assert_eq!(my_err_kind(&mk(9999)), None);
    }

    // ── Cell mapping ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn cells_map_ints_floats_text_and_decimal() {
        let f = ColumnFlags::empty();
        assert!(matches!(
            my_cell(
                mysql::Value::Int(7),
                ColumnType::MYSQL_TYPE_LONGLONG,
                f,
                "c"
            )
            .unwrap(),
            Value::Int(7)
        ));
        assert!(matches!(
            my_cell(mysql::Value::Double(1.5), ColumnType::MYSQL_TYPE_DOUBLE, f, "c").unwrap(),
            Value::Float(x) if (x - 1.5).abs() < f64::EPSILON
        ));
        // DECIMAL arrives as exact decimal text → string (Row.getDecimal parses it losslessly).
        match my_cell(
            mysql::Value::Bytes(b"12.50".to_vec()),
            ColumnType::MYSQL_TYPE_NEWDECIMAL,
            f,
            "c",
        )
        .unwrap()
        {
            Value::Str(s) => assert_eq!(&*s, "12.50"),
            other => panic!("expected string, got {other:?}"),
        }
        // A BINARY blob stays bytes; a TEXT blob is a string.
        assert!(matches!(
            my_cell(
                mysql::Value::Bytes(vec![1, 2]),
                ColumnType::MYSQL_TYPE_BLOB,
                ColumnFlags::BINARY_FLAG,
                "c"
            )
            .unwrap(),
            Value::Bytes(_)
        ));
        assert!(matches!(
            my_cell(
                mysql::Value::Bytes(b"hi".to_vec()),
                ColumnType::MYSQL_TYPE_BLOB,
                f,
                "c"
            )
            .unwrap(),
            Value::Str(_)
        ));
        // Temporal values steer to CAST AS CHAR.
        assert!(my_cell(
            mysql::Value::Date(2026, 1, 1, 0, 0, 0, 0),
            ColumnType::MYSQL_TYPE_DATETIME,
            f,
            "c"
        )
        .is_err());
    }

    // ── DSN redaction (shared helper, exercised against mysql shapes) ────────────────────────────

    #[test]
    fn mysql_dsn_redacts_password() {
        assert_eq!(
            redact_dsn_password("mysql://user:s3cr3t@localhost:3306/db"),
            "mysql://user:***@localhost:3306/db"
        );
        assert_eq!(redact_dsn_password("mariadb://u@h/db"), "mariadb://u@h/db");
    }
}
