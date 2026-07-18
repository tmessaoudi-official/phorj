use crate::value::Value;
// The `String.format` renderer lives in the sibling `text_format` module (M-Decomp, Invariant 13);
// `text_natives()` below registers it as `Core.String.format`.

pub(super) fn text_len(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Int(s.len() as i64)),
        _ => Err("String.length expects (string)".into()),
    }
}
pub(super) fn text_upper(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.to_ascii_uppercase().into())),
        _ => Err("String.upperCase expects (string)".into()),
    }
}
pub(super) fn text_lower(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.to_ascii_lowercase().into())),
        _ => Err("String.lowerCase expects (string)".into()),
    }
}
pub(super) fn text_trim(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.trim().into())),
        _ => Err("String.trim expects (string)".into()),
    }
}
pub(super) fn text_contains(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(sub)] => Ok(Value::Bool(s.contains(sub.as_str()))),
        _ => Err("String.contains expects (string, string)".into()),
    }
}
// ASCII-oriented like the rest of Core.Text (PHP under `-n` has no mbstring). `reverse` reverses by
// chars (== bytes for ASCII, matching PHP `strrev`); `equalsIgnoreCase`/`containsIgnoreCase` fold only
// ASCII letters (== PHP `strcasecmp`/`stripos` in the C locale). Non-ASCII is a documented edge.
pub(super) fn text_reverse(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.chars().rev().collect())),
        _ => Err("String.reverse expects (string)".into()),
    }
}
pub(super) fn text_equals_ignore_case(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(a), Value::Str(b)] => Ok(Value::Bool(a.eq_ignore_ascii_case(b))),
        _ => Err("String.equalsIgnoreCase expects (string, string)".into()),
    }
}
pub(super) fn text_contains_ignore_case(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(h), Value::Str(n)] => Ok(Value::Bool(
            h.to_ascii_lowercase().contains(&n.to_ascii_lowercase()),
        )),
        _ => Err("String.containsIgnoreCase expects (string, string)".into()),
    }
}
pub(super) fn text_split(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(sep)] => {
            // An empty separator is ill-defined and FAULTS (output-parity pass 2026-07-05): PHP
            // `explode("")` hard-throws `ValueError`, while Rust `str::split("")` would return a
            // per-char-with-empty-ends list — a byte-identity break. To split into characters use
            // `String.characters` (code-point-safe).
            if sep.is_empty() {
                return Err("String.split: separator must not be empty".into());
            }
            let parts: Vec<Value> = s
                .split(sep.as_str())
                .map(|p| Value::Str(p.into()))
                .collect();
            Ok(Value::List(std::rc::Rc::new(parts)))
        }
        _ => Err("String.split expects (string, string)".into()),
    }
}
/// `String.characters(string) -> List<string>` — each Unicode CODE POINT as its own one-char string
/// (`"café"` → `["c","a","f","é"]`, not broken UTF-8). The named, code-point-safe way to split into
/// characters (parallels `String.lines`); `split(s, "")` faults. Empty string → empty list. Erases to
/// PHP `preg_split('//u', …, PREG_SPLIT_NO_EMPTY)` (code points without mbstring — same kernel as
/// `__phorj_text_reverse`).
pub(super) fn text_characters(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::List(std::rc::Rc::new(
            s.chars()
                .map(|c| Value::Str(crate::phstr::PhStr::new(c.encode_utf8(&mut [0u8; 4]))))
                .collect(),
        ))),
        _ => Err("String.characters expects (string)".into()),
    }
}

pub(super) fn text_split_once(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // Split on the FIRST occurrence → `[head, tail]`; `[whole]` (1 elem) if `sep` is absent.
        // Matches PHP `explode($sep, $s, 2)` exactly for a non-empty separator (the only use).
        [Value::Str(s), Value::Str(sep)] => {
            let parts: Vec<Value> = match s.split_once(sep.as_str()) {
                Some((head, tail)) => vec![Value::Str(head.into()), Value::Str(tail.into())],
                None => vec![Value::Str(s.clone())],
            };
            Ok(Value::List(std::rc::Rc::new(parts)))
        }
        _ => Err("String.splitOnce expects (string, string)".into()),
    }
}
// `capitalize(string) -> string` — uppercase the first character if it is an ASCII lowercase letter,
// else unchanged. Byte-for-byte PHP `ucfirst` (which only upcases a leading a-z byte; a multibyte first
// codepoint is left as-is). ASCII-scoped, like `upper`/`reverse` — documented (no mbstring under `php -n`).
pub(super) fn text_capitalize(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => {
            let out = match s.as_bytes().first() {
                Some(b) if b.is_ascii_lowercase() => {
                    let mut v = s.as_bytes().to_vec();
                    v[0] = b - 32;
                    String::from_utf8(v)
                        .expect("only a leading ASCII byte was changed")
                        .into()
                }
                _ => s.clone(),
            };
            Ok(Value::Str(out))
        }
        _ => Err("String.capitalize expects (string)".into()),
    }
}

// `capitalizeWords(string) -> string` — uppercase the first ASCII letter of each word (a word starts at
// string start or after a whitespace byte ` \t\r\n\f\v`). Byte-for-byte PHP `ucwords` (ASCII-scoped,
// like `capitalize`/`upper` — documented; no mbstring under `php -n`).
pub(super) fn text_capitalize_words(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => {
            let mut v = s.as_bytes().to_vec();
            let mut prev_delim = true; // string start counts as a word boundary
            for b in v.iter_mut() {
                if prev_delim && b.is_ascii_lowercase() {
                    *b -= 32;
                }
                prev_delim = matches!(*b, b' ' | b'\t' | b'\r' | b'\n' | 0x0c | 0x0b);
            }
            Ok(Value::Str(
                String::from_utf8(v)
                    .expect("only ASCII letters were changed")
                    .into(),
            ))
        }
        _ => Err("String.capitalizeWords expects (string)".into()),
    }
}

// `translate(string, from, to) -> string` — replace each byte present in `from` with the byte at the
// same index in `to` (the shorter of from/to bounds the pairing; a from-byte's FIRST pairing wins, like
// PHP). Byte-for-byte PHP `strtr($s, $from, $to)` (byte-level; ASCII/bytes-scoped).
pub(super) fn text_translate(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(from), Value::Str(to)] => {
            let from = from.as_bytes();
            let to = to.as_bytes();
            let n = from.len().min(to.len());
            let mut map: [u8; 256] = std::array::from_fn(|i| i as u8);
            let mut set = [false; 256]; // first pairing wins (matches PHP strtr)
            for i in 0..n {
                let k = from[i] as usize;
                if !set[k] {
                    map[k] = to[i];
                    set[k] = true;
                }
            }
            let out: Vec<u8> = s.as_bytes().iter().map(|&b| map[b as usize]).collect();
            Ok(Value::Str(
                String::from_utf8_lossy(&out).into_owned().into(),
            ))
        }
        _ => Err("String.translate expects (string, string, string)".into()),
    }
}
// `lines(string) -> List<string>` — split on `\n` (an embedded `\r` is left in the line, matching PHP
// `explode("\n", s)`). An empty string → `[""]`; a trailing `\n` → a trailing `""` (explode semantics).
pub(super) fn text_lines(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => {
            let parts: Vec<Value> = s.split('\n').map(|p| Value::Str(p.into())).collect();
            Ok(Value::List(std::rc::Rc::new(parts)))
        }
        _ => Err("String.lines expects (string)".into()),
    }
}
pub(super) fn text_join(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(items), Value::Str(sep)] => {
            let mut parts: Vec<&str> = Vec::with_capacity(items.len());
            for it in items.iter() {
                match it {
                    Value::Str(s) => parts.push(s.as_str()),
                    other => {
                        return Err(format!(
                            "String.join expects List<string>, found element of type {}",
                            other.type_name()
                        ))
                    }
                }
            }
            Ok(Value::Str(parts.join(sep.as_str()).into()))
        }
        _ => Err("String.join expects (List<string>, string)".into()),
    }
}
pub(super) fn text_replace(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(from), Value::Str(to)] => {
            Ok(Value::Str(s.replace(from.as_str(), to.as_str()).into()))
        }
        _ => Err("String.replace expects (string, string, string)".into()),
    }
}
pub(super) fn text_starts_with(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(pre)] => Ok(Value::Bool(s.starts_with(pre.as_str()))),
        _ => Err("String.startsWith expects (string, string)".into()),
    }
}
pub(super) fn text_ends_with(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(suf)] => Ok(Value::Bool(s.ends_with(suf.as_str()))),
        _ => Err("String.endsWith expects (string, string)".into()),
    }
}
/// DEC-243 — PHP-parity `levenshtein()`: classic Wagner–Fischer on BYTES (PHP's semantics —
/// byte-oriented, unit costs). Two rows of the DP matrix; O(len(a)*len(b)).
/// DEC-256 — codepoint count (transpilable tier: PHP leg is PCRE-`/us`).
pub(super) fn text_codepoint_length(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Int(s.chars().count() as i64)),
        _ => Err("String.codepointLength expects (string)".into()),
    }
}

/// DEC-256 — the codepoints as their scalar values (transpilable via the gated PHP helper).
pub(super) fn text_codepoints(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::List(std::rc::Rc::new(
            s.chars().map(|c| Value::Int(c as i64)).collect(),
        ))),
        _ => Err("String.codepoints expects (string)".into()),
    }
}

/// DEC-256 — FULL Unicode case mapping (native-only: one-to-many expansions like ß→SS).
pub(super) fn text_unicode_upper(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.to_uppercase().into())),
        _ => Err("String.unicodeUpper expects (string)".into()),
    }
}

pub(super) fn text_unicode_lower(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.to_lowercase().into())),
        _ => Err("String.unicodeLower expects (string)".into()),
    }
}

/// DEC-256 — UAX #29 extended grapheme clusters (native-only; `unicode-segmentation`).
#[cfg(feature = "unicode")]
pub(super) fn text_grapheme_length(args: &[Value], _: &mut String) -> Result<Value, String> {
    use unicode_segmentation::UnicodeSegmentation;
    match args {
        [Value::Str(s)] => Ok(Value::Int(s.graphemes(true).count() as i64)),
        _ => Err("String.graphemeLength expects (string)".into()),
    }
}

#[cfg(feature = "unicode")]
pub(super) fn text_graphemes(args: &[Value], _: &mut String) -> Result<Value, String> {
    use unicode_segmentation::UnicodeSegmentation;
    match args {
        [Value::Str(s)] => Ok(Value::List(std::rc::Rc::new(
            s.graphemes(true)
                .map(|g| Value::Str(g.to_string().into()))
                .collect(),
        ))),
        _ => Err("String.graphemes expects (string)".into()),
    }
}

pub(super) fn text_levenshtein(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(a), Value::Str(b)] => {
            let (a, b) = (a.as_bytes(), b.as_bytes());
            let mut prev: Vec<i64> = (0..=b.len() as i64).collect();
            let mut cur = vec![0i64; b.len() + 1];
            for (i, &ca) in a.iter().enumerate() {
                cur[0] = i as i64 + 1;
                for (j, &cb) in b.iter().enumerate() {
                    let sub = prev[j] + i64::from(ca != cb);
                    cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
                }
                std::mem::swap(&mut prev, &mut cur);
            }
            Ok(Value::Int(prev[b.len()]))
        }
        _ => Err("String.levenshtein expects (string, string)".into()),
    }
}

/// DEC-243 — PHP-parity `similar_text()` count: Oliver's algorithm on BYTES — find the longest
/// common substring, recurse on both sides, sum the lengths (exactly PHP's php_similar_str).
fn sim_count(a: &[u8], b: &[u8]) -> i64 {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    // Longest common substring positions (first-found on ties, matching PHP's scan order).
    let (mut pos1, mut pos2, mut max) = (0usize, 0usize, 0usize);
    for i in 0..a.len() {
        for j in 0..b.len() {
            let mut k = 0;
            while i + k < a.len() && j + k < b.len() && a[i + k] == b[j + k] {
                k += 1;
            }
            if k > max {
                (pos1, pos2, max) = (i, j, k);
            }
        }
    }
    if max == 0 {
        return 0;
    }
    max as i64 + sim_count(&a[..pos1], &b[..pos2]) + sim_count(&a[pos1 + max..], &b[pos2 + max..])
}

pub(super) fn text_similar(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(a), Value::Str(b)] => Ok(Value::Int(sim_count(a.as_bytes(), b.as_bytes()))),
        _ => Err("String.similarText expects (string, string)".into()),
    }
}

/// DEC-243 — PHP's by-reference `$percent` twin as an honest VALUE return:
/// `sim * 200.0 / (len(a)+len(b))`, `0.0` when both are empty (PHP leaves the ref untouched at 0).
pub(super) fn text_similar_percent(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(a), Value::Str(b)] => {
            let total = (a.len() + b.len()) as f64;
            if total == 0.0 {
                return Ok(Value::Float(0.0));
            }
            let sim = sim_count(a.as_bytes(), b.as_bytes()) as f64;
            Ok(Value::Float(sim * 200.0 / total))
        }
        _ => Err("String.similarTextPercent expects (string, string)".into()),
    }
}

pub(super) fn text_repeat(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // PHP `str_repeat` requires count >= 0 (a `ValueError` otherwise); a negative count faults
        // cleanly here (EV-7 — never panic; `n as usize` on a negative i64 would be a huge alloc).
        [Value::Str(s), Value::Int(n)] => {
            if *n < 0 {
                return Err("String.repeat count must be >= 0".into());
            }
            Ok(Value::Str(s.repeat(*n as usize).into()))
        }
        _ => Err("String.repeat expects (string, int)".into()),
    }
}
/// Shared byte-level pad (PHP `str_pad`): if `s` is already >= `width` bytes (or `pad` is empty), `s`
/// is returned unchanged; otherwise `pad` is repeated (last copy truncated) to fill the gap, on the
/// left or right. Byte-based to match PHP (no mbstring); the example domain is ASCII. An empty pad
/// faults cleanly (PHP `ValueError`); a multibyte pad truncated mid-char yields invalid UTF-8 →
/// faults rather than panicking (EV-7).
pub(super) fn text_pad(s: &str, width: i64, pad: &str, left: bool) -> Result<Value, String> {
    let cur = s.len();
    let want = if width < 0 { 0 } else { width as usize };
    if cur >= want {
        return Ok(Value::Str(s.into()));
    }
    if pad.is_empty() {
        return Err("String.pad: pad string must not be empty".into());
    }
    let needed = want - cur;
    let pb = pad.as_bytes();
    let padding: Vec<u8> = (0..needed).map(|i| pb[i % pb.len()]).collect();
    let mut out = Vec::with_capacity(want);
    if left {
        out.extend_from_slice(&padding);
        out.extend_from_slice(s.as_bytes());
    } else {
        out.extend_from_slice(s.as_bytes());
        out.extend_from_slice(&padding);
    }
    String::from_utf8(out)
        .map(|s| Value::Str(s.into()))
        .map_err(|_| "String.pad: pad split a multibyte character (use an ASCII pad)".into())
}
pub(super) fn text_pad_left(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Int(w), Value::Str(p)] => text_pad(s, *w, p, true),
        _ => Err("String.padLeft expects (string, int, string)".into()),
    }
}
pub(super) fn text_pad_right(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Int(w), Value::Str(p)] => text_pad(s, *w, p, false),
        _ => Err("String.padRight expects (string, int, string)".into()),
    }
}
/// `indexOf(string, string) -> int?` — the byte offset of the first occurrence of `needle`, else
/// `null` (PHP `strpos`, mapped from `false`). An empty needle is `0` (PHP 8 + Rust `find` agree).
pub(super) fn text_index_of(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(needle)] => Ok(s
            .find(needle.as_str())
            .map_or(Value::Null, |i| Value::Int(i as i64))),
        _ => Err("String.indexOf expects (string, string)".into()),
    }
}
/// `lastIndexOf(string, string) -> int?` — the byte offset of the **last** occurrence of `needle`,
/// else `null` (PHP `strrpos`, mapped from `false`). An empty needle is `strlen(s)` (PHP 8 + Rust
/// `rfind` agree). The byte/`int?` complement of `indexOf`.
pub(super) fn text_last_index_of(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(needle)] => Ok(s
            .rfind(needle.as_str())
            .map_or(Value::Null, |i| Value::Int(i as i64))),
        _ => Err("String.lastIndexOf expects (string, string)".into()),
    }
}
/// `removePrefix(string, string) -> string` — drop a leading `prefix` if present, else return `s`
/// unchanged (Kotlin/Swift ergonomics; PHP `str_starts_with` + `substr`). An empty prefix is a no-op.
pub(super) fn text_remove_prefix(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(pre)] => {
            Ok(Value::Str(s.strip_prefix(pre.as_str()).unwrap_or(s).into()))
        }
        _ => Err("String.removePrefix expects (string, string)".into()),
    }
}
/// `removeSuffix(string, string) -> string` — drop a trailing `suffix` if present, else return `s`
/// unchanged (PHP `str_ends_with` + `substr`). An empty suffix is a no-op.
pub(super) fn text_remove_suffix(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(suf)] => {
            Ok(Value::Str(s.strip_suffix(suf.as_str()).unwrap_or(s).into()))
        }
        _ => Err("String.removeSuffix expects (string, string)".into()),
    }
}
/// The float grammar (M4 `parseFloat`): `[+-]? digits? (. digits?)? ([eE][+-]?digits)?` with the
/// **strict**/**permissive** difference being only the leading/trailing dot. STRICT requires leading
/// integer digits and (if a dot is present) trailing fractional digits — `1`, `1.5`, `-2.5e3` ok;
/// `.5`, `5.` rejected. PERMISSIVE additionally accepts a lone leading or trailing dot (`.5`, `5.`),
/// requiring only one digit overall. **Both reject `inf`/`nan`** (the grammar requires digits, so
/// those non-numeric words never match) — this is what keeps `parseFloat` byte-identical with PHP,
/// whose `(float)` cast can't produce inf/nan and whose rendering would otherwise diverge.
pub(super) fn valid_float(s: &str, permissive: bool) -> bool {
    let b = s.as_bytes();
    let n = b.len();
    let mut i = 0;
    if i < n && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < n && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut had_dot = false;
    let mut frac_digits = 0;
    if i < n && b[i] == b'.' {
        had_dot = true;
        i += 1;
        let f0 = i;
        while i < n && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - f0;
    }
    if permissive {
        if int_digits == 0 && frac_digits == 0 {
            return false; // a lone `.` (or `+`/`-`) is not a number
        }
    } else {
        if int_digits == 0 || (had_dot && frac_digits == 0) {
            return false; // strict: digits before, and after any dot
        }
    }
    if i < n && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < n && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let e0 = i;
        while i < n && b[i].is_ascii_digit() {
            i += 1;
        }
        if i - e0 == 0 {
            return false; // exponent marker with no digits
        }
    }
    i == n // every byte consumed
}
/// `parseFloat(string, bool permissive = false) -> float?` — parse a base-10 float, or `None` when the
/// string fails the grammar (see [`valid_float`]). Rust's `f64::from_str` is the value source of truth
/// (run on the validator-accepted slice); the gated PHP helper `__phorj_parse_float` mirrors the
/// grammar + cast. The `permissive` flag has a default of `false` (M4 default parameters), so
/// `parseFloat(s)` is strict and `parseFloat(s, true)` is lax.
pub(super) fn text_parse_float(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Bool(permissive)] => {
            if valid_float(s, *permissive) {
                Ok(s.parse::<f64>().map_or(Value::Null, Value::Float))
            } else {
                Ok(Value::Null)
            }
        }
        _ => Err("String.parseFloat expects (string, bool)".into()),
    }
}
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
                (begin + *length).min(n)
            };
            String::from_utf8(bytes[begin as usize..end as usize].to_vec())
                .map(|s| Value::Str(s.into()))
                .map_err(|_| "String.substring split a multibyte character (byte-indexed)".into())
        }
        _ => Err("String.substring expects (string, int, int)".into()),
    }
}

/// The `Core.String` registry entries (M3 Track B Wave 2). NOTE the PHP arg order: `explode`/`implode`
/// take the separator first, and `str_replace` is `(search, replace, subject)` — the `php` closures
/// reorder accordingly so the erasure matches Phorj's `(subject, …)` argument order.
pub(super) fn text_is_empty(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Bool(s.is_empty())),
        _ => Err("String.isEmpty expects (string)".into()),
    }
}

pub(super) fn text_trim_start(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.trim_start().into())),
        _ => Err("String.trimStart expects (string)".into()),
    }
}

pub(super) fn text_trim_end(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(s.trim_end().into())),
        _ => Err("String.trimEnd expects (string)".into()),
    }
}

/// `Text.count(string, string) -> int` — non-overlapping occurrences of the substring (PHP
/// `substr_count`). An empty needle is a clean fault (PHP `substr_count` rejects it too).
pub(super) fn text_count(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), Value::Str(sub)] => {
            if sub.is_empty() {
                return Err("String.count: the substring must not be empty".into());
            }
            Ok(Value::Int(
                i64::try_from(s.matches(sub.as_str()).count()).unwrap_or(i64::MAX),
            ))
        }
        _ => Err("String.count expects (string, string)".into()),
    }
}
