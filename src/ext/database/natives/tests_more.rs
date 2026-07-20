//! In-module unit tests for `Core.DatabaseModule` (part 2): the writes + robustness slice
//! (`executeMany`, insert-id helpers, timeout remap/end-to-end, `onQuery` storage), the multi-driver
//! DSN dispatch + credential injection, and the typed array-accessor validation. Shares the helpers
//! defined in [`super::tests`].

use super::driver::inject_pg_password;
use super::handles::as_conn;
use super::ops::{
    exec_returning_id_inner, execute_many_inner, last_insert_id_inner, remap_timeout,
};
use super::rows::list_from_cell;
use super::tests::{err_of, exec1, ok_of, scalar, x};
use super::wrappers::*;
use crate::value::Value;
use std::rc::Rc;

// ── DEC-208 slice D: writes + robustness ──────────────────────────────────────────────────

/// `execute_many` inserts every row atomically and returns the total; a mid-batch failure rolls the
/// WHOLE batch back (savepoint), leaving nothing behind.
#[test]
fn execute_many_atomic() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    exec1(
        &db,
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
        &mut out,
    );
    let s = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO t(id, v) VALUES(?, ?)".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    let rows = Value::List(Rc::new(vec![
        Value::List(Rc::new(vec![Value::Int(1), Value::Str("a".into())])),
        Value::List(Rc::new(vec![Value::Int(2), Value::Str("b".into())])),
    ]));
    // The `_inner` bodies return the RAW `Result` (the public HO natives `wrap` it); assert directly.
    let n = execute_many_inner(&[s, rows]).unwrap();
    assert!(n.eq_val(&Value::Int(2)));
    assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 2);

    // A duplicate PK mid-batch → the whole savepoint rolls back (still 2 rows, none of the batch).
    let s2 = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO t(id, v) VALUES(?, ?)".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    let bad = Value::List(Rc::new(vec![
        Value::List(Rc::new(vec![Value::Int(3), Value::Str("c".into())])),
        Value::List(Rc::new(vec![Value::Int(1), Value::Str("dup".into())])),
    ]));
    let msg = execute_many_inner(&[s2, bad]).unwrap_err();
    assert!(msg.starts_with("<<UniqueViolationError>>"), "got: {msg}");
    assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 2);
}

/// `execReturningId` / `lastInsertId` report the auto-generated rowid.
#[test]
fn insert_id_helpers() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    exec1(
        &db,
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
        &mut out,
    );
    let ins = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO t(v) VALUES('a')".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    let id = exec_returning_id_inner(&[ins]).unwrap();
    assert!(id.eq_val(&Value::Int(1)));
    exec1(&db, "INSERT INTO t(v) VALUES('b')", &mut out);
    let last = last_insert_id_inner(std::slice::from_ref(&db)).unwrap();
    assert!(last.eq_val(&Value::Int(2)));
}

/// `remap_timeout` reclassifies a transient `SerializationFailureError` as `TimeoutError` only when a timeout
/// is armed (mapping unit — deterministic, no lock races).
#[test]
fn remap_timeout_only_when_armed() {
    let armed = remap_timeout(
        Err("<<SerializationFailureError>>Core.DatabaseModule: database is locked".into()),
        true,
    );
    assert!(
        matches!(&armed, Err(m) if m.starts_with("<<TimeoutError>>")),
        "got: {armed:?}"
    );
    let unarmed = remap_timeout(Err("<<SerializationFailureError>>x".into()), false);
    assert!(matches!(&unarmed, Err(m) if m.starts_with("<<SerializationFailureError>>")));
    // A non-busy error is never touched, armed or not.
    let other = remap_timeout(Err("<<SyntaxError>>x".into()), true);
    assert!(matches!(&other, Err(m) if m.starts_with("<<SyntaxError>>")));
}

/// End-to-end: with `db.timeout(ms)` armed, a genuine lock contention (a second connection blocked
/// by a held write lock) surfaces as `TimeoutError` (not `SerializationFailureError`). Deterministic: the
/// first connection holds the lock for the whole test, so the second always exhausts its busy wait.
#[test]
fn armed_timeout_maps_busy_to_timeout_end_to_end() {
    let mut out = String::new();
    let path = std::env::temp_dir().join(format!(
        "phorj_db_to_{}_{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dsn = format!("sqlite:{}", path.display());
    let c1 = ok_of(db_open(&[Value::Str(dsn.as_str().into())], &mut out).unwrap());
    exec1(&c1, "CREATE TABLE t(x INTEGER)", &mut out);
    // c1 opens a transaction and takes a write lock, holding it for the rest of the test.
    ok_of(db_begin(std::slice::from_ref(&c1), &mut out).unwrap());
    exec1(&c1, "INSERT INTO t(x) VALUES(1)", &mut out);
    // c2 arms a short busy timeout, then its write waits for and fails to get the lock → TimeoutError.
    let c2 = ok_of(db_open(&[Value::Str(dsn.as_str().into())], &mut out).unwrap());
    ok_of(db_timeout(&[c2.clone(), Value::Int(30)], &mut out).unwrap());
    let s = ok_of(
        db_prepare(
            &[c2, Value::Str("INSERT INTO t(x) VALUES(2)".into())],
            &mut out,
        )
        .unwrap(),
    );
    let msg = err_of(x(&[s], &mut out).unwrap());
    let _ = std::fs::remove_file(&path);
    assert!(msg.starts_with("<<TimeoutError>>"), "got: {msg}");
}

/// A hook registered via `onQuery` is stored and returned by the shared cell; a Pure store never
/// fails. (Invocation with `(sql, ms)` is exercised end-to-end by `tests/database.rs`, which has a real
/// backend to run the closure.)
#[test]
fn on_query_stores_the_hook() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    // A non-closure stand-in is fine here: the native only stores the value; the checker enforces
    // the `(string, int) => void` shape at the call site.
    let sentinel = Value::Int(42);
    ok_of(db_on_query(&[db.clone(), sentinel], &mut out).unwrap());
    let conn = as_conn(&db).unwrap();
    assert!(conn.hook.borrow().is_some());
}

// ── DEC-208 slice I: multi-driver DSN dispatch + slice G credential injection ────────────────

/// A `sqlite:`/`:memory:` DSN dispatches to the SQLite driver (unchanged behaviour): the open
/// succeeds and a round-trip through the resulting connection works — the dispatch never misroutes a
/// sqlite DSN. (The postgres branch is proven by `postgres_dsn_without_feature_is_a_clean_error`.)
#[test]
fn sqlite_dsn_dispatches_to_sqlite() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str("sqlite::memory:".into())], &mut out).unwrap());
    assert!(as_conn(&db).is_ok());
    exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);
    assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 0);
}

/// A `postgres://` DSN dispatches to the postgres driver — which, when `database-postgres` is OFF, is a
/// clean feature-gated `ConnectionError`, NEVER a fall-through to the SQLite file path (which would
/// silently create a file literally named `postgres://…`).
#[cfg(not(feature = "database-postgres"))]
#[test]
fn postgres_dsn_without_feature_is_a_clean_error() {
    let mut out = String::new();
    let msg = err_of(db_open(&[Value::Str("postgres://u@h/db".into())], &mut out).unwrap());
    assert!(msg.starts_with("<<ConnectionError>>"), "got: {msg}");
    assert!(msg.contains("not compiled in"), "got: {msg}");
}

/// `dsnWithPassword` (slice G) percent-encodes and injects a password into a postgres DSN, replaces
/// an existing one, and leaves a non-postgres DSN untouched.
#[test]
fn dsn_with_password_injects_for_postgres_only() {
    assert_eq!(
        inject_pg_password("postgres://user@host:5432/db", "p@ss:w/d"),
        "postgres://user:p%40ss%3Aw%2Fd@host:5432/db"
    );
    assert_eq!(
        inject_pg_password("postgres://user:old@host/db", "new"),
        "postgres://user:new@host/db"
    );
    assert_eq!(
        inject_pg_password("sqlite::memory:", "x"),
        "sqlite::memory:"
    );
    // Slice J: mysql/mariadb URL DSNs take the injected credential too (withPassword must never
    // silently no-op on a MySQL DSN).
    assert_eq!(
        inject_pg_password("mysql://user@host:3306/db", "pw"),
        "mysql://user:pw@host:3306/db"
    );
    assert_eq!(
        inject_pg_password("mariadb://u:old@h/db", "new"),
        "mariadb://u:new@h/db"
    );
}

// ── Typed array-accessor validation (DEC-208 slice K, server-free) ──────────────────────────

#[test]
fn list_from_cell_validates_elements_strictly() {
    let ok = Value::List(Rc::new(vec![Value::Int(1), Value::Int(2)]));
    assert!(matches!(
        list_from_cell(&ok, "c", "getIntList", "int", false, |v| matches!(
            v,
            Value::Int(_)
        )),
        Ok(Value::List(_))
    ));
    // A NULL element is rejected with the array_remove steering.
    let holed = Value::List(Rc::new(vec![Value::Int(1), Value::Null]));
    let err = list_from_cell(&holed, "c", "getIntList", "int", false, |v| {
        matches!(v, Value::Int(_))
    })
    .unwrap_err();
    assert!(err.contains("NULL element at [1]"), "{err}");
    // A wrong element type names the offender.
    let mixed = Value::List(Rc::new(vec![Value::Str("x".into())]));
    let err = list_from_cell(&mixed, "c", "getIntList", "int", false, |v| {
        matches!(v, Value::Int(_))
    })
    .unwrap_err();
    assert!(err.contains("element [0] is string, not int"), "{err}");
    // Whole-array NULL: OrNull admits, strict rejects; a scalar cell is "not an array".
    assert!(matches!(
        list_from_cell(&Value::Null, "c", "getIntListOrNull", "int", true, |_| true),
        Ok(Value::Null)
    ));
    assert!(list_from_cell(&Value::Null, "c", "getIntList", "int", false, |_| true).is_err());
    let err =
        list_from_cell(&Value::Int(3), "c", "getIntList", "int", false, |_| true).unwrap_err();
    assert!(err.contains("not an array"), "{err}");
}
