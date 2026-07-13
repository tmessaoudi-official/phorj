//! `Core.Db` — the enhanced-PDO database primitive (DEC-208), backed by `rusqlite` (bundled SQLite).
//!
//! Feature-gated (`db`) and native-only. This module owns the runtime layer: the opaque connection /
//! statement handles ([`DbConn`] / [`DbStmt`], carried by [`Value::Db`] via the [`DbObject`] trait)
//! and the native bodies for `open` / `prepare` / `bind` / `bindNamed` / `query` / `exec` and the
//! `Row` accessors. The language *surface* that dispatches `db.prepare(sql).bind(v).query()` onto these
//! natives (built-in-class recognition, gated on `import Core.Db`) lands in the checker/compiler/
//! interpreter in the next slice.
//!
//! **Spine treatment.** Every native here is `pure: false`, so `uses_impure_native` (tests/
//! differential.rs) auto-excludes any `import Core.Db` program from the byte-identity differential —
//! live DB I/O cannot be byte-identical across rusqlite and PHP PDO. Correctness is validated by the
//! in-module unit tests below and the `tests/db.rs` fixture. `run ≡ runvm` still holds unconditionally:
//! both backends call these one shared `eval` bodies. The `php` emitters (faithful PDO, DEC-208 LADDER
//! case 1) are finalized in the DEC-208 transpile slice; they are not byte-identity-gated (quarantined).

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{DbObject, HKey, Value};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

/// A live SQLite connection handle (`Value::Db` payload). Shared-mutable: cloning the `Value::Db`
/// shares this `Rc`, so all bindings name the same connection.
#[derive(Debug)]
struct DbConn {
    conn: Rc<RefCell<rusqlite::Connection>>,
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
/// per statement (the surface's contract) — mixing is a runtime error.
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
    conn: Rc<RefCell<rusqlite::Connection>>,
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

/// Downcast a `Value::Db` handle to a concrete resource, or a clean fault (checker-unreachable once
/// the surface enforces the receiver types, but the natives stay total).
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
/// else (list/map/instance/…) is a clean fault, never a silent coercion (DEC-208 "no silent coercion").
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

fn sql_err(e: rusqlite::Error) -> String {
    format!("Core.Db: {e}")
}

/// `new Db(dsn)` → open a connection. `dsn` is `"sqlite:PATH"` or `"sqlite::memory:"` (the PDO DSN
/// shape); a bare path is also accepted. The parsed native is `pure: false` (opens a real resource).
fn db_open(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let dsn = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.Db.open expects (string dsn)".into()),
    };
    let conn = if dsn == "sqlite::memory:" || dsn == ":memory:" {
        rusqlite::Connection::open_in_memory()
    } else {
        let path = dsn.strip_prefix("sqlite:").unwrap_or(dsn);
        rusqlite::Connection::open(path)
    }
    .map_err(sql_err)?;
    Ok(Value::Db(Rc::new(DbConn {
        conn: Rc::new(RefCell::new(conn)),
    })))
}

/// `db.prepare(sql)` → a lazily-executed statement handle carrying the connection + SQL.
fn db_prepare(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (conn, sql) = match args {
        [c, Value::Str(s)] => (as_conn(c)?, s.as_str().to_string()),
        _ => return Err("Core.Db.prepare expects (Db, string sql)".into()),
    };
    Ok(Value::Db(Rc::new(DbStmt {
        conn: Rc::clone(&conn.conn),
        sql,
        binds: RefCell::new(Binds::None),
    })))
}

/// `stmt.bind(value)` → append a positional bind; returns the same shared handle (chainable).
fn db_bind(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (stmt, val) = match args {
        [s, v] => (as_stmt(s)?, v),
        _ => return Err("Core.Db.bind expects (Statement, value)".into()),
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
fn db_bind_named(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (stmt, name, val) = match args {
        [s, Value::Str(n), v] => (as_stmt(s)?, n.as_str().to_string(), v),
        _ => return Err("Core.Db.bindNamed expects (Statement, string name, value)".into()),
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
fn db_query(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.Db.query expects (Statement)".into()),
    };
    let conn = stmt.conn.borrow();
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
fn db_exec(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.Db.exec expects (Statement)".into()),
    };
    let conn = stmt.conn.borrow();
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

/// Look up a column in a `Row` (a `Map`), or a clean fault if the column is absent.
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

fn row_get_int(args: &[Value], _out: &mut String) -> Result<Value, String> {
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

fn row_get_string(args: &[Value], _out: &mut String) -> Result<Value, String> {
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

fn row_get_float(args: &[Value], _out: &mut String) -> Result<Value, String> {
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

fn row_get_bool(args: &[Value], _out: &mut String) -> Result<Value, String> {
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

/// The `Core.Db` registry entries. Every native is `pure: false` (opens/uses a real DB resource) so
/// any `import Core.Db` program is auto-quarantined from the byte-identity differential. The `php`
/// emitters map to PDO (DEC-208 LADDER case 1); they are finalized + fixture-checked in the transpile
/// slice and are not byte-identity-gated here.
pub fn db_natives() -> Vec<NativeFn> {
    let db = || Ty::Named("Db".into(), vec![]);
    let stmt = || Ty::Named("Statement".into(), vec![]);
    let row = || Ty::Named("Row".into(), vec![]);
    vec![
        NativeFn {
            module: "Core.Db",
            name: "open",
            params: vec![Ty::String],
            ret: db(),
            pure: false,
            eval: NativeEval::Pure(db_open),
            php: |a| format!("new \\PDO({})", a.first().map_or("''", |s| s)),
        },
        NativeFn {
            module: "Core.Db",
            name: "prepare",
            params: vec![db(), Ty::String],
            ret: stmt(),
            pure: false,
            eval: NativeEval::Pure(db_prepare),
            php: |a| format!("{}->prepare({})", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Db",
            name: "bind",
            params: vec![stmt(), Ty::Empty],
            ret: stmt(),
            pure: false,
            eval: NativeEval::Pure(db_bind),
            // Positional binds are collected and passed to execute() in the transpile slice; the
            // receiver PHP is threaded through for now (finalized there).
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Db",
            name: "bindNamed",
            params: vec![stmt(), Ty::String, Ty::Empty],
            ret: stmt(),
            pure: false,
            eval: NativeEval::Pure(db_bind_named),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Db",
            name: "query",
            params: vec![stmt()],
            ret: Ty::List(Box::new(row())),
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
            module: "Core.Db",
            name: "exec",
            params: vec![stmt()],
            ret: Ty::Int,
            pure: false,
            eval: NativeEval::Pure(db_exec),
            php: |a| format!("{}->execute()", a[0]),
        },
        NativeFn {
            module: "Core.Db",
            name: "getInt",
            params: vec![row(), Ty::String],
            ret: Ty::Int,
            pure: false,
            eval: NativeEval::Pure(row_get_int),
            php: |a| format!("(int) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Db",
            name: "getString",
            params: vec![row(), Ty::String],
            ret: Ty::String,
            pure: false,
            eval: NativeEval::Pure(row_get_string),
            php: |a| format!("(string) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Db",
            name: "getFloat",
            params: vec![row(), Ty::String],
            ret: Ty::Float,
            pure: false,
            eval: NativeEval::Pure(row_get_float),
            php: |a| format!("(float) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Db",
            name: "getBool",
            params: vec![row(), Ty::String],
            ret: Ty::Bool,
            pure: false,
            eval: NativeEval::Pure(row_get_bool),
            php: |a| format!("(bool) {}[{}]", a[0], a[1]),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end runtime round-trip (the slice-1 fixture, in-process): open in-memory → DDL → insert
    /// (positional + named binds) → query back → Row accessors. Proves the rusqlite integration
    /// through the `Value` model, independent of the language surface (which lands next slice).
    #[test]
    fn db_runtime_round_trip() {
        let mut out = String::new();
        let db = db_open(&[Value::Str("sqlite::memory:".into())], &mut out).unwrap();

        // CREATE TABLE (no binds) via exec.
        let stmt = db_prepare(
            &[
                db.clone(),
                Value::Str("CREATE TABLE users(name TEXT, age INTEGER)".into()),
            ],
            &mut out,
        )
        .unwrap();
        assert!(db_exec(&[stmt], &mut out).unwrap().eq_val(&Value::Int(0)));

        // INSERT with positional binds.
        let ins = db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO users(name, age) VALUES(?, ?)".into()),
            ],
            &mut out,
        )
        .unwrap();
        let ins = db_bind(&[ins, Value::Str("Ada".into())], &mut out).unwrap();
        let ins = db_bind(&[ins, Value::Int(36)], &mut out).unwrap();
        assert!(db_exec(&[ins], &mut out).unwrap().eq_val(&Value::Int(1)));

        // INSERT with named binds.
        let ins2 = db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO users(name, age) VALUES(:n, :a)".into()),
            ],
            &mut out,
        )
        .unwrap();
        let ins2 = db_bind_named(
            &[ins2, Value::Str("n".into()), Value::Str("Grace".into())],
            &mut out,
        )
        .unwrap();
        let ins2 =
            db_bind_named(&[ins2, Value::Str("a".into()), Value::Int(45)], &mut out).unwrap();
        assert!(db_exec(&[ins2], &mut out).unwrap().eq_val(&Value::Int(1)));

        // Query back, ordered, and read via Row accessors.
        let sel = db_prepare(
            &[
                db.clone(),
                Value::Str("SELECT name, age FROM users WHERE age > ? ORDER BY age".into()),
            ],
            &mut out,
        )
        .unwrap();
        let sel = db_bind(&[sel, Value::Int(30)], &mut out).unwrap();
        let rows = db_query(&[sel], &mut out).unwrap();
        let Value::List(rows) = rows else {
            panic!("query must return a list")
        };
        assert_eq!(rows.len(), 2);

        // Row 0 = Ada / 36.
        assert!(
            row_get_string(&[rows[0].clone(), Value::Str("name".into())], &mut out)
                .unwrap()
                .eq_val(&Value::Str("Ada".into()))
        );
        assert!(
            row_get_int(&[rows[0].clone(), Value::Str("age".into())], &mut out)
                .unwrap()
                .eq_val(&Value::Int(36))
        );
        // Row 1 = Grace / 45.
        assert!(
            row_get_string(&[rows[1].clone(), Value::Str("name".into())], &mut out)
                .unwrap()
                .eq_val(&Value::Str("Grace".into()))
        );
    }

    #[test]
    fn mixing_bind_styles_is_an_error() {
        let mut out = String::new();
        let db = db_open(&[Value::Str(":memory:".into())], &mut out).unwrap();
        let s = db_prepare(&[db, Value::Str("SELECT ?, :x".into())], &mut out).unwrap();
        let s = db_bind(&[s, Value::Int(1)], &mut out).unwrap();
        let err = db_bind_named(&[s, Value::Str("x".into()), Value::Int(2)], &mut out).unwrap_err();
        assert!(err.contains("cannot mix"), "got: {err}");
    }

    #[test]
    fn get_int_on_null_is_a_typed_error() {
        let mut out = String::new();
        let db = db_open(&[Value::Str(":memory:".into())], &mut out).unwrap();
        let s = db_prepare(&[db, Value::Str("SELECT NULL AS x".into())], &mut out).unwrap();
        let rows = db_query(&[s], &mut out).unwrap();
        let Value::List(rows) = rows else { panic!() };
        let err = row_get_int(&[rows[0].clone(), Value::Str("x".into())], &mut out).unwrap_err();
        assert!(err.contains("NULL"), "got: {err}");
    }
}
