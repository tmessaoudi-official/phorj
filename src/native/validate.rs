//! `Core.Validation` — syntactic string predicates (native-stdlib wave, Tier A).
//!
//! Pure, deterministic, std-only. Each predicate is `string -> bool`. Phorj has no regex crate (the
//! library is std-only), so the checks are hand-rolled in Rust and the PHP side emits a `preg_match`
//! with the *same* anchored, explicit-char-class pattern — so the two cannot disagree (no
//! `filter_var`, whose validation semantics we'd have to chase). These are *format* checks, not
//! semantic validators (e.g. `isInt` is `^[+-]?[0-9]+$`, not "fits in an i64").

use super::*;
use crate::types::Ty;
use crate::value::Value;

/// `^[+-]?[0-9]+$`
fn is_int(s: &str) -> bool {
    let b = s.as_bytes();
    let start = usize::from(b.first().is_some_and(|&c| c == b'+' || c == b'-'));
    b.len() > start && b[start..].iter().all(u8::is_ascii_digit)
}

/// `^[+-]?[0-9]+(\.[0-9]+)?$`
fn is_number(s: &str) -> bool {
    let b = s.as_bytes();
    let start = usize::from(b.first().is_some_and(|&c| c == b'+' || c == b'-'));
    let digits = &b[start..];
    match digits.iter().position(|&c| c == b'.') {
        None => !digits.is_empty() && digits.iter().all(u8::is_ascii_digit),
        Some(dot) => {
            let (int_part, frac_part) = (&digits[..dot], &digits[dot + 1..]);
            !int_part.is_empty()
                && int_part.iter().all(u8::is_ascii_digit)
                && !frac_part.is_empty()
                && frac_part.iter().all(u8::is_ascii_digit)
        }
    }
}

/// `^[A-Za-z]+$`
fn is_alpha(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphabetic())
}

/// `^[A-Za-z0-9]+$`
fn is_alnum(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// `^[0-9A-Fa-f]+$`
fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

// The ctype-class predicates (isLower..isPrintable) hand-roll the ASCII char class in Rust and emit a
// PHP `preg_match` over the SAME explicit `\xNN` class WITH the `D` (PCRE_DOLLAR_ENDONLY) flag. Two
// reasons over PHP's `ctype_*`: (1) `ctype_*` is a SHARED extension, absent under the hermetic `php -n`
// oracle (the ctype_digit CI bug) — PCRE is always compiled; (2) the `D` flag makes `$` match only the
// absolute string end, so unlike the pre-D `preg_match` validators above these do NOT count a string
// with a trailing `\n` (KNOWN_ISSUES VALIDATION-regex-trailing-newline). `is_lower`/`is_upper`/`is_punct`/
// `is_control`/`is_visible`/`is_printable` map 1:1 to a std `is_ascii_*` method; `is_whitespace` spells
// its set explicitly because PHP `ctype_space`'s set includes 0x0B (vertical tab) which Rust
// `u8::is_ascii_whitespace` OMITS — the Rust set below matches the emitted `[\x09-\x0D\x20]` class.

/// `ctype_lower` — non-empty, all ASCII lowercase letters.
fn is_lower(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase())
}
/// `ctype_upper` — non-empty, all ASCII uppercase letters.
fn is_upper(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_uppercase())
}
/// `ctype_space` — non-empty, all whitespace. The set is spelled out because PHP `ctype_space`
/// counts 0x0B (vertical tab) but `u8::is_ascii_whitespace` does not.
fn is_whitespace(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r'))
}
/// `ctype_punct` — non-empty, all printable non-alphanumeric non-space (== `is_ascii_punctuation`).
fn is_punct(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_punctuation())
}
/// `ctype_cntrl` — non-empty, all control chars (0x00–0x1F, 0x7F).
fn is_control(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_control())
}
/// `ctype_graph` — non-empty, all visible chars (printable excluding space, 0x21–0x7E).
fn is_visible(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_graphic())
}
/// `ctype_print` — non-empty, all printable chars including space (0x20–0x7E).
fn is_printable(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

/// `^(?!.*\.\.)[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}$` (D flag).
/// A syntactic e-mail check (not a semantic/deliverability one): a local part, `@`, then a DOTTED
/// domain whose final label (TLD) is >= 2 ASCII letters — so `user@localhost` is false. Consecutive
/// dots are rejected anywhere via the `(?!.*\.\.)` negative lookahead → `s.contains("..")` here, so
/// `a..b@c.com` is false. Byte-identity with the PCRE is airtight: PCRE acceptance requires the
/// string hold no `\n` (every char class excludes it, and the `D` flag pins `$` to the absolute end),
/// and on a `\n`-free string `(?!.*\.\.)` is exactly `!contains("..")`.
fn is_email(s: &str) -> bool {
    // (?!.*\.\.) — no two adjacent dots anywhere.
    if s.contains("..") {
        return false;
    }
    // The local class excludes `@` and every domain class excludes `@`, so a match holds EXACTLY one
    // `@`: split on the first, reject a second.
    let mut it = s.splitn(2, '@');
    let local = it.next().unwrap_or("");
    let Some(domain) = it.next() else {
        return false; // no '@'
    };
    if domain.contains('@') {
        return false; // more than one '@'
    }
    // local = [A-Za-z0-9._%+-]+
    if local.is_empty()
        || !local
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'%' | b'+' | b'-'))
    {
        return false;
    }
    // domain = [A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}
    //        = HEAD "." TLD, where TLD is letters-only (>= 2) and HEAD is >= 1 dot-separated,
    //          non-empty [A-Za-z0-9-] labels. The final `\.[A-Za-z]{2,}$` binds to the LAST dot
    //          (TLD is letters-only to `$`), so `rsplit_once('.')` splits identically.
    let Some((head, tld)) = domain.rsplit_once('.') else {
        return false; // undotted domain (e.g. `localhost`)
    };
    if tld.len() < 2 || !tld.bytes().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    !head.is_empty()
        && head.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}

/// `^https?://[A-Za-z0-9.-]+(:[0-9]+)?(/[^\x00-\x20]*)?$` (D flag).
/// A syntactic URL check: an `http`/`https` scheme, a host of `[A-Za-z0-9.-]`, an optional `:port`
/// (digits), and an optional `/path` of any non-control, non-space bytes. The path class is
/// `[^\x00-\x20]` (bytes > 0x20) rather than `[^\s]`: it is trivially byte-identical to the Rust
/// `c > 0x20` scan and sidesteps PCRE's `\s`-set (which is a divergence hazard). The `D` flag pins
/// `$` to the absolute end (no trailing-`\n` acceptance).
fn is_url(s: &str) -> bool {
    let rest = if let Some(r) = s.strip_prefix("https://") {
        r
    } else if let Some(r) = s.strip_prefix("http://") {
        r
    } else {
        return false;
    };
    // host = [A-Za-z0-9.-]+ — the class excludes `:` and `/`, so it ends at the first of either.
    let host_end = rest
        .bytes()
        .position(|c| c == b':' || c == b'/')
        .unwrap_or(rest.len());
    let host = &rest[..host_end];
    if host.is_empty()
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-'))
    {
        return false;
    }
    let mut tail = &rest[host_end..];
    // (:[0-9]+)? — a lone `:` (or `:` + non-digits) can be consumed by nothing else, so an invalid
    // port fails the whole match, exactly as this early return does.
    if let Some(after) = tail.strip_prefix(':') {
        let port_end = after.bytes().position(|c| c == b'/').unwrap_or(after.len());
        let port = &after[..port_end];
        if port.is_empty() || !port.bytes().all(|c| c.is_ascii_digit()) {
            return false;
        }
        tail = &after[port_end..];
    }
    // (/[^\x00-\x20]*)? — optional; empty tail matches the empty option.
    if tail.is_empty() {
        return true;
    }
    let Some(path) = tail.strip_prefix('/') else {
        return false; // trailing bytes that are neither a port nor a `/`-path
    };
    path.bytes().all(|c| c > 0x20)
}

fn pred(args: &[Value], f: fn(&str) -> bool, who: &str) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Bool(f(s))),
        _ => Err(format!("Validation.{who} expects (string)")),
    }
}
fn is_int_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_int, "isInt")
}
fn is_number_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_number, "isNumber")
}
fn is_alpha_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_alpha, "isAlpha")
}
fn is_alnum_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_alnum, "isAlnum")
}
fn is_hex_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_hex, "isHex")
}
fn is_lower_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_lower, "isLower")
}
fn is_upper_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_upper, "isUpper")
}
fn is_whitespace_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_whitespace, "isWhitespace")
}
fn is_punct_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_punct, "isPunctuation")
}
fn is_control_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_control, "isControl")
}
fn is_visible_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_visible, "isVisible")
}
fn is_printable_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_printable, "isPrintable")
}
fn is_email_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_email, "isEmail")
}
fn is_url_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    pred(a, is_url, "isUrl")
}

/// The `Core.Validation` registry entries. Each `string -> bool`, the Rust hand-roll mirrored by a PHP
/// `preg_match(pattern) === 1` over the identical anchored pattern (the char-class predicates add the
/// `D` flag — see the note above `is_lower`).
pub(crate) fn validate_natives() -> Vec<NativeFn> {
    fn entry(
        name: &'static str,
        eval: fn(&[Value], &mut String) -> Result<Value, String>,
        lift_from: &'static [&'static str],
        php: fn(&[String]) -> String,
    ) -> NativeFn {
        NativeFn {
            module: "Core.Validation",
            name,
            params: vec![Ty::String],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(eval),
            lift_from,
            php,
        }
    }
    vec![
        entry("isInt", is_int_native, &[], |a| {
            format!("(preg_match('/^[+-]?[0-9]+$/', {}) === 1)", parg(a, 0))
        }),
        entry("isNumber", is_number_native, &[], |a| {
            format!(
                "(preg_match('/^[+-]?[0-9]+(\\.[0-9]+)?$/', {}) === 1)",
                parg(a, 0)
            )
        }),
        entry("isAlpha", is_alpha_native, &[], |a| {
            format!("(preg_match('/^[A-Za-z]+$/', {}) === 1)", parg(a, 0))
        }),
        entry("isAlnum", is_alnum_native, &[], |a| {
            format!("(preg_match('/^[A-Za-z0-9]+$/', {}) === 1)", parg(a, 0))
        }),
        entry("isHex", is_hex_native, &[], |a| {
            format!("(preg_match('/^[0-9A-Fa-f]+$/', {}) === 1)", parg(a, 0))
        }),
        // ctype-class predicates → `preg_match` over an explicit `\xNN` char class with the `D`
        // (PCRE_DOLLAR_ENDONLY) flag. PCRE is always compiled (`ctype_*` is a SHARED extension absent
        // under the hermetic `php -n` oracle — the ctype_digit bug); `\xNN` ranges avoid delimiter
        // escaping and match the Rust `is_ascii_*` kernels exactly; `D` makes `$` match only the
        // absolute end, killing the trailing-`\n` divergence the pre-D validators above still carry.
        entry("isLower", is_lower_native, &[], |a| {
            format!("(preg_match('/^[a-z]+$/D', {}) === 1)", parg(a, 0))
        }),
        entry("isUpper", is_upper_native, &[], |a| {
            format!("(preg_match('/^[A-Z]+$/D', {}) === 1)", parg(a, 0))
        }),
        // ctype_space set = { \t \n \x0B \f \r space } = 0x09–0x0D plus 0x20.
        entry("isWhitespace", is_whitespace_native, &[], |a| {
            format!(
                "(preg_match('/^[\\x09-\\x0D\\x20]+$/D', {}) === 1)",
                parg(a, 0)
            )
        }),
        // punctuation = printable non-alnum non-space: 0x21–2F, 0x3A–40, 0x5B–60, 0x7B–7E.
        entry("isPunctuation", is_punct_native, &[], |a| {
            format!(
                "(preg_match('/^[\\x21-\\x2F\\x3A-\\x40\\x5B-\\x60\\x7B-\\x7E]+$/D', {}) === 1)",
                parg(a, 0)
            )
        }),
        entry("isControl", is_control_native, &[], |a| {
            format!(
                "(preg_match('/^[\\x00-\\x1F\\x7F]+$/D', {}) === 1)",
                parg(a, 0)
            )
        }),
        // visible = printable excluding space (0x21–0x7E).
        entry("isVisible", is_visible_native, &[], |a| {
            format!("(preg_match('/^[\\x21-\\x7E]+$/D', {}) === 1)", parg(a, 0))
        }),
        // printable including space (0x20–0x7E).
        entry("isPrintable", is_printable_native, &[], |a| {
            format!("(preg_match('/^[\\x20-\\x7E]+$/D', {}) === 1)", parg(a, 0))
        }),
        // isEmail — dotted domain + letters-only TLD (>=2); `(?!.*\.\.)` bars consecutive dots.
        entry("isEmail", is_email_native, &[], |a| {
            format!(
                "(preg_match('/^(?!.*\\.\\.)[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\\.[A-Za-z0-9-]+)*\\.[A-Za-z]{{2,}}$/D', {}) === 1)",
                parg(a, 0)
            )
        }),
        // isUrl — http/https scheme, host, optional :port, optional /path (delimiter `/` escaped as `\/`).
        entry("isUrl", is_url_native, &[], |a| {
            format!(
                "(preg_match('/^https?:\\/\\/[A-Za-z0-9.-]+(:[0-9]+)?(\\/[^\\x00-\\x20]*)?$/D', {}) === 1)",
                parg(a, 0)
            )
        }),
    ]
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
