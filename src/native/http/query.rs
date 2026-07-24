//! `application/x-www-form-urlencoded` parsing (query strings AND form bodies) — the exact
//! semantics the PHP twin `__phorj_http_parse_query` mirrors byte-for-byte (never PHP's
//! `parse_str`, which mangles dots/spaces in keys and is last-wins):
//!   * pairs split on `&`; empty segments skipped;
//!   * key/value split at the FIRST `=` (values may contain `=`); no `=` → value `""`;
//!   * form-decode both key and value: `+` → space, `%XX` (exactly two hex digits) → byte,
//!     an invalid `%`-escape is kept literally;
//!   * a component whose DECODED bytes are not valid UTF-8 falls back to the UNDECODED original
//!     (parity-safe rule — phorj strings are UTF-8; PHP checks `preg_match('//u')` the same way);
//!   * duplicate keys: FIRST-occurrence key order, values appended (D8b first-wins + getAll).

/// Form-decode one component per the module rules; `None` = decoded bytes are not valid UTF-8.
/// `plus_is_space` distinguishes form components (true) from path segments (false — a literal
/// `+` in a path stays `+`, per RFC 3986; only `%XX` decodes there).
fn pct_decode(s: &str, plus_is_space: bool) -> Option<String> {
    let raw = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            // `%XX` needs exactly two hex digits; `raw.get` is None past the end (truncated escape).
            b'%' => match raw.get(i + 1..i + 3).and_then(|h| {
                let hi = (h[0] as char).to_digit(16)?;
                let lo = (h[1] as char).to_digit(16)?;
                Some(u8::try_from(hi * 16 + lo).expect("two hex digits fit a byte"))
            }) {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                }
                None => {
                    out.push(b'%');
                    i += 1;
                }
            },
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// Decode with the undecoded-original fallback (module doc rule 4).
fn decode_component(s: &str) -> String {
    pct_decode(s, true).unwrap_or_else(|| s.to_string())
}

/// Decode a request-target PATH (`%XX` only, `+` literal; same fallback rule).
pub(crate) fn decode_path(s: &str) -> String {
    pct_decode(s, false).unwrap_or_else(|| s.to_string())
}

/// Parse a query/form string into first-wins-ordered `(key, values)` pairs.
pub(crate) fn parse_query_pairs(s: &str) -> Vec<(String, Vec<String>)> {
    let mut pairs: Vec<(String, Vec<String>)> = Vec::new();
    for seg in s.split('&') {
        if seg.is_empty() {
            continue;
        }
        let (k, v) = match seg.find('=') {
            Some(eq) => (&seg[..eq], &seg[eq + 1..]),
            None => (seg, ""),
        };
        let key = decode_component(k);
        let val = decode_component(v);
        match pairs.iter_mut().find(|(pk, _)| *pk == key) {
            Some((_, vs)) => vs.push(val),
            None => pairs.push((key, vec![val])),
        }
    }
    pairs
}
