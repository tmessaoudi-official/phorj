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
