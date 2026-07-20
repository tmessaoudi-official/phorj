//! In-module unit tests for `Core.DatabaseModule` (part 1): the runtime round-trip, bind-style
//! rejection, taxonomy tagging, and the transaction / savepoint / close + column-introspection cases.
//! The shared helpers ([`ok_of`]/[`err_of`]/[`exec1`]/[`scalar`]/[`q`]/[`x`]) are reused by
//! [`super::tests_more`] (part 2).

use super::wrappers::*;
use crate::value::Value;

// Slice D flipped `db_query`/`db_exec` from `Pure` to `HigherOrder`; the in-module tests call them
// directly, so these shims supply a no-op closure invoker (no hook registered → never invoked) and
// keep the `(args, &mut out)` call ergonomics. Return shape is unchanged (`Ok(wrap(..))`).
pub(super) fn q(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let mut noop = |_: &Value, _: Vec<Value>| Ok(Value::Null);
    db_query(args, &mut noop)
}
pub(super) fn x(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let mut noop = |_: &Value, _: Vec<Value>| Ok(Value::Null);
    db_exec(args, &mut noop)
}

/// Extract the payload of a `Result.Success(v)` value the natives now return; panic on `Failure`.
pub(super) fn ok_of(v: Value) -> Value {
    match v {
        Value::Enum(e) if e.variant.as_ref() == "Ok" => e.payload[0].clone(),
        other => panic!("expected DatabaseResult.Ok, got {other:?}"),
    }
}

/// Extract the message of a `Result.Failure(msg)` value; panic on `Success`.
pub(super) fn err_of(v: Value) -> String {
    match v {
        Value::Enum(e) if e.variant.as_ref() == "Err" => match &e.payload[0] {
            Value::Str(s) => s.as_str().to_string(),
            other => panic!("Failure payload not a string: {other:?}"),
        },
        other => panic!("expected DatabaseResult.Err, got {other:?}"),
    }
}

/// End-to-end runtime round-trip (in-process): open in-memory → DDL → insert (positional + named
/// binds) → query back → Row accessors. Proves the rusqlite integration through the `Value` model
/// and the `Result`-returning protocol, independent of the language surface (which lands next slice).
#[test]
fn db_runtime_round_trip() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str("sqlite::memory:".into())], &mut out).unwrap());

    // CREATE TABLE (no binds) via exec.
    let stmt = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("CREATE TABLE users(name TEXT, age INTEGER)".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    assert!(ok_of(x(&[stmt], &mut out).unwrap()).eq_val(&Value::Int(0)));

    // INSERT with positional binds.
    let ins = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO users(name, age) VALUES(?, ?)".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    // bind() mutates the shared handle in place and returns a discarded unit carrier (DEC-292),
    // so chain via the SAME handle — exactly what the prelude does with `this`.
    ok_of(db_bind(&[ins.clone(), Value::Str("Ada".into())], &mut out).unwrap());
    ok_of(db_bind(&[ins.clone(), Value::Int(36)], &mut out).unwrap());
    assert!(ok_of(x(&[ins], &mut out).unwrap()).eq_val(&Value::Int(1)));

    // INSERT with named binds.
    let ins2 = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("INSERT INTO users(name, age) VALUES(:n, :a)".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    ok_of(
        db_bind_named(
            &[
                ins2.clone(),
                Value::Str("n".into()),
                Value::Str("Grace".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    ok_of(
        db_bind_named(
            &[ins2.clone(), Value::Str("a".into()), Value::Int(45)],
            &mut out,
        )
        .unwrap(),
    );
    assert!(ok_of(x(&[ins2], &mut out).unwrap()).eq_val(&Value::Int(1)));

    // Query back, ordered, and read via Row accessors.
    let sel = ok_of(
        db_prepare(
            &[
                db.clone(),
                Value::Str("SELECT name, age FROM users WHERE age > ? ORDER BY age".into()),
            ],
            &mut out,
        )
        .unwrap(),
    );
    ok_of(db_bind(&[sel.clone(), Value::Int(30)], &mut out).unwrap());
    let rows = ok_of(q(&[sel], &mut out).unwrap());
    let Value::List(rows) = rows else {
        panic!("query must return a list")
    };
    assert_eq!(rows.len(), 2);

    // Row 0 = Ada / 36.
    assert!(ok_of(
        row_get_string(&[rows[0].clone(), Value::Str("name".into())], &mut out).unwrap()
    )
    .eq_val(&Value::Str("Ada".into())));
    assert!(
        ok_of(row_get_int(&[rows[0].clone(), Value::Str("age".into())], &mut out).unwrap())
            .eq_val(&Value::Int(36))
    );
    // Row 1 = Grace / 45.
    assert!(ok_of(
        row_get_string(&[rows[1].clone(), Value::Str("name".into())], &mut out).unwrap()
    )
    .eq_val(&Value::Str("Grace".into())));
}

#[test]
fn mixing_bind_styles_is_a_failure() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    let s = ok_of(db_prepare(&[db, Value::Str("SELECT ?, :x".into())], &mut out).unwrap());
    ok_of(db_bind(&[s.clone(), Value::Int(1)], &mut out).unwrap());
    // A DB usage error is a catchable Result.Failure, NOT a hard fault.
    let msg = err_of(db_bind_named(&[s, Value::Str("x".into()), Value::Int(2)], &mut out).unwrap());
    assert!(msg.contains("cannot mix"), "got: {msg}");
}

#[test]
fn get_int_on_null_is_a_failure() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    let s = ok_of(db_prepare(&[db, Value::Str("SELECT NULL AS x".into())], &mut out).unwrap());
    let rows = ok_of(q(&[s], &mut out).unwrap());
    let Value::List(rows) = rows else { panic!() };
    let msg = err_of(row_get_int(&[rows[0].clone(), Value::Str("x".into())], &mut out).unwrap());
    assert!(msg.contains("NULL"), "got: {msg}");
}

// ── DEC-208 slice C: transactions, savepoints, taxonomy, close ─────────────────────────────

/// Open an in-memory DB and run one `exec` statement, panicking on any failure.
pub(super) fn exec1(db: &Value, sql: &str, out: &mut String) {
    let s = ok_of(db_prepare(&[db.clone(), Value::Str(sql.into())], out).unwrap());
    ok_of(x(&[s], out).unwrap());
}

/// Read a single-int scalar from `sql`.
pub(super) fn scalar(db: &Value, sql: &str, col: &str, out: &mut String) -> i64 {
    let s = ok_of(db_prepare(&[db.clone(), Value::Str(sql.into())], out).unwrap());
    let rows = ok_of(q(&[s], out).unwrap());
    let Value::List(rows) = rows else {
        panic!("query returns a list")
    };
    let v = ok_of(row_get_int(&[rows[0].clone(), Value::Str(col.into())], out).unwrap());
    match v {
        Value::Int(n) => n,
        other => panic!("expected int, got {other:?}"),
    }
}

/// A UNIQUE / PRIMARY KEY collision maps (via the extended result code) to the `UniqueViolationError`
/// marker the prelude classifier reads.
#[test]
fn unique_violation_is_tagged() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    exec1(&db, "CREATE TABLE t(id INTEGER PRIMARY KEY)", &mut out);
    exec1(&db, "INSERT INTO t(id) VALUES(1)", &mut out);
    let s = ok_of(
        db_prepare(
            &[db, Value::Str("INSERT INTO t(id) VALUES(1)".into())],
            &mut out,
        )
        .unwrap(),
    );
    let msg = err_of(x(&[s], &mut out).unwrap());
    assert!(msg.starts_with("<<UniqueViolationError>>"), "got: {msg}");
}

/// A malformed statement maps to the `SyntaxError` marker.
#[test]
fn syntax_error_is_tagged() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    let s = ok_of(db_prepare(&[db, Value::Str("SELCT oops".into())], &mut out).unwrap());
    let msg = err_of(q(&[s], &mut out).unwrap());
    assert!(msg.starts_with("<<SyntaxError>>"), "got: {msg}");
}

/// A committed transaction persists; a rolled-back one is discarded.
#[test]
fn commit_persists_rollback_discards() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);

    ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap());
    exec1(&db, "INSERT INTO t(n) VALUES(1)", &mut out);
    ok_of(db_commit(std::slice::from_ref(&db), &mut out).unwrap());
    assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);

    ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap());
    exec1(&db, "INSERT INTO t(n) VALUES(2)", &mut out);
    ok_of(db_rollback(std::slice::from_ref(&db), &mut out).unwrap());
    // Still one row — the second insert was rolled back.
    assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);
}

/// A nested `begin` is a SAVEPOINT: rolling it back leaves the outer transaction's work intact.
#[test]
fn savepoint_partial_rollback() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);

    ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap()); // outer
    exec1(&db, "INSERT INTO t(n) VALUES(1)", &mut out);
    ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap()); // savepoint
    exec1(&db, "INSERT INTO t(n) VALUES(2)", &mut out);
    ok_of(db_rollback(std::slice::from_ref(&db), &mut out).unwrap()); // roll back savepoint only
    ok_of(db_commit(std::slice::from_ref(&db), &mut out).unwrap()); // commit outer
                                                                    // Only the outer insert (n=1) survives.
    assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);
    assert_eq!(scalar(&db, "SELECT n FROM t", "n", &mut out), 1);
}

/// `close` is idempotent and invalidates every derived handle; a later op faults with the
/// `ConnectionError` marker.
#[test]
fn close_invalidates_connection() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);
    ok_of(db_close(std::slice::from_ref(&db), &mut out).unwrap());
    // Idempotent: a second close is still Ok.
    ok_of(db_close(std::slice::from_ref(&db), &mut out).unwrap());
    let msg = err_of(db_prepare(&[db, Value::Str("SELECT 1".into())], &mut out).unwrap());
    assert!(msg.starts_with("<<ConnectionError>>"), "got: {msg}");
}

#[test]
fn column_names_are_selection_ordered() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    let s = ok_of(
        db_prepare(
            &[db, Value::Str("SELECT 1 AS a, 2 AS b, 3 AS c".into())],
            &mut out,
        )
        .unwrap(),
    );
    let rows = ok_of(q(&[s], &mut out).unwrap());
    let Value::List(rows) = rows else { panic!() };
    let cols = ok_of(row_column_names(&[rows[0].clone()], &mut out).unwrap());
    let Value::List(cols) = cols else {
        panic!("columnNames must return a list")
    };
    let got: Vec<&str> = cols
        .iter()
        .map(|v| match v {
            Value::Str(s) => s.as_str(),
            _ => panic!("column name not a string"),
        })
        .collect();
    assert_eq!(got, vec!["a", "b", "c"]);
}

#[test]
fn is_null_reports_null_and_present() {
    let mut out = String::new();
    let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
    let s = ok_of(
        db_prepare(
            &[db, Value::Str("SELECT NULL AS a, 7 AS b".into())],
            &mut out,
        )
        .unwrap(),
    );
    let rows = ok_of(q(&[s], &mut out).unwrap());
    let Value::List(rows) = rows else { panic!() };
    assert!(
        ok_of(row_is_null(&[rows[0].clone(), Value::Str("a".into())], &mut out).unwrap())
            .eq_val(&Value::Bool(true))
    );
    assert!(
        ok_of(row_is_null(&[rows[0].clone(), Value::Str("b".into())], &mut out).unwrap())
            .eq_val(&Value::Bool(false))
    );
    // A missing column is a strict DB error (reuses `row_cell`).
    let msg = err_of(row_is_null(&[rows[0].clone(), Value::Str("zzz".into())], &mut out).unwrap());
    assert!(msg.contains("no column"), "got: {msg}");
}
