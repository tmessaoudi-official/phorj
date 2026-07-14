#![cfg(feature = "db")]
//! `Core.Db` (DEC-208) end-to-end fixture.
//!
//! The enhanced-PDO surface opens a real bundled-SQLite database (`rusqlite`), so its example is
//! `pure:false` → quarantined from the byte-identity differential (live DB I/O can't be byte-identical
//! across rusqlite and PHP PDO). This is therefore the SOLE gate that runs the shipped
//! `examples/db/basic.phg` through the real language surface — `new Db(dsn)` → `prepare` → `bind`/
//! `bindNamed` → `exec`/`query` → typed `Row` accessors, with a catchable `DbError` — on BOTH backends.
//! The PHP leg is excluded; `run ≡ runvm` must hold (both call the one shared native bodies). Compiled
//! only under `--features db` (see the pre-push gate's `--features db` step).

use phorj::cli::{cmd_run, cmd_treewalk};

#[test]
fn db_example_runs_on_both_backends() {
    let src = std::fs::read_to_string("examples/db/basic.phg").expect("read examples/db/basic.phg");
    // Two rows survive the `age > 30` filter, ordered by age: Ada/36 then Grace/45.
    let expected = "Ada is 36\nGrace is 45\n";
    let tree = cmd_treewalk(&src).expect("basic.phg runs on the interpreter");
    assert_eq!(tree, expected);
    // run ≡ runvm: the VM must produce byte-identical stdout.
    assert_eq!(cmd_run(&src).expect("basic.phg runs on the VM"), tree);
}

// ── DEC-208 S2: typed-generic hydration (`queryInto` / `queryOneInto`) ───────────────────────────

/// Assert that a program produces `expected` on BOTH backends (interpreter reference + VM), byte-
/// identically (`run ≡ runvm`).
fn both(src: &str, expected: &str) {
    let tree = cmd_treewalk(src).expect("program runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    assert_eq!(
        cmd_run(src).expect("program runs on the VM"),
        tree,
        "run ≡ runvm"
    );
}

#[test]
fn db_typed_example_runs_on_both_backends() {
    let src = std::fs::read_to_string("examples/db/typed.phg").expect("read examples/db/typed.phg");
    let expected = "Ada (36) aka Countess\n\
                    Grace (45) aka -\n\
                    one: Ada\n\
                    none: <none>\n\
                    too many: Core.Db.queryOneInto: expected at most one row for `User`\n";
    both(&src, expected);
}

/// The shared scaffold: a two-row `users(name TEXT, age INTEGER)` table, then `body` inside a
/// `try { … } catch (DbError e) { print(e.message) }`.
fn typed_program(body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.Db;
import Core.Db.Db;
import Core.Db.DbError;
class User {{ constructor(public string name, public int age) {{}} }}
function main(): void {{
  try {{
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users(name, age) VALUES(?, ?)").bind("Ada").bind(36).exec();
    discard db.prepare("INSERT INTO users(name, age) VALUES(?, ?)").bind("Grace").bind(45).exec();
    {body}
  }} catch (DbError e) {{ Output.printLine("caught: {{e.message}}"); }}
}}
"#
    )
}

#[test]
fn db_query_into_maps_every_row() {
    let src = typed_program(
        r#"List<User> users = db.prepare("SELECT name, age FROM users ORDER BY age").queryInto();
       for (User u in users) { Output.printLine("{u.name}/{u.age}"); }"#,
    );
    both(&src, "Ada/36\nGrace/45\n");
}

#[test]
fn db_query_into_propagates_with_question_mark() {
    // The idiomatic `throws DbError` helper: `queryInto()?` in a non-`try` propagating context —
    // the sink type is still inferred through the `?`.
    let src = r#"package Main;
import Core.Output;
import Core.Db;
import Core.Db.Db;
import Core.Db.Statement;
import Core.Db.DbError;
class User { constructor(public string name, public int age) {} }
function loadAll(Statement s): List<User> throws DbError {
  List<User> u = s.queryInto()?;
  return u;
}
function main(): void {
  try {
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users VALUES(?, ?)").bind("Ada").bind(36).exec();
    for (User u in loadAll(db.prepare("SELECT name, age FROM users"))) { Output.printLine(u.name); }
  } catch (DbError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada\n");
}

#[test]
fn db_query_one_into_single_row_is_the_object() {
    let src = typed_program(
        r#"User? u = db.prepare("SELECT name, age FROM users WHERE name = ?").bind("Ada").queryOneInto();
       Output.printLine("{u?.name ?? "<null>"}");"#,
    );
    both(&src, "Ada\n");
}

#[test]
fn db_query_one_into_zero_rows_is_null() {
    let src = typed_program(
        r#"User? u = db.prepare("SELECT name, age FROM users WHERE name = ?").bind("Zzz").queryOneInto();
       Output.printLine("{u?.name ?? "<null>"}");"#,
    );
    both(&src, "<null>\n");
}

#[test]
fn db_query_one_into_many_rows_throws_db_error() {
    let src = typed_program(
        r#"User? u = db.prepare("SELECT name, age FROM users").queryOneInto();
       Output.printLine("{u?.name ?? "<null>"}");"#,
    );
    both(
        &src,
        "caught: Core.Db.queryOneInto: expected at most one row for `User`\n",
    );
}

#[test]
fn db_query_into_type_mismatch_throws() {
    // `age` is a non-optional `int`, but the column is aliased to a text value → DbError.
    let src = typed_program(
        r#"List<User> users = db.prepare("SELECT name, 'x' AS age FROM users").queryInto();
       Output.printLine("{List.length(users)}");"#,
    );
    both(
        &src,
        "caught: Core.Db.getInt: column `age` is string, not int\n",
    );
}

#[test]
fn db_query_into_missing_column_throws() {
    let src = typed_program(
        r#"List<User> users = db.prepare("SELECT name FROM users").queryInto();
       Output.printLine("{List.length(users)}");"#,
    );
    both(
        &src,
        "caught: Core.Db.getInt: no column `age` in this row\n",
    );
}

#[test]
fn db_query_into_null_into_non_optional_throws() {
    let src = typed_program(
        r#"List<User> users = db.prepare("SELECT name, NULL AS age FROM users").queryInto();
       Output.printLine("{List.length(users)}");"#,
    );
    both(
        &src,
        "caught: Core.Db.getInt: column `age` is NULL (use int?)\n",
    );
}

#[test]
fn db_query_into_optional_field_admits_null() {
    // A `string?` field: a NULL column maps to `null`, a present value maps through.
    let src = r#"package Main;
import Core.Output;
import Core.Db;
import Core.Db.Db;
import Core.Db.DbError;
class Row2 { constructor(public string name, public int? age) {} }
function main(): void {
  try {
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO t VALUES(?, ?)").bind("Ada").bind(36).exec();
    discard db.prepare("INSERT INTO t VALUES(?, NULL)").bind("Grace").exec();
    List<Row2> rows = db.prepare("SELECT name, age FROM t ORDER BY name").queryInto();
    for (Row2 r in rows) { Output.printLine("{r.name}={r.age ?? -1}"); }
  } catch (DbError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada=36\nGrace=-1\n");
}
