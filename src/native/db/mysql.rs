//! The MySQL/MariaDB [`DriverConn`] backend for `Core.Db` (DEC-208 slice J), over the SYNC `mysql`
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
//!   `bytes`); temporal types return a clear DbError steering to `CAST(col AS CHAR)` — matching the
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

use super::{redact_dsn_password, Binds, DriverConn, PosBind};
use crate::value::{HKey, Value};
use mysql::consts::{ColumnFlags, ColumnType};
use mysql::prelude::Queryable;
use mysql::{Conn, Opts, Params, Row};
use std::cell::RefCell;
use std::rc::Rc;

/// Classify a `mysql` error into the DEC-208 taxonomy marker (spec §6), keyed off the server error
/// code: `1062`/`1586` duplicate entry → `UniqueViolation`; `1213` deadlock → `SerializationFailure`
/// (the transient class retry targets); `1205` lock-wait timeout / `3024` max_execution_time exceeded
/// / `1969` MariaDB max_statement_time → `Timeout`; FK/NOT-NULL/CHECK violations →
/// `ConstraintViolation`; `1064` parse error → `SyntaxError`; access/handshake failures →
/// `ConnectionError`. Client-side transport errors (Io/Driver/Url) are `ConnectionError`.
fn my_err_kind(e: &mysql::Error) -> Option<&'static str> {
    match e {
        mysql::Error::MySqlError(se) => Some(match se.code {
            1062 | 1586 => "UniqueViolation",
            1213 => "SerializationFailure",
            1205 | 3024 | 1969 => "Timeout",
            1048 | 1216 | 1217 | 1364 | 1451 | 1452 | 3819 => "ConstraintViolation",
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

/// Render a `mysql` error as the `DbResult.Err` message the prelude throws on, prefixed with the
/// `<<Kind>>` taxonomy marker. The crate's `Display` is the server/client message — it never contains
/// the DSN password (redacted at connect), so it is safe verbatim.
fn my_sql_err(e: mysql::Error) -> String {
    let kind = my_err_kind(&e);
    let base = format!("Core.Db: {e}");
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
/// injected by the `Db.withPassword` factory, slice G) is parsed into [`Opts`] and never retained —
/// only the redacted DSN is stored, so a connect error prints the host but never the password.
pub(super) fn open(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    let normalized = match dsn.strip_prefix("mariadb://") {
        Some(rest) => format!("mysql://{rest}"),
        None => dsn.to_string(),
    };
    let redacted = redact_dsn_password(dsn);
    let opts = Opts::from_url(&normalized)
        .map_err(|e| format!("<<ConnectionError>>Core.Db: invalid mysql DSN `{redacted}`: {e}"))?;
    let conn = Conn::new(opts).map_err(|e| {
        let base = format!("Core.Db: cannot connect to `{redacted}`: {e}");
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

/// Phorj value → a MySQL bind parameter. No prepared-type narrowing is needed (the server coerces
/// binary-protocol params); a `bool` binds as `0`/`1` (MySQL `BOOL` IS `TINYINT(1)`). A non-scalar is
/// a clean error, never a silent coercion — matching the sibling drivers' bindable set.
fn my_param(v: &Value) -> Result<mysql::Value, String> {
    Ok(match v {
        Value::Int(n) => mysql::Value::Int(*n),
        Value::Float(f) => mysql::Value::Double(*f),
        Value::Str(s) => mysql::Value::Bytes(s.as_bytes().to_vec()),
        Value::Bool(b) => mysql::Value::Int(i64::from(*b)),
        Value::Bytes(b) => mysql::Value::Bytes((**b).clone()),
        Value::Null => mysql::Value::NULL,
        other => {
            return Err(format!(
                "Core.Db: cannot bind a {} value (bind a decimal as text)",
                other.type_name()
            ))
        }
    })
}

/// Expand the accumulated positional binds against the SQL's `?` placeholders: a
/// [`One`](PosBind::One) keeps its `?`; a [`List`](PosBind::List) expands its single `?` to
/// `?,?,…` (`NULL` when empty) — the typed `IN`-list bind (slice D). Quote-aware; a `?`/bind count
/// mismatch is a clean DB error, mirroring the SQLite driver's `expand_placeholders`.
fn expand_positional(sql: &str, pbs: &[PosBind]) -> Result<(String, Vec<mysql::Value>), String> {
    let mut out = String::with_capacity(sql.len());
    let mut params: Vec<mysql::Value> = Vec::new();
    let mut consumed = 0usize;
    let mut in_s = false;
    let mut in_d = false;
    for c in sql.chars() {
        match c {
            '\'' if !in_d => {
                in_s = !in_s;
                out.push(c);
            }
            '"' if !in_s => {
                in_d = !in_d;
                out.push(c);
            }
            '?' if !in_s && !in_d => {
                let b = pbs
                    .get(consumed)
                    .ok_or_else(|| "Core.Db: more ? placeholders than bound values".to_string())?;
                consumed += 1;
                match b {
                    PosBind::One(v) => {
                        params.push(my_param(v)?);
                        out.push('?');
                    }
                    PosBind::List(vs) if vs.is_empty() => out.push_str("NULL"),
                    PosBind::List(vs) => {
                        for (j, v) in vs.iter().enumerate() {
                            if j > 0 {
                                out.push(',');
                            }
                            params.push(my_param(v)?);
                            out.push('?');
                        }
                    }
                }
            }
            _ => out.push(c),
        }
    }
    if consumed != pbs.len() {
        return Err(format!(
            "Core.Db: {} bound value(s) but {} ? placeholder(s) in the SQL",
            pbs.len(),
            consumed
        ));
    }
    Ok((out, params))
}

/// Rewrite `:name` named binds to positional `?` in first-appearance order (MySQL has no native named
/// parameters). Quote-aware; a repeated `:name` re-pushes its value (each `?` needs its own slot). A
/// `:name` with no matching bind, or a bound name never referenced, is a clean DB error.
fn translate_named(
    sql: &str,
    pairs: &[(String, Value)],
) -> Result<(String, Vec<mysql::Value>), String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    let mut params: Vec<mysql::Value> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    let mut in_s = false;
    let mut in_d = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' && !in_d {
            in_s = !in_s;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '"' && !in_s {
            in_d = !in_d;
            out.push(c);
            i += 1;
            continue;
        }
        if !in_s
            && !in_d
            && c == ':'
            && matches!(chars.get(i + 1), Some(&n) if n.is_ascii_alphabetic() || n == '_')
        {
            let mut j = i + 1;
            let mut name = String::new();
            while let Some(&n) = chars.get(j) {
                if n.is_ascii_alphanumeric() || n == '_' {
                    name.push(n);
                    j += 1;
                } else {
                    break;
                }
            }
            let v = pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("Core.Db: named parameter `:{name}` was not bound"))?;
            params.push(my_param(&v)?);
            if !used.contains(&name) {
                used.push(name);
            }
            out.push('?');
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    if let Some((n, _)) = pairs.iter().find(|(n, _)| !used.contains(n)) {
        return Err(format!(
            "Core.Db: bound named parameter `:{n}` does not appear in the SQL"
        ));
    }
    Ok((out, params))
}

/// Rewrite `sql` + the accumulated [`Binds`] into `?`-form SQL and the ordered MySQL params.
fn translate(sql: &str, binds: &Binds) -> Result<(String, Vec<mysql::Value>), String> {
    match binds {
        Binds::None => Ok((sql.to_string(), Vec::new())),
        Binds::Positional(pbs) => expand_positional(sql, pbs),
        Binds::Named(pairs) => translate_named(sql, pairs),
    }
}

/// True iff a blob-family column is a genuine BINARY blob (`BLOB`/`VARBINARY`) rather than a
/// `TEXT`-family column — both arrive as blob types; the `BINARY_FLAG` distinguishes them.
fn is_binary_blob(ty: ColumnType, flags: ColumnFlags) -> bool {
    matches!(
        ty,
        ColumnType::MYSQL_TYPE_TINY_BLOB
            | ColumnType::MYSQL_TYPE_MEDIUM_BLOB
            | ColumnType::MYSQL_TYPE_LONG_BLOB
            | ColumnType::MYSQL_TYPE_BLOB
            | ColumnType::MYSQL_TYPE_STRING
            | ColumnType::MYSQL_TYPE_VAR_STRING
            | ColumnType::MYSQL_TYPE_VARCHAR
    ) && flags.contains(ColumnFlags::BINARY_FLAG)
}

/// A fetched MySQL cell → phorj value, dispatched on the wire value with the column's declared type
/// disambiguating byte payloads: `DECIMAL`/`NEWDECIMAL` bytes are the exact decimal TEXT (feeding
/// `Row.getDecimal` losslessly); `TEXT`-family bytes are `string`; a `BINARY_FLAG` blob is `bytes`;
/// temporal values return a clear DbError steering to `CAST(col AS CHAR)` (the Postgres `::text`
/// guidance, adapted).
fn my_cell(
    v: mysql::Value,
    ty: ColumnType,
    flags: ColumnFlags,
    name: &str,
) -> Result<Value, String> {
    Ok(match v {
        mysql::Value::NULL => Value::Null,
        mysql::Value::Int(n) => Value::Int(n),
        mysql::Value::UInt(u) => match i64::try_from(u) {
            Ok(n) => Value::Int(n),
            Err(_) => {
                return Err(format!(
                    "Core.Db: column `{name}` holds unsigned value {u}, out of int range"
                ))
            }
        },
        mysql::Value::Float(f) => Value::Float(f64::from(f)),
        mysql::Value::Double(d) => Value::Float(d),
        mysql::Value::Bytes(b) => {
            if is_binary_blob(ty, flags) {
                Value::Bytes(Rc::new(b))
            } else {
                match String::from_utf8(b) {
                    Ok(s) => Value::Str(s.into()),
                    Err(_) => {
                        return Err(format!(
                            "Core.Db: column `{name}` holds non-UTF-8 text — read it as BLOB"
                        ))
                    }
                }
            }
        }
        mysql::Value::Date(..) | mysql::Value::Time(..) => {
            return Err(format!(
                "Core.Db: column `{name}` has a temporal mysql type — select it as text \
                 (e.g. `SELECT CAST({name} AS CHAR)`) to read date/time values"
            ))
        }
    })
}

/// A `mysql::Row` → a `Value::Map` (column-name → value), selection-ordered — the same `Row` shape
/// the sibling drivers produce, so the generic row accessors stay backend-agnostic.
fn my_row_to_map(row: Row) -> Result<Value, String> {
    let columns = row.columns();
    let mut vals = row.unwrap();
    let mut pairs = Vec::with_capacity(vals.len());
    for (i, col) in columns.iter().enumerate() {
        let v = std::mem::replace(&mut vals[i], mysql::Value::NULL);
        let name = col.name_str().to_string();
        let val = my_cell(v, col.column_type(), col.flags(), &name)?;
        pairs.push((HKey::Str(name.into()), val));
    }
    Ok(Value::Map(Rc::new(pairs)))
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
                            "Core.Db.executeMany: each row must be a list, got {}",
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
        // `Timeout` taxonomy). MariaDB has no such variable — fall back to its `max_statement_time`
        // (SECONDS, fractional allowed; exceeding it → 1969 → `Timeout`). `0` disables either.
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
    use super::*;

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
        assert_eq!(my_err_kind(&mk(1062)), Some("UniqueViolation"));
        assert_eq!(my_err_kind(&mk(1213)), Some("SerializationFailure"));
        assert_eq!(my_err_kind(&mk(1205)), Some("Timeout"));
        assert_eq!(my_err_kind(&mk(3024)), Some("Timeout"));
        assert_eq!(my_err_kind(&mk(1452)), Some("ConstraintViolation"));
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
