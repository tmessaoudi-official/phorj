//! `String.substring` — byte-indexed, faulting on a split multibyte character (split out of `text.rs`,
//! Invariant 13: that file is grandfathered and may not grow). DEC-560: the PHP leg faults the same way.

use super::*;

/// `substring(string, int, int) -> string` — a byte-indexed slice mirroring PHP `substr($s, start,
/// len)` exactly (negative start/len count from the end; out-of-range clamps to empty). Byte-based
/// (no mbstring); a slice that splits a multibyte char yields invalid UTF-8 → faults (EV-7).
pub(super) fn text_substring(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Int(start), Value::Int(length)] => {
            let bytes = s.as_bytes();
            let n = bytes.len() as i64;
            let begin = if *start < 0 {
                (n + *start).max(0)
            } else {
                (*start).min(n)
            };
            let end = if *length < 0 {
                (n + *length).max(begin)
            } else {
                begin.saturating_add(*length).min(n) // row 5z: PHP_INT_MAX means "to the end"
            };
            String::from_utf8(bytes[begin as usize..end as usize].to_vec())
                .map(|s| Value::Str(s.into()))
                .map_err(|_| "String.substring split a multibyte character (byte-indexed)".into())
        }
        _ => Err("String.substring expects (string, int, int)".into()),
    }
}

/// The PHP emission: `__phorj_substring` (DEC-560) — plain `substr` returns the broken bytes where phorj faults.
pub(super) fn php_substring(a: &[String]) -> String {
    format!(
        "__phorj_substring({}, {}, {})",
        parg(a, 0),
        parg(a, 1),
        parg(a, 2)
    )
}
