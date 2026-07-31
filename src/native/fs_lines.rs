//! `Core.FileSystemModule` STREAMING line reads — the native half of DEC-347.
//!
//! The ruling: `FileSystem.lines(path): Iterator<string>` over an **offset-chunk native, NO file
//! handle**. A `FileHandle` type was rejected (blocked by C4 — no transpiling precedent for an opaque
//! handle, and `emit_type` would emit an unsatisfiable PHP class hint), so the iterator's entire state
//! is a byte OFFSET the prelude carries in an `int`. Zero new `Value`, zero new type, zero new
//! transpile machinery, and a later swap to a real handle stays non-breaking because the user-facing
//! syntax never mentions the mechanism.
//!
//! **Why a CHUNK and not a line.** A `lineAt(path, offset)` native would have to open + seek + read
//! per line, which is one `open(2)` per line — hopeless against PHP's `fgets` on an already-open
//! handle (Invariant 18 benches this against PHP). Reading ~64 KiB per call amortises the syscalls
//! across every line in the chunk while memory stays O(chunk), which is what the ruling's O(1)-memory
//! requirement actually needs.
//!
//! **The chunk always ends on a line boundary** (or at EOF), so the prelude never has to stitch a
//! partial line across two reads — a split that is easy to get wrong and impossible to notice on small
//! files, because it only shows up when a line straddles the chunk edge. When the target size lands
//! mid-line the read EXTENDS to the next newline rather than truncating, so a single line longer than
//! the target is returned whole. That also keeps every returned chunk a valid UTF-8 boundary for free:
//! `\n` is ASCII and cannot appear inside a multi-byte sequence.

use super::fs_bodies::classify;
use crate::value::Value;
use std::io::{Read, Seek, SeekFrom};

/// The read-ahead target. Not a hard cap: a chunk grows past it to finish the line in progress (see
/// the module note), so a file of one enormous line still streams correctly rather than stalling.
const CHUNK_TARGET: usize = 64 * 1024;

/// `readLinesChunk(path, offset)` — the bytes from `offset` up to a line boundary at or after
/// [`CHUNK_TARGET`], as a UTF-8 string, or `null` at end of file.
///
/// The returned text KEEPS its line terminators. That is deliberate and load-bearing: the prelude
/// advances its offset by the chunk's byte length, so the terminators are what make the next offset
/// exact. Stripping them here would leave the caller unable to compute where it got to — and computing
/// it from the character count would be wrong for any non-ASCII line.
pub(super) fn read_lines_chunk_inner(args: &[Value]) -> Result<Value, String> {
    let (path, offset) = match args {
        [Value::Str(p), Value::Int(o)] => (p.as_str(), *o),
        _ => {
            return Err(
                "Core.FileSystemModule.__readLinesChunk expects (string path, int offset)"
                    .to_string(),
            )
        }
    };
    if offset < 0 {
        return Err(format!(
            "<<FileSystemIoError>>Core.FileSystemModule.lines: `{path}`: negative offset {offset}"
        ));
    }
    let mut f = std::fs::File::open(path).map_err(|e| classify("lines", path, &e))?;
    f.seek(SeekFrom::Start(offset as u64))
        .map_err(|e| classify("lines", path, &e))?;

    let mut buf: Vec<u8> = Vec::with_capacity(CHUNK_TARGET);
    let mut window = [0u8; 8192];
    // Phase 1: read up to the target.
    while buf.len() < CHUNK_TARGET {
        let want = (CHUNK_TARGET - buf.len()).min(window.len());
        let n = f
            .read(&mut window[..want])
            .map_err(|e| classify("lines", path, &e))?;
        if n == 0 {
            break; // EOF
        }
        buf.extend_from_slice(&window[..n]);
    }
    // Phase 2: if we stopped mid-line, keep going to the next `\n` (or EOF). Byte-at-a-time is right
    // here: it must not overshoot the newline, since anything past it belongs to the NEXT chunk and
    // there is no handle to push it back into.
    if !buf.is_empty() && !buf.ends_with(b"\n") {
        let mut one = [0u8; 1];
        loop {
            let n = f.read(&mut one).map_err(|e| classify("lines", path, &e))?;
            if n == 0 {
                break; // EOF mid-line: the last line has no terminator, which is legal
            }
            buf.push(one[0]);
            if one[0] == b'\n' {
                break;
            }
        }
    }
    if buf.is_empty() {
        return Ok(Value::Null); // at or past EOF — the iterator's termination signal
    }
    // A chunk ends on a `\n` or at EOF, so it is always a whole number of lines and therefore a valid
    // UTF-8 boundary. Invalid UTF-8 *within* the file is still a real error, reported the same way
    // `readText` reports it rather than being silently replaced.
    let text = String::from_utf8(buf).map_err(|_| {
        format!("<<FileSystemIoError>>Core.FileSystemModule.lines: `{path}` is not UTF-8 — use readBytes")
    })?;
    Ok(Value::Str(text.into()))
}

/// `splitLines(chunk)` — the chunk's complete lines, terminators STRIPPED, as a `List<string>`.
///
/// **This is a PERFORMANCE fix, and it is why it exists in Rust** (measured 2026-07-31). The prelude
/// originally did this split itself, appending each line with `List.append` — which CLONES the whole
/// list per call (`native::list::list_append`), so decoding one 64 KiB chunk of ~1200 lines cost ~720k
/// element clones: O(n²). Measured against PHP's `fgets` loop on 40k lines, phorj was **58x slower**
/// (295 ms vs 5.07 ms). One Rust pass with a single allocation is what makes the WIN-OR-FLAG bar
/// reachable at all; doing it in the prelude was the wrong layer, not a tuning knob.
///
/// Three rules, matching what the prelude used to do — kept together so the PHP twin has ONE thing to
/// mirror: split on `\n`; drop the trailing EMPTY element the final terminator produces (otherwise a
/// file of N lines yields N+1); strip a preceding `\r` so CRLF reads identically to LF.
pub(super) fn split_lines_inner(args: &[Value]) -> Result<Value, String> {
    let chunk = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.FileSystemModule.__splitLines expects (string chunk)".to_string()),
    };
    let mut parts: Vec<&str> = chunk.split('\n').collect();
    // The chunk ends with `\n` unless it ended at EOF; that terminator leaves a trailing "".
    if parts.last() == Some(&"") {
        parts.pop();
    }
    let out: Vec<Value> = parts
        .into_iter()
        .map(|l| Value::Str(l.strip_suffix('\r').unwrap_or(l).into()))
        .collect();
    Ok(Value::List(std::rc::Rc::new(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        let p =
            std::env::temp_dir().join(format!("phorj-lines-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p.to_string_lossy().into_owned()
    }

    fn chunk(path: &str, offset: i64) -> Option<String> {
        match read_lines_chunk_inner(&[Value::Str(path.into()), Value::Int(offset)]).unwrap() {
            Value::Str(s) => Some(s.as_str().to_string()),
            Value::Null => None,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_small_file_comes_back_in_one_chunk_with_terminators_intact() {
        let p = tmp("small");
        std::fs::write(&p, b"alpha\nbeta\ngamma\n").unwrap();
        assert_eq!(chunk(&p, 0).as_deref(), Some("alpha\nbeta\ngamma\n"));
        // Advancing by the chunk's BYTE length lands exactly at EOF.
        assert_eq!(chunk(&p, 17), None);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_final_line_without_a_terminator_is_returned_whole() {
        let p = tmp("noeol");
        std::fs::write(&p, b"one\ntwo").unwrap();
        assert_eq!(chunk(&p, 0).as_deref(), Some("one\ntwo"));
        assert_eq!(chunk(&p, 7), None);
        let _ = std::fs::remove_file(&p);
    }

    /// THE property the chunking exists for: a chunk never ends mid-line, so the prelude never has to
    /// stitch. With lines that do not divide the target evenly, the read EXTENDS past it.
    #[test]
    fn a_chunk_never_ends_mid_line_even_when_the_target_falls_inside_one() {
        let p = tmp("boundary");
        // Lines of 1000 bytes: the 64 KiB target lands inside line 66, so the chunk must extend.
        let line = "x".repeat(999);
        let body: String = (0..100).map(|_| format!("{line}\n")).collect();
        std::fs::write(&p, body.as_bytes()).unwrap();
        let c = chunk(&p, 0).expect("a chunk");
        assert!(
            c.ends_with('\n'),
            "chunk ended mid-line at {} bytes",
            c.len()
        );
        assert!(
            c.len().is_multiple_of(1000),
            "not a whole number of lines: {}",
            c.len()
        );
        assert!(c.len() >= CHUNK_TARGET, "chunk stopped short: {}", c.len());
        let _ = std::fs::remove_file(&p);
    }

    /// A single line LONGER than the target still streams — it comes back whole rather than truncated,
    /// which is what keeps the no-stitching guarantee true in the pathological case.
    #[test]
    fn one_line_longer_than_the_chunk_target_is_returned_whole() {
        let p = tmp("huge");
        let huge = "y".repeat(CHUNK_TARGET * 3);
        std::fs::write(&p, format!("{huge}\ntail\n").as_bytes()).unwrap();
        let c = chunk(&p, 0).expect("a chunk");
        assert_eq!(c.len(), CHUNK_TARGET * 3 + 1, "the long line was split");
        assert!(c.ends_with('\n'));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn an_empty_file_yields_null_immediately() {
        let p = tmp("empty");
        std::fs::write(&p, b"").unwrap();
        assert_eq!(chunk(&p, 0), None);
        let _ = std::fs::remove_file(&p);
    }

    /// A missing file must be the SAME typed taxonomy the rest of the module uses, not a bespoke
    /// message — `classify` is what single-sources that.
    #[test]
    fn a_missing_file_classifies_as_not_found() {
        let e = read_lines_chunk_inner(&[Value::Str("/nope/nope/nope".into()), Value::Int(0)])
            .expect_err("must fail");
        // `classify` emits the BARE taxonomy tag (`<<NotFound>>`); the prelude is what maps it to the
        // `FileSystem`-prefixed error class. Asserting the tag keeps this test on the native's contract.
        assert!(e.contains("<<NotFound>>"), "{e}");
    }

    #[test]
    fn a_negative_offset_is_rejected_rather_than_wrapping() {
        let p = tmp("neg");
        std::fs::write(&p, b"a\n").unwrap();
        let e = read_lines_chunk_inner(&[Value::Str(p.clone().into()), Value::Int(-1)])
            .expect_err("must fail");
        assert!(e.contains("negative offset"), "{e}");
        let _ = std::fs::remove_file(&p);
    }

    fn lines_of(chunk: &str) -> Vec<String> {
        match split_lines_inner(&[Value::Str(chunk.into())]).unwrap() {
            Value::List(xs) => xs
                .iter()
                .map(|v| match v {
                    Value::Str(s) => s.as_str().to_string(),
                    other => panic!("unexpected {other:?}"),
                })
                .collect(),
            other => panic!("unexpected {other:?}"),
        }
    }

    /// The three rules in one place: the trailing terminator does NOT produce an extra line, a blank
    /// line IS a line, and CRLF strips to the same text as LF.
    #[test]
    fn split_lines_drops_the_final_terminator_keeps_blanks_and_strips_cr() {
        assert_eq!(lines_of("a\nb\n"), vec!["a", "b"]);
        assert_eq!(
            lines_of("a\n\nb\n"),
            vec!["a", "", "b"],
            "a blank line is content"
        );
        assert_eq!(
            lines_of("a\r\nb\r\n"),
            vec!["a", "b"],
            "CRLF must read like LF"
        );
        // No trailing terminator: the last line still counts.
        assert_eq!(lines_of("a\nb"), vec!["a", "b"]);
        assert_eq!(lines_of(""), Vec::<String>::new());
        // A lone terminator is ONE empty line, not zero and not two.
        assert_eq!(lines_of("\n"), vec![""]);
    }
}
