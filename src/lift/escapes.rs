//! PHP string-literal escapes (scout row 5n, §LIFT-UNICODE-ESCAPE) — ONE decoder for the lexer's
//! plain strings and the parser's interpolated literal runs. Each path used to carry its own table and
//! each was wrong somewhere different (the lexer decoded `\n` in single quotes and `\'` in double
//! quotes; the interpolation path turned `\{` into `{`), so a string's meaning depended on whether it
//! happened to contain a `$`. The semantics are PHP's, measured against the PHP 8.5 oracle —
//! `lifter_tests_escapes.rs` pins every entry.
//!
//! Escapes decode to BYTES (`\x`/octal name bytes, and `"\xE2\x80\x99"` is one character), and the
//! finished literal is checked as UTF-8 by [`finish`]: a phorj string cannot hold what PHP's can.

/// Decode the escape at `chars[i]` — a `\` with at least one character after it — into `out`, and
/// return how many characters it consumed. `double` selects PHP's double-quoted table; a single-quoted
/// string decodes only `\\` and `\'`. A backslash that starts no escape is kept with the character
/// after it, as PHP keeps it (`"\{"` is two characters — PHP has no escaped interpolation hole).
pub(super) fn decode_escape(
    chars: &[char],
    i: usize,
    double: bool,
    out: &mut Vec<u8>,
) -> Result<usize, String> {
    let e = chars[i + 1];
    let simple = match (double, e) {
        (_, '\\') => Some(b'\\'),
        (false, '\'') => Some(b'\''),
        (true, 'n') => Some(b'\n'),
        (true, 't') => Some(b'\t'),
        (true, 'r') => Some(b'\r'),
        (true, 'v') => Some(0x0B),
        (true, 'e') => Some(0x1B),
        (true, 'f') => Some(0x0C),
        (true, '$') => Some(b'$'),
        (true, '"') => Some(b'"'),
        _ => None,
    };
    if let Some(b) = simple {
        out.push(b);
        return Ok(2);
    }
    if double && e.is_digit(8) {
        let digits = run(chars, i + 1, 3, |c| c.is_digit(8));
        let v = u32::from_str_radix(&digits, 8).expect("octal digits");
        if v > 0o377 {
            return Err(format!(
                "the octal escape `\\{digits}` overflows a byte (PHP warns and wraps it to \
                 `\\{:o}`) — not lifted; write the byte you mean",
                v & 0o377
            ));
        }
        out.push(v as u8);
        return Ok(1 + digits.len());
    }
    if double && e == 'x' {
        let digits = run(chars, i + 2, 2, |c| c.is_ascii_hexdigit());
        if !digits.is_empty() {
            out.push(u8::from_str_radix(&digits, 16).expect("hex digits"));
            return Ok(2 + digits.len());
        }
    }
    if double && e == 'u' && chars.get(i + 2) == Some(&'{') {
        return unicode(chars, i, out);
    }
    out.push(b'\\');
    push_char(out, e);
    Ok(2)
}

/// `\u{HEX}` — PHP accepts any number of hex digits naming a codepoint up to U+10FFFF, and refuses an
/// empty, non-hex or unterminated escape at parse time; a surrogate it encodes as bytes that are not
/// UTF-8, so it is refused here too.
fn unicode(chars: &[char], i: usize, out: &mut Vec<u8>) -> Result<usize, String> {
    let start = i + 3;
    let close = chars[start..].iter().position(|&c| c == '}');
    let body: String = match close {
        Some(n) => chars[start..start + n].iter().collect(),
        None => return Err("an unterminated `\\u{…}` escape (PHP refuses it too)".into()),
    };
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "`\\u{{{body}}}` is not a codepoint escape (PHP refuses it too)"
        ));
    }
    let digits = body.trim_start_matches('0');
    let cp = if digits.len() > 6 {
        u32::MAX
    } else {
        u32::from_str_radix(if digits.is_empty() { "0" } else { digits }, 16).expect("hex")
    };
    if cp > 0x10FFFF {
        return Err(format!(
            "`\\u{{{body}}}` is past U+10FFFF (PHP refuses it too)"
        ));
    }
    let Some(c) = char::from_u32(cp) else {
        return Err(format!(
            "`\\u{{{body}}}` is a surrogate — PHP emits bytes that are not UTF-8, which a phorj \
             string cannot hold; not lifted"
        ));
    };
    push_char(out, c);
    Ok(3 + body.chars().count() + 1)
}

/// Up to `max` characters from `chars[from..]` satisfying `ok`.
fn run(chars: &[char], from: usize, max: usize, ok: impl Fn(char) -> bool) -> String {
    chars
        .iter()
        .skip(from)
        .take(max)
        .take_while(|&&c| ok(c))
        .collect()
}

/// Append `c`'s UTF-8 bytes.
pub(super) fn push_char(out: &mut Vec<u8>, c: char) {
    let mut buf = [0u8; 4];
    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}

/// The decoded literal as a `String`, or a refusal naming why: `\x`/octal escapes can spell bytes
/// that are not UTF-8, which PHP strings hold and phorj strings do not.
pub(super) fn finish(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|e| {
        format!(
            "a string literal holds bytes that are not UTF-8 (at byte {}, spelled by a `\\x` or \
             octal escape) — a phorj string is UTF-8, so it is not lifted",
            e.utf8_error().valid_up_to()
        )
    })
}
