#![cfg(feature = "database-mysql")]
//! `Core.Database` MySQL/MariaDB driver (DEC-208 slice J) — LIVE round-trip, gated on a reachable server.
//!
//! A real MySQL round-trip needs a live server, which the build environment does not always have. So
//! this test is OPT-IN via the `PHORJ_MYSQL_TEST_DSN` env var (the `db_postgres` discipline): unset →
//! the test SKIPS LOUDLY (prints how to enable it) and passes, so the standard gate never requires a
//! live MySQL. Set it to a DSN and the full round-trip runs and is asserted on BOTH backends
//! (`interp ≡ VM`) — e.g.
//!
//! ```text
//! PHORJ_MYSQL_TEST_DSN='mysql://developer:developer@localhost:42708/testx' \
//!   cargo test --features database-mysql --test db_mysql
//! ```
//!
//! The deterministic, server-free coverage of the driver — placeholder handling (`?` pass-through +
//! `IN (?)` expansion, `:name`→`?` translation), error-code→taxonomy mapping, cell mapping (ints,
//! floats, DECIMAL-as-text, TEXT vs BINARY blobs, temporal steering), and credential redaction —
//! lives in the `src/ext/database/mysql.rs` unit tests, which DO run in every `--features database-mysql`
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
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.Database;
import Core.Database.Connection;
import Core.Database.Statement;
import Core.Database.Row;
import Core.Database.DatabaseError;
import Core.Database.UniqueViolationError;

#[Entry(kind: EntryKind.Cli)] function main(): void {{
    try {{
        Connection db = new Connection("{dsn}");
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
             in the src/ext/database/mysql.rs unit tests regardless."
        );
        return;
    };
    let src = program(&dsn);
    let expected = "1=Ada/12.50\n2=Grace/0.00\nreturning=3\nunique-violation\nbulk=2\ntx=7\n";
    let tree = cmd_treewalk(&src).expect("mysql round-trip runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    // interp ≡ VM: the VM must produce byte-identical stdout.
    assert_eq!(
        cmd_run(&src).expect("mysql round-trip runs on the VM"),
        tree,
        "interp ≡ VM"
    );
}

/// DEC-351's D5 fold-in — NESTED savepoints against a live MySQL/MariaDB.
///
/// The nested `commit`/`rollback` path had ZERO coverage on MySQL and Postgres, and the SQL it emitted
/// had drifted into two non-portable forms (a bare `RELEASE`, and a `;`-joined `ROLLBACK TO … ; RELEASE …`
/// pair through the single-statement `control`). MySQL is the backend that REJECTS both, so this is the
/// file where the bug would have surfaced first — it simply had no nested-savepoint test to surface it.
/// `savepoint.rs` now single-sources the portable spellings; this proves they compose on the wire.
///
/// Its own throwaway table (`phorj_my_sp`), synthetic data only.
fn savepoint_program(dsn: &str) -> String {
    format!(
        r#"
package Main;
import Core.Runtime.Entry; import Core.Runtime.EntryKind;
import Core.Output;
import Core.Database;
import Core.Database.Connection;
import Core.Database.Row;
import Core.Database.DatabaseError;

function bal(Connection db): int throws DatabaseError {{
    List<Row> rows = db.prepare("SELECT bal FROM phorj_my_sp WHERE id = 1")?.query()?;
    return rows[0].getInt("bal");
}}

function put(Connection db, int n): void throws DatabaseError {{
    discard db.prepare("UPDATE phorj_my_sp SET bal = ? WHERE id = 1")?.bind(n)?.exec()?;
}}

#[Entry(kind: EntryKind.Cli)] function main(): void {{
    try {{
        Connection db = new Connection("{dsn}");
        discard db.prepare("DROP TABLE IF EXISTS phorj_my_sp").exec();
        discard db.prepare("CREATE TABLE phorj_my_sp(id INT PRIMARY KEY, bal INT) ENGINE=InnoDB").exec();
        discard db.prepare("INSERT INTO phorj_my_sp(id, bal) VALUES(1, 100)").exec();

        // A nested ROLLBACK: `ROLLBACK TO SAVEPOINT` + `RELEASE SAVEPOINT`, two statements.
        db.begin();
        put(db, 200);
        db.begin();
        put(db, 300);
        db.rollback();
        Output.printLine("after-inner-rollback={{bal(db)}} depth={{db.transactionDepth()}}");
        db.commit();
        Output.printLine("after-outer-commit={{bal(db)}} depth={{db.transactionDepth()}}");

        // A nested COMMIT: `RELEASE SAVEPOINT` — the form MySQL rejects when the keyword is missing.
        db.begin();
        put(db, 400);
        db.begin();
        put(db, 500);
        db.commit();
        Output.printLine("after-inner-commit={{bal(db)}} depth={{db.transactionDepth()}}");
        db.rollback();
        Output.printLine("after-outer-rollback={{bal(db)}} depth={{db.transactionDepth()}}");

        // Three levels, unwound in one call (each level a `ROLLBACK TO`+`RELEASE` pair).
        db.begin(); put(db, 1);
        db.begin(); put(db, 2);
        db.begin(); put(db, 3);
        db.rollbackAll();
        Output.printLine("after-rollback-all={{bal(db)}} depth={{db.transactionDepth()}}");

        discard db.prepare("DROP TABLE phorj_my_sp").exec();
        db.close();
    }} catch (DatabaseError e) {{
        Output.printLine("unexpected: {{e.message}}");
    }}
}}
"#
    )
}

#[test]
fn nested_savepoints_compose_on_a_live_mysql() {
    let Ok(dsn) = std::env::var("PHORJ_MYSQL_TEST_DSN") else {
        eprintln!(
            "db_mysql: SKIP (nested savepoints) — set PHORJ_MYSQL_TEST_DSN to a live MySQL/MariaDB DSN. \
             The server-free half of this coverage (the portable-form ratchet over every emitter) runs \
             in the src/ext/database/natives/savepoint.rs unit tests regardless."
        );
        return;
    };
    let src = savepoint_program(&dsn);
    let expected = "after-inner-rollback=200 depth=1\n\
                    after-outer-commit=200 depth=0\n\
                    after-inner-commit=500 depth=1\n\
                    after-outer-rollback=200 depth=0\n\
                    after-rollback-all=200 depth=0\n";
    let tree = cmd_treewalk(&src).expect("nested savepoints run on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    assert_eq!(
        cmd_run(&src).expect("nested savepoints run on the VM"),
        tree,
        "interp ≡ VM"
    );
}
