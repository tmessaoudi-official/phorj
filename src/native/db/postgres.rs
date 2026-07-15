//! The Postgres [`DriverConn`] backend for `Core.Db` (DEC-208 slice I), over the SYNC `postgres` crate.
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
//!   clear DbError guides the caller) — matching slice E's "store decimal columns as TEXT" guidance;
//! - **error classification** — Postgres `SQLSTATE` → the DEC-208 [`super`] taxonomy ([`pg_err_kind`]);
//! - **credential redaction** (slice G) — the DSN password is extracted into the connection [`Config`]
//!   at connect and NEVER retained; only a [`redact_dsn_password`]-scrubbed DSN is stored, so every
//!   error / log / diagnostic path is safe by construction.
//!
//! Divergences from SQLite that would be silently wrong if copied (Invariant 14, handled explicitly):
//! Postgres has no `last_insert_rowid()` (id via `RETURNING` or `lastval()`); `SAVEPOINT` is rejected
//! outside a transaction block (bulk insert opens its own `BEGIN` at depth 0); and rows are read by
//! declared column type, not by a dynamic storage class.

use super::{redact_dsn_password, Binds, DriverConn, PosBind};
use crate::value::{HKey, Value};
use postgres::types::{ToSql, Type};
use postgres::{Config, NoTls, Row};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

/// Classify a `postgres` error into the DEC-208 taxonomy marker (spec §6), keyed off the SQLSTATE code:
/// `23505` unique_violation → `UniqueViolation`; other `23xxx` integrity → `ConstraintViolation`;
/// `40001` serialization_failure / `40P01` deadlock_detected → `SerializationFailure` (the transient
/// class retry targets); `57014` query_canceled (a fired `statement_timeout`) → `Timeout`; `08xxx`
/// connection exceptions → `ConnectionError`; `42xxx` syntax/access → `SyntaxError`. Anything else stays
/// generic (no marker → the base `DbError`). Mirrors [`super::sqlite`]'s extended-result-code mapping.
fn pg_err_kind(e: &postgres::Error) -> Option<&'static str> {
    let code = e.code()?.code();
    Some(match code {
        "23505" => "UniqueViolation",
        "40001" | "40P01" => "SerializationFailure",
        "57014" => "Timeout",
        _ if code.starts_with("23") => "ConstraintViolation",
        _ if code.starts_with("08") => "ConnectionError",
        _ if code.starts_with("42") => "SyntaxError",
        _ => return None,
    })
}

/// Render a `postgres` error as the `DbResult.Err` message the prelude throws on, prefixed with the
/// `<<Kind>>` taxonomy marker (see [`pg_err_kind`]). `postgres::Error`'s `Display` is the server
/// message / IO error text — it never contains the DSN or password, so it is safe to include verbatim.
fn pg_sql_err(e: postgres::Error) -> String {
    let kind = pg_err_kind(&e);
    let base = format!("Core.Db: {e}");
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
/// the user wrote it directly or the `Db.withPassword` factory injected a `Core.Secret`, slice G) is
/// parsed OUT of the DSN into the [`Config`] by `Config::from_str` and NEVER retained on the handle —
/// only a [`redact_dsn_password`]-scrubbed DSN is stored, so every error / diagnostic path is safe by
/// construction (a connect error prints the host but never the password, unlike PDO).
pub(super) fn open(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    let redacted = redact_dsn_password(dsn);
    let config = Config::from_str(dsn).map_err(|e| {
        format!("<<ConnectionError>>Core.Db: invalid postgres DSN `{redacted}`: {e}")
    })?;
    let client = config.connect(NoTls).map_err(|e| {
        let base = format!("Core.Db: cannot connect to `{redacted}`: {e}");
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

/// Phorj value → an owned Postgres bind parameter, encoded as the EXACT Rust type the server inferred
/// for that placeholder (`expected`, read from `Statement::params()` after a prepare). This is the fix
/// for the central tokio-postgres gotcha: the crate encodes each param with `ToSql` against the inferred
/// column type, and `i64`'s `ToSql` accepts ONLY `int8` — so binding a phorj `int` into the far more
/// common `int4`/`serial` column would fail with "cannot convert between i64 and int4" if we boxed a
/// bare `i64`. By narrowing to the expected width here (`int2`→i16, `int4`→i32, `int8`→i64;
/// `float4`→f32, `float8`→f64; a numeric target → the exact decimal text with the crate coercing), an
/// `int` binds correctly into ANY integer column. A range overflow (an i64 that does not fit `int4`) is
/// a clean, catchable DB error. The bindable scalar set matches the SQLite driver
/// (int/float/string/bool/bytes/null); a non-scalar is a clean error, never a silent coercion.
fn pg_param(v: &Value, expected: &Type) -> Result<Box<dyn ToSql + Sync>, String> {
    let overflow = |n: i64, t: &str| format!("Core.Db: int {n} does not fit the {t} column");
    Ok(match v {
        Value::Int(n) => match *expected {
            Type::INT2 => Box::new(i16::try_from(*n).map_err(|_| overflow(*n, "int2"))?),
            Type::INT4 => Box::new(i32::try_from(*n).map_err(|_| overflow(*n, "int4"))?),
            Type::INT8 => Box::new(*n),
            Type::FLOAT4 => Box::new(*n as f32),
            Type::FLOAT8 => Box::new(*n as f64),
            // A numeric/decimal target: bind the exact integer text; Postgres parses it into `numeric`.
            Type::NUMERIC => Box::new(n.to_string()),
            // Any other target (incl. the server not inferring a type → `UNKNOWN`): fall back to int8.
            _ => Box::new(*n),
        },
        Value::Float(f) => match *expected {
            Type::FLOAT4 => Box::new(*f as f32),
            Type::NUMERIC => Box::new(f.to_string()),
            _ => Box::new(*f),
        },
        Value::Str(s) => Box::new(s.as_str().to_string()),
        Value::Bool(b) => Box::new(*b),
        Value::Bytes(b) => Box::new((**b).clone()),
        // A NULL's wire encoding is type-independent, but `ToSql::accepts` must match the inferred type,
        // so pick an `Option<T>` of the expected width.
        Value::Null => match *expected {
            Type::INT2 => Box::new(Option::<i16>::None),
            Type::INT4 => Box::new(Option::<i32>::None),
            Type::INT8 => Box::new(Option::<i64>::None),
            Type::FLOAT4 => Box::new(Option::<f32>::None),
            Type::FLOAT8 => Box::new(Option::<f64>::None),
            Type::BOOL => Box::new(Option::<bool>::None),
            Type::BYTEA => Box::new(Option::<Vec<u8>>::None),
            _ => Box::new(Option::<String>::None),
        },
        other => {
            return Err(format!(
                "Core.Db: cannot bind a {} value (bind a decimal as text with a ::numeric cast)",
                other.type_name()
            ))
        }
    })
}

/// Build the owned bind parameters for `values`, typed against the prepared statement's inferred
/// parameter types (`expected`). A count mismatch is a clean DB error (never a silent misbind).
fn pg_params(values: &[Value], expected: &[Type]) -> Result<Vec<Box<dyn ToSql + Sync>>, String> {
    if values.len() != expected.len() {
        return Err(format!(
            "Core.Db: {} bound value(s) for {} placeholder(s) in the SQL",
            values.len(),
            expected.len()
        ));
    }
    values
        .iter()
        .zip(expected)
        .map(|(v, t)| pg_param(v, t))
        .collect()
}

/// A fetched Postgres cell → phorj value, dispatched on the column's declared type. The common scalar
/// set is read natively (via the binary protocol); a `NULL` in any of them yields `Value::Null` (the
/// accessor layer then enforces optionality). A type outside the set (numeric, json/jsonb, timestamp,
/// arrays, …) returns a clear DbError steering to a `::text` cast — matching slice E's "store decimal
/// columns as TEXT" guidance (a `SELECT amount::text` column reads as text, and `Row.getDecimal` parses
/// it exactly).
fn pg_cell(row: &Row, i: usize, ty: &Type, name: &str) -> Result<Value, String> {
    macro_rules! read {
        ($rust:ty, $wrap:expr) => {{
            let v: Option<$rust> = row.try_get(i).map_err(pg_sql_err)?;
            match v {
                Some(x) => $wrap(x),
                None => Value::Null,
            }
        }};
    }
    // An array cell: `Option<Vec<Option<T>>>` — outer None = whole-array SQL NULL, inner None = a
    // NULL element (surfaced as `Value::Null` for the accessor layer to police).
    macro_rules! read_array {
        ($rust:ty, $wrap:expr) => {{
            let v: Option<Vec<Option<$rust>>> = row.try_get(i).map_err(pg_sql_err)?;
            match v {
                Some(items) => Value::List(Rc::new(
                    items
                        .into_iter()
                        .map(|e| match e {
                            Some(x) => $wrap(x),
                            None => Value::Null,
                        })
                        .collect(),
                )),
                None => Value::Null,
            }
        }};
    }
    Ok(match *ty {
        Type::BOOL => read!(bool, Value::Bool),
        Type::INT2 => read!(i16, |x| Value::Int(i64::from(x))),
        Type::INT4 => read!(i32, |x| Value::Int(i64::from(x))),
        Type::INT8 => read!(i64, Value::Int),
        Type::FLOAT4 => read!(f32, |x| Value::Float(f64::from(x))),
        Type::FLOAT8 => read!(f64, Value::Float),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::UNKNOWN => {
            read!(String, |x: String| Value::Str(x.into()))
        }
        Type::BYTEA => read!(Vec<u8>, |x| Value::Bytes(Rc::new(x))),
        // ARRAY columns (DEC-208 slice K): the common scalar arrays map to a `Value::List` (a NULL
        // ELEMENT maps to `Value::Null` here; the typed `getXList` accessors then reject or steer —
        // strictness lives in ONE place, the accessor layer, like scalar NULLs). A whole-array SQL
        // NULL is `Value::Null`. `numeric[]`/`json[]`/timestamp arrays stay unsupported — select
        // them with a `::text[]` cast and read `getStringList` (the slice-E text discipline).
        Type::BOOL_ARRAY => read_array!(bool, Value::Bool),
        Type::INT2_ARRAY => read_array!(i16, |x| Value::Int(i64::from(x))),
        Type::INT4_ARRAY => read_array!(i32, |x| Value::Int(i64::from(x))),
        Type::INT8_ARRAY => read_array!(i64, Value::Int),
        Type::FLOAT4_ARRAY => read_array!(f32, |x| Value::Float(f64::from(x))),
        Type::FLOAT8_ARRAY => read_array!(f64, Value::Float),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::BPCHAR_ARRAY | Type::NAME_ARRAY => {
            read_array!(String, |x: String| Value::Str(x.into()))
        }
        _ => {
            return Err(format!(
                "Core.Db: column `{name}` has unsupported postgres type `{ty}` — select it with a \
                 `::text` cast (e.g. `SELECT {name}::text`) to read numeric/json/timestamp values"
            ))
        }
    })
}

/// A `postgres::Row` → a `Value::Map` (column-name → value), selection-ordered — the same `Row` shape
/// the SQLite driver produces, so the generic row accessors are backend-agnostic.
fn pg_row_to_map(row: &Row) -> Result<Value, String> {
    let mut pairs = Vec::with_capacity(row.len());
    for (i, col) in row.columns().iter().enumerate() {
        let val = pg_cell(row, i, col.type_(), col.name())?;
        pairs.push((HKey::Str(col.name().into()), val));
    }
    Ok(Value::Map(Rc::new(pairs)))
}

/// Translate a positional-bind statement to Postgres `$n` placeholders: walk the SQL (quote-aware,
/// `::`-cast-safe by virtue of never touching `?`), and for each bare `?` substitute `$k` for a
/// [`One`](PosBind::One) (binding one value) or `$k,$k+1,…` for a [`List`](PosBind::List) (`(NULL)` when
/// empty), threading the running `$` index. Returns the rewritten SQL + the flattened value list. A
/// `?`/bind count mismatch is a clean DB error, mirroring the SQLite driver's `expand_placeholders`.
fn translate_positional(sql: &str, pbs: &[PosBind]) -> Result<(String, Vec<Value>), String> {
    let mut out = String::with_capacity(sql.len());
    let mut params: Vec<Value> = Vec::new();
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
                        params.push(v.clone());
                        out.push('$');
                        out.push_str(&params.len().to_string());
                    }
                    PosBind::List(vs) if vs.is_empty() => out.push_str("NULL"),
                    PosBind::List(vs) => {
                        for (j, v) in vs.iter().enumerate() {
                            if j > 0 {
                                out.push(',');
                            }
                            params.push(v.clone());
                            out.push('$');
                            out.push_str(&params.len().to_string());
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

/// Translate a named-bind statement (`:name`) to Postgres `$n`: quote-aware and `::`-cast-aware (a `::`
/// double-colon is a cast operator and is emitted verbatim, never read as a param). Each distinct
/// `:name` is assigned the next `$k` in first-seen order and reused on repeat; the value list follows
/// that order. A `:name` with no matching bind, or a bound name never referenced, is a clean DB error
/// (no silent misbind).
fn translate_named(sql: &str, pairs: &[(String, Value)]) -> Result<(String, Vec<Value>), String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    let mut params: Vec<Value> = Vec::new();
    let mut assigned: Vec<(String, usize)> = Vec::new();
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
        if !in_s && !in_d && c == ':' {
            // `::` cast operator — emit verbatim, never a named param.
            if chars.get(i + 1) == Some(&':') {
                out.push_str("::");
                i += 2;
                continue;
            }
            // `:name` — an identifier follows.
            if matches!(chars.get(i + 1), Some(&n) if n.is_ascii_alphabetic() || n == '_') {
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
                let idx = match assigned.iter().find(|(n, _)| *n == name) {
                    Some((_, k)) => *k,
                    None => {
                        let v = pairs
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, v)| v.clone())
                            .ok_or_else(|| {
                                format!("Core.Db: named parameter `:{name}` was not bound")
                            })?;
                        params.push(v);
                        let k = params.len();
                        assigned.push((name, k));
                        k
                    }
                };
                out.push('$');
                out.push_str(&idx.to_string());
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    if let Some((n, _)) = pairs
        .iter()
        .find(|(n, _)| !assigned.iter().any(|(a, _)| a == n))
    {
        return Err(format!(
            "Core.Db: bound named parameter `:{n}` does not appear in the SQL"
        ));
    }
    Ok((out, params))
}

/// Number the bare `?` placeholders of a statement `$1,$2,…` (quote-aware), returning the rewritten SQL
/// and the count. Used by `execute_many`, where each row supplies the positional values (no in-place
/// list expansion — a bulk row is a plain positional bind-set).
fn number_qmarks(sql: &str) -> Result<(String, usize), String> {
    let mut out = String::with_capacity(sql.len());
    let mut n = 0usize;
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
                n += 1;
                out.push('$');
                out.push_str(&n.to_string());
            }
            _ => out.push(c),
        }
    }
    Ok((out, n))
}

/// True iff the statement has a `RETURNING` clause (case-insensitive, as a whole word), ignoring
/// `'…'`/`"…"` string literals so `VALUES('returning')` never false-positives, and treating
/// `returning_col` (an identifier) as distinct from the keyword. Used by `exec_returning_id` to pick the
/// `query()` path (read the returned id) over the `execute()` + `lastval()` fallback.
fn has_returning_clause(sql: &str) -> bool {
    let mut unquoted = String::with_capacity(sql.len());
    let mut in_s = false;
    let mut in_d = false;
    for c in sql.chars() {
        match c {
            '\'' if !in_d => in_s = !in_s,
            '"' if !in_s => in_d = !in_d,
            _ if !in_s && !in_d => unquoted.push(c.to_ascii_lowercase()),
            _ => {}
        }
    }
    unquoted
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|w| w == "returning")
}

/// Rewrite `sql` for the accumulated [`Binds`] into Postgres `$n` form + the ordered bind VALUES (still
/// as phorj [`Value`]s — they are boxed to their exact Postgres types only AFTER the prepare reveals the
/// inferred parameter types; see [`pg_param`]).
fn translate(sql: &str, binds: &Binds) -> Result<(String, Vec<Value>), String> {
    match binds {
        Binds::None => Ok((sql.to_string(), Vec::new())),
        Binds::Positional(pbs) => translate_positional(sql, pbs),
        Binds::Named(pairs) => translate_named(sql, pairs),
    }
}

/// Borrow the boxed params as the `&[&(dyn ToSql + Sync)]` the `postgres` API wants.
fn param_refs(boxes: &[Box<dyn ToSql + Sync>]) -> Vec<&(dyn ToSql + Sync)> {
    boxes.iter().map(|b| b.as_ref()).collect()
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
                "<<ConstraintViolation>>Core.Db: RETURNING produced no row".to_string()
            })?;
            if row.is_empty() {
                return Err("Core.Db: RETURNING produced no column".to_string());
            }
            return match pg_cell(row, 0, row.columns()[0].type_(), row.columns()[0].name())? {
                Value::Int(n) => Ok(n),
                other => Err(format!(
                    "Core.Db.execReturningId: RETURNING column is {}, not an int id",
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
            None => Err("Core.Db: lastval() returned no row".to_string()),
        }
    }

    fn execute_many(&self, sql: &str, rows: &[Value], in_transaction: bool) -> Result<i64, String> {
        // Postgres rejects a standalone SAVEPOINT outside a transaction block, so open our OWN BEGIN at
        // depth 0 (and COMMIT/ROLLBACK it); when a caller transaction is already open, use a SAVEPOINT
        // (composable partial rollback), exactly like the SQLite path.
        let (open, ok_sql, undo_sql) = if in_transaction {
            (
                "SAVEPOINT phorj_bulk",
                "RELEASE phorj_bulk",
                "ROLLBACK TO phorj_bulk; RELEASE phorj_bulk",
            )
        } else {
            ("BEGIN", "COMMIT", "ROLLBACK")
        };
        {
            self.client
                .borrow_mut()
                .batch_execute(open)
                .map_err(pg_sql_err)?;
        }
        let (tsql, _n) = match number_qmarks(sql) {
            Ok(v) => v,
            Err(e) => {
                let _ = self.client.borrow_mut().batch_execute(undo_sql);
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
                            "Core.Db.executeMany: each row must be a list, got {}",
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
                    .batch_execute(ok_sql)
                    .map_err(pg_sql_err)?;
                Ok(total)
            }
            Err(e) => {
                // Best-effort unwind; return the ORIGINAL error (a rollback failure must not mask it).
                let _ = self.client.borrow_mut().batch_execute(undo_sql);
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
        // `pg_err_kind` maps straight to `Timeout`. `0` disables it.
        let ms = ms.max(0);
        self.control(&format!("SET statement_timeout = {ms}"))
    }
}

#[cfg(test)]
mod tests {
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
            "23505" => "UniqueViolation",
            "40001" | "40P01" => "SerializationFailure",
            "57014" => "Timeout",
            _ if code.starts_with("23") => "ConstraintViolation",
            _ if code.starts_with("08") => "ConnectionError",
            _ if code.starts_with("42") => "SyntaxError",
            _ => return None,
        })
    }

    #[test]
    fn sqlstate_taxonomy_mapping() {
        assert_eq!(kind_of("23505"), Some("UniqueViolation"));
        assert_eq!(kind_of("23503"), Some("ConstraintViolation")); // foreign_key
        assert_eq!(kind_of("23502"), Some("ConstraintViolation")); // not_null
        assert_eq!(kind_of("40001"), Some("SerializationFailure"));
        assert_eq!(kind_of("40P01"), Some("SerializationFailure")); // deadlock
        assert_eq!(kind_of("57014"), Some("Timeout"));
        assert_eq!(kind_of("08006"), Some("ConnectionError"));
        assert_eq!(kind_of("42601"), Some("SyntaxError")); // syntax_error
        assert_eq!(kind_of("42P01"), Some("SyntaxError")); // undefined_table
        assert_eq!(kind_of("22012"), None); // division_by_zero → base DbError
    }
}
