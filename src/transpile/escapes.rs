//! PHP string-literal shaping helpers: fault-message literal extraction + the three double-quoted
//! escapers (standalone, interpolating, and `bytes`). Split out of `transpile/mod.rs` (M-Decomp) to
//! keep the root under the file-size cap; re-globbed by the root so the emit modules keep reaching
//! them via `use super::*`. Pure code movement — no emit-logic change.

use super::*;

/// Escape a literal string chunk for embedding in a PHP double-quoted string.
/// `$` is escaped so PHP does not attempt its own interpolation on emitted literals.
/// The literal text of a fault intrinsic's string-literal message (M-faults 2a); empty if absent. The
/// checker guarantees the argument is a single `StrPart::Literal`.
pub(super) fn lit_arg(e: Option<&Expr>) -> String {
    if let Some(Expr::Str(parts, _)) = e {
        if let [StrPart::Literal(s)] = &parts[..] {
            return s.clone();
        }
    }
    String::new()
}

pub(super) fn php_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => out.push_str("\\$"),
            _ => push_control_escaped(&mut out, c),
        }
    }
    out
}

/// Push `c`, turning a CONTROL character into a PHP escape sequence rather than emitting it raw.
///
/// **This is a byte-identity fix, not cosmetics** (found 2026-07-31). A raw newline inside an emitted
/// PHP string literal is semantically fine on its own line — but the emitter renders some constructs on
/// ONE line (a closure body, for instance), and collapsing a statement onto one line turns a newline
/// *inside a literal* into a space. So `function(): string { return "a\nb\n"; }` printed `a\nb\n` on
/// both Rust backends and `a b ` through PHP: a live Invariant-1 divergence, invisible until a
/// newline-bearing literal appeared inside a closure. Escaping at the literal is the root fix — a
/// literal that contains no raw newline cannot be corrupted by ANY downstream single-line rendering,
/// present or future, which patching the closure emitter alone would not have guaranteed.
///
/// `\n`/`\r`/`\t` get their readable forms; every other C0 control and DEL becomes `\xHH` with two
/// digits always, so PHP's greedy `\x` cannot merge with a following hex character (the same rule
/// [`php_escape_bytes`] already applied — this brings the text escapers up to it).
fn push_control_escaped(out: &mut String, c: char) {
    match c {
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if (c as u32) < 0x20 || c as u32 == 0x7F => out.push_str(&format!("\\x{:02x}", c as u32)),
        c => out.push(c),
    }
}

/// Escape a literal segment for emission *inside an interpolating* PHP double-quoted string (B-9).
/// Like [`php_escape`] for `\` and `"`, but escapes `$` **only where PHP would actually interpolate**
/// — i.e. when the next char is an identifier start (`[A-Za-z_]`), a `{`/`$` (the `${…}`/`$$`
/// complex-var forms), or the segment end (conservative: the following segment may begin with one of
/// those, incl. an emitted `{$…}` hole). `$5`, `$ `, a trailing-symbol `$` etc. stay bare — cleaner
/// PHP with identical output. Used only by [`emit_string`](expr); the other `php_escape` call sites
/// emit standalone/quoted contexts and keep the unconditional form.
pub(super) fn php_escape_interp(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => {
                let interpolates = match chars.peek() {
                    Some(n) => n.is_ascii_alphabetic() || *n == '_' || *n == '{' || *n == '$',
                    None => true, // trailing `$`: next segment might start a var / `{$…}` hole
                };
                out.push_str(if interpolates { "\\$" } else { "$" });
            }
            // Same control-character escaping as the standalone form — an INTERPOLATING literal is just
            // as vulnerable to a single-line rendering eating its newlines.
            _ => push_control_escaped(&mut out, c),
        }
    }
    out
}

/// Escape a `bytes` literal for a PHP double-quoted string. Printable ASCII is emitted verbatim (with
/// `\` `"` `$` escaped); every other octet becomes a two-digit `\xHH` (always two digits so PHP's
/// greedy `\x` escape can't merge with a following hex character). PHP strings are byte arrays, so the
/// round-trip is exact (M6 W0).
pub(super) fn php_escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'$' => out.push_str("\\$"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out
}
