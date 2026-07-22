#![cfg(feature = "database-postgres")]
//! `Core.DatabaseModule` Postgres driver (DEC-208 slice I) — LIVE round-trip, gated on a reachable server.
//!
//! A real Postgres round-trip needs a live server, which the build environment does not always have. So
//! this test is OPT-IN via the `PHORJ_PG_TEST_DSN` env var (the same discipline as the PHP oracle's
//! `PHORJ_REQUIRE_PHP`): unset → the test SKIPS LOUDLY (prints how to enable it) and passes, so the
//! standard gate never requires a live Postgres. Set it to a DSN and the full round-trip runs and is
//! asserted on BOTH backends (`interp ≡ VM`) — e.g.
//!
//! ```text
//! PHORJ_PG_TEST_DSN='postgres://developer:developer@localhost:42710/testx' \
//!   cargo test --features database-postgres --test db_postgres
//! ```
//!
//! The deterministic, server-free coverage of the driver — DSN dispatch, `?`/`:name`→`$n` translation,
//! SQLSTATE→taxonomy mapping, and credential redaction — lives in the `src/ext/database/postgres.rs` unit
//! tests, which DO run in every gate. This file proves the wire path end-to-end when a server exists.
//!
//! The test uses only its own throwaway table (`phorj_pg_it`, dropped at start and end) with synthetic
//! data — it never reads or touches any application schema in the target database.

use phorj::cli::{cmd_run, cmd_treewalk};

/// Build the round-trip program with `dsn` spliced in. Exercises: throwing connect; DDL via `exec`;
/// positional (`?`) + named (`:n`) binds; `query` with value mapping (int4/text/bool); `execReturningId`
/// via a `RETURNING` clause; and the typed `UniqueViolationError` taxonomy from a PG `23505` SQLSTATE.
fn program(dsn: &str) -> String {
    format!(
        r#"
package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.DatabaseModule;
import Core.DatabaseModule.Database;
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
import Core.DatabaseModule.UniqueViolationError;

#[Entry] function main(): void {{
    try {{
        Database db = new Database("{dsn}");
        discard db.prepare("DROP TABLE IF EXISTS phorj_pg_it").exec();
        discard db.prepare("CREATE TABLE phorj_pg_it(id INT PRIMARY KEY, name TEXT, active BOOLEAN)").exec();

        // Positional binds (`?` -> `$1,$2,$3`).
        discard db
            .prepare("INSERT INTO phorj_pg_it(id, name, active) VALUES(?, ?, ?)")
            .bind(1)
            .bind("Ada")
            .bind(true)
            .exec();
        // Named binds (`:id`/`:name` -> `$1,$2`).
        discard db
            .prepare("INSERT INTO phorj_pg_it(id, name, active) VALUES(:id, :name, false)")
            .bindNamed("id", 2)
            .bindNamed("name", "Grace")
            .exec();

        // Query back, ordered — value mapping int4/text.
        List<Row> rows = db
            .prepare("SELECT id, name FROM phorj_pg_it WHERE id > ? ORDER BY id")
            .bind(0)
            .query();
        for (Row r in rows) {{
            int id = r.getInt("id");
            string name = r.getString("name");
            Output.printLine("{{id}}={{name}}");
        }}

        // RETURNING id -> execReturningId reads the first returned column.
        int newId = db
            .prepare("INSERT INTO phorj_pg_it(id, name, active) VALUES(3, 'Lin', true) RETURNING id")
            .execReturningId();
        Output.printLine("returning={{newId}}");

        // A duplicate PK -> PG 23505 -> the typed UniqueViolationError subtype.
        try {{
            discard db.prepare("INSERT INTO phorj_pg_it(id, name, active) VALUES(1, 'dup', true)").exec();
            Output.printLine("no-dup-error");
        }} catch (UniqueViolationError e) {{
            Output.printLine("unique-violation");
        }}

        // executeMany — the PG-divergent path: at tx-depth 0 it opens its OWN `BEGIN`/`COMMIT`
        // (Postgres rejects a standalone `SAVEPOINT`, unlike SQLite). Two homogeneous int rows.
        int bulk = db
            .prepare("INSERT INTO phorj_pg_it(id, name, active) VALUES(?, 'bulk', true)")
            .executeMany([[20], [21]]);
        Output.printLine("bulk={{bulk}}");

        // Array mapping (slice K): int[]/text[] columns read as typed lists via the array accessors;
        // a List<string> hydration field routes there too.
        discard db.prepare("CREATE TABLE phorj_pg_arr(id INT, nums INT[], tags TEXT[])").exec();
        discard db.prepare("INSERT INTO phorj_pg_arr VALUES(1, ARRAY[1,2,3], ARRAY['a','b'])").exec();
        List<Row> arows = db.prepare("SELECT nums, tags FROM phorj_pg_arr").query();
        for (Row ar in arows) {{
            List<int> nums = ar.getIntList("nums");
            List<string> tags = ar.getStringList("tags");
            Output.printLine("nums={{List.length(nums)}} first={{nums[0]}} tags={{List.length(tags)}}");
        }}
        discard db.prepare("DROP TABLE phorj_pg_arr").exec();

        // A closure transaction — BEGIN/COMMIT through the portable control SQL, returning a value.
        int tx = db.transaction(function(): int throws DatabaseError {{
            Statement s = db.prepare("UPDATE phorj_pg_it SET name = 'upd' WHERE id = 1")?;
            discard s.exec()?;
            return 7;
        }});
        Output.printLine("tx={{tx}}");

        discard db.prepare("DROP TABLE phorj_pg_it").exec();
        db.close();
    }} catch (DatabaseError e) {{
        Output.printLine("unexpected: {{e.message}}");
    }}
}}
"#
    )
}

#[test]
fn postgres_round_trip_on_both_backends() {
    let Ok(dsn) = std::env::var("PHORJ_PG_TEST_DSN") else {
        eprintln!(
            "db_postgres: SKIP — set PHORJ_PG_TEST_DSN to a live Postgres DSN to run the round-trip \
             (e.g. postgres://user:pw@host:5432/db). The deterministic driver coverage runs in the \
             src/ext/database/postgres.rs unit tests regardless."
        );
        return;
    };
    let src = program(&dsn);
    let expected =
        "1=Ada\n2=Grace\nreturning=3\nunique-violation\nbulk=2\nnums=3 first=1 tags=2\ntx=7\n";
    let tree = cmd_treewalk(&src).expect("postgres round-trip runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    // interp ≡ VM: the VM must produce byte-identical stdout.
    assert_eq!(
        cmd_run(&src).expect("postgres round-trip runs on the VM"),
        tree,
        "interp ≡ VM"
    );
}
