//! `Core.Debug` (DEC-238) end-to-end fixture: dump's pass-through + capture + printing, dd's
//! dump-and-exit, and `Runtime.exit`'s clean-termination semantics — on BOTH backends, including
//! the exit CODE via the Batch-1-B channel (`cmd_treewalk_exit` / `cmd_run_exit`). The rendering
//! format itself is pinned by the `src/native/debug.rs` unit tests AND, across all THREE backends,
//! by `conformance/lang/dump.phg` (the PHP twin `__phorj_debug_render` renders byte-identically).

use phorj::cli::{cmd_run, cmd_run_exit, cmd_transpile, cmd_treewalk, cmd_treewalk_exit};

fn both(src: &str, expected: &str) {
    let tree = cmd_treewalk(src).expect("program runs on the interpreter");
    assert_eq!(tree, expected, "interpreter output");
    assert_eq!(
        cmd_run(src).expect("program runs on the VM"),
        tree,
        "run ≡ runvm"
    );
}

fn both_exit(src: &str, expected_out: &str, expected_code: i64) {
    let (out, code) = cmd_treewalk_exit(src).expect("interpreter runs");
    assert_eq!(
        (out.as_str(), code),
        (expected_out, expected_code),
        "interpreter"
    );
    let (out, code) = cmd_run_exit(src).expect("VM runs");
    assert_eq!((out.as_str(), code), (expected_out, expected_code), "VM");
}

#[test]
fn dump_prints_passes_through_and_captures() {
    let src = r#"package Main;
import Core.Output;
import Core.String;
import Core.Debug;
import Core.Debug.Debug;
class User { constructor(public string name, public int age) {} }
function main(): void {
  int doubled = Debug.dump(21).value() * 2;
  Output.printLine("doubled {doubled}");
  string snap = Debug.dump(new User("Ada", 36)).text();
  bool hasClass = String.contains(snap, "User \{");
  Output.printLine("snap-has-class {hasClass}");
  discard Debug.dump(["k" => [1, 2]]);
}
"#;
    both(
        src,
        "21\ndoubled 42\nUser { age: 36, name: \"Ada\" }\nsnap-has-class true\n{ \"k\" => [1, 2] }\n",
    );
}

#[test]
fn runtime_exit_is_clean_and_carries_the_code() {
    let src = r#"package Main;
import Core.Output;
import Core.Runtime;
function main(): void {
  Output.printLine("before");
  Runtime.exit(3);
  Output.printLine("unreachable");
}
"#;
    both_exit(src, "before\n", 3);
}

#[test]
fn dd_dumps_then_exits_one() {
    let src = r#"package Main;
import Core.Output;
import Core.Debug;
import Core.Debug.Debug;
function main(): void {
  Output.printLine("checking");
  Debug.dd([1, 2]);
  Output.printLine("unreachable");
}
"#;
    both_exit(src, "checking\n[1, 2]\n", 1);
}

/// exit(0) is a NORMAL success termination — distinguishable from a fault in every harness.
#[test]
fn exit_zero_is_success() {
    let src = r#"package Main;
import Core.Output;
import Core.Runtime;
function main(): void {
  Output.printLine("done early");
  Runtime.exit(0);
}
"#;
    both_exit(src, "done early\n", 0);
}

/// The gate is LIFTED: `Core.Debug` transpiles, emitting the `__phorj_debug_render` twin (+ the
/// enum table). The three-backend BYTE-IDENTITY of the format is pinned by
/// `conformance/lang/dump.phg` (interpreter + VM + real PHP against one golden).
#[test]
fn debug_transpiles_with_the_twin_renderer() {
    let src = r#"package Main;
import Core.Output;
import Core.Debug;
import Core.Debug.Debug;
function main(): void { discard Debug.dump([1, 2]); }
"#;
    let php = cmd_transpile(src).expect("Core.Debug transpiles now");
    assert!(php.contains("__phorj_debug_render"), "twin missing:\n{php}");
    assert!(
        php.contains("__phorj_debug_enums"),
        "enum table missing:\n{php}"
    );
}
