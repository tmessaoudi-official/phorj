#![cfg(feature = "db")]
//! `Core.DatabaseModule` (DEC-208) end-to-end fixture.
//!
//! The enhanced-PDO surface opens a real bundled-SQLite database (`rusqlite`), so its example is
//! `pure:false` → quarantined from the byte-identity differential (live DB I/O can't be byte-identical
//! across rusqlite and PHP PDO). This is therefore the SOLE gate that runs the shipped
//! `examples/db/basic.phg` through the real language surface — `new Database(dsn)` → `prepare` → `bind`/
//! `bindNamed` → `exec`/`query` → typed `Row` accessors, with a catchable `DatabaseError` — on BOTH backends.
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
/// not a runtime `DatabaseError`). Checked on the interpreter path (the checker runs identically for the VM).
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
                    too many: Core.DatabaseModule.queryOneInto: expected at most one row for `User`\n\
                    turbofish: Ada\n\
                    turbofish: Grace\n\
                    turbofish one: Grace\n";
    both(&src, expected);
}

/// The shared scaffold: a two-row `users(name TEXT, age INTEGER)` table, then `body` inside a
/// `try { … } catch (DatabaseError e) { print(e.message) }`.
fn typed_program(body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class User {{ constructor(public string name, public int age) {{}} }}
function main(): void {{
  try {{
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users(name, age) VALUES(?, ?)").bind("Ada").bind(36).exec();
    discard db.prepare("INSERT INTO users(name, age) VALUES(?, ?)").bind("Grace").bind(45).exec();
    {body}
  }} catch (DatabaseError e) {{ Output.printLine("caught: {{e.message}}"); }}
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
    // The idiomatic `throws DatabaseError` helper: `queryInto()?` in a non-`try` propagating context —
    // the sink type is still inferred through the `?`.
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.DatabaseError;
class User { constructor(public string name, public int age) {} }
function loadAll(Statement s): List<User> throws DatabaseError {
  List<User> u = s.queryInto()?;
  return u;
}
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users VALUES(?, ?)").bind("Ada").bind(36).exec();
    for (User u in loadAll(db.prepare("SELECT name, age FROM users"))) { Output.printLine(u.name); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
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
        "caught: Core.DatabaseModule.queryOneInto: expected at most one row for `User`\n",
    );
}

#[test]
fn db_query_into_type_mismatch_throws() {
    // `age` is a non-optional `int`, but the column is aliased to a text value → DatabaseError.
    let src = typed_program(
        r#"List<User> users = db.prepare("SELECT name, 'x' AS age FROM users").queryInto();
       Output.printLine("{List.length(users)}");"#,
    );
    both(
        &src,
        "caught: Core.DatabaseModule.getInt: column `age` is string, not int\n",
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
        "caught: Core.DatabaseModule.getInt: no column `age` in this row\n",
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
        "caught: Core.DatabaseModule.getInt: column `age` is NULL (use int?)\n",
    );
}

#[test]
fn db_query_into_optional_field_admits_null() {
    // A `string?` field: a NULL column maps to `null`, a present value maps through.
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Row2 { constructor(public string name, public int? age) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO t VALUES(?, ?)").bind("Ada").bind(36).exec();
    discard db.prepare("INSERT INTO t VALUES(?, NULL)").bind("Grace").exec();
    List<Row2> rows = db.prepare("SELECT name, age FROM t ORDER BY name").queryInto();
    for (Row2 r in rows) { Output.printLine("{r.name}={r.age ?? -1}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
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
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Country {{ constructor(public string code, public string name) {{}} }}
class Customer {{ constructor(public int id, public string name, public Country country) {{}} }}
class Order {{ constructor(public int id, public int total, public Customer customer) {{}} }}
class Sale {{ constructor(public string product, public Order order, public Country? shipTo) {{}} }}
function main(): void {{
  try {{
    Database db = new Database("sqlite::memory:");
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
  }} catch (DatabaseError e) {{ Output.printLine("caught: {{e.message}}"); }}
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
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Order { constructor(public int id, public int total) {} }
class Sale { constructor(public string product, public Order order) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE sales(product TEXT, oid INTEGER, ototal INTEGER)").exec();
    discard db.prepare("INSERT INTO sales VALUES('Book', 10, NULL)").exec();
    List<Sale> ss = db.prepare("SELECT product AS product, oid AS \"order.id\", ototal AS \"order.total\" FROM sales").queryInto();
    Output.printLine("{List.length(ss)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule.getInt: column `order.total` is NULL (use int?)\n",
    );
}

#[test]
fn db_hydrate_cycle_is_rejected() {
    // A self-referential row class cannot be eagerly whole-graph hydrated (unbounded) → compile error,
    // not a compiler stack overflow. The optional back-reference is still caught (cycle check on entry).
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Employee { constructor(public string name, public Employee? manager) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    List<Employee> es = db.prepare("SELECT name FROM e").queryInto();
    Output.printLine("{List.length(es)}");
  } catch (DatabaseError e) { Output.printLine("caught"); }
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
    // More than one row → DatabaseError (queryScalar requires exactly one).
    let src = typed_program(
        r#"int n = db.prepare("SELECT age FROM users").queryScalar();
       Output.printLine("{n}");"#,
    );
    both(
        &src,
        "caught: Core.DatabaseModule.queryScalar: expected exactly one row\n",
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
        "caught: Core.DatabaseModule.queryScalar: expected exactly one column\n",
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
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class User { constructor(public string name, public int age) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(id INTEGER, name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users VALUES(1, 'Ada', 36)").exec();
    discard db.prepare("INSERT INTO users VALUES(2, 'Grace', 45)").exec();
    Map<int, User> byId = db.prepare("SELECT id, name, age FROM users").queryMap();
    User? one = Map.get(byId, 2);
    Output.printLine("{one?.name ?? "?"}/{one?.age ?? -1}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Grace/45\n");
}

// ── DEC-208 slice B2: column naming strategy (SnakeToCamel) ──────────────────────────────────────

#[test]
fn db_naming_example_runs_on_both_backends() {
    let src =
        std::fs::read_to_string("examples/db/naming.phg").expect("read examples/db/naming.phg");
    let expected = "1: Ada (@ada) lives on Rue de Rivoli, 75001\n\
                    2: Grace (@grace) lives on Baker Street, NW16XE\n";
    both(&src, expected);
}

#[test]
fn db_naming_snake_to_camel_maps_camel_fields() {
    // `.namingStrategy(new Naming.SnakeToCamel())` makes a `userName` field read the `user_name`
    // column and `firstName` read `first_name` — the desugar bakes the snake_case column literal in.
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Naming;
import Core.DatabaseModule.DatabaseError;
class Member { constructor(public string userName, public string firstName) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE members(user_name TEXT, first_name TEXT)").exec();
    discard db.prepare("INSERT INTO members VALUES('ada', 'Ada')").exec();
    discard db.prepare("INSERT INTO members VALUES('grace', 'Grace')").exec();
    List<Member> ms = db.prepare("SELECT user_name, first_name FROM members ORDER BY user_name")
      .namingStrategy(new Naming.SnakeToCamel()).queryInto();
    for (Member m in ms) { Output.printLine("{m.firstName}/@{m.userName}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada/@ada\nGrace/@grace\n");
}

#[test]
fn db_naming_default_exact_needs_exact_column() {
    // The strict-exact DEFAULT is unchanged: with no `namingStrategy`, a camelCase field looks up a
    // camelCase column, so a snake_case column is a runtime `no column` DatabaseError (not a naming bug).
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Member { constructor(public string userName) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE members(user_name TEXT)").exec();
    discard db.prepare("INSERT INTO members VALUES('ada')").exec();
    List<Member> ms = db.prepare("SELECT user_name FROM members").queryInto();
    for (Member m in ms) { Output.printLine(m.userName); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule.getString: no column `userName` in this row\n",
    );
}

#[test]
fn db_naming_snake_to_camel_nested_entity() {
    // The transform applies PER dotted segment: a nested `homeAddress.streetName` reads the alias
    // `"home_address.street_name"` (segment `home_address` from the field, `.street_name` from the sub).
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Naming;
import Core.DatabaseModule.DatabaseError;
class Address { constructor(public string streetName) {} }
class Member { constructor(public string userName, public Address homeAddress) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE m(user_name TEXT, street_name TEXT)").exec();
    discard db.prepare("INSERT INTO m VALUES('ada', 'Rue de Rivoli')").exec();
    List<Member> ms = db.prepare("SELECT user_name AS user_name, street_name AS \"home_address.street_name\" FROM m")
      .namingStrategy(new Naming.SnakeToCamel()).queryInto();
    for (Member x in ms) { Output.printLine("@{x.userName}: {x.homeAddress.streetName}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "@ada: Rue de Rivoli\n");
}

#[test]
fn db_naming_query_map_entity_value_under_strategy() {
    // `queryMap` with an ENTITY value hydrates it by field name, so the strategy applies to the value
    // fields; the key (first column) and any scalar are read by position and are unaffected.
    let src = r#"package Main;
import Core.Output;
import Core.Map;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Naming;
import Core.DatabaseModule.DatabaseError;
class Member { constructor(public string userName, public string firstName) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE m(id INTEGER, user_name TEXT, first_name TEXT)").exec();
    discard db.prepare("INSERT INTO m VALUES(1, 'ada', 'Ada')").exec();
    discard db.prepare("INSERT INTO m VALUES(2, 'grace', 'Grace')").exec();
    Map<int, Member> byId = db.prepare("SELECT id, user_name, first_name FROM m")
      .namingStrategy(new Naming.SnakeToCamel()).queryMap();
    Member? g = Map.get(byId, 2);
    Output.printLine("{g?.firstName ?? "?"}/@{g?.userName ?? "?"}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Grace/@grace\n");
}

#[test]
fn db_naming_non_literal_argument_is_rejected() {
    // The strategy must be a compile-time `new Naming.X()` literal — a variable cannot drive a
    // compile-time column rewrite, and silently falling back to `Exact` would be a forbidden downgrade.
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Naming;
import Core.DatabaseModule.DatabaseError;
class U { constructor(public string userName) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    Naming n = new Naming.SnakeToCamel();
    List<U> us = db.prepare("SELECT 1 AS user_name").namingStrategy(n).queryInto();
    for (U u in us) { Output.printLine(u.userName); }
  } catch (DatabaseError e) { Output.printLine("x"); }
}
"#;
    fails_with(src, "E-DB-NAMING-NOT-CONST");
}

#[test]
fn db_naming_unknown_variant_is_rejected() {
    // An unrecognized `Naming` variant is not a valid compile-time strategy literal either.
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Naming;
import Core.DatabaseModule.DatabaseError;
class U { constructor(public string userName) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    List<U> us = db.prepare("SELECT 1 AS user_name").namingStrategy(new Naming.Bogus()).queryInto();
    for (U u in us) { Output.printLine(u.userName); }
  } catch (DatabaseError e) { Output.printLine("x"); }
}
"#;
    fails_with(src, "E-DB-NAMING-NOT-CONST");
}

// ── DEC-208 slice C: transactions, savepoints, taxonomy, close ───────────────────────────────────

/// The shipped `examples/db/transactions.phg` — the SOLE gate that runs the transaction/savepoint/
/// taxonomy/close surface through the real language on BOTH backends (it is quarantined from the
/// byte-identity differential like every `Core.DatabaseModule` example).
#[test]
fn db_transactions_example_runs_on_both_backends() {
    let src = std::fs::read_to_string("examples/db/transactions.phg")
        .expect("read examples/db/transactions.phg");
    let expected = "after commit: acct1=70 acct2=30\n\
                    caught UniqueViolationError; transaction rolled back\n\
                    after rollback: acct1=70 acct2=30\n\
                    after nested: acct1=500 acct2=30\n\
                    after close: Core.DatabaseModule: the connection is closed\n";
    both(&src, expected);
}

/// A scaffold: a one-row `acct(id PK, bal)` table (id=1, bal=100), then `body` runs inside an
/// `act(db): void throws DatabaseError` helper (so it may use idiomatic `?` propagation), which `main`
/// drives inside `try { … } catch (DatabaseError e) { print }`.
fn tx_program(body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
import Core.DatabaseModule.UniqueViolationError;
function bal(Database db): int throws DatabaseError {{
  Statement s = db.prepare("SELECT bal FROM acct WHERE id = 1")?;
  List<Row> rows = s.query()?;
  return rows[0].getInt("bal")?;
}}
function run(Database db, string sql): void throws DatabaseError {{
  Statement s = db.prepare(sql)?;
  discard s.exec()?;
}}
function act(Database db): void throws DatabaseError {{
  {body}
}}
function main(): void {{
  try {{
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE acct(id INTEGER PRIMARY KEY, bal INTEGER)").exec();
    discard db.prepare("INSERT INTO acct(id, bal) VALUES(1, 100)").exec();
    act(db);
  }} catch (DatabaseError e) {{ Output.printLine("caught: {{e.message}}"); }}
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
       } catch (UniqueViolationError e) {
         Output.printLine("rolled back on: {e.message}");
       } finally {
         if (!ok) { db.rollbackQuiet(); }
       }
       Output.printLine("bal={bal(db)?}");"#,
    );
    // The UPDATE-to-999 is discarded by the rollback: re-querying shows the original 100.
    both(
        &src,
        "rolled back on: Core.DatabaseModule: UNIQUE constraint failed: acct.id\nbal=100\n",
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
    // `catch (UniqueViolationError e)` catches the precise subtype; the base `catch (DatabaseError)` never runs.
    let src = tx_program(
        r#"try {
         run(db, "INSERT INTO acct(id, bal) VALUES(1, 5)")?;
       } catch (UniqueViolationError e) {
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
    both(
        &src,
        "caught: Core.DatabaseModule: the connection is closed\n",
    );
}

// ── DEC-208 slice C: the CLOSURE form of transactions (`db.transaction(fn[, retries])`) ──
// Unblocked by DEC-222 (throwing-closure function types). BEGIN → run closure → COMMIT on normal return
// (returning its value) / auto-ROLLBACK + re-throw the ORIGINAL typed error on a throw; a nested call is
// a SAVEPOINT; `transaction(fn, retries)` re-runs on the transient `SerializationFailureError` only (DEC-249 default param; the former `transactionRetry` is retired).

/// The shipped `examples/db/transaction-closure.phg` — the SOLE gate that runs the closure-form
/// transaction surface (commit / value-return / auto-rollback-and-rethrow / nested savepoint / retry)
/// through the real language on BOTH backends (quarantined from the byte-identity differential).
#[test]
fn db_transaction_closure_example_runs_on_both_backends() {
    let src = std::fs::read_to_string("examples/db/transaction-closure.phg")
        .expect("read examples/db/transaction-closure.phg");
    let expected = "after commit: acct1=70 acct2=30\n\
                    total in tx: 100\n\
                    caught UniqueViolationError; transaction rolled back\n\
                    after rollback: acct1=70 acct2=30\n\
                    inner savepoint rolled back: outer continues\n\
                    after nested: acct1=500 acct2=30\n\
                    after retry: acct2=42 (succeeded on attempt 2)\n";
    both(&src, expected);
}

#[test]
fn db_transaction_closure_commits_on_normal_return() {
    // The closure's writes persist after a normal return (COMMIT), and the closure's value is returned.
    let src = tx_program(
        r#"int v = db.transaction(function(): int throws DatabaseError {
             run(db, "UPDATE acct SET bal = 150 WHERE id = 1")?;
             return bal(db)?;
           })?;
       Output.printLine("returned={v} persisted={bal(db)?}");"#,
    );
    both(&src, "returned=150 persisted=150\n");
}

#[test]
fn db_transaction_closure_auto_rolls_back_and_rethrows_the_typed_error() {
    // A throw inside the closure auto-rolls-back AND re-propagates the ORIGINAL typed error — caught as
    // the precise `UniqueViolationError` subtype outside the transaction; the balance change is discarded.
    let src = tx_program(
        r#"try {
         discard db.transaction(function(): int throws DatabaseError {
           run(db, "UPDATE acct SET bal = 999 WHERE id = 1")?;
           run(db, "INSERT INTO acct(id, bal) VALUES(1, 0)")?; // duplicate PK -> UniqueViolationError
           return 0;
         })?;
       } catch (UniqueViolationError e) {
         Output.printLine("rolled back on: {e.message}");
       }
       Output.printLine("bal={bal(db)?}");"#,
    );
    both(
        &src,
        "rolled back on: Core.DatabaseModule: UNIQUE constraint failed: acct.id\nbal=100\n",
    );
}

#[test]
fn db_transaction_closure_nested_is_a_savepoint() {
    // A nested `db.transaction` is a SAVEPOINT: the inner throw rolls back only the inner change; the
    // outer transaction (caught the inner failure) commits its own change. acct stays 1 row (id=1).
    let src = tx_program(
        r#"db.transaction(function(): void throws DatabaseError {
             run(db, "UPDATE acct SET bal = 200 WHERE id = 1")?;
             try {
               db.transaction(function(): void throws DatabaseError {
                 run(db, "UPDATE acct SET bal = 777 WHERE id = 1")?;
                 run(db, "INSERT INTO acct(id, bal) VALUES(1, 0)")?; // dup PK -> throws
               });
             } catch (DatabaseError inner) { Output.printLine("inner rolled back"); }
           })?;
       Output.printLine("bal={bal(db)?}");"#,
    );
    // The inner UPDATE-to-777 is discarded to the savepoint; the outer UPDATE-to-200 survives + commits.
    both(&src, "inner rolled back\nbal=200\n");
}

/// A scaffold with a captured mutable counter and the `SerializationFailureError` import, for the retry
/// tests: `body` runs inside `act(db): void throws DatabaseError`, `tries` is a shared counter object.
fn retry_program(body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
import Core.DatabaseModule.UniqueViolationError;
import Core.DatabaseModule.SerializationFailureError;
class Tries {{ mutable int n; constructor() {{ this.n = 0; }} function bump(): int {{ this.n = this.n + 1; return this.n; }} }}
function bal(Database db): int throws DatabaseError {{
  Statement s = db.prepare("SELECT bal FROM acct WHERE id = 1")?;
  List<Row> rows = s.query()?;
  return rows[0].getInt("bal")?;
}}
function run(Database db, string sql): void throws DatabaseError {{ Statement s = db.prepare(sql)?; discard s.exec()?; }}
function act(Database db, Tries tries): void throws DatabaseError {{
  {body}
}}
function main(): void {{
  try {{
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE acct(id INTEGER PRIMARY KEY, bal INTEGER)").exec();
    discard db.prepare("INSERT INTO acct(id, bal) VALUES(1, 100)").exec();
    act(db, new Tries());
  }} catch (DatabaseError e) {{ Output.printLine("caught: {{e.message}}"); }}
}}
"#
    )
}

#[test]
fn db_transaction_retry_succeeds_after_a_transient_failure() {
    // The closure throws a transient `SerializationFailureError` on the first attempt, then succeeds; with
    // retries=2 the transaction is re-run and the write lands (on the 2nd attempt).
    let src = retry_program(
        r#"db.transaction(function(): void throws DatabaseError {
             int k = tries.bump();
             if (k <= 1) { throw new SerializationFailureError("busy"); }
             run(db, "UPDATE acct SET bal = 42 WHERE id = 1")?;
           }, 2)?;
       Output.printLine("bal={bal(db)?} attempts={tries.n}");"#,
    );
    both(&src, "bal=42 attempts=2\n");
}

#[test]
fn db_transaction_retry_gives_up_after_the_budget_and_propagates() {
    // The closure always throws `SerializationFailureError`; with retries=1 (2 attempts) the budget is
    // exhausted and the LAST transient error propagates (still a catchable `SerializationFailureError`).
    let src = retry_program(
        r#"try {
         db.transaction(function(): void throws DatabaseError {
           discard tries.bump();
           throw new SerializationFailureError("always busy");
         }, 1)?;
       } catch (SerializationFailureError e) {
         Output.printLine("gave up after {tries.n} attempts: {e.message}");
       }"#,
    );
    // A user-thrown SerializationFailureError carries its message verbatim (no `Core.DatabaseModule:` native prefix).
    both(&src, "gave up after 2 attempts: always busy\n");
}

#[test]
fn db_transaction_retry_does_not_retry_a_non_transient_error() {
    // A non-transient `DatabaseError` (a UNIQUE violation) is NOT retried — it rolls back and propagates on
    // the FIRST attempt, even with a generous retry budget. `tries.n` proves exactly one attempt ran.
    let src = retry_program(
        r#"try {
         db.transaction(function(): void throws DatabaseError {
           discard tries.bump();
           run(db, "INSERT INTO acct(id, bal) VALUES(1, 0)")?; // duplicate PK -> UniqueViolationError
         }, 5)?;
       } catch (UniqueViolationError e) {
         Output.printLine("not retried; attempts={tries.n}");
       }
       Output.printLine("bal={bal(db)?}");"#,
    );
    both(&src, "not retried; attempts=1\nbal=100\n");
}

// ── DEC-208 slice D: writes + robustness (lastInsertId / executeMany / bindList / timeout / onQuery) ──

/// The SOLE gate that runs the slice-D write surface (`execReturningId`/`lastInsertId`/`executeMany`/
/// `bindList`/`timeout`/`onQuery`) through the real language on BOTH backends (quarantined from the
/// byte-identity differential like every `Core.DatabaseModule` example). The `onQuery` hook logs only the SQL text
/// (its `ms` is wall-clock → excluded, or `run ≢ runvm`).
#[test]
fn db_writes_example_runs_on_both_backends() {
    let src =
        std::fs::read_to_string("examples/db/writes.phg").expect("read examples/db/writes.phg");
    let expected = concat!(
        "inserted Ada -> id 1\n",
        "inserted Grace -> id 2\n",
        "bulk inserted 3\n",
        "  [query] SELECT name FROM people WHERE id IN (?) ORDER BY id\n",
        "in-list: Ada\n",
        "in-list: Bob\n",
        "in-list: Dan\n",
        "in-list count 3\n",
    );
    both(&src, expected);
}

/// A scaffold: an empty `people(id PK, name, city)` table, then `body` inside a `try/catch(DatabaseError)`.
fn writes_program(body: &str) -> String {
    format!(
        r#"package Main;
import Core.Output;
import Core.List;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
function main(): void {{
  try {{
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE people(id INTEGER PRIMARY KEY, name TEXT, city TEXT)").exec();
    {body}
  }} catch (DatabaseError e) {{ Output.printLine("caught: {{e.message}}"); }}
}}
"#
    )
}

#[test]
fn db_exec_returning_id_and_last_insert_id_agree() {
    // execReturningId returns the new PK; a following lastInsertId reads the most recent one.
    let src = writes_program(
        r#"int a = db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").bind("Ada").bind("Paris").execReturningId();
       discard db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").bind("Bo").bind("X").exec();
       int last = db.lastInsertId();
       Output.printLine("{a}/{last}");"#,
    );
    both(&src, "1/2\n");
}

#[test]
fn db_execute_many_inserts_all_rows() {
    let src = writes_program(
        r#"int n = db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").executeMany([["A", "P"], ["B", "Q"], ["C", "R"]]);
       List<Row> rows = db.prepare("SELECT name FROM people ORDER BY id").query();
       Output.printLine("{n}/{List.length(rows)}");"#,
    );
    both(&src, "3/3\n");
}

#[test]
fn db_execute_many_rolls_back_whole_batch_on_error() {
    // A duplicate PK mid-batch fails; the savepoint rolls back the ENTIRE batch (nothing persists).
    // Rows are homogeneous (all string) — a phorj list literal must share one element type, so a
    // mixed-column bulk row needs per-row typed bindings; here a TEXT primary key gives the collision.
    let src = r#"package Main;
import Core.Output;
import Core.List;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(k TEXT PRIMARY KEY, v TEXT)").exec();
    try {
      discard db.prepare("INSERT INTO t(k, v) VALUES(?, ?)").executeMany([["1", "a"], ["1", "b"]]);
    } catch (DatabaseError e) { Output.printLine("err"); }
    List<Row> rows = db.prepare("SELECT k FROM t").query();
    Output.printLine("rows={List.length(rows)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "err\nrows=0\n");
}

#[test]
fn db_bind_list_expands_and_filters() {
    let src = writes_program(
        r#"discard db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").executeMany([["A", "P"], ["B", "Q"], ["C", "R"], ["D", "S"]]);
       List<Row> rows = db.prepare("SELECT name FROM people WHERE id IN (?) ORDER BY id").bindList([1, 3]).query();
       mutable string acc = "";
       for (Row r in rows) { acc = acc + r.getString("name"); }
       Output.printLine("{acc}");"#,
    );
    both(&src, "AC\n");
}

#[test]
fn db_bind_list_empty_matches_nothing() {
    // An empty IN-list expands to `IN (NULL)` → matches no rows (never a syntax error).
    let src = writes_program(
        r#"discard db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").executeMany([["A", "P"], ["B", "Q"]]);
       List<int> none = new List<int>();
       List<Row> rows = db.prepare("SELECT name FROM people WHERE id IN (?)").bindList(none).query();
       Output.printLine("{List.length(rows)}");"#,
    );
    both(&src, "0\n");
}

#[test]
fn db_bind_list_mixes_with_positional_bind() {
    // `bind()` and `bindList()` interleave: the ? placeholders map left-to-right (bind → city, bindList
    // → the IN-list). Only rows matching BOTH the city bind and the id list are returned.
    let src = writes_program(
        r#"discard db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").executeMany([["A", "P"], ["B", "P"], ["C", "Q"]]);
       List<Row> rows = db.prepare("SELECT name FROM people WHERE city = ? AND id IN (?) ORDER BY id").bind("P").bindList([1, 2, 3]).query();
       mutable string acc = "";
       for (Row r in rows) { acc = acc + r.getString("name"); }
       Output.printLine("{acc}");"#,
    );
    both(&src, "AB\n");
}

#[test]
fn db_on_query_hook_fires_with_sql_and_ms() {
    // The hook fires after each exec/query with the (original) SQL text + an int ms. `ms` is wall-clock
    // so only `ms >= 0` (always true) is printed — printing ms raw would break run ≡ runvm.
    let src = writes_program(
        r#"discard db.onQuery(function(string sql, int ms) => Output.printLine("hook:{sql}:{ms >= 0}"));
       discard db.prepare("INSERT INTO people(name, city) VALUES(?, ?)").bind("A").bind("P").exec();
       List<Row> rows = db.prepare("SELECT name FROM people").query();
       Output.printLine("done {List.length(rows)}");"#,
    );
    both(
        &src,
        "hook:INSERT INTO people(name, city) VALUES(?, ?):true\n\
         hook:SELECT name FROM people:true\n\
         done 1\n",
    );
}

// ── DEC-208 slice E: value mapping (enum / decimal / Json) ───────────────────────────────────────

#[test]
fn db_mapping_example_runs_on_both_backends() {
    let src =
        std::fs::read_to_string("examples/db/mapping.phg").expect("read examples/db/mapping.phg");
    let expected = "Ada: pro credit=19.99 meta=[1,2,3] overdraft=-5.50 extra={\"beta\":true} | billing enterprise 100.00 {\"seats\":9}\n\
                    Bob: free credit=0.10 meta={\"n\":0} overdraft=0.00 extra=- | billing free 0.00 []\n";
    both(&src, expected);
}

/// An enum field maps from a TEXT column by matching the column value against the variant name.
#[test]
fn db_maps_enum_by_variant_name() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
enum Status { Active(), Suspended() }
class Acct { constructor(public string name, public Status status) {} }
function label(Status s): string { return match (s) { Active() => "A", Suspended() => "S" }; }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(name TEXT, status TEXT)").exec();
    discard db.prepare("INSERT INTO t VALUES('Ada', 'Active')").exec();
    discard db.prepare("INSERT INTO t VALUES('Bob', 'Suspended')").exec();
    List<Acct> rows = db.prepare("SELECT name, status FROM t ORDER BY name").queryInto();
    for (Acct r in rows) { Output.printLine("{r.name}={label(r.status)}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada=A\nBob=S\n");
}

/// An unknown column value for an enum field is a catchable `DatabaseError` (strict — no silent coercion).
#[test]
fn db_maps_enum_unknown_value_throws() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
enum Status { Active(), Suspended() }
class Acct { constructor(public string name, public Status status) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(name TEXT, status TEXT)").exec();
    discard db.prepare("INSERT INTO t VALUES('X', 'Bogus')").exec();
    List<Acct> rows = db.prepare("SELECT name, status FROM t").queryInto();
    Output.printLine("{List.length(rows)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule: column `status` value is not a variant of enum `Status`\n",
    );
}

/// An optional enum field (`Status?`) admits a NULL column (→ `null`) and maps a present value.
#[test]
fn db_maps_optional_enum_admits_null() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
enum Status { Active() }
class Acct { constructor(public string name, public Status? status) {} }
function show(Status? s): string { if (var x = s) { return "active"; } return "none"; }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(name TEXT, status TEXT)").exec();
    discard db.prepare("INSERT INTO t VALUES('Ada', 'Active')").exec();
    discard db.prepare("INSERT INTO t VALUES('Bob', NULL)").exec();
    List<Acct> rows = db.prepare("SELECT name, status FROM t ORDER BY name").queryInto();
    for (Acct r in rows) { Output.printLine("{r.name}={show(r.status)}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada=active\nBob=none\n");
}

/// A `decimal` field maps EXACTLY from a TEXT column: `0.1 + 0.2` is exactly `0.3` (a value `float`
/// cannot represent — it would print `0.30000000000000004`).
#[test]
fn db_maps_decimal_exactly() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Money { constructor(public decimal a, public decimal b) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE m(a TEXT, b TEXT)").exec();
    discard db.prepare("INSERT INTO m VALUES('0.1', '0.2')").exec();
    List<Money> ms = db.prepare("SELECT a, b FROM m").queryInto();
    for (Money x in ms) { Output.printLine("{x.a + x.b}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "0.3\n");
}

/// A `decimal` field also maps from INTEGER (exact, scale 0) and REAL (shortest round-trip) columns —
/// the non-TEXT storage classes the task names. (TEXT stays the exact-money path; a REAL column that
/// round-trips to a long decimal is why the convention is "store decimal columns as TEXT".)
#[test]
fn db_maps_decimal_from_integer_and_real_columns() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Nums { constructor(public decimal i, public decimal half, public decimal tenth) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(i INTEGER, half REAL, tenth REAL)").exec();
    discard db.prepare("INSERT INTO t VALUES(42, 0.5, 0.1)").exec();
    List<Nums> rows = db.prepare("SELECT i, half, tenth FROM t").queryInto();
    for (Nums n in rows) { Output.printLine("{n.i} {n.half} {n.tenth}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "42 0.5 0.1\n");
}

/// A NULL column into a non-optional `decimal` field throws; a `decimal?` admits NULL.
#[test]
fn db_maps_decimal_null_into_non_optional_throws() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Money { constructor(public decimal amount) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE m(amount TEXT)").exec();
    discard db.prepare("INSERT INTO m VALUES(NULL)").exec();
    List<Money> ms = db.prepare("SELECT amount FROM m").queryInto();
    Output.printLine("{List.length(ms)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule.getDecimal: column `amount` is NULL (use decimal?)\n",
    );
}

/// A non-decimal TEXT value for a `decimal` field is a catchable `DatabaseError`.
#[test]
fn db_maps_decimal_invalid_text_throws() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Money { constructor(public decimal amount) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE m(amount TEXT)").exec();
    discard db.prepare("INSERT INTO m VALUES('not-a-number')").exec();
    List<Money> ms = db.prepare("SELECT amount FROM m").queryInto();
    Output.printLine("{List.length(ms)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule.getDecimal: column `amount` value `not-a-number` is not a valid decimal\n",
    );
}

/// A `Json` field is parsed from a TEXT column via `Core.Json`; a `Json?` admits NULL.
#[test]
fn db_maps_json_and_optional_admits_null() {
    let src = r#"package Main;
import Core.Output;
import Core.Json;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Doc { constructor(public Json body, public Json? note) {} }
function showNote(Json? j): string { if (var x = j) { return Json.stringify(x); } return "-"; }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE d(body TEXT, note TEXT)").exec();
    discard db.prepare("INSERT INTO d VALUES('[1,2]', '\{\"n\":1\}')").exec();
    discard db.prepare("INSERT INTO d VALUES('\{\"k\":true\}', NULL)").exec();
    List<Doc> ds = db.prepare("SELECT body, note FROM d").queryInto();
    for (Doc x in ds) { Output.printLine("{Json.stringify(x.body)} / {showNote(x.note)}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "[1,2] / {\"n\":1}\n{\"k\":true} / -\n");
}

/// Invalid JSON text for a `Json` field is a catchable `DatabaseError`.
#[test]
fn db_maps_invalid_json_throws() {
    let src = r#"package Main;
import Core.Output;
import Core.Json;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Doc { constructor(public Json body) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE d(body TEXT)").exec();
    discard db.prepare("INSERT INTO d VALUES('not json')").exec();
    List<Doc> ds = db.prepare("SELECT body FROM d").queryInto();
    Output.printLine("{List.length(ds)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule: column `body` does not contain valid JSON\n",
    );
}

/// Value mapping COMPOSES with nested hydration: a nested entity's enum + decimal + Json fields are
/// hydrated from dotted `"inner.*"` columns in the same query.
#[test]
fn db_maps_enum_decimal_json_inside_nested_entity() {
    let src = r#"package Main;
import Core.Output;
import Core.Json;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
enum Tier { Gold(), Silver() }
class Wallet { constructor(public Tier tier, public decimal balance, public Json flags) {} }
class User { constructor(public string name, public Wallet wallet) {} }
function tierName(Tier t): string { return match (t) { Gold() => "gold", Silver() => "silver" }; }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE u(name TEXT, tier TEXT, balance TEXT, flags TEXT)").exec();
    discard db.prepare("INSERT INTO u VALUES('Ada', 'Gold', '12.50', '[true]')").exec();
    List<User> us = db.prepare("SELECT name, tier AS \"wallet.tier\", balance AS \"wallet.balance\", flags AS \"wallet.flags\" FROM u").queryInto();
    for (User x in us) { Output.printLine("{x.name}: {tierName(x.wallet.tier)} {x.wallet.balance} {Json.stringify(x.wallet.flags)}"); }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada: gold 12.50 [true]\n");
}

/// An enum with a data-carrying variant cannot be mapped from a single column — a compile error, not a
/// silent mismap (only ZERO-payload variants are supported).
#[test]
fn db_maps_enum_with_payload_variant_is_rejected() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
enum Shape { Circle(float radius), Square() }
class Row4 { constructor(public string name, public Shape shape) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    List<Row4> rows = db.prepare("SELECT name, shape FROM t").queryInto();
    Output.printLine("{List.length(rows)}");
  } catch (DatabaseError e) { Output.printLine("caught"); }
}
"#;
    fails_with(src, "E-DB-HYDRATE-ENUM-PAYLOAD");
}

// ── DEC-208 slice A wiring — explicit turbofish on the query…() family ───────────────────────────────

/// `var users = stmt.queryInto<User>();` — the turbofish IS the sink type; no annotation needed.
#[test]
fn db_query_into_turbofish_with_var_binding() {
    let src = typed_program(
        r#"var users = db.prepare("SELECT name, age FROM users ORDER BY age").queryInto<User>();
       for (User u in users) { Output.printLine("{u.name}/{u.age}"); }"#,
    );
    both(&src, "Ada/36\nGrace/45\n");
}

/// `var one = stmt.queryOneInto<User>();` — turbofish makes the sink `User?`.
#[test]
fn db_query_one_into_turbofish_with_var_binding() {
    let src = typed_program(
        r#"var one = db.prepare("SELECT name, age FROM users WHERE name = ?").bind("Ada").queryOneInto<User>();
       Output.printLine("{one?.name ?? "<none>"}");"#,
    );
    both(&src, "Ada\n");
}

/// `var n = stmt.queryScalar<int>();` — the turbofish is the scalar type itself.
#[test]
fn db_query_scalar_turbofish_with_var_binding() {
    let src = typed_program(
        r#"var n = db.prepare("SELECT COUNT(*) FROM users").queryScalar<int>();
       Output.printLine("{n}");"#,
    );
    both(&src, "2\n");
}

/// `var byName = stmt.queryMap<string, User>();` — two explicit type arguments.
#[test]
fn db_query_map_turbofish_with_var_binding() {
    let src = typed_program(
        r#"var byName = db.prepare("SELECT name, name, age FROM users ORDER BY age").queryMap<string, User>();
       Output.printLine("{Map.get(byName, "Grace")?.age ?? -1}");"#,
    );
    both(&src, "45\n");
}

/// The turbofish threads through `?`-propagation exactly like the contextual form.
#[test]
fn db_query_into_turbofish_propagates_with_question_mark() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.DatabaseError;
class User { constructor(public string name, public int age) {} }
function loadAll(Statement s): List<User> throws DatabaseError {
  var u = s.queryInto<User>()?;
  return u;
}
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE users(name TEXT, age INTEGER)").exec();
    discard db.prepare("INSERT INTO users(name, age) VALUES('Ada', 36)").exec();
    for (User u in loadAll(db.prepare("SELECT name, age FROM users"))) {
      Output.printLine("{u.name}/{u.age}");
    }
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(src, "Ada/36\n");
}

/// Wrong turbofish arity is a compile-time reject (`E-TYPE-ARG-COUNT`), diagnosed by the desugar pass
/// (it consumes the call pre-check, so the generic checker never sees these type arguments).
#[test]
fn db_query_map_turbofish_wrong_arity_is_rejected() {
    let src = typed_program(
        r#"var m = db.prepare("SELECT name, age FROM users").queryMap<int>();
       Output.printLine("x");"#,
    );
    fails_with(&src, "E-TYPE-ARG-COUNT");
}

/// An explicit turbofish WINS over a disagreeing annotation — the helper's typed return then fails the
/// binding like any ordinary assignment type error (explicit > contextual, never a silent pick).
#[test]
fn db_query_into_turbofish_disagreeing_annotation_is_a_type_error() {
    let src = typed_program(
        r#"List<int> users = db.prepare("SELECT name, age FROM users").queryInto<User>();
       Output.printLine("{List.length(users)}");"#,
    );
    fails_with(&src, "List<User>");
}

/// THE LADDER RULE: `Core.DatabaseModule` is native-only — transpiling a program that imports it is a clean,
/// module-specific hard error (`E-TRANSPILE-DB`), never a wall of prelude-internal unknown-ident
/// errors and never a silently-diverging PHP program.
#[test]
fn db_program_transpile_is_a_clean_ladder_error() {
    let src = typed_program(r#"Output.printLine("unreachable");"#);
    match phorj::cli::cmd_transpile(&src) {
        Ok(php) => panic!("expected E-TRANSPILE-DB, but transpile succeeded: {php:?}"),
        Err(e) => {
            assert!(
                e.contains("E-TRANSPILE-DB"),
                "error {e:?} lacks E-TRANSPILE-DB"
            );
            assert!(
                !e.contains("E-UNKNOWN-IDENT"),
                "ladder error must not be the unknown-ident wall: {e:?}"
            );
        }
    }
}

/// THE LADDER RULE, raw-native leg (DEC-277 build): importing the RAW `Core.Native.Database`
/// module directly must hit the same gate — several of its `php` emitters are placeholders, so a
/// transpile that slipped past would be a silently-diverging PHP program, not a refusal.
#[test]
fn raw_native_database_import_transpile_is_a_clean_ladder_error() {
    let src = "package Main;\nimport Core.Output;\nimport Core.Native.Database;\n\
        function main(): void { Output.printLine(\"unreachable\"); }\n";
    match phorj::cli::cmd_transpile(src) {
        Ok(php) => panic!("expected E-TRANSPILE-DB, but transpile succeeded: {php:?}"),
        Err(e) => assert!(
            e.contains("E-TRANSPILE-DB"),
            "error {e:?} lacks E-TRANSPILE-DB"
        ),
    }
}

// ── DEC-208 item H: streaming (`stream()` / `streamInto<T>()`) ───────────────────────────────────────

/// Untyped streaming (DEC-257 reshape): `stmt.stream()` → `RowStream`, an `Iterator<Row>` —
/// foreach pulls row-at-a-time via the lowered hasNext/next while-pull.
#[test]
fn db_stream_delivers_rows_one_at_a_time() {
    let src = typed_program(
        r#"for (Row row in db.prepare("SELECT name, age FROM users ORDER BY age").stream()) {
         Output.printLine("{row.getString("name")}/{row.getInt("age")}");
       }"#,
    );
    // typed_program does not import Row — extend the scaffold inline instead.
    let src = src.replace(
        "import Core.DatabaseModule.DatabaseError;",
        "import Core.DatabaseModule.DatabaseError;\nimport Core.DatabaseModule.Row;",
    );
    both(&src, "Ada/36\nGrace/45\n");
}

/// Typed lazy streaming with a turbofish (DEC-257 reshape): `stmt.streamInto<User>()` is an
/// `Iterator<User>` — foreach hydrates one row per pull.
#[test]
fn db_stream_into_turbofish_hydrates_per_row() {
    let src = typed_program(
        r#"for (User user in db.prepare("SELECT name, age FROM users ORDER BY age").streamInto<User>()) {
         Output.printLine("{user.name}/{user.age}");
       }"#,
    );
    both(&src, "Ada/36\nGrace/45\n");
}

/// Contextual sink form: `DatabaseStream<User> s = stmt.streamInto();`.
#[test]
fn db_stream_into_contextual_sink() {
    let src = typed_program(
        r#"DatabaseStream<User> s = db.prepare("SELECT name, age FROM users ORDER BY age").streamInto();
       User first = s.next();
       Output.printLine("{first.name}");"#,
    );
    let src = src.replace(
        "import Core.DatabaseModule.DatabaseError;",
        "import Core.DatabaseModule.DatabaseError;\nimport Core.DatabaseModule.DatabaseStream;",
    );
    both(&src, "Ada\n");
}

/// LAZINESS PROOF: hydration runs per PULLED row. The second row would throw on hydration (NULL age
/// into a non-optional `int`), but pulling only the first row never hydrates it — no error. The same
/// query through `queryInto()` (eager) throws.
#[test]
fn db_stream_into_hydrates_lazily_early_exit_skips_bad_rows() {
    let src = typed_program(
        r#"discard db.prepare("INSERT INTO users(name, age) VALUES('Broken', NULL)").exec();
       var s = db.prepare("SELECT name, age FROM users ORDER BY age NULLS LAST").streamInto<User>();
       User first = s.next();
       Output.printLine("pulled: {first.name}");"#,
    );
    both(&src, "pulled: Ada\n");
}

/// DEC-257 misuse contract: pulling `next()` past exhaustion FAULTS ("iterator exhausted"),
/// identically on both backends. foreach never triggers this (the lowering always asks hasNext).
#[test]
fn db_stream_next_past_end_faults_iterator_exhausted() {
    let src = typed_program(
        r#"var s = db.prepare("SELECT name, age FROM users WHERE age < 0").stream();
       Row r = s.next();
       Output.printLine("unreachable");"#,
    );
    let src = src.replace(
        "import Core.DatabaseModule.DatabaseError;",
        "import Core.DatabaseModule.DatabaseError;\nimport Core.DatabaseModule.Row;",
    );
    let tree = cmd_treewalk(&src).expect_err("interpreter faults past the end");
    assert!(tree.contains("iterator exhausted"), "{tree}");
    let vm = cmd_run(&src).expect_err("vm faults past the end");
    assert!(vm.contains("iterator exhausted"), "{vm}");
}

/// The same bad row through the EAGER `queryInto()` throws — the contrast half of the laziness proof.
#[test]
fn db_query_into_eager_hydration_hits_the_bad_row() {
    let src = typed_program(
        r#"discard db.prepare("INSERT INTO users(name, age) VALUES('Broken', NULL)").exec();
       List<User> all = db.prepare("SELECT name, age FROM users ORDER BY age NULLS LAST").queryInto();
       Output.printLine("{List.length(all)}");"#,
    );
    both(
        &src,
        "caught: Core.DatabaseModule.getInt: column `age` is NULL (use int?)\n",
    );
}

/// `streamInto` wrong turbofish arity → the same `E-TYPE-ARG-COUNT` as the other query calls.
#[test]
fn db_stream_into_wrong_arity_is_rejected() {
    let src = typed_program(
        r#"var s = db.prepare("SELECT name, age FROM users").streamInto<string, int>();
       Output.printLine("x");"#,
    );
    fails_with(&src, "E-TYPE-ARG-COUNT");
}

/// A non-`DatabaseStream` sink for `streamInto()` is a clean bad-sink error.
#[test]
fn db_stream_into_bad_sink_is_rejected() {
    let src = typed_program(
        r#"List<User> s = db.prepare("SELECT name, age FROM users").streamInto();
       Output.printLine("x");"#,
    );
    fails_with(&src, "E-DB-INTO-BAD-SINK");
}

// ── DEC-208 slice K: typed array-column accessors + `List<scalar>` hydration fields ─────────────────

/// The array accessors are wired end-to-end: on SQLite (no array columns) a `getStringList` on a text
/// column is the clean cross-driver "not an array" DatabaseError — proving the prelude method, native, and
/// error path; the POSITIVE mapping (a real `text[]` → `List<string>`) is exercised by the live
/// Postgres round-trip (`tests/db_postgres.rs`) where arrays exist.
#[test]
fn db_get_string_list_on_non_array_column_is_clean_error() {
    let src = typed_program(
        r#"List<Row> rows = db.prepare("SELECT name FROM users WHERE name = 'Ada'").query();
       for (Row r in rows) { discard r.getStringList("name"); }
       Output.printLine("unreachable");"#,
    );
    let src = src.replace(
        "import Core.DatabaseModule.DatabaseError;",
        "import Core.DatabaseModule.DatabaseError;\nimport Core.DatabaseModule.Row;",
    );
    both(
        &src,
        "caught: Core.DatabaseModule.getStringList: column `name` is string, not an array\n",
    );
}

/// A `List<string>` hydration FIELD routes through `getStringList` (slice K wiring through
/// `accessor_for`): on SQLite this surfaces as the same clean "not an array" error, proving the
/// desugar generated the array accessor for the list-typed field.
#[test]
fn db_query_into_list_field_routes_to_array_accessor() {
    let src = r#"package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.DatabaseError;
class Tagged { constructor(public string name, public List<string> tags) {} }
function main(): void {
  try {
    Database db = new Database("sqlite::memory:");
    discard db.prepare("CREATE TABLE t(name TEXT, tags TEXT)").exec();
    discard db.prepare("INSERT INTO t VALUES('Ada', 'x,y')").exec();
    List<Tagged> rows = db.prepare("SELECT name, tags FROM t").queryInto();
    Output.printLine("{List.length(rows)}");
  } catch (DatabaseError e) { Output.printLine("caught: {e.message}"); }
}
"#;
    both(
        src,
        "caught: Core.DatabaseModule.getStringList: column `tags` is string, not an array\n",
    );
}
