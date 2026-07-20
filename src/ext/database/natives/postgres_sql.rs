//! The Postgres SQL-translation and value-mapping helpers (DEC-208 slice I), split out of
//! [`super::postgres`] for the file-size cap (Invariant 13): phorj `?`/`:name` placeholders →
//! Postgres `$n` ([`translate_positional`]/[`translate_named`]/[`number_qmarks`]/[`translate`]),
//! `RETURNING`-clause detection ([`has_returning_clause`]), and the phorj [`Value`] ↔ Postgres binary
//! type mapping ([`pg_param`]/[`pg_params`]/[`pg_cell`]/[`pg_row_to_map`]/[`param_refs`]).

use super::handles::{Binds, PosBind};
use super::postgres::pg_sql_err;
use crate::value::{HKey, Value};
use postgres::types::{ToSql, Type};
use postgres::Row;
use std::rc::Rc;

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
    let overflow =
        |n: i64, t: &str| format!("Core.DatabaseModule: int {n} does not fit the {t} column");
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
                "Core.DatabaseModule: cannot bind a {} value (bind a decimal as text with a ::numeric cast)",
                other.type_name()
            ))
        }
    })
}

/// Build the owned bind parameters for `values`, typed against the prepared statement's inferred
/// parameter types (`expected`). A count mismatch is a clean DB error (never a silent misbind).
pub(super) fn pg_params(
    values: &[Value],
    expected: &[Type],
) -> Result<Vec<Box<dyn ToSql + Sync>>, String> {
    if values.len() != expected.len() {
        return Err(format!(
            "Core.DatabaseModule: {} bound value(s) for {} placeholder(s) in the SQL",
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
/// arrays, …) returns a clear DatabaseError steering to a `::text` cast — matching slice E's "store decimal
/// columns as TEXT" guidance (a `SELECT amount::text` column reads as text, and `Row.getDecimal` parses
/// it exactly).
pub(super) fn pg_cell(row: &Row, i: usize, ty: &Type, name: &str) -> Result<Value, String> {
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
                "Core.DatabaseModule: column `{name}` has unsupported postgres type `{ty}` — select it with a \
                 `::text` cast (e.g. `SELECT {name}::text`) to read numeric/json/timestamp values"
            ))
        }
    })
}

/// A `postgres::Row` → a `Value::Map` (column-name → value), selection-ordered — the same `Row` shape
/// the SQLite driver produces, so the generic row accessors are backend-agnostic.
pub(super) fn pg_row_to_map(row: &Row) -> Result<Value, String> {
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
pub(super) fn translate_positional(
    sql: &str,
    pbs: &[PosBind],
) -> Result<(String, Vec<Value>), String> {
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
                let b = pbs.get(consumed).ok_or_else(|| {
                    "Core.DatabaseModule: more ? placeholders than bound values".to_string()
                })?;
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
            "Core.DatabaseModule: {} bound value(s) but {} ? placeholder(s) in the SQL",
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
pub(super) fn translate_named(
    sql: &str,
    pairs: &[(String, Value)],
) -> Result<(String, Vec<Value>), String> {
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
                                format!(
                                    "Core.DatabaseModule: named parameter `:{name}` was not bound"
                                )
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
            "Core.DatabaseModule: bound named parameter `:{n}` does not appear in the SQL"
        ));
    }
    Ok((out, params))
}

/// Number the bare `?` placeholders of a statement `$1,$2,…` (quote-aware), returning the rewritten SQL
/// and the count. Used by `execute_many`, where each row supplies the positional values (no in-place
/// list expansion — a bulk row is a plain positional bind-set).
pub(super) fn number_qmarks(sql: &str) -> Result<(String, usize), String> {
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
pub(super) fn has_returning_clause(sql: &str) -> bool {
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
pub(super) fn translate(sql: &str, binds: &Binds) -> Result<(String, Vec<Value>), String> {
    match binds {
        Binds::None => Ok((sql.to_string(), Vec::new())),
        Binds::Positional(pbs) => translate_positional(sql, pbs),
        Binds::Named(pairs) => translate_named(sql, pairs),
    }
}

/// Borrow the boxed params as the `&[&(dyn ToSql + Sync)]` the `postgres` API wants.
pub(super) fn param_refs(boxes: &[Box<dyn ToSql + Sync>]) -> Vec<&(dyn ToSql + Sync)> {
    boxes.iter().map(|b| b.as_ref()).collect()
}
