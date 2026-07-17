//! Checker tests — DEC-208 slice F, the `W-SQL-INJECTION` compile-time lint. The lint is
//! type-directed: it fires only on `Core.DatabaseModule`'s `Database.prepare(<interpolated SQL>)` when a hole splices a
//! NON-constant value into the SQL text, steering the developer to a `?` placeholder + `.bind(...)`.
//! It is a non-fatal lint (rides the warning channel, never fails the build).
//!
//! Like the `W-SECRET` tests these declare the `Database`/`Statement` shape inline — `check()` never injects
//! the `Core.DatabaseModule` prelude, and the lint keys on the class name `Database` + the method `prepare` + an active
//! `Core.DatabaseModule` import (the "nothing in the wind" gate), all of which a stub satisfies.

use super::support::*;

/// A minimal `Core.DatabaseModule`-shaped stub: a `Database` with `prepare` (the linted method) and a neighbouring
/// method `other` (to prove the lint is prepare-specific), plus an opaque `Statement`.
const DB_STUB: &str = "class Statement {} \
    class Database { \
        constructor(string dsn) {} \
        function prepare(string sql): Statement { return new Statement(); } \
        function other(string sql): void {} \
    }";

/// Build a program that imports `Core.DatabaseModule` (satisfying the lint's import gate) and declares the stub.
fn with_import(body: &str) -> String {
    format!("import Core.DatabaseModule; {DB_STUB} {body}")
}

fn warns_sql(src: &str) -> bool {
    warnings_of(src)
        .iter()
        .any(|w| w.code == Some("W-SQL-INJECTION"))
}

#[test]
fn interpolated_variable_into_prepare_warns() {
    // A local variable spliced into the SQL text — the classic injection risk.
    let src = with_import(
        "function main(): void { \
             Database db = new Database(\"sqlite::memory:\"); \
             int id = 5; \
             Statement s = db.prepare(\"SELECT * FROM users WHERE id = {id}\"); \
         }",
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        warns_sql(&src),
        "expected W-SQL-INJECTION, got {:?}",
        warnings_of(&src)
    );
}

#[test]
fn placeholder_literal_is_clean() {
    // A `?` placeholder (no interpolation) is the correct pattern — never warns.
    let src = with_import(
        "function main(): void { \
             Database db = new Database(\"sqlite::memory:\"); \
             Statement s = db.prepare(\"SELECT * FROM users WHERE id = ?\"); \
         }",
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        !warns_sql(&src),
        "unexpected W-SQL-INJECTION, got {:?}",
        warnings_of(&src)
    );
}

#[test]
fn constant_only_interpolation_is_clean() {
    // Every hole is a literal constant — static SQL, no user data, no warning.
    let src = with_import(
        "function main(): void { \
             Database db = new Database(\"sqlite::memory:\"); \
             Statement s = db.prepare(\"SELECT * FROM t LIMIT {10} OFFSET {0}\"); \
         }",
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        !warns_sql(&src),
        "unexpected W-SQL-INJECTION, got {:?}",
        warnings_of(&src)
    );
}

#[test]
fn plain_literal_is_clean() {
    // No interpolation at all — never warns.
    let src = with_import(
        "function main(): void { \
             Database db = new Database(\"sqlite::memory:\"); \
             Statement s = db.prepare(\"SELECT 1\"); \
         }",
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        !warns_sql(&src),
        "unexpected W-SQL-INJECTION, got {:?}",
        warnings_of(&src)
    );
}

#[test]
fn mixed_constant_and_variable_holes_warn() {
    // One constant hole and one variable hole — the presence of ANY non-constant hole warns.
    let src = with_import(
        "function main(): void { \
             Database db = new Database(\"sqlite::memory:\"); \
             string name = \"ada\"; \
             Statement s = db.prepare(\"SELECT * FROM t LIMIT {10} WHERE n = {name}\"); \
         }",
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        warns_sql(&src),
        "expected W-SQL-INJECTION, got {:?}",
        warnings_of(&src)
    );
}

#[test]
fn without_core_db_import_a_user_db_class_never_warns() {
    // Import gate ("nothing in the wind"): a user class coincidentally named `Database` with a `prepare`
    // method is NOT the Core.DatabaseModule `Database` — no import, no lint, even with an interpolated variable.
    let src = format!(
        "{DB_STUB} function main(): void {{ \
             Database db = new Database(\"x\"); \
             int id = 5; \
             Statement s = db.prepare(\"SELECT * FROM users WHERE id = {{id}}\"); \
         }}"
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        !warns_sql(&src),
        "unexpected W-SQL-INJECTION on a non-Core.DatabaseModule class, got {:?}",
        warnings_of(&src)
    );
}

#[test]
fn interpolation_into_a_non_prepare_method_never_warns() {
    // Type-directed to `prepare` specifically — a different `Database` method with an interpolated argument
    // is not a prepared statement, so the lint does not fire.
    let src = with_import(
        "function main(): void { \
             Database db = new Database(\"sqlite::memory:\"); \
             int id = 5; \
             db.other(\"SELECT * FROM users WHERE id = {id}\"); \
         }",
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
    assert!(
        !warns_sql(&src),
        "unexpected W-SQL-INJECTION on a non-prepare method, got {:?}",
        warnings_of(&src)
    );
}
