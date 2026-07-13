//! `Core.Log` (DEC-220) end-to-end fixture.
//!
//! Core.Log natives are `pure: false` (they write `[LEVEL]` lines to stderr), so `uses_impure_native`
//! auto-quarantines `examples/guide/logging.phg` from the byte-identity differential. This fixture is
//! therefore the SOLE gate that exercises the shipped example through the real language surface —
//! `import Core.Log` resolution + `Log.*` namespaced-native dispatch — rather than calling
//! `log_natives()` directly (which the unit tests do). It asserts STDOUT (the captured output buffer);
//! the `[LEVEL]` lines go to the process's real stderr, which is not captured here and need not be
//! (logs are the out-of-band sink). `run ≡ runvm` holds — only the PHP leg is quarantined.

use phorj::cli::{cmd_run, cmd_treewalk};

#[test]
fn logging_example_runs_on_both_backends() {
    let src = std::fs::read_to_string("examples/guide/logging.phg").expect("read logging.phg");
    // STDOUT is only the `Output.printLine` result; every `Log.*` line went to (uncaptured) stderr.
    let tree = cmd_treewalk(&src).expect("logging.phg runs on the interpreter");
    assert_eq!(tree, "sum = 6\n");
    // run ≡ runvm: the VM must produce byte-identical stdout (both call the one shared native body).
    assert_eq!(cmd_run(&src).expect("logging.phg runs on the VM"), tree);
}
