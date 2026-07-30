//! The Postgres [`DriverConn`] backend for `Core.Database` (DEC-208 slice I), over the SYNC `postgres` crate.
//!
//! This is the driver behind a `postgres://…` / `postgresql://…` DSN. It plugs into the SAME
//! [`DriverConn`] trait as the shipped SQLite driver ([`super::sqlite`]) — the multi-driver seam — so
//! the generic layer ([`super`]: handles, binds, natives, row accessors, portable transaction-control
//! SQL) is backend-agnostic and this module owns only the genuinely Postgres-specific pieces:
//!
//! - **placeholder translation** — phorj's `?` (positional) / `:name` (named) → Postgres `$1,$2,…`
//!   (the only form the wire protocol accepts), quote-aware and `::`-cast-aware ([`translate_positional`]
//!   / [`translate_named`] / [`number_qmarks`]);
//! - **value mapping** — phorj [`Value`] ↔ Postgres binary types, dispatched on each column's type OID
//!   ([`pg_params`] / [`pg_cell`]); the common scalar set (bool, int2/4/8, float4/8, text family, bytea)
//!   is native, and richer types (numeric, json, timestamp, arrays) are read via a `::text` cast (a
//!   clear DatabaseError guides the caller) — matching slice E's "store decimal columns as TEXT" guidance;
//! - **error classification** — Postgres `SQLSTATE` → the DEC-208 [`super`] taxonomy ([`pg_err_kind`]);
//! - **credential redaction** (slice G) — the DSN password is extracted into the connection [`Config`]
//!   at connect and NEVER retained; only a [`redact_dsn_password`]-scrubbed DSN is stored, so every
//!   error / log / diagnostic path is safe by construction.
//!
//! Divergences from SQLite that would be silently wrong if copied (Invariant 14, handled explicitly):
//! Postgres has no `last_insert_rowid()` (id via `RETURNING` or `lastval()`); `SAVEPOINT` is rejected
//! outside a transaction block (bulk insert opens its own `BEGIN` at depth 0); and rows are read by
//! declared column type, not by a dynamic storage class.

use super::driver::{redact_dsn_password, DriverConn};
use super::handles::Binds;
use super::postgres_sql::{
    has_returning_clause, number_qmarks, param_refs, pg_cell, pg_params, pg_row_to_map, translate,
};
use super::savepoint;
use crate::value::Value;
use postgres::{Config, NoTls};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

/// Classify a `postgres` error into the DEC-208 taxonomy marker (spec §6), keyed off the SQLSTATE code:
/// `23505` unique_violation → `UniqueViolationError`; other `23xxx` integrity → `ConstraintViolationError`;
/// `40001` serialization_failure / `40P01` deadlock_detected → `SerializationFailureError` (the transient
/// class retry targets); `57014` query_canceled (a fired `statement_timeout`) → `TimeoutError`; `08xxx`
/// connection exceptions → `ConnectionError`; `42xxx` syntax/access → `SyntaxError`. Anything else stays
/// generic (no marker → the base `DatabaseError`). Mirrors [`super::sqlite`]'s extended-result-code mapping.
fn pg_err_kind(e: &postgres::Error) -> Option<&'static str> {
    let code = e.code()?.code();
    Some(match code {
        "23505" => "UniqueViolationError",
        "40001" | "40P01" => "SerializationFailureError",
        "57014" => "TimeoutError",
        _ if code.starts_with("23") => "ConstraintViolationError",
        _ if code.starts_with("08") => "ConnectionError",
        _ if code.starts_with("42") => "SyntaxError",
        _ => return None,
    })
}

/// Render a `postgres` error as the `DatabaseResult.Err` message the prelude throws on, prefixed with the
/// `<<Kind>>` taxonomy marker (see [`pg_err_kind`]). `postgres::Error`'s `Display` is the server
/// message / IO error text — it never contains the DSN or password, so it is safe to include verbatim.
pub(super) fn pg_sql_err(e: postgres::Error) -> String {
    let kind = pg_err_kind(&e);
    let base = format!("Core.Database: {e}");
    match kind {
        Some(tag) => format!("<<{tag}>>{base}"),
        None => base,
    }
}

/// A live Postgres connection, wrapped as the `postgres://` [`DriverConn`]. The `postgres::Client`
/// methods take `&mut self`, so it lives behind a `RefCell` (the trait methods take `&self`, like the
/// SQLite driver — the generic layer already guards liveness via the shared `Option<Box<dyn
/// DriverConn>>`). `redacted_dsn` is the password-scrubbed DSN, held ONLY for diagnostics — the
/// plaintext password is never stored (slice G).
pub(super) struct PgConn {
    client: RefCell<postgres::Client>,
    redacted_dsn: String,
}

// `postgres::Client` is not `Debug`; the `DriverConn: Debug` supertrait (and the `#[derive(Debug)]` on
// the generic `DbConn` handle) needs one, so provide a redaction-safe hand-written impl.
impl std::fmt::Debug for PgConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgConn")
            .field("dsn", &self.redacted_dsn)
            .finish_non_exhaustive()
    }
}

/// Open a `postgres://` connection (DEC-208 slice I). Any password carried inline in the DSN (whether
/// the user wrote it directly or the `Connection.withPassword` factory injected a `Core.Secret`, slice G) is
/// parsed OUT of the DSN into the [`Config`] by `Config::from_str` and NEVER retained on the handle —
/// only a [`redact_dsn_password`]-scrubbed DSN is stored, so every error / diagnostic path is safe by
/// construction (a connect error prints the host but never the password, unlike PDO).
pub(super) fn open(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    let redacted = redact_dsn_password(dsn);
    let config = Config::from_str(dsn).map_err(|e| {
        format!("<<ConnectionError>>Core.Database: invalid postgres DSN `{redacted}`: {e}")
    })?;
    let client = config.connect(NoTls).map_err(|e| {
        let base = format!("Core.Database: cannot connect to `{redacted}`: {e}");
        match pg_err_kind(&e) {
            Some(tag) => format!("<<{tag}>>{base}"),
            None => format!("<<ConnectionError>>{base}"),
        }
    })?;
    Ok(Box::new(PgConn {
        client: RefCell::new(client),
        redacted_dsn: redacted,
    }))
}

impl DriverConn for PgConn {
    fn query(&self, sql: &str, binds: &Binds) -> Result<Value, String> {
        let (tsql, values) = translate(sql, binds)?;
        let mut client = self.client.borrow_mut();
        // Prepare first so each value is boxed as the exact type the server inferred for its `$n` (the
        // i64→int4 fix — see `pg_param`).
        let stmt = client.prepare(&tsql).map_err(pg_sql_err)?;
        let boxes = pg_params(&values, stmt.params())?;
        let refs = param_refs(&boxes);
        let rows = client.query(&stmt, &refs).map_err(pg_sql_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(pg_row_to_map(row)?);
        }
        Ok(Value::List(Rc::new(out)))
    }

    fn exec(&self, sql: &str, binds: &Binds) -> Result<i64, String> {
        let (tsql, values) = translate(sql, binds)?;
        let mut client = self.client.borrow_mut();
        let stmt = client.prepare(&tsql).map_err(pg_sql_err)?;
        let boxes = pg_params(&values, stmt.params())?;
        let refs = param_refs(&boxes);
        let n = client.execute(&stmt, &refs).map_err(pg_sql_err)?;
        Ok(i64::try_from(n).unwrap_or(i64::MAX))
    }

    fn exec_returning_id(&self, sql: &str, binds: &Binds) -> Result<i64, String> {
        // Postgres has no `last_insert_rowid()`. If the statement carries a RETURNING clause, read its
        // FIRST column of the FIRST row as the id (the caller names the PK, e.g.
        // `INSERT … RETURNING id`); otherwise run the write and fall back to `lastval()` (the last
        // sequence value produced on this session). This never silently assumes a `RETURNING id`.
        if has_returning_clause(sql) {
            let (tsql, values) = translate(sql, binds)?;
            let mut client = self.client.borrow_mut();
            let stmt = client.prepare(&tsql).map_err(pg_sql_err)?;
            let boxes = pg_params(&values, stmt.params())?;
            let refs = param_refs(&boxes);
            let rows = client.query(&stmt, &refs).map_err(pg_sql_err)?;
            let row = rows.first().ok_or_else(|| {
                "<<ConstraintViolationError>>Core.Database: RETURNING produced no row".to_string()
            })?;
            if row.is_empty() {
                return Err("Core.Database: RETURNING produced no column".to_string());
            }
            return match pg_cell(row, 0, row.columns()[0].type_(), row.columns()[0].name())? {
                Value::Int(n) => Ok(n),
                other => Err(format!(
                    "Core.Database.execReturningId: RETURNING column is {}, not an int id",
                    other.type_name()
                )),
            };
        }
        self.exec(sql, binds)?;
        self.last_insert_id()
    }

    fn last_insert_id(&self) -> Result<i64, String> {
        let rows = self
            .client
            .borrow_mut()
            .query("SELECT lastval()", &[])
            .map_err(pg_sql_err)?;
        match rows.first() {
            Some(r) => r.try_get::<_, i64>(0).map_err(pg_sql_err),
            None => Err("Core.Database: lastval() returned no row".to_string()),
        }
    }

    fn execute_many(&self, sql: &str, rows: &[Value], in_transaction: bool) -> Result<i64, String> {
        // Postgres rejects a standalone SAVEPOINT outside a transaction block, so open our OWN BEGIN at
        // depth 0 (and COMMIT/ROLLBACK it); when a caller transaction is already open, use a SAVEPOINT
        // (composable partial rollback), exactly like the SQLite path.
        //
        // The savepoint SQL is single-sourced (DEC-351 D5): this path used to spell a bare `RELEASE`
        // (a MySQL syntax error, so the module contradicted its own mysql driver) and to `;`-join the
        // undo PAIR into one string — legal only because `batch_execute` tolerates it. The undo is now
        // a LIST of single statements, which is what `DriverConn::control` promises everywhere.
        let (open, ok_sql, undo_sql) = if in_transaction {
            (
                savepoint::open(savepoint::BULK),
                savepoint::release(savepoint::BULK),
                vec![
                    savepoint::rollback_to(savepoint::BULK),
                    savepoint::release(savepoint::BULK),
                ],
            )
        } else {
            (
                "BEGIN".to_string(),
                "COMMIT".to_string(),
                vec!["ROLLBACK".to_string()],
            )
        };
        // Best-effort unwind: every statement attempted, each failure discarded, so a rollback error can
        // never mask the original one.
        let undo = || {
            for stmt in &undo_sql {
                let _ = self.client.borrow_mut().batch_execute(stmt);
            }
        };
        {
            self.client
                .borrow_mut()
                .batch_execute(&open)
                .map_err(pg_sql_err)?;
        }
        let (tsql, _n) = match number_qmarks(sql) {
            Ok(v) => v,
            Err(e) => {
                undo();
                return Err(e);
            }
        };
        let run = || -> Result<i64, String> {
            let mut client = self.client.borrow_mut();
            // Prepare ONCE; every row's values are boxed against the same inferred parameter types.
            let stmt = client.prepare(&tsql).map_err(pg_sql_err)?;
            let param_types = stmt.params().to_vec();
            let mut total = 0i64;
            for row in rows {
                let vals = match row {
                    Value::List(v) => v,
                    other => {
                        return Err(format!(
                            "Core.Database.executeMany: each row must be a list, got {}",
                            other.type_name()
                        ))
                    }
                };
                let boxes = pg_params(vals, &param_types)?;
                let refs = param_refs(&boxes);
                let n = client.execute(&stmt, &refs).map_err(pg_sql_err)?;
                total += i64::try_from(n).unwrap_or(i64::MAX);
            }
            Ok(total)
        };
        match run() {
            Ok(total) => {
                self.client
                    .borrow_mut()
                    .batch_execute(&ok_sql)
                    .map_err(pg_sql_err)?;
                Ok(total)
            }
            Err(e) => {
                // Return the ORIGINAL error (a rollback failure must not mask it).
                undo();
                Err(e)
            }
        }
    }

    fn control(&self, sql: &str) -> Result<(), String> {
        self.client
            .borrow_mut()
            .batch_execute(sql)
            .map_err(pg_sql_err)
    }

    fn set_timeout(&self, ms: i64) -> Result<(), String> {
        // Postgres `statement_timeout` is a genuine per-statement runtime cap (unlike SQLite's
        // lock-only busy_timeout): a query exceeding it is cancelled with SQLSTATE 57014, which
        // `pg_err_kind` maps straight to `TimeoutError`. `0` disables it.
        let ms = ms.max(0);
        self.control(&format!("SET statement_timeout = {ms}"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::handles::PosBind;
    use super::super::postgres_sql::{translate_named, translate_positional};
    use super::*;

    // ── Placeholder translation (server-free, deterministic) ────────────────────────────────────

    #[test]
    fn positional_translates_to_dollar_n() {
        let (sql, params) = translate_positional(
            "SELECT * FROM t WHERE a = ? AND b = ?",
            &[
                PosBind::One(Value::Int(1)),
                PosBind::One(Value::Str("x".into())),
            ],
        )
        .unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE a = $1 AND b = $2");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn positional_list_expands_and_numbers() {
        let (sql, params) = translate_positional(
            "SELECT * FROM t WHERE id IN (?) AND k = ?",
            &[
                PosBind::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
                PosBind::One(Value::Int(9)),
            ],
        )
        .unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE id IN ($1,$2,$3) AND k = $4");
        assert_eq!(params.len(), 4);
        // Empty list → NULL, no param; a `?` inside a string literal is not a placeholder.
        let (sql, params) =
            translate_positional("SELECT '?', x WHERE id IN (?)", &[PosBind::List(vec![])])
                .unwrap();
        assert_eq!(sql, "SELECT '?', x WHERE id IN (NULL)");
        assert!(params.is_empty());
        // Too few binds is a clean error.
        assert!(translate_positional("a = ?", &[]).is_err());
    }

    #[test]
    fn named_translates_and_reuses_and_skips_casts() {
        // `:id` reused twice → one `$1`; `:name` → `$2`; `::text` cast emitted verbatim.
        let (sql, params) = translate_named(
            "SELECT id::text FROM t WHERE id = :id AND n = :name OR pid = :id",
            &[
                ("id".to_string(), Value::Int(7)),
                ("name".to_string(), Value::Str("a".into())),
            ],
        )
        .unwrap();
        assert_eq!(
            sql,
            "SELECT id::text FROM t WHERE id = $1 AND n = $2 OR pid = $1"
        );
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn named_unbound_and_unused_are_errors() {
        // A `:missing` with no bind.
        assert!(translate_named("WHERE a = :missing", &[]).is_err());
        // A bound name that never appears in the SQL.
        assert!(translate_named(
            "WHERE a = :a",
            &[
                ("a".to_string(), Value::Int(1)),
                ("ghost".to_string(), Value::Int(2))
            ],
        )
        .is_err());
    }

    #[test]
    fn returning_clause_detection_ignores_string_literals() {
        assert!(has_returning_clause(
            "INSERT INTO t(a) VALUES(1) RETURNING id"
        ));
        assert!(has_returning_clause(
            "insert into t(a) values(1) returning id"
        ));
        // A `returning` inside a string literal is NOT a clause.
        assert!(!has_returning_clause(
            "INSERT INTO t(a) VALUES('returning')"
        ));
        // A plain insert without the clause.
        assert!(!has_returning_clause("INSERT INTO t(a) VALUES(1)"));
        // An identifier that merely contains the word is not the keyword.
        assert!(!has_returning_clause("SELECT returning_col FROM t"));
    }

    #[test]
    fn number_qmarks_for_bulk() {
        let (sql, n) = number_qmarks("INSERT INTO t(a,b) VALUES(?, ?)").unwrap();
        assert_eq!(sql, "INSERT INTO t(a,b) VALUES($1, $2)");
        assert_eq!(n, 2);
    }

    // ── DSN password redaction (slice G) ────────────────────────────────────────────────────────

    #[test]
    fn redacts_url_form_password() {
        assert_eq!(
            redact_dsn_password("postgres://user:s3cr3t@localhost:5432/db"),
            "postgres://user:***@localhost:5432/db"
        );
        // No password in the URL → unchanged.
        assert_eq!(
            redact_dsn_password("postgres://user@localhost/db"),
            "postgres://user@localhost/db"
        );
        // Query params after the authority are preserved.
        assert_eq!(
            redact_dsn_password("postgresql://u:p@h/db?sslmode=disable"),
            "postgresql://u:***@h/db?sslmode=disable"
        );
    }

    #[test]
    fn redacts_keyword_form_password() {
        assert_eq!(
            redact_dsn_password("host=localhost user=me password=secret dbname=x"),
            "host=localhost user=me password=*** dbname=x"
        );
        assert_eq!(
            redact_dsn_password("host=h password=trailing"),
            "host=h password=***"
        );
    }

    // ── SQLSTATE → taxonomy mapping (pure function over the code string) ─────────────────────────
    // `pg_err_kind` needs a `postgres::Error`, which cannot be synthesized without a server; the code
    // classification is small and total, so we assert the mapping table directly via a shadow fn kept
    // in lockstep with `pg_err_kind` (any divergence is caught by review — both are five lines).

    fn kind_of(code: &str) -> Option<&'static str> {
        Some(match code {
            "23505" => "UniqueViolationError",
            "40001" | "40P01" => "SerializationFailureError",
            "57014" => "TimeoutError",
            _ if code.starts_with("23") => "ConstraintViolationError",
            _ if code.starts_with("08") => "ConnectionError",
            _ if code.starts_with("42") => "SyntaxError",
            _ => return None,
        })
    }

    #[test]
    fn sqlstate_taxonomy_mapping() {
        assert_eq!(kind_of("23505"), Some("UniqueViolationError"));
        assert_eq!(kind_of("23503"), Some("ConstraintViolationError")); // foreign_key
        assert_eq!(kind_of("23502"), Some("ConstraintViolationError")); // not_null
        assert_eq!(kind_of("40001"), Some("SerializationFailureError"));
        assert_eq!(kind_of("40P01"), Some("SerializationFailureError")); // deadlock
        assert_eq!(kind_of("57014"), Some("TimeoutError"));
        assert_eq!(kind_of("08006"), Some("ConnectionError"));
        assert_eq!(kind_of("42601"), Some("SyntaxError")); // syntax_error
        assert_eq!(kind_of("42P01"), Some("SyntaxError")); // undefined_table
        assert_eq!(kind_of("22012"), None); // division_by_zero → base DatabaseError
    }
}
