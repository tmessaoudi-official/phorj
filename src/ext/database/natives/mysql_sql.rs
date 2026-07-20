//! The MySQL/MariaDB SQL-translation and value-mapping helpers (DEC-208 slice J), split out of
//! [`super::mysql`] for the file-size cap (Invariant 13): `?`/`:name` placeholder handling
//! ([`expand_positional`]/[`translate_named`]/[`translate`]) and the phorj [`Value`] ↔ [`mysql::Value`]
//! mapping ([`my_param`]/[`my_cell`]/[`my_row_to_map`], with the blob-family [`is_binary_blob`] guard).

use super::handles::{Binds, PosBind};
use crate::value::{HKey, Value};
use mysql::consts::{ColumnFlags, ColumnType};
use mysql::Row;
use std::rc::Rc;

/// Phorj value → a MySQL bind parameter. No prepared-type narrowing is needed (the server coerces
/// binary-protocol params); a `bool` binds as `0`/`1` (MySQL `BOOL` IS `TINYINT(1)`). A non-scalar is
/// a clean error, never a silent coercion — matching the sibling drivers' bindable set.
pub(super) fn my_param(v: &Value) -> Result<mysql::Value, String> {
    Ok(match v {
        Value::Int(n) => mysql::Value::Int(*n),
        Value::Float(f) => mysql::Value::Double(*f),
        Value::Str(s) => mysql::Value::Bytes(s.as_bytes().to_vec()),
        Value::Bool(b) => mysql::Value::Int(i64::from(*b)),
        Value::Bytes(b) => mysql::Value::Bytes((**b).clone()),
        Value::Null => mysql::Value::NULL,
        other => {
            return Err(format!(
                "Core.DatabaseModule: cannot bind a {} value (bind a decimal as text)",
                other.type_name()
            ))
        }
    })
}

/// Expand the accumulated positional binds against the SQL's `?` placeholders: a
/// [`One`](PosBind::One) keeps its `?`; a [`List`](PosBind::List) expands its single `?` to
/// `?,?,…` (`NULL` when empty) — the typed `IN`-list bind (slice D). Quote-aware; a `?`/bind count
/// mismatch is a clean DB error, mirroring the SQLite driver's `expand_placeholders`.
pub(super) fn expand_positional(
    sql: &str,
    pbs: &[PosBind],
) -> Result<(String, Vec<mysql::Value>), String> {
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
                let b = pbs.get(consumed).ok_or_else(|| {
                    "Core.DatabaseModule: more ? placeholders than bound values".to_string()
                })?;
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
            "Core.DatabaseModule: {} bound value(s) but {} ? placeholder(s) in the SQL",
            pbs.len(),
            consumed
        ));
    }
    Ok((out, params))
}

/// Rewrite `:name` named binds to positional `?` in first-appearance order (MySQL has no native named
/// parameters). Quote-aware; a repeated `:name` re-pushes its value (each `?` needs its own slot). A
/// `:name` with no matching bind, or a bound name never referenced, is a clean DB error.
pub(super) fn translate_named(
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
                .ok_or_else(|| {
                    format!("Core.DatabaseModule: named parameter `:{name}` was not bound")
                })?;
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
            "Core.DatabaseModule: bound named parameter `:{n}` does not appear in the SQL"
        ));
    }
    Ok((out, params))
}

/// Rewrite `sql` + the accumulated [`Binds`] into `?`-form SQL and the ordered MySQL params.
pub(super) fn translate(sql: &str, binds: &Binds) -> Result<(String, Vec<mysql::Value>), String> {
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
/// temporal values return a clear DatabaseError steering to `CAST(col AS CHAR)` (the Postgres `::text`
/// guidance, adapted).
pub(super) fn my_cell(
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
                "Core.DatabaseModule: column `{name}` holds unsigned value {u}, out of int range"
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
                            "Core.DatabaseModule: column `{name}` holds non-UTF-8 text — read it as BLOB"
                        ))
                    }
                }
            }
        }
        mysql::Value::Date(..) | mysql::Value::Time(..) => {
            return Err(format!(
            "Core.DatabaseModule: column `{name}` has a temporal mysql type — select it as text \
                 (e.g. `SELECT CAST({name} AS CHAR)`) to read date/time values"
        ))
        }
    })
}

/// A `mysql::Row` → a `Value::Map` (column-name → value), selection-ordered — the same `Row` shape
/// the sibling drivers produce, so the generic row accessors stay backend-agnostic.
pub(super) fn my_row_to_map(row: Row) -> Result<Value, String> {
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
