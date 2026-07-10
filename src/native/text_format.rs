//! `String.format` — the PHP-style `%` sprintf renderer (W3-5 / DEC-199), split out of
//! `text.rs` (Invariant 13, M-Decomp) as a cohesive unit: the parsed-directive shape, the
//! shared parser (also driven by the compile-time gate `count_format_directives`), the
//! flag/width padder, and the per-conversion renderers (`%s`/`%d`/`%f`/`%e`/`%E`/`%g`/`%G`/
//! radix). `text_natives()` (in `text.rs`) registers `text_format` as `Core.String.format`;
//! the emitted PHP mirror `__phorj_format` lives in `transpile::program`.
use crate::value::Value;

/// One parsed `%…` directive: flags + width + optional precision + conversion char. Shared shape for
/// the Rust renderer here and the compile-time gate (`count_format_directives`) so they agree exactly.
pub(crate) struct FormatDirective {
    pub minus: bool,
    pub zero: bool,
    pub plus: bool,
    pub width: usize,
    pub precision: Option<usize>,
    pub conv: char,
}

/// Parse a `%…` directive body (the caller has consumed the `%`, and `%%` is handled separately). The
/// iterator is advanced past `[flags][width][.precision]conv`. Returns the directive, or an error
/// string for a dangling `%` or an unsupported shape (precision on `%s`/`%d`, an unknown conversion) —
/// so the runtime renderer and the compile-time gate reject exactly the same specs (this slice: flags
/// `-`/`0`/`+`, width, float-only precision on `%f`/`%e`/`%E`/`%g`/`%G`, conversions `s`/`d`/`f`/`e`/`E`/`g`/`G`/`x`/`X`/`o`/`b`).
pub(crate) fn parse_format_directive(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<FormatDirective, String> {
    let (mut minus, mut zero, mut plus) = (false, false, false);
    while let Some(&c) = chars.peek() {
        match c {
            '-' => minus = true,
            '0' => zero = true,
            '+' => plus = true,
            _ => break,
        }
        chars.next();
    }
    let mut width = 0usize;
    while let Some(&c) = chars.peek() {
        if let Some(d) = c.to_digit(10) {
            width = width * 10 + d as usize;
            chars.next();
        } else {
            break;
        }
    }
    let mut precision = None;
    if chars.peek() == Some(&'.') {
        chars.next();
        let mut p = 0usize;
        while let Some(&c) = chars.peek() {
            if let Some(d) = c.to_digit(10) {
                p = p * 10 + d as usize;
                chars.next();
            } else {
                break;
            }
        }
        precision = Some(p);
    }
    let conv = chars
        .next()
        .ok_or_else(|| "String.format: dangling `%` at the end of the format string".to_string())?;
    match conv {
        // Precision is supported on the float conversions `%f`/`%e`/`%E`/`%g`/`%G` — `%s`/`%d` and the
        // integer-radix conversions `%x`/`%X`/`%o`/`%b` (slice 3a) reject it (precision-as-min-digits
        // is a later slice).
        's' | 'd' | 'x' | 'X' | 'o' | 'b' if precision.is_some() => Err(format!(
            "String.format: precision on `%{conv}` is not supported yet (only the float conversions \
             `%f`/`%e`/`%E`/`%g`/`%G` take a precision this version)"
        )),
        's' | 'd' | 'f' | 'e' | 'E' | 'g' | 'G' | 'x' | 'X' | 'o' | 'b' => Ok(FormatDirective {
            minus,
            zero,
            plus,
            width,
            precision,
            conv,
        }),
        other => Err(format!(
            "String.format: unsupported directive `%{other}` (this version supports \
             %s, %d, %f, %e, %E, %g, %G, %x, %X, %o, %b, %%)"
        )),
    }
}

/// Pad a rendered numeric/string body to `width` per the flags — matching PHP `sprintf`: left-justify
/// (`-`) beats zero-pad; zero-pad puts the pad *after* the sign (`%05d` of -42 → `-0042`); otherwise
/// space-pad on the left. `sign` is the already-computed sign prefix (`""`/`"-"`/`"+"`); for `%s` it is
/// empty. Widths are byte-based (PHP semantics); the pad chars are single-byte, so this stays
/// byte-identical for multi-byte bodies too.
fn pad_format(sign: &str, body: &str, d: &FormatDirective) -> String {
    let cur = sign.len() + body.len();
    if cur >= d.width {
        return format!("{sign}{body}");
    }
    let fill = d.width - cur;
    if d.minus {
        format!("{sign}{body}{}", " ".repeat(fill))
    } else if d.zero {
        format!("{sign}{}{body}", "0".repeat(fill))
    } else {
        format!("{}{sign}{body}", " ".repeat(fill))
    }
}

/// Strip trailing zeros (and a now-trailing dot) from a `%g` FIXED-style body — but only when it has a
/// decimal point, so `"100"` stays `"100"` while `"100.000"` → `"100"` and `"0.500"` → `"0.5"`.
fn strip_g_fixed_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// Strip trailing zeros from a `%g` SCIENTIFIC mantissa, keeping at least one fraction digit:
/// `"1.00000"` → `"1.0"`, `"1.20000"` → `"1.2"`, `"1"` → `"1.0"`. PHP's `%g` always renders `D.D…e±X`
/// in scientific form (a deviation from C, which would strip to bare `"1e+20"`).
fn strip_g_sci_mantissa(m: &str) -> String {
    match m.split_once('.') {
        Some((int, frac)) => {
            let frac = frac.trim_end_matches('0');
            if frac.is_empty() {
                format!("{int}.0")
            } else {
                format!("{int}.{frac}")
            }
        }
        None => format!("{m}.0"),
    }
}

/// Render the MAGNITUDE of a `%g`/`%G` directive (C-printf `%g`; the caller supplies the sign). `prec`
/// is the SIGNIFICANT-digit count (default 6, normalized to ≥ 1 here); `upper` selects the `E` separator.
///
/// Algorithm (byte-matches php-8.5.8): round `mag` to `prec` significant digits via Rust `{:.*e}` (which
/// matches PHP's round-half-to-even) and read the exponent `X`. If `-4 ≤ X < prec` render FIXED-style —
/// placing the decimal point in the rounded digit string by `X` (string placement, so the value is never
/// re-rounded → no double-rounding) then stripping trailing zeros fully; otherwise render SCIENTIFIC-style
/// (mantissa keeps at least `.0`, exponent re-stamped to PHP's always-signed min-1-digit form, as in `%e`).
/// Non-finite `mag` (`inf`/`NaN`) has no exponent to place — Rust prints `inf`/`NaN`, returned verbatim
/// (PHP `INF`/`NaN` — a documented `%f`-class divergence on `inf`, kept out of examples).
fn format_g_body(mag: f64, prec: usize, upper: bool) -> String {
    let p = prec.max(1);
    let sci = format!("{:.*e}", p - 1, mag);
    let (mantissa, exp) = match sci.split_once('e') {
        Some(pair) => pair,
        None => return sci, // non-finite: no exponent to place
    };
    let x: i32 = match exp.parse() {
        Ok(x) => x,
        Err(_) => return sci, // defensive: unparseable exponent → pass through (never panic)
    };
    if x >= -4 && x < p as i32 {
        // FIXED style. `digits` = the `p` significant digits with the dot removed ("1.23457" → "123457").
        let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
        let body = if x >= 0 {
            // `x < p` ⇒ `x + 1 ≤ digits.len()`, so the split never overruns.
            let cut = (x as usize) + 1;
            let (int, frac) = digits.split_at(cut);
            if frac.is_empty() {
                int.to_string()
            } else {
                format!("{int}.{frac}")
            }
        } else {
            // x ∈ -4..=-1: "0." + (-x-1) leading zeros + all significant digits.
            format!("0.{}{}", "0".repeat((-x - 1) as usize), digits)
        };
        strip_g_fixed_zeros(&body)
    } else {
        // SCIENTIFIC style (same exponent re-stamp as `%e`).
        let m = strip_g_sci_mantissa(mantissa);
        let (esign, edigits) = match exp.strip_prefix('-') {
            Some(rest) => ('-', rest),
            None => ('+', exp),
        };
        let esep = if upper { 'E' } else { 'e' };
        format!("{m}{esep}{esign}{edigits}")
    }
}

/// `String.format` (W3-5 / DEC-199) — PHP-style `%` sprintf, rendered STRICTLY. Slice 1 shipped
/// `%s`/`%d`/`%%`; slice 2 adds flags (`-`/`0`/`+`), width, and `%f` (with `.precision`, default 6).
/// `%s` renders any scalar via the interpolation kernel (`as_display`); `%d` requires an int (else a
/// clean fault — the phorj upgrade over PHP's silent coercion); `%f` an int/float. The float rounding
/// is Rust `format!("{:.p}", x)` which is round-half-to-even, matching PHP `sprintf` exactly (verified).
/// The emitted PHP helper `__phorj_format` mirrors this byte-for-byte (it delegates `%d`/`%f` to real
/// `sprintf` and hand-pads `%s`); see `emit_runtime_helpers`.
pub(crate) fn text_format(args: &[Value], _: &mut String) -> Result<Value, String> {
    let (spec, items) = match args {
        [Value::Str(spec), Value::List(items)] => (spec, items),
        _ => return Err("String.format expects (string, list)".into()),
    };
    let mut out = String::new();
    let mut chars = spec.chars().peekable();
    let mut ai = 0usize;
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }
        let d = parse_format_directive(&mut chars)?;
        let v = items.get(ai).ok_or_else(|| {
            format!(
                "String.format: the format string needs at least {} value(s)",
                ai + 1
            )
        })?;
        ai += 1;
        match d.conv {
            's' => {
                let body = v.as_display().ok_or_else(|| {
                    format!("String.format: cannot format {} with %s", v.type_name())
                })?;
                out.push_str(&pad_format("", &body, &d));
            }
            'd' => {
                let n = match v {
                    Value::Int(n) => *n,
                    other => {
                        return Err(format!(
                            "String.format: %d expects an int, found {}",
                            other.type_name()
                        ))
                    }
                };
                let sign = if n < 0 {
                    "-"
                } else if d.plus {
                    "+"
                } else {
                    ""
                };
                out.push_str(&pad_format(sign, &n.unsigned_abs().to_string(), &d));
            }
            'f' => {
                let f = match v {
                    Value::Int(n) => *n as f64,
                    Value::Float(x) => *x,
                    other => {
                        return Err(format!(
                            "String.format: %f expects a number, found {}",
                            other.type_name()
                        ))
                    }
                };
                // Sign is by VALUE (`< 0.0`), not IEEE sign bit: PHP signs a `%f` iff the value is
                // negative, so `-0.0` renders "0.000000" (unsigned) and even a value that rounds to zero
                // keeps its sign (`%.2f` of -0.001 → "-0.00"). `is_sign_negative()` would wrongly sign
                // `-0.0` → a run≠php byte-identity break (verified vs php-8.5.8). Same rule as `%e`.
                let sign = if f < 0.0 {
                    "-"
                } else if d.plus {
                    "+"
                } else {
                    ""
                };
                let mag = format!("{:.*}", d.precision.unwrap_or(6), f.abs());
                out.push_str(&pad_format(sign, &mag, &d));
            }
            // Scientific notation (slice 3b): `%e`/`%E`. PHP renders `[sign]D.DDDDDD e[+-]EXP` — a single
            // lead digit, `precision` fraction digits (default 6), and an exponent that is ALWAYS signed
            // with a MINIMUM of one digit and NO leading zeros (`e+3`, `e+20`, `e-1`, `e+0`) — unlike
            // C/Rust's minimum-two-digit exponent. Rust `{:.*e}` on the magnitude gives the exact same
            // mantissa and round-half-to-even (verified vs php-8.5.8), plus an unsigned min-1-digit
            // exponent (`e0`/`e-4`/`e20`); we only re-stamp the exponent SIGN and pick the separator case.
            // Sign is by value (`< 0.0`): `-0.0` is not negative → no sign, matching PHP (which — unlike
            // `%g` — never signs a `-0.0` here). Non-finite input has no `e` to reformat (Rust prints
            // `NaN`/`inf`); we pass the body through unchanged (PHP prints `NaN`/`INF` — a pre-existing
            // `%f`-class divergence, kept out of examples, never a byte-identity claim).
            'e' | 'E' => {
                let f = match v {
                    Value::Int(n) => *n as f64,
                    Value::Float(x) => *x,
                    other => {
                        return Err(format!(
                            "String.format: %{} expects a number, found {}",
                            d.conv,
                            other.type_name()
                        ))
                    }
                };
                let sign = if f < 0.0 {
                    "-"
                } else if d.plus {
                    "+"
                } else {
                    ""
                };
                let raw = format!("{:.*e}", d.precision.unwrap_or(6), f.abs());
                let body = match raw.split_once('e') {
                    Some((mantissa, exp)) => {
                        let (esign, edigits) = match exp.strip_prefix('-') {
                            Some(rest) => ('-', rest),
                            None => ('+', exp),
                        };
                        let esep = if d.conv == 'E' { 'E' } else { 'e' };
                        format!("{mantissa}{esep}{esign}{edigits}")
                    }
                    None => raw,
                };
                out.push_str(&pad_format(sign, &body, &d));
            }
            // Shortest-repr (slice 3c): `%g`/`%G` — C-printf `%g`, precision = significant digits
            // (default 6). Unlike `%e`/`%f`, `%g` signs by the IEEE sign bit, so `-0.0` renders "-0"
            // (verified vs php-8.5.8: `%+g` of -0.0 → "-0", of +0.0 → "+0"). Body via `format_g_body`.
            'g' | 'G' => {
                let f = match v {
                    Value::Int(n) => *n as f64,
                    Value::Float(x) => *x,
                    other => {
                        return Err(format!(
                            "String.format: %{} expects a number, found {}",
                            d.conv,
                            other.type_name()
                        ))
                    }
                };
                let sign = if f.is_sign_negative() {
                    "-"
                } else if d.plus {
                    "+"
                } else {
                    ""
                };
                let body = format_g_body(f.abs(), d.precision.unwrap_or(6), d.conv == 'G');
                out.push_str(&pad_format(sign, &body, &d));
            }
            // Integer-radix conversions (slice 3a): hex `%x`/`%X`, octal `%o`, binary `%b`. UNSIGNED —
            // a negative int renders as its 64-bit two's-complement bit pattern, matching PHP `sprintf`
            // on a 64-bit build (`%x` of -1 → "ffff…"), so `n as u64` is the exact bridge. No sign is
            // ever emitted (radix conversions are unsigned; a `+`/space flag is inert, as in PHP).
            'x' | 'X' | 'o' | 'b' => {
                let n = match v {
                    Value::Int(n) => *n,
                    other => {
                        return Err(format!(
                            "String.format: %{} expects an int, found {}",
                            d.conv,
                            other.type_name()
                        ))
                    }
                };
                let u = n as u64;
                let body = match d.conv {
                    'x' => format!("{u:x}"),
                    'X' => format!("{u:X}"),
                    'o' => format!("{u:o}"),
                    'b' => format!("{u:b}"),
                    _ => unreachable!("outer match restricts to x/X/o/b"),
                };
                out.push_str(&pad_format("", &body, &d));
            }
            _ => unreachable!("parse_format_directive only returns s/d/f/e/E/g/G/x/X/o/b"),
        }
    }
    if ai != items.len() {
        return Err(format!(
            "String.format: the format string uses {ai} value(s) but {} were given",
            items.len()
        ));
    }
    Ok(Value::Str(out))
}

#[cfg(test)]
#[path = "text_format_tests.rs"]
mod tests;
