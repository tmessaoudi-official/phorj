//! URL percent-encoding (native-stdlib wave, Tier A; merged into the Uri module by DEC-279).
//!
//! Pure, deterministic, std-only. The CURRENT surface is the `Core.Native.Uri` rows here, wrapped
//! by `Uri.encodeForm`/`encodeComponent`/`decodeForm`/`decodeComponent` statics in the
//! `Core.UriModule` prelude. The old `Core.Url` module rows are KEPT REGISTERED as a deprecated
//! twin (every `(Core.Url, *)` symbol has a `deprecation_of` row → `W-DEPRECATED` with the new
//! path; the natives keep working for ≥1 minor release per `docs/DEPRECATION.md`).
//!
//! Encoders (`string -> string`) and decoders (`string -> string?`) are byte-identical to PHP
//! `urlencode` / `rawurlencode` / `urldecode` / `rawurldecode`. The `encodeForm`/`decodeForm` pair
//! is the `application/x-www-form-urlencoded` form (space ⇒ `+`, `~` encoded); the
//! `encodeComponent`/`decodeComponent` pair (né `encodeUriComponent`/`decodeUriComponent` — the
//! `Uri` qualifier makes the infix redundant) is RFC 3986 (space ⇒ `%20`, `~` left as-is).
//! Decoders return `string?` — `null` when the decoded bytes are not valid UTF-8 (a Phorj
//! `string` is UTF-8; the PHP side mirrors with a `//u` check), so they stay byte-identical.

use super::*;
use crate::types::Ty;
use crate::value::Value;

/// Percent-encode `s`. `raw` selects RFC-3986 form (space → `%20`, `~` unreserved); otherwise the
/// form-encoded variant (space → `+`, `~` encoded). Uppercase hex, matching PHP.
fn pct_encode(s: &str, raw: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let unreserved =
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') || (raw && b == b'~');
        if unreserved {
            out.push(b as char);
        } else if !raw && b == b' ' {
            out.push('+');
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xf) as usize] as char);
        }
    }
    out
}

/// Percent-decode `s` (lenient, like PHP: an invalid `%` escape is left literal). `raw=false` also
/// turns `+` into a space. Returns `None` when the decoded bytes are not valid UTF-8.
fn pct_decode(s: &str, raw: bool) -> Option<String> {
    let bytes = s.as_bytes();
    let hexval = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hexval(bytes[i + 1]), hexval(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
            out.push(b'%'); // invalid escape → literal '%'
            i += 1;
        } else if !raw && b == b'+' {
            out.push(b' ');
            i += 1;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn encode_native(args: &[Value], raw: bool, who: &str) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(pct_encode(s, raw).into())),
        _ => Err(format!("Uri.{who} expects (string)")),
    }
}
fn decode_native(args: &[Value], raw: bool, who: &str) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(match pct_decode(s, raw) {
            Some(d) => Value::Str(d.into()),
            None => Value::Null,
        }),
        _ => Err(format!("Uri.{who} expects (string)")),
    }
}
fn url_encode_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    encode_native(a, false, "encodeForm")
}
fn raw_url_encode_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    encode_native(a, true, "encodeComponent")
}
fn url_decode_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    decode_native(a, false, "decodeForm")
}
fn raw_url_decode_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    decode_native(a, true, "decodeComponent")
}

/// PHP emission for a decoder: decode, then return the string only if it is valid UTF-8 (matching the
/// Rust `String::from_utf8` guard), else `null` — so the `string?` result stays byte-identical. Uses
/// the same `preg_match('//u', …) === 1` validity idiom as `Core.Bytes.toString` (PCRE is core).
fn php_decode(func: &str, arg: &str) -> String {
    format!("(preg_match('//u', ($__u = {func}({arg}))) === 1 ? $__u : null)")
}

/// The percent-encoding registry entries: the current `Core.Native.Uri` rows (wrapped by the
/// `Uri.*` prelude statics) + the deprecated `Core.Url` twin rows (DEC-279 — same eval/php bodies,
/// flagged in [`super::deprecation_of`], removed after the deprecation window).
pub(crate) fn url_natives() -> Vec<NativeFn> {
    let row = |module, name, decode: bool, eval, php| NativeFn {
        module,
        name,
        params: vec![Ty::String],
        ret: if decode {
            Ty::Optional(Box::new(Ty::String))
        } else {
            Ty::String
        },
        pure: true,
        eval: NativeEval::Pure(eval),
        php,
    };
    vec![
        // The current surface (DEC-279): percent-encoding lives in the Uri module.
        row(
            "Core.Native.Uri",
            "encodeForm",
            false,
            url_encode_native,
            |a| format!("urlencode({})", parg(a, 0)),
        ),
        row(
            "Core.Native.Uri",
            "encodeComponent",
            false,
            raw_url_encode_native,
            |a| format!("rawurlencode({})", parg(a, 0)),
        ),
        row(
            "Core.Native.Uri",
            "decodeForm",
            true,
            url_decode_native,
            |a| php_decode("urldecode", parg(a, 0)),
        ),
        row(
            "Core.Native.Uri",
            "decodeComponent",
            true,
            raw_url_decode_native,
            |a| php_decode("rawurldecode", parg(a, 0)),
        ),
        // The deprecated `Core.Url` twin (kept working; W-DEPRECATED points at the new path).
        row("Core.Url", "encodeForm", false, url_encode_native, |a| {
            format!("urlencode({})", parg(a, 0))
        }),
        row(
            "Core.Url",
            "encodeUriComponent",
            false,
            raw_url_encode_native,
            |a| format!("rawurlencode({})", parg(a, 0)),
        ),
        row("Core.Url", "decodeForm", true, url_decode_native, |a| {
            php_decode("urldecode", parg(a, 0))
        }),
        row(
            "Core.Url",
            "decodeUriComponent",
            true,
            raw_url_decode_native,
            |a| php_decode("rawurldecode", parg(a, 0)),
        ),
    ]
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
