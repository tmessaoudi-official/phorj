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
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
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
            _ => out.push(c),
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
