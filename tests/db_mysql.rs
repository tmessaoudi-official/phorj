#![cfg(feature = "db-mysql")]
//! `Core.DatabaseModule` MySQL/MariaDB driver (DEC-208 slice J) — LIVE round-trip, gated on a reachable server.
//!
//! A real MySQL round-trip needs a live server, which the build environment does not always have. So
//! this test is OPT-IN via the `PHORJ_MYSQL_TEST_DSN` env var (the `db_postgres` discipline): unset →
//! the test SKIPS LOUDLY (prints how to enable it) and passes, so the standard gate never requires a
//! live MySQL. Set it to a DSN and the full round-trip runs and is asserted on BOTH backends
//! (`run ≡ runvm`) — e.g.
//!
//! ```text
//! PHORJ_MYSQL_TEST_DSN='mysql://developer:developer@localhost:42708/testx' \
//!   cargo test --features db-mysql --test db_mysql
//! ```
//!
//! The deterministic, server-free coverage of the driver — placeholder handling (`?` pass-through +
//! `IN (?)` expansion, `:name`→`?` translation), error-code→taxonomy mapping, cell mapping (ints,
//! floats, DECIMAL-as-text, TEXT vs BINARY blobs, temporal steering), and credential redaction —
//! lives in the `src/native/db/mysql.rs` unit tests, which DO run in every `--features db-mysql`
//! gate. This file proves the wire path end-to-end when a server exists.
//!
//! The test uses only its own throwaway table (`phorj_my_it`, dropped at start and end) with
//! synthetic data — it never reads or touches any application schema in the target database.

use phorj::cli::{cmd_run, cmd_treewalk};

/// Build the round-trip program with `dsn` spliced in. Exercises: throwing connect; DDL via `exec`;
/// positional (`?`) + named (`:n`) binds; `query` with value mapping (int/text/DECIMAL-as-text);
/// `execReturningId` via the connection's `last_insert_id` (MySQL has no RETURNING — the SQLite-shaped
/// path); the typed `UniqueViolationError` taxonomy from MySQL error 1062; and `executeMany`'s
/// MySQL-divergent depth-0 `BEGIN` path.
fn program(dsn: &str) -> String {
    format!(
        r#"
package Main;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
import Core.DatabaseModule.UniqueViolationError;

function main(): void {{
    try {{
        Database db = new Database("{dsn}");
        discard db.prepare("DROP TABLE IF EXISTS phorj_my_it").exec();
        discard db.prepare("CREATE TABLE phorj_my_it(id INT AUTO_INCREMENT PRIMARY KEY, name TEXT, amount DECIMAL(10,2))").exec();

        // Positional binds (`?` native to MySQL).
        discard db
            .prepare("INSERT INTO phorj_my_it(id, name, amount) VALUES(?, ?, ?)")
            .bind(1)
            .bind("Ada")
            .bind("12.50")
            .exec();
        // Named binds (`:id`/`:name` -> `?` translation).
        discard db
            .prepare("INSERT INTO phorj_my_it(id, name, amount) VALUES(:id, :name, NULL)")
            .bindNamed("id", 2)
            .bindNamed("name", "Grace")
            .exec();

        // Query back, ordered — value mapping int/text + DECIMAL arriving as exact decimal text.
        List<Row> rows = db
            .prepare("SELECT id, name, amount FROM phorj_my_it WHERE id > ? ORDER BY id")
            .bind(0)
            .query();
        for (Row r in rows) {{
            int id = r.getInt("id");
            string name = r.getString("name");
            decimal? amount = r.getDecimalOrNull("amount");
            Output.printLine("{{id}}={{name}}/{{amount ?? 0.00d}}");
        }}

        // AUTO_INCREMENT id via the connection's last_insert_id (no RETURNING in MySQL).
        int newId = db
            .prepare("INSERT INTO phorj_my_it(name, amount) VALUES('Lin', NULL)")
            .execReturningId();
        Output.printLine("returning={{newId}}");

        // A duplicate PK -> MySQL 1062 -> the typed UniqueViolationError subtype.
        try {{
            discard db.prepare("INSERT INTO phorj_my_it(id, name, amount) VALUES(1, 'dup', NULL)").exec();
            Output.printLine("no-dup-error");
        }} catch (UniqueViolationError e) {{
            Output.printLine("unique-violation");
        }}

        // executeMany — the MySQL-divergent path: at tx-depth 0 it opens its OWN `BEGIN`/`COMMIT`
        // (a standalone SAVEPOINT is rejected under autocommit, unlike SQLite).
        int bulk = db
            .prepare("INSERT INTO phorj_my_it(id, name, amount) VALUES(?, 'bulk', NULL)")
            .executeMany([[20], [21]]);
        Output.printLine("bulk={{bulk}}");

        // A closure transaction — BEGIN/COMMIT through the portable control SQL, returning a value.
        int tx = db.transaction(function(): int throws DatabaseError {{
            Statement s = db.prepare("UPDATE phorj_my_it SET name = 'upd' WHERE id = 1")?;
            discard s.exec()?;
            return 7;
        }});
        Output.printLine("tx={{tx}}");

        discard db.prepare("DROP TABLE phorj_my_it").exec();
        db.close();
    }} catch (DatabaseError e) {{
        Output.printLine("unexpected: {{e.message}}");
    }}
}}
"#
    )
}

#[test]
fn mysql_round_trip_on_both_backends() {
    let Ok(dsn) = std::env::var("PHORJ_MYSQL_TEST_DSN") else {
        eprintln!(
            "db_mysql: SKIP — set PHORJ_MYSQL_TEST_DSN to a live MySQL/MariaDB DSN to run the \
             round-trip (e.g. mysql://user:pw@host:3306/db). The deterministic driver coverage runs \
             in the src/native/db/mysql.rs unit tests regardless."
        );
        return;
    };
    let src = program(&dsn);
    let expected = "1=Ada/12.50\n2=Grace/0.00\nreturning=3\nunique-violation\nbulk=2\ntx=7\n";
    let tree = cmd_treewalk(&src).expect("mysql round-trip runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    // run ≡ runvm: the VM must produce byte-identical stdout.
    assert_eq!(
        cmd_run(&src).expect("mysql round-trip runs on the VM"),
        tree,
        "run ≡ runvm"
    );
}
