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

/// Assert that a program FAILS to compile with a message containing `needle` (a compile-time reject,
/// not a runtime `DbError`). Checked on the interpreter path (the checker runs identically for the VM).
fn fails_with(src: &str, needle: &str) {
    match cmd_treewalk(src) {
        Ok(out) => panic!("expected a compile error containing {needle:?}, but it ran: {out:?}"),
        Err(e) => assert!(e.contains(needle), "error {e:?} did not contain {needle:?}"),
    }
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

// ── DEC-208 slice B: nested hydration + queryScalar + queryMap ────────────────────────────────────

#[test]
fn db_nested_example_runs_on_both_backends() {
    let src =
        std::fs::read_to_string("examples/db/nested.phg").expect("read examples/db/nested.phg");
    let expected = "Book: order 10 total 299 by Ada of France, ships to Japan\n\
                    Pen: order 20 total 150 by Grace of Japan, ships to -\n\
                    sales: 2\n\
                    100 -> Book\n\
                    200 -> Pen\n";
    both(&src, expected);
}

/// The shared nested scaffold: a `sales` join producing dotted-aliased columns for a required nested
/// `Order` and an optional nested `Country` (`shipTo`, LEFT JOIN). `body` runs inside a `try/catch`.
fn nested_program(rows: &str, body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.Map;
import Core.Db;
import Core.Db.Db;
import Core.Db.DbError;
class Country {{ constructor(public string code, public string name) {{}} }}
class Customer {{ constructor(public int id, public string name, public Country country) {{}} }}
class Order {{ constructor(public int id, public int total, public Customer customer) {{}} }}
class Sale {{ constructor(public string product, public Order order, public Country? shipTo) {{}} }}
function main(): void {{
  try {{
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE countries(code TEXT, name TEXT)").exec();
    discard db.prepare("CREATE TABLE customers(id INTEGER, name TEXT, country_code TEXT)").exec();
    discard db.prepare("CREATE TABLE orders(id INTEGER, total INTEGER, customer_id INTEGER)").exec();
    discard db.prepare("CREATE TABLE sales(id INTEGER, product TEXT, order_id INTEGER, ship_to_code TEXT)").exec();
    discard db.prepare("INSERT INTO countries VALUES('FR', 'France')").exec();
    discard db.prepare("INSERT INTO countries VALUES('JP', 'Japan')").exec();
    discard db.prepare("INSERT INTO customers VALUES(1, 'Ada', 'FR')").exec();
    discard db.prepare("INSERT INTO customers VALUES(2, 'Grace', 'JP')").exec();
    discard db.prepare("INSERT INTO orders VALUES(10, 299, 1)").exec();
    discard db.prepare("INSERT INTO orders VALUES(20, 150, 2)").exec();
    {rows}
    {body}
  }} catch (DbError e) {{ Output.printLine("caught: {{e.message}}"); }}
}}
"#
    )
}

/// The deep (depth-4) select with a required `order.*` graph and an optional `shipTo.*` LEFT JOIN.
const NESTED_SELECT: &str = "SELECT s.product AS product, o.id AS \\\"order.id\\\", o.total AS \\\"order.total\\\", c.id AS \\\"order.customer.id\\\", c.name AS \\\"order.customer.name\\\", co.code AS \\\"order.customer.country.code\\\", co.name AS \\\"order.customer.country.name\\\", ship.code AS \\\"shipTo.code\\\", ship.name AS \\\"shipTo.name\\\" FROM sales s JOIN orders o ON o.id = s.order_id JOIN customers c ON c.id = o.customer_id JOIN countries co ON co.code = c.country_code LEFT JOIN countries ship ON ship.code = s.ship_to_code ORDER BY s.id";

#[test]
fn db_nested_hydrates_deep_graph_and_optional_present() {
    // A sale that DOES ship (shipTo present) — the whole 4-deep graph is hydrated.
    let src = nested_program(
        "discard db.prepare(\"INSERT INTO sales VALUES(100, 'Book', 10, 'JP')\").exec();",
        &format!(
            r#"List<Sale> ss = db.prepare("{NESTED_SELECT}").queryInto();
       for (Sale s in ss) {{ Output.printLine("{{s.product}}/{{s.order.customer.country.name}}/{{s.shipTo?.name ?? "-"}}"); }}"#
        ),
    );
    both(&src, "Book/France/Japan\n");
}

#[test]
fn db_nested_optional_entity_is_null_when_all_columns_null() {
    // A sale with ship_to_code NULL → the LEFT JOIN yields all-NULL shipTo columns → `shipTo` is null.
    let src = nested_program(
        "discard db.prepare(\"INSERT INTO sales VALUES(200, 'Pen', 20, NULL)\").exec();",
        &format!(
            r#"List<Sale> ss = db.prepare("{NESTED_SELECT}").queryInto();
       for (Sale s in ss) {{ Output.printLine("{{s.product}}/{{s.shipTo?.name ?? "-"}}"); }}"#
        ),
    );
    both(&src, "Pen/-\n");
}

#[test]
fn db_nested_required_partial_null_throws() {
    // A REQUIRED nested `Order` with a NULL `order.total` column is NOT a null-parent — the strict
    // `getInt` on the non-optional subfield throws (this is what distinguishes required from optional).
    let src = r#"package Main;
import Core.Output;
import Core.Db;
import Core.Db.Db;
import Core.Db.DbError;
class Order { constructor(public int id, public int total) {} }
class Sale { constructor(public string product, public Order order) {} }
function main(): void {
  try {
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE sales(product TEXT, oid INTEGER, ototal INTEGER)").exec();
    discard db.prepare("INSERT INTO sales VALUES('Book', 10, NULL)").exec();
    List<Sale> ss = db.prepare("SELECT product AS product, oid AS \"order.id\", ototal AS \"order.total\" FROM sales").queryInto();
    Output.printLine("{List.length(ss)}");
  } catch (DbError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.Db.getInt: column `order.total` is NULL (use int?)\n",
    );
}

#[test]
fn db_hydrate_cycle_is_rejected() {
    // A self-referential row class cannot be eagerly whole-graph hydrated (unbounded) → compile error,
    // not a compiler stack overflow. The optional back-reference is still caught (cycle check on entry).
    let src = r#"package Main;
import Core.Output;
import Core.Db;
import Core.Db.Db;
import Core.Db.DbError;
class Employee { constructor(public string name, public Employee? manager) {} }
function main(): void {
  try {
    Db db = new Db("sqlite::memory:");
    List<Employee> es = db.prepare("SELECT name FROM e").queryInto();
    Output.printLine("{List.length(es)}");
  } catch (DbError e) { Output.printLine("caught"); }
}
"#;
    fails_with(src, "E-DB-HYDRATE-CYCLE");
}

#[test]
fn db_query_scalar_returns_a_typed_value() {
    let src = typed_program(
        r#"int n = db.prepare("SELECT COUNT(*) FROM users").queryScalar();
       Output.printLine("{n}");"#,
    );
    both(&src, "2\n");
}

#[test]
fn db_query_scalar_wrong_row_count_throws() {
    // More than one row → DbError (queryScalar requires exactly one).
    let src = typed_program(
        r#"int n = db.prepare("SELECT age FROM users").queryScalar();
       Output.printLine("{n}");"#,
    );
    both(
        &src,
        "caught: Core.Db.queryScalar: expected exactly one row\n",
    );
}

#[test]
fn db_query_scalar_wrong_column_count_throws() {
    let src = typed_program(
        r#"int n = db.prepare("SELECT age, name FROM users WHERE name = 'Ada'").queryScalar();
       Output.printLine("{n}");"#,
    );
    both(
        &src,
        "caught: Core.Db.queryScalar: expected exactly one column\n",
    );
}

#[test]
fn db_query_map_scalar_value_keys_by_first_column() {
    // Map<int, string>: keyed by the first column (age), value = the second column (name).
    let src = typed_program(
        r#"Map<int, string> byAge = db.prepare("SELECT age, name FROM users").queryMap();
       Output.printLine("36 -> {Map.get(byAge, 36) ?? "?"}");
       Output.printLine("45 -> {Map.get(byAge, 45) ?? "?"}");"#,
    );
    both(&src, "36 -> Ada\n45 -> Grace\n");
}

#[test]
fn db_query_map_string_key() {
    // Map<string, int>: a string key column.
    let src = typed_program(
        r#"Map<string, int> byName = db.prepare("SELECT name, age FROM users").queryMap();
       Output.printLine("Ada -> {Map.get(byName, "Ada") ?? -1}");
       Output.printLine("Grace -> {Map.get(byName, "Grace") ?? -1}");"#,
    );
    both(&src, "Ada -> 36\nGrace -> 45\n");
}

#[test]
fn db_query_map_entity_value_hydrates_by_field_name() {
    // Map<int, User>: value is a hydrated entity (by field name from the whole row); key is the first
    // column. Extra columns (the id key) are ignored by the entity mapping.
    let src = r#"package Main;
import Core.Output;
import Core.Map;
import Core.Db;
import Core.Db.Db;
import Core.Db.DbError;
class User { constructor(public string name, public int age) {} }
function main(): void {
  try {
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(id INTEGER, name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users VALUES(1, 'Ada', 36)").exec();
    discard db.prepare("INSERT INTO users VALUES(2, 'Grace', 45)").exec();
    Map<int, User> byId = db.prepare("SELECT id, name, age FROM users").queryMap();
    User? one = Map.get(byId, 2);
    Output.printLine("{one?.name ?? "?"}/{one?.age ?? -1}");
  } catch (DbError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Grace/45\n");
}

// ── DEC-208 slice C: transactions, savepoints, taxonomy, close ───────────────────────────────────

/// The shipped `examples/db/transactions.phg` — the SOLE gate that runs the transaction/savepoint/
/// taxonomy/close surface through the real language on BOTH backends (it is quarantined from the
/// byte-identity differential like every `Core.Db` example).
#[test]
fn db_transactions_example_runs_on_both_backends() {
    let src = std::fs::read_to_string("examples/db/transactions.phg")
        .expect("read examples/db/transactions.phg");
    let expected = "after commit: acct1=70 acct2=30\n\
                    caught UniqueViolation; transaction rolled back\n\
                    after rollback: acct1=70 acct2=30\n\
                    after nested: acct1=500 acct2=30\n\
                    after close: Core.Db: the connection is closed\n";
    both(&src, expected);
}

/// A scaffold: a one-row `acct(id PK, bal)` table (id=1, bal=100), then `body` runs inside an
/// `act(db): void throws DbError` helper (so it may use idiomatic `?` propagation), which `main`
/// drives inside `try { … } catch (DbError e) { print }`.
fn tx_program(body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.Db;
import Core.Db.Db;
import Core.Db.Statement;
import Core.Db.Row;
import Core.Db.DbError;
import Core.Db.UniqueViolation;
function bal(Db db): int throws DbError {{
  Statement s = db.prepare("SELECT bal FROM acct WHERE id = 1")?;
  List<Row> rows = s.query()?;
  return rows[0].getInt("bal")?;
}}
function run(Db db, string sql): void throws DbError {{
  Statement s = db.prepare(sql)?;
  discard s.exec()?;
}}
function act(Db db): void throws DbError {{
  {body}
}}
function main(): void {{
  try {{
    Db db = new Db("sqlite::memory:");
    discard db.prepare("CREATE TABLE acct(id INTEGER PRIMARY KEY, bal INTEGER)").exec();
    discard db.prepare("INSERT INTO acct(id, bal) VALUES(1, 100)").exec();
    act(db);
  }} catch (DbError e) {{ Output.printLine("caught: {{e.message}}"); }}
}}
"#
    )
}

#[test]
fn db_commit_persists() {
    let src = tx_program(
        r#"db.begin()?;
       run(db, "UPDATE acct SET bal = 150 WHERE id = 1")?;
       db.commit()?;
       Output.printLine("{bal(db)?}");"#,
    );
    both(&src, "150\n");
}

#[test]
fn db_rollback_on_throw_via_finally_idiom() {
    // The auto-rollback idiom: a UNIQUE violation inside the transaction unwinds through `finally`,
    // which rolls back — the balance change is discarded and the typed error propagates.
    let src = tx_program(
        r#"db.begin()?;
       mutable bool ok = false;
       try {
         run(db, "UPDATE acct SET bal = 999 WHERE id = 1")?;
         run(db, "INSERT INTO acct(id, bal) VALUES(1, 0)")?;
         db.commit()?;
         ok = true;
       } catch (UniqueViolation e) {
         Output.printLine("rolled back on: {e.message}");
       } finally {
         if (!ok) { db.rollbackQuiet(); }
       }
       Output.printLine("bal={bal(db)?}");"#,
    );
    // The UPDATE-to-999 is discarded by the rollback: re-querying shows the original 100.
    both(
        &src,
        "rolled back on: Core.Db: UNIQUE constraint failed: acct.id\nbal=100\n",
    );
}

#[test]
fn db_savepoint_partial_rollback() {
    // Nested begin = savepoint: the inner rollback leaves the outer update intact.
    let src = tx_program(
        r#"db.begin()?;
       run(db, "UPDATE acct SET bal = 200 WHERE id = 1")?;
       db.begin()?;
       run(db, "UPDATE acct SET bal = 777 WHERE id = 1")?;
       db.rollback()?;
       db.commit()?;
       Output.printLine("{bal(db)?}");"#,
    );
    both(&src, "200\n");
}

#[test]
fn db_unique_violation_caught_specifically() {
    // `catch (UniqueViolation e)` catches the precise subtype; the base `catch (DbError)` never runs.
    let src = tx_program(
        r#"try {
         run(db, "INSERT INTO acct(id, bal) VALUES(1, 5)")?;
       } catch (UniqueViolation e) {
         Output.printLine("unique");
       }"#,
    );
    both(&src, "unique\n");
}

#[test]
fn db_close_then_use_is_connection_error() {
    let src = tx_program(
        r#"db.close();
       discard bal(db)?;"#,
    );
    both(&src, "caught: Core.Db: the connection is closed\n");
}
