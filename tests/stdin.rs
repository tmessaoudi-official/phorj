//! `Core.Input` (DEC-281) quarantine-seam tests under CONTROLLED stdin.
//!
//! The stdin natives are `pure: false` (results depend on the process's stdin), so the
//! byte-identity differential SKIPS any program importing `Core.Input` (via the
//! `Core.Native.Input` → `Core.Input` prelude-twin mapping in `uses_impure_native`). They are
//! exercised here instead, with `set_stdin_override` injecting a deterministic buffer — reset
//! between the two backend runs, since reads consume the override's cursor.

use phorj::cli::{cmd_run, cmd_treewalk};
use phorj::native::set_stdin_override;
use std::sync::Mutex;

/// The override is a process global — serialize the tests that set it (poison-tolerant).
static STDIN_LOCK: Mutex<()> = Mutex::new(());

/// Run `src` on both Rust backends with `input` injected, assert both equal `expected`.
fn both_with_input(src: &str, input: &[u8], expected: &str) {
    let _g = STDIN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_stdin_override(Some(input.to_vec()));
    let tw = cmd_treewalk(src).expect("treewalk runs");
    assert_eq!(tw, expected, "interpreter output");
    set_stdin_override(Some(input.to_vec()));
    let vm = cmd_run(src).expect("vm runs");
    assert_eq!(vm, expected, "vm output");
    set_stdin_override(None);
}

#[test]
fn read_all_returns_the_whole_pipe() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.Input;
import Core.String;
#[Entry] function main(): void {
    string all = Input.readAll();
    Output.printLine("len={String.length(all)}");
    Output.printLine(all);
}"#;
    both_with_input(src, b"hello\nworld", "len=11\nhello\nworld\n");
}

#[test]
fn read_all_bytes_is_exact() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.Input;
import Core.Bytes;
#[Entry] function main(): void {
    bytes b = Input.readAllBytes();
    Output.printLine("{Bytes.length(b)}");
}"#;
    // Invalid UTF-8 bytes survive exactly in the bytes form.
    both_with_input(src, &[0xFF, 0xFE, b'a', b'\n'], "4\n");
}

#[test]
fn read_line_strips_one_terminator_and_nulls_at_eof() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.Input;
import Core.String;
#[Entry] function main(): void {
    mutable int n = 0;
    while (true) {
        string? l = Input.readLine();
        if (var line = l) {
            n = n + 1;
            Output.printLine("{n}:[{line}] len={String.length(line)}");
        } else {
            break;
        }
    }
    Output.printLine("eof");
}"#;
    // `a\r\r\n` keeps ONE trailing `\r` (exactly-one-terminator strip); `b\rc` keeps its inner
    // `\r`; the last line has no terminator at all.
    both_with_input(
        src,
        b"a\r\r\nb\rc\nplain\nnoeol",
        "1:[a\r] len=2\n2:[b\rc] len=3\n3:[plain] len=5\n4:[noeol] len=5\neof\n",
    );
}

#[test]
fn lines_iterator_is_foreach_able_and_lazy_past_eof() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.Input;
#[Entry] function main(): void {
    for (string line in Input.lines()) {
        Output.printLine("> {line}");
    }
    Output.printLine("done");
}"#;
    both_with_input(src, b"one\ntwo\nthree", "> one\n> two\n> three\ndone\n");
    both_with_input(src, b"", "done\n");
}

#[test]
fn read_all_after_read_line_gets_the_remainder() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.Input;
#[Entry] function main(): void {
    string? first = Input.readLine();
    Output.printLine("first={first ?? "-"}");
    Output.printLine("rest=[{Input.readAll()}]");
}"#;
    both_with_input(
        src,
        b"head\ntail1\ntail2",
        "first=head\nrest=[tail1\ntail2]\n",
    );
}

#[test]
fn is_interactive_is_false_under_a_pipe() {
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
import Core.Input;
#[Entry] function main(): void {
    Output.printLine("{Input.isInteractive()}");
}"#;
    // An override models piped (non-tty) stdin.
    both_with_input(src, b"", "false\n");
}

#[test]
fn input_is_import_gated() {
    // Nothing in the wind: without `import Core.Input;` the `Input` name does not exist.
    let src = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;
#[Entry] function main(): void { Output.printLine(Input.readAll()); }"#;
    let err = cmd_treewalk(src).unwrap_err();
    assert!(
        err.contains("Input"),
        "should name the missing module: {err}"
    );
}
