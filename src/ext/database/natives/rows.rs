//! The Row cell accessors for `Core.DatabaseModule` (DEC-208): the strict scalar getters
//! (`getInt`/`getString`/`getFloat`/`getBool` + their `OrNull` variants), the exact-money
//! `getDecimal`, the typed array-column getters (`getIntList` … `getBoolListOrNull`), and the column
//! introspection primitives (`columnNames`/`isNull`). Each returns `Ok(payload)` on success and
//! `Err(db-error-message)` on a DB error; the public natives ([`super::wrappers`]) `wrap` them.

use crate::value::{HKey, Value};
use std::rc::Rc;

/// Look up a column in a `Row` (a `Map`), or a DB error if the column is absent.
fn row_cell<'a>(args: &'a [Value], who: &str) -> Result<(&'a Value, &'a str), String> {
    match args {
        [Value::Map(pairs), Value::Str(key)] => {
            let k = key.as_str();
            pairs
                .iter()
                .find(|(hk, _)| matches!(hk, HKey::Str(s) if s.as_str() == k))
                .map(|(_, v)| (v, k))
                .ok_or_else(|| format!("Core.DatabaseModule.{who}: no column `{k}` in this row"))
        }
        _ => Err(format!(
            "Core.DatabaseModule.{who} expects (Row, string column)"
        )),
    }
}

pub(super) fn get_int_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getInt")?;
    match v {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getInt: column `{k}` is NULL (use int?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getInt: column `{k}` is {}, not int",
            other.type_name()
        )),
    }
}

pub(super) fn get_string_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getString")?;
    match v {
        Value::Str(s) => Ok(Value::Str(s.clone())),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getString: column `{k}` is NULL (use string?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getString: column `{k}` is {}, not string",
            other.type_name()
        )),
    }
}

pub(super) fn get_float_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getFloat")?;
    match v {
        Value::Float(f) => Ok(Value::Float(*f)),
        // SQLite stores an integral REAL as INTEGER; widen int→float for a float column, matching PDO.
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getFloat: column `{k}` is NULL (use float?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getFloat: column `{k}` is {}, not float",
            other.type_name()
        )),
    }
}

pub(super) fn get_bool_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getBool")?;
    match v {
        // SQLite has no bool: it round-trips as 0/1 integer (matching the `to_sql` bind side).
        Value::Int(0) => Ok(Value::Bool(false)),
        Value::Int(_) => Ok(Value::Bool(true)),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getBool: column `{k}` is NULL (use bool?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getBool: column `{k}` is {}, not bool",
            other.type_name()
        )),
    }
}

// --- Nullable Row accessors (DEC-208 S2): a `T?`-typed hydration field admits a SQL NULL, so these
// return `null` for a NULL column instead of faulting. A wrong non-null storage type is still a DB
// error, and a missing column is still a DB error (`row_cell`). Shared by the dynamic path and the
// generic hydration desugar. ---

pub(super) fn get_int_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getIntOrNull")?;
    match v {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getIntOrNull: column `{k}` is {}, not int",
            other.type_name()
        )),
    }
}

pub(super) fn get_string_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getStringOrNull")?;
    match v {
        Value::Str(s) => Ok(Value::Str(s.clone())),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getStringOrNull: column `{k}` is {}, not string",
            other.type_name()
        )),
    }
}

pub(super) fn get_float_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getFloatOrNull")?;
    match v {
        Value::Float(f) => Ok(Value::Float(*f)),
        // SQLite stores an integral REAL as INTEGER; widen int→float, matching the non-nullable accessor.
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getFloatOrNull: column `{k}` is {}, not float",
            other.type_name()
        )),
    }
}

pub(super) fn get_bool_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getBoolOrNull")?;
    match v {
        // SQLite has no bool: it round-trips as 0/1 integer (matching the `to_sql` bind side).
        Value::Int(0) => Ok(Value::Bool(false)),
        Value::Int(_) => Ok(Value::Bool(true)),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getBoolOrNull: column `{k}` is {}, not bool",
            other.type_name()
        )),
    }
}

// --- Decimal accessor (DEC-208 slice E): a `decimal`-typed hydration field maps its column here.
// Exact money, never float: a TEXT column is parsed EXACTLY via the shared `…d`-literal grammar
// (`value::decimal_of`) — the money path; an INTEGER is exact at scale 0. A REAL is converted through
// its shortest round-trip decimal string (a REAL column cannot store money exactly — store decimal
// columns as TEXT for guaranteed exactness; this is a best-effort convenience). A missing column /
// wrong storage type / NULL-into-non-optional is a strict DB error (no silent coercion); the nullable
// accessor admits NULL. Shared by the dynamic path and the generic hydration desugar. ---

/// Convert a fetched cell to a phorj `decimal` (DEC-208 slice E). See the section note for the
/// TEXT/INTEGER/REAL conventions. `null_ok` selects the `decimal?` (admit NULL) vs `decimal` (strict)
/// behaviour; `who` names the accessor for the error message.
fn decimal_from_cell(v: &Value, k: &str, who: &str, null_ok: bool) -> Result<Value, String> {
    match v {
        // Already a decimal (defensive — SQLite storage classes never produce this, but a row is a
        // general `Map`, so the accessor stays total).
        Value::Decimal { .. } => Ok(v.clone()),
        Value::Int(n) => Ok(Value::Decimal {
            unscaled: i128::from(*n),
            scale: 0,
        }),
        // The money path: parse the exact decimal grammar from the stored text (no float round-trip).
        Value::Str(s) => match crate::value::decimal_of(s) {
            Some((unscaled, scale)) => Ok(Value::Decimal { unscaled, scale }),
            None => Err(format!(
                "Core.DatabaseModule.{who}: column `{k}` value `{s}` is not a valid decimal"
            )),
        },
        // Best-effort REAL → shortest round-trip decimal string → exact decimal of THAT string.
        Value::Float(f) => match crate::value::decimal_of(&format!("{f}")) {
            Some((unscaled, scale)) => Ok(Value::Decimal { unscaled, scale }),
            None => Err(format!(
                "Core.DatabaseModule.{who}: column `{k}` REAL value cannot be represented as a decimal"
            )),
        },
        Value::Null if null_ok => Ok(Value::Null),
        Value::Null => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is NULL (use decimal?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is {}, not decimal",
            other.type_name()
        )),
    }
}

pub(super) fn get_decimal_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getDecimal")?;
    decimal_from_cell(v, k, "getDecimal", false)
}

pub(super) fn get_decimal_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getDecimalOrNull")?;
    decimal_from_cell(v, k, "getDecimalOrNull", true)
}

// --- Typed ARRAY-column accessors (DEC-208 slice K): a Postgres `int[]`/`text[]`/`float8[]`/`bool[]`
// cell arrives as a `Value::List` (see `postgres::pg_cell`); these read it as a typed `List<scalar>`.
// STRICT like the scalar accessors: a non-array column, a wrong element type, or a NULL ELEMENT is a
// clean catchable DatabaseError (Postgres arrays are nullable per element; phorj `List<int>` elements are
// not — the error steers to filtering NULLs in SQL, e.g. `array_remove(col, NULL)`). The `OrNull`
// variants admit a whole-array SQL NULL (→ `null`), never NULL elements. SQLite/MySQL never produce
// a list cell, so on those drivers the error reads "not an array" — the honest cross-driver story
// (arrays are a Postgres capability; the SAME class hydrates everywhere else via scalar columns).

/// Validate every element of an array cell with `check`, or explain which element broke.
pub(super) fn list_from_cell(
    v: &Value,
    k: &str,
    who: &str,
    elem: &str,
    or_null: bool,
    check: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    match v {
        Value::List(items) => {
            for (i, it) in items.iter().enumerate() {
                if matches!(it, Value::Null) {
                    return Err(format!(
                        "Core.DatabaseModule.{who}: column `{k}` has a NULL element at [{i}] — filter them in \
                         SQL (e.g. array_remove({k}, NULL)) or select a non-null projection"
                    ));
                }
                if !check(it) {
                    return Err(format!(
                        "Core.DatabaseModule.{who}: column `{k}` element [{i}] is {}, not {elem}",
                        it.type_name()
                    ));
                }
            }
            Ok(v.clone())
        }
        Value::Null if or_null => Ok(Value::Null),
        Value::Null => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is NULL (use List<{elem}>? / the OrNull accessor)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is {}, not an array",
            other.type_name()
        )),
    }
}

macro_rules! list_accessor_inner {
    ($fn_name:ident, $who:literal, $elem:literal, $or_null:literal, $pat:pat) => {
        pub(super) fn $fn_name(args: &[Value]) -> Result<Value, String> {
            let (v, k) = row_cell(args, $who)?;
            list_from_cell(v, k, $who, $elem, $or_null, |it| matches!(it, $pat))
        }
    };
}
list_accessor_inner!(
    get_int_list_inner,
    "getIntList",
    "int",
    false,
    Value::Int(_)
);
list_accessor_inner!(
    get_string_list_inner,
    "getStringList",
    "string",
    false,
    Value::Str(_)
);
list_accessor_inner!(
    get_float_list_inner,
    "getFloatList",
    "float",
    false,
    Value::Float(_)
);
list_accessor_inner!(
    get_bool_list_inner,
    "getBoolList",
    "bool",
    false,
    Value::Bool(_)
);
list_accessor_inner!(
    get_int_list_or_null_inner,
    "getIntListOrNull",
    "int",
    true,
    Value::Int(_)
);
list_accessor_inner!(
    get_string_list_or_null_inner,
    "getStringListOrNull",
    "string",
    true,
    Value::Str(_)
);
list_accessor_inner!(
    get_float_list_or_null_inner,
    "getFloatListOrNull",
    "float",
    true,
    Value::Float(_)
);
list_accessor_inner!(
    get_bool_list_or_null_inner,
    "getBoolListOrNull",
    "bool",
    true,
    Value::Bool(_)
);

// --- Column introspection (DEC-208 slice B): two capabilities the desugared `queryScalar` /
// `queryMap` / nested-hydration helpers need, routed through the SAME `DatabaseResult`/`wrap` protocol as
// the accessors (NOT a duplication of `getX` — genuinely new operations). `columnNames` gives the
// ORDERED column names of a row (the row is an insertion-ordered `Map`, so selection order is
// preserved) — `queryScalar` reads the sole column whose name is unpredictable (`COUNT(*)`), and
// `queryMap` keys on the first / reads the second. `isNull` reports whether a column is SQL NULL
// (type-agnostic) — the nested-optional-entity hydration tests "all this entity's columns are NULL"
// (a LEFT JOIN miss → the whole entity is `null`); it cannot use `== null` (phorj rejects a
// cross-type `T? == null` comparison), so this boolean primitive is required. ---

/// `row.columnNames()` → the ordered `List<string>` of this row's column names (selection order).
pub(super) fn column_names_inner(args: &[Value]) -> Result<Value, String> {
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
        _ => Err("Core.DatabaseModule.columnNames expects (Row)".into()),
    }
}

/// `row.isNull(column)` → `true` iff the column is SQL NULL; a DB error if the column is absent
/// (reusing `row_cell`, so a missing nested column is a strict error exactly like the accessors).
pub(super) fn is_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, _k) = row_cell(args, "isNull")?;
    Ok(Value::Bool(matches!(v, Value::Null)))
}
