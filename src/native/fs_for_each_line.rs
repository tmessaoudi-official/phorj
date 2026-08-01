//! `Core.FileSystemModule.forEachLine` — the native-driven line reader (DEC-422(a)).
//!
//! **Why it exists.** DEC-347's `FileSystem.lines(path)` is an `Iterator<string>`, which is the right
//! shape for composition but a MEASURED 4x loss against PHP's `fgets` loop (21.0 ms vs 5.2 ms on 40k
//! lines), recorded OWED under DEC-365's no-hidden-loss rule. The residual is structural rather than a
//! tuning miss: an iterator pays two phorj-level virtual calls per element (`hasNext` then `next`)
//! against PHP's C loop, and no amount of work inside that design removes them. The developer ruled
//! BOTH answers in (2026-07-31, DEC-422): this API, which removes the per-element cost for the line
//! case, and a JIT vertical for foreach-over-`Iterator`, which removes it for every iterator. They are
//! complementary — this one is self-contained and lands first.
//!
//! **What makes it faster, precisely.** Two things, and neither is a micro-optimisation:
//!   1. **No per-element virtual call.** The whole loop runs here; the closure is invoked directly
//!      through the backend's re-entrant [`ClosureInvoker`], the same mechanism `List.map` uses, so one
//!      body drives the interpreter AND the VM (parity by construction, not by two implementations).
//!   2. **ONE open file handle for the whole read.** `readLinesChunk` re-`open(2)`s and `seek`s per
//!      chunk, because a chunk native has nowhere to keep a handle — the DEC-347 ruling rejected a
//!      `FileHandle` type (C4: no transpiling precedent for an opaque handle). Owning the loop means
//!      the handle simply lives on the stack, and the offset bookkeeping disappears with it.
//!
//! **What it costs the caller.** `lines` composes — it is a value that flows into any function taking
//! an `Iterator<string>`, and its loop body can `break`, `return`, or throw whatever the enclosing
//! function declares. `forEachLine` does none of that: the body is a closure, so there is no `break`
//! and no `return` from the caller, and the closure may throw only `FileSystemError` (the same
//! restriction `withLock` carries, and for the same reason — a native's parameter type is fixed in
//! Rust, and `Ty::Function`'s throws set is covariant in the "fewer" direction only). Both APIs stay:
//! this is the fast path for the common case, not a replacement.
//!
//! Terminator handling is IDENTICAL to `lines` on purpose — `\n` stripped, a preceding `\r` stripped
//! too, an empty line still a line. The two APIs disagreeing on any file shape would be a bug in one
//! of them, which is exactly what `tests/fs.rs` asserts by running both over the same fixtures.

use super::fs_bodies::classify;
use crate::native::ClosureInvoker;
use crate::value::Value;
use std::io::{BufRead, BufReader};

/// How a `forEachLine` read ended. The two failure kinds must NOT be conflated, which is why this is
/// an enum rather than a `Result<Value, String>`:
///
///   * an **I/O** failure is the module's own, and belongs in `FileSystemResult.Err` so the prelude
///     turns it into a catchable typed `FileSystemError` — that is the whole DEC-313 contract;
///   * a **closure** failure is the CALLER's, arriving from [`ClosureInvoker`] as the backend's throw
///     sentinel (or a plain fault string). Wrapping it would re-label the user's own `RuntimeError` as
///     a filesystem error and make it catchable by the wrong clause. It propagates untouched.
///
/// Collapsing both into one `Err` is the obvious shortcut and is exactly the silent-semantic-downgrade
/// Invariant 14 forbids.
pub(super) enum ForEachEnd {
    /// Every line was delivered.
    Done,
    /// The module's own failure, already carrying its `<<Kind>>` marker.
    Io(String),
    /// The closure's failure — propagate verbatim, sentinel and all.
    Closure(String),
}

/// `forEachLine(path, fn)` — call `fn` once per line, terminators stripped.
///
/// Stops at the first failure of either kind: a closure that throws on line 3 of a million-line file
/// reads three lines and no more.
pub(super) fn for_each_line_inner(args: &[Value], call: &mut ClosureInvoker) -> ForEachEnd {
    let (path, f) = match args {
        [Value::Str(p), f] => (p.as_str(), f),
        _ => {
            return ForEachEnd::Io(
                "Core.FileSystemModule.forEachLine expects (string path, (string) => void fn)"
                    .to_string(),
            )
        }
    };
    // `File::open` on a DIRECTORY succeeds on Linux and fails only at read time, with an error whose
    // kind is not `IsADirectory` on every platform — so check explicitly, matching what `readText`
    // does and what the PHP twin's `is_dir` pre-check must mirror.
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return ForEachEnd::Io(classify("forEachLine", path, &e)),
    };
    if meta.is_dir() {
        return ForEachEnd::Io(format!(
            "<<IsADirectory>>Core.FileSystemModule.forEachLine: `{path}`"
        ));
    }
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return ForEachEnd::Io(classify("forEachLine", path, &e)),
    };
    let mut reader = BufReader::new(file);
    // Read as BYTES, not `lines()`: a UTF-8 failure must be reported as this module's own error rather
    // than as an `io::Error` with a different kind, and `read_until` lets the terminator strip and the
    // UTF-8 check sit in one place.
    let mut buf: Vec<u8> = Vec::with_capacity(256);
    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf) {
            Ok(n) => n,
            Err(e) => return ForEachEnd::Io(classify("forEachLine", path, &e)),
        };
        if n == 0 {
            break; // EOF. A final line with no terminator was already delivered by the previous pass.
        }
        // Strip in this order: the `\n` first, then a `\r` that preceded it. A lone `\r` in the middle
        // of a line is DATA and is left alone — same rule as `split_lines_inner`.
        let mut line: &[u8] = &buf;
        if line.last() == Some(&b'\n') {
            line = &line[..line.len() - 1];
            if line.last() == Some(&b'\r') {
                line = &line[..line.len() - 1];
            }
        }
        let Ok(text) = std::str::from_utf8(line) else {
            return ForEachEnd::Io(format!(
                "<<FileSystemIoError>>Core.FileSystemModule.forEachLine: `{path}` is not UTF-8 — use readBytes"
            ));
        };
        if let Err(e) = call(f, &[Value::Str(text.into())]) {
            return ForEachEnd::Closure(e);
        }
    }
    ForEachEnd::Done
}

#[cfg(test)]
mod tests {
    use super::{for_each_line_inner, ForEachEnd};
    use crate::value::Value;

    fn tmp(name: &str) -> String {
        let p = std::env::temp_dir().join(format!(
            "phorj-foreachline-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p.to_string_lossy().into_owned()
    }

    /// Drive the native with a mock invoker that records every line it is handed — the same shape the
    /// backends supply, so the body under test is the one that ships.
    fn lines_of(path: &str) -> Result<Vec<String>, String> {
        let mut seen: Vec<String> = Vec::new();
        let mut invoke = |_f: &Value, args: &[Value]| {
            match args.first() {
                Some(Value::Str(s)) => seen.push(s.as_str().to_string()),
                other => panic!("expected one string arg, got {other:?}"),
            }
            Ok(Value::Null)
        };
        match for_each_line_inner(&[Value::Str(path.into()), Value::Bool(true)], &mut invoke) {
            ForEachEnd::Done => Ok(seen),
            // The two failure channels are collapsed HERE, in the test helper, only because these
            // cases assert one at a time; the native keeps them apart, which is the point.
            ForEachEnd::Io(e) | ForEachEnd::Closure(e) => Err(e),
        }
    }

    /// The shapes that break line readers, all in one place — and each must match what `lines` yields,
    /// which `tests/fs.rs` asserts end-to-end on all three legs.
    #[test]
    fn terminators_blank_lines_crlf_and_a_missing_final_newline_all_read_like_lines_does() {
        let p = tmp("shapes");
        std::fs::write(&p, "alpha\nbeta\n\ngamma\n").unwrap();
        assert_eq!(lines_of(&p).unwrap(), ["alpha", "beta", "", "gamma"]);
        // A final line with NO terminator is still a line.
        std::fs::write(&p, "one\ntwo").unwrap();
        assert_eq!(lines_of(&p).unwrap(), ["one", "two"]);
        // CRLF reads identically to LF — the `\r` is a terminator, not data.
        std::fs::write(&p, "r1\r\nr2\r\n").unwrap();
        assert_eq!(lines_of(&p).unwrap(), ["r1", "r2"]);
        // A lone `\r` MID-line is data and survives.
        std::fs::write(&p, "a\rb\n").unwrap();
        assert_eq!(lines_of(&p).unwrap(), ["a\rb"]);
        // An empty file is an empty iteration, not an error.
        std::fs::write(&p, "").unwrap();
        assert_eq!(lines_of(&p).unwrap(), Vec::<String>::new());
        let _ = std::fs::remove_file(&p);
    }

    /// A line far longer than any internal buffer is delivered whole, not split. The chunk-based
    /// `lines` had to extend past its target for this; here it falls out of `read_until`, but it is
    /// asserted all the same — it is the property that only fails on real input.
    #[test]
    fn a_line_longer_than_the_read_buffer_arrives_in_one_piece() {
        let p = tmp("longline");
        let long = "x".repeat(300_000);
        std::fs::write(&p, format!("{long}\nshort\n")).unwrap();
        let got = lines_of(&p).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].len(), 300_000);
        assert_eq!(got[1], "short");
        let _ = std::fs::remove_file(&p);
    }

    /// A missing path and a DIRECTORY are both catchable typed failures, not panics — and the
    /// directory case is the one `File::open` alone would let through on Linux.
    #[test]
    fn a_missing_path_and_a_directory_are_classified_errors() {
        let missing = lines_of("/nonexistent-phorj-foreachline/x.txt").unwrap_err();
        assert!(
            missing.starts_with("<<NotFound>>"),
            "want a NotFound marker, got {missing}"
        );
        let dir = lines_of(&std::env::temp_dir().to_string_lossy()).unwrap_err();
        assert!(
            dir.starts_with("<<IsADirectory>>"),
            "want an IsADirectory marker, got {dir}"
        );
    }

    /// The closure's failure STOPS the read and propagates unchanged — a body that fails on line 2 of
    /// a long file must not have been handed line 3. (The backends turn a phorj `throw` into exactly
    /// this `Err`, via their throw sentinel.)
    #[test]
    fn a_failing_closure_stops_the_read_at_that_line() {
        let p = tmp("stops");
        std::fs::write(&p, "a\nb\nc\nd\n").unwrap();
        let mut seen: Vec<String> = Vec::new();
        let mut invoke = |_f: &Value, args: &[Value]| {
            let Some(Value::Str(s)) = args.first() else {
                panic!("expected a string arg")
            };
            seen.push(s.as_str().to_string());
            if s.as_str() == "b" {
                return Err("boom".to_string());
            }
            Ok(Value::Null)
        };
        let end = for_each_line_inner(
            &[Value::Str(p.clone().into()), Value::Bool(true)],
            &mut invoke,
        );
        // CLOSURE, not Io: a caller's failure must not be re-labelled as a filesystem error, or it
        // becomes catchable by a `catch (FileSystemError e)` that has nothing to do with it.
        match end {
            ForEachEnd::Closure(e) => assert_eq!(e, "boom"),
            ForEachEnd::Io(e) => panic!("a closure failure was mis-channelled as I/O: {e}"),
            ForEachEnd::Done => panic!("the failure was swallowed"),
        }
        assert_eq!(seen, ["a", "b"], "the read continued past the failure");
        let _ = std::fs::remove_file(&p);
    }
}
