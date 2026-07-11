//! Decimal (fixed-point i128) kernels: align/scale, exact + rounded division, formatting.

use super::*;

/// Project a `Value` operand of a decimal op onto `(unscaled, scale)`: a `Decimal` verbatim, an `Int`
/// widened to scale 0 (the `decimal op int ⇒ decimal` rule). `None` for anything else — checker-
/// unreachable (the checker guarantees decimal operands are `decimal`/`int`), handled defensively.
fn dec_parts(v: &Value) -> Option<(i128, u8)> {
    match v {
        Value::Decimal { unscaled, scale } => Some((*unscaled, *scale)),
        Value::Int(n) => Some((i128::from(*n), 0)),
        _ => None,
    }
}

/// Multiply `unscaled` by `10^exp`, checked (an alignment that leaves `i128` range faults). Used to
/// align two decimals to a common scale before add/sub/compare.
fn dec_scale_up(unscaled: i128, exp: u8) -> Option<i128> {
    let factor = 10i128.checked_pow(u32::from(exp))?;
    unscaled.checked_mul(factor)
}

/// Align `(a, sa)` and `(b, sb)` to the common scale `max(sa, sb)`, returning the two scaled unscaled
/// values plus that scale. `None` on an alignment overflow (i128 range) — the caller turns it into a
/// clean [`FAULT_DECIMAL_OVERFLOW`] fault. Shared by add/sub and comparison so every path aligns
/// identically.
fn dec_align(a: i128, sa: u8, b: i128, sb: u8) -> Option<(i128, i128, u8)> {
    let scale = sa.max(sb);
    let au = dec_scale_up(a, scale - sa)?;
    let bu = dec_scale_up(b, scale - sb)?;
    Some((au, bu, scale))
}

/// Exact decimal addition (M-NUM S1): result scale = `max(scales)`; align then `checked_add`. Any
/// i128 overflow (incl. the alignment) ⇒ [`FAULT_DECIMAL_OVERFLOW`]. Accepts mixed `(Decimal, Int)`
/// (the int widens to scale 0). Mirrors `bcadd($a, $b, max)`.
pub fn decimal_add(a: &Value, b: &Value) -> Result<Value, String> {
    let (au, sa) = dec_parts(a).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (bu, sb) = dec_parts(b).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (x, y, scale) =
        dec_align(au, sa, bu, sb).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let unscaled = x
        .checked_add(y)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    Ok(Value::Decimal { unscaled, scale })
}

/// Exact decimal subtraction (M-NUM S1): result scale = `max(scales)`; align then `checked_sub`.
/// Mirrors `bcsub($a, $b, max)`. Same overflow + mixed-operand rules as [`decimal_add`].
pub fn decimal_sub(a: &Value, b: &Value) -> Result<Value, String> {
    let (au, sa) = dec_parts(a).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (bu, sb) = dec_parts(b).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (x, y, scale) =
        dec_align(au, sa, bu, sb).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let unscaled = x
        .checked_sub(y)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    Ok(Value::Decimal { unscaled, scale })
}

/// Exact decimal multiplication (M-NUM S1): result scale = `sa + sb` (no truncation), unscaled =
/// `a.unscaled checked_mul b.unscaled`. Mirrors `bcmul($a, $b, sa + sb)`. Same overflow + mixed-
/// operand rules as [`decimal_add`]. The scale sum can't itself overflow `u8` for realistic inputs
/// (two scale-127 decimals would need ~10^254 magnitude — far past i128 — and overflow the mul long
/// before the scale add); a `u8` scale-add overflow is treated as an overflow fault, defensively.
pub fn decimal_mul(a: &Value, b: &Value) -> Result<Value, String> {
    let (au, sa) = dec_parts(a).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (bu, sb) = dec_parts(b).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let scale = sa
        .checked_add(sb)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let unscaled = au
        .checked_mul(bu)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    Ok(Value::Decimal { unscaled, scale })
}

/// Exact decimal remainder (bare `%`, 2026-06-27): align to `max(scales)`, then `x % y` — exact and
/// representable at that scale (no rounding, unlike `/`). A zero divisor faults
/// ([`FAULT_DECIMAL_MOD_ZERO`]); the sign follows the dividend (Rust `%` / PHP `bcmod`). Accepts a
/// mixed `(Decimal, Int)` (the int widens to scale 0). Mirrors `bcmod($a, $b, max)`.
pub fn decimal_rem(a: &Value, b: &Value) -> Result<Value, String> {
    let (au, sa) = dec_parts(a).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (bu, sb) = dec_parts(b).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (x, y, scale) =
        dec_align(au, sa, bu, sb).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    if y == 0 {
        return Err(FAULT_DECIMAL_MOD_ZERO.to_string());
    }
    let unscaled = x
        .checked_rem(y)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    Ok(Value::Decimal { unscaled, scale })
}

/// Euclidean gcd of two non-zero `u128` magnitudes (used by exact decimal division to reduce the
/// quotient fraction to lowest terms before testing termination).
fn u128_gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// Bare exact-or-fault decimal division (`/`, 2026-06-27). The value `(a/b)` is returned **exactly** in
/// its minimal-scale form when the quotient terminates (`10d/4d → 2.5`, `1d/8d → 0.125`); a
/// **non-terminating** quotient (`1d/3d`) faults [`FAULT_DECIMAL_NONTERMINATING`], a zero divisor
/// faults [`FAULT_DECIMAL_DIV_ZERO`], and an exact result past `i128` range / scale 255 faults
/// [`FAULT_DECIMAL_OVERFLOW`]. (Use `Decimal.div(a, b, scale, mode)` for a *rounded* quotient.)
/// Algorithm: reduce `a/b` to lowest terms `p/q·10^(sb-sa)`; if `q` (after stripping factors of 2 and
/// 5) is not 1 the decimal repeats → fault; otherwise the exact unscaled is `p·2^(m-i)·5^(m-j)` at
/// scale derived from `max(i,j)` and the `10^(sb-sa)` factor, then trailing zeros are stripped to the
/// canonical minimal form. The emitted PHP `__phorj_dec_div_exact` (bcdiv + exactness check + strip)
/// mirrors this byte-for-byte.
pub fn decimal_div_exact(a: &Value, b: &Value) -> Result<Value, String> {
    let ovf = || FAULT_DECIMAL_OVERFLOW.to_string();
    let (au, sa) = dec_parts(a).ok_or_else(ovf)?;
    let (bu, sb) = dec_parts(b).ok_or_else(ovf)?;
    if bu == 0 {
        return Err(FAULT_DECIMAL_DIV_ZERO.to_string());
    }
    if au == 0 {
        return Ok(Value::Decimal {
            unscaled: 0,
            scale: 0,
        });
    }
    let neg = (au < 0) ^ (bu < 0);
    let mut p = au.unsigned_abs();
    let mut q = bu.unsigned_abs();
    let g = u128_gcd(p, q);
    p /= g;
    q /= g;
    // Strip factors of 2 and 5 from the reduced denominator; anything left ⇒ non-terminating.
    let mut i = 0u32;
    while q % 2 == 0 {
        q /= 2;
        i += 1;
    }
    let mut j = 0u32;
    while q % 5 == 0 {
        q /= 5;
        j += 1;
    }
    if q != 1 {
        return Err(FAULT_DECIMAL_NONTERMINATING.to_string());
    }
    let m = i.max(j);
    let mul2 = 2u128.checked_pow(m - i).ok_or_else(ovf)?;
    let mul5 = 5u128.checked_pow(m - j).ok_or_else(ovf)?;
    let mut mag = p
        .checked_mul(mul2)
        .ok_or_else(ovf)?
        .checked_mul(mul5)
        .ok_or_else(ovf)?;
    // value = mag · 10^(delta - m), delta = sb - sa. exp <= 0 ⇒ that magnitude at scale -exp; exp > 0
    // ⇒ scale 0 after multiplying in the extra factor.
    let delta = i64::from(sb) - i64::from(sa);
    let exp = delta - i64::from(m);
    let scale = if exp <= 0 {
        let s = -exp;
        if s > i64::from(u8::MAX) {
            return Err(FAULT_DECIMAL_OVERFLOW.to_string());
        }
        s as u8
    } else {
        let factor = 10u128
            .checked_pow(u32::try_from(exp).map_err(|_| ovf())?)
            .ok_or_else(ovf)?;
        mag = mag.checked_mul(factor).ok_or_else(ovf)?;
        0
    };
    if mag > i128::MAX as u128 {
        return Err(FAULT_DECIMAL_OVERFLOW.to_string());
    }
    let mut unscaled = if neg { -(mag as i128) } else { mag as i128 };
    // Canonical minimal form: strip trailing zeros (division has no inherent scale, unlike `* + -`),
    // matching the PHP helper's rtrim — so `2.50d / 1d` is `2.5`, not `2.50`.
    let mut scale = scale;
    while scale > 0 && unscaled % 10 == 0 {
        unscaled /= 10;
        scale -= 1;
    }
    Ok(Value::Decimal { unscaled, scale })
}

/// Decimal negation (unary `-`): negate `unscaled` (checked — `i128::MIN` would overflow). The scale
/// is preserved; rendering never produces `-0` (see [`fmt_decimal`]).
pub fn decimal_neg(unscaled: i128, scale: u8) -> Result<Value, String> {
    let unscaled = unscaled
        .checked_neg()
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    Ok(Value::Decimal { unscaled, scale })
}

/// The seven rounding modes a `Decimal.div`/`Decimal.round` accepts (M-NUM S2). The injected
/// `RoundingMode` enum's variant *names* map onto these — the natives read `Value::Enum.variant` and
/// project it here via [`RoundMode::from_variant`], so the rounding kernel is variant-name-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundMode {
    /// Ties away from zero (`2.5 -> 3`, `-2.5 -> -3`).
    HalfUp,
    /// Ties toward zero (`2.5 -> 2`, `-2.5 -> -2`).
    HalfDown,
    /// Ties to the nearest even digit — banker's rounding (`2.5 -> 2`, `3.5 -> 4`).
    HalfEven,
    /// Always away from zero (`2.1 -> 3`, `-2.1 -> -3`).
    Up,
    /// Always toward zero — truncate (`2.9 -> 2`, `-2.9 -> -2`); equals a raw `bcdiv`.
    Down,
    /// Always toward `+∞` (`2.1 -> 3`, `-2.9 -> -2`).
    Ceiling,
    /// Always toward `-∞` (`2.9 -> 2`, `-2.1 -> -3`).
    Floor,
}

impl RoundMode {
    /// Map a `RoundingMode` enum variant name to a [`RoundMode`], or `None` for an unknown variant
    /// (checker-unreachable — the injected enum has exactly these seven variants and the native's
    /// signature requires a `RoundingMode`; handled defensively rather than panicking, EV-7).
    pub fn from_variant(variant: &str) -> Option<RoundMode> {
        Some(match variant {
            "HalfUp" => RoundMode::HalfUp,
            "HalfDown" => RoundMode::HalfDown,
            "HalfEven" => RoundMode::HalfEven,
            "Up" => RoundMode::Up,
            "Down" => RoundMode::Down,
            "Ceiling" => RoundMode::Ceiling,
            "Floor" => RoundMode::Floor,
            _ => return None,
        })
    }
}

/// Round the exact rational `n / d` to an integer under `mode` (M-NUM S2) — the single-sourced
/// rounding primitive both backends call and the PHP `__phorj_dec_div`/`_round` helpers replicate
/// step-for-step. The caller guarantees `d != 0` (a zero divisor is the `FAULT_DECIMAL_DIV_ZERO`
/// fault, checked before this). Any `checked_*` overflow ⇒ [`FAULT_DECIMAL_OVERFLOW`].
///
/// The half-decision compares `|rem|` against `d - |rem|` (both non-negative, so no `2*rem`
/// doubling that could overflow `i128`). i128 MIN edges are handled via `unsigned_abs`/`checked_neg`
/// — never a bare `-x` or `x.abs()`.
pub fn round_div(n: i128, d: i128, mode: RoundMode) -> Result<i128, String> {
    // 1. Normalise the divisor's sign so `d > 0`; the quotient sign is unchanged. `d == 0` is the
    //    caller's responsibility (div-by-zero fault).
    let (n, d) = if d < 0 {
        (
            n.checked_neg().ok_or(FAULT_DECIMAL_OVERFLOW)?,
            d.checked_neg().ok_or(FAULT_DECIMAL_OVERFLOW)?,
        )
    } else {
        (n, d)
    };
    // 2. Truncating quotient + dividend-signed remainder (matches BCMath `bcdiv`/`bcmod`).
    let q = n / d; // d != 0 here, and d > 0 so no MIN/-1 overflow
    let rem = n % d;
    if rem == 0 {
        return Ok(q); // exact
    }
    // `s` = sign of the dividend (and of the exact quotient): the direction "away from zero".
    let s: i128 = if n > 0 { 1 } else { -1 };
    // Magnitudes for the half-comparison. `d > 0`, so `d` is its own magnitude; `|rem| <= d - 1 < d`,
    // so `d - abs_rem` is `>= 1 > 0` and never underflows.
    let abs_rem =
        i128::try_from(rem.unsigned_abs()).map_err(|_| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let complement = d - abs_rem; // safe: 0 < abs_rem < d
    let bump = |q: i128, by: i128| q.checked_add(by).ok_or(FAULT_DECIMAL_OVERFLOW.to_string());
    let result = match mode {
        RoundMode::Down => q,
        RoundMode::Up => bump(q, s)?,
        RoundMode::Ceiling => {
            if n > 0 {
                bump(q, 1)?
            } else {
                q
            }
        }
        RoundMode::Floor => {
            if n < 0 {
                bump(q, -1)?
            } else {
                q
            }
        }
        RoundMode::HalfUp => {
            if abs_rem >= complement {
                bump(q, s)?
            } else {
                q
            }
        }
        RoundMode::HalfDown => {
            if abs_rem > complement {
                bump(q, s)?
            } else {
                q
            }
        }
        RoundMode::HalfEven => match abs_rem.cmp(&complement) {
            Ordering::Greater => bump(q, s)?,
            Ordering::Less => q,
            // Exact tie: round to even — bump only if `q` is currently odd.
            Ordering::Equal => {
                if q % 2 != 0 {
                    bump(q, s)?
                } else {
                    q
                }
            }
        },
    };
    Ok(result)
}

/// `Decimal.div(a, b, scale, mode)` (M-NUM S2): the exact rational `a / b`, rounded to `scale`
/// fractional digits under `mode`. Computes `N = a.unscaled * 10^(b.scale + scale)` and
/// `D = b.unscaled * 10^a.scale` (both checked), then `round_div(N, D, mode)` at `scale`.
/// `b == 0` ⇒ [`FAULT_DECIMAL_DIV_ZERO`]; `scale < 0` ⇒ [`FAULT_DECIMAL_SCALE`]; any i128 overflow ⇒
/// [`FAULT_DECIMAL_OVERFLOW`]. Mirrored by the PHP `__phorj_dec_div` helper.
pub fn decimal_div(a: &Value, b: &Value, scale: i64, mode: RoundMode) -> Result<Value, String> {
    let (au, sa) = dec_parts(a).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let (bu, sb) = dec_parts(b).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let out_scale = scale_u8(scale)?;
    if bu == 0 {
        return Err(FAULT_DECIMAL_DIV_ZERO.to_string());
    }
    // N = au * 10^(sb + out_scale); D = bu * 10^sa. Both exponents are non-negative `u8` sums.
    let n_exp = u32::from(sb)
        .checked_add(u32::from(out_scale))
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let n = pow10_mul(au, n_exp)?;
    let d = pow10_mul(bu, u32::from(sa))?;
    let unscaled = round_div(n, d, mode)?;
    Ok(Value::Decimal {
        unscaled,
        scale: out_scale,
    })
}

/// `Decimal.round(d, scale, mode)` (M-NUM S2): re-scale `d` to exactly `scale` fractional digits.
/// Scaling up (`scale >= d.scale`) is exact (`unscaled * 10^Δ`, checked, no rounding); scaling down
/// rounds via `round_div(unscaled, 10^Δ, mode)`. `scale < 0` ⇒ [`FAULT_DECIMAL_SCALE`]; overflow ⇒
/// [`FAULT_DECIMAL_OVERFLOW`]. Mirrored by the PHP `__phorj_dec_round` helper.
pub fn decimal_round(d: &Value, scale: i64, mode: RoundMode) -> Result<Value, String> {
    let (du, sd) = dec_parts(d).ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())?;
    let out_scale = scale_u8(scale)?;
    let unscaled = if out_scale >= sd {
        // Exact up-scale: multiply by 10^(out_scale - sd).
        pow10_mul(du, u32::from(out_scale - sd))?
    } else {
        // Down-scale: divide by 10^(sd - out_scale) with rounding.
        let divisor = pow10(u32::from(sd - out_scale))?;
        round_div(du, divisor, mode)?
    };
    Ok(Value::Decimal {
        unscaled,
        scale: out_scale,
    })
}

/// `Convert.decimalToInt(decimal)` (M-NUM S3): the decimal's integer part, truncated toward zero
/// (drop the fraction), or `None` if that integer part is outside the i64 range. Computed exactly on
/// the i128 carrier — `unscaled / 10^scale` truncates toward zero (i128 `/` rounds toward zero, like
/// PHP `intdiv`/`bcdiv`), then `i64::try_from` range-checks. No string parsing, no BCMath. Single-
/// sourced; mirrored by the PHP `__phorj_dec_to_int` helper (which splits the carrier string before
/// the dot). A non-decimal value is checker-unreachable (handled defensively as `None`).
pub fn decimal_to_int(d: &Value) -> Option<i64> {
    let (unscaled, scale) = match d {
        Value::Decimal { unscaled, scale } => (*unscaled, *scale),
        _ => return None,
    };
    // 10^scale fits i128 for any realistic scale; an absurd scale (>38) overflows pow → None.
    let divisor = 10i128.checked_pow(u32::from(scale))?;
    let int_part = unscaled / divisor; // i128 `/` truncates toward zero
    i64::try_from(int_part).ok()
}

/// `decimal as int` (M4 as-matrix) — **exact-or-null**: `Some(i)` only when the decimal has a zero
/// fractional part and the integer is in i64 range (`3.00d → 3`, `3.50d → None`). Unlike
/// [`decimal_to_int`] (truncate, used by `Convert.decimalToInt`), it never drops a fraction silently
/// — the `as` "no silent loss" rule. Mirrored by the PHP `__phorj_dec_to_int_exact` helper.
pub fn decimal_to_int_exact(d: &Value) -> Option<i64> {
    let (unscaled, scale) = match d {
        Value::Decimal { unscaled, scale } => (*unscaled, *scale),
        _ => return None,
    };
    let divisor = 10i128.checked_pow(u32::from(scale))?;
    if unscaled % divisor != 0 {
        return None; // non-integral → null (exact-or-null)
    }
    i64::try_from(unscaled / divisor).ok()
}

/// `10^exp` as a checked `i128`, [`FAULT_DECIMAL_OVERFLOW`] on overflow (M-NUM S2 helper).
fn pow10(exp: u32) -> Result<i128, String> {
    10i128
        .checked_pow(exp)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())
}

/// `value * 10^exp` as a checked `i128`, [`FAULT_DECIMAL_OVERFLOW`] on overflow (M-NUM S2 helper).
fn pow10_mul(value: i128, exp: u32) -> Result<i128, String> {
    value
        .checked_mul(pow10(exp)?)
        .ok_or_else(|| FAULT_DECIMAL_OVERFLOW.to_string())
}

/// Validate + narrow a `scale` argument (an `int`, so `i64`) to the `u8` a `Value::Decimal` stores.
/// A negative scale is [`FAULT_DECIMAL_SCALE`]; a scale past `u8::MAX` (255, far beyond any realistic
/// money use) is also [`FAULT_DECIMAL_SCALE`] (M-NUM S2). The PHP helpers throw on `scale < 0` too.
fn scale_u8(scale: i64) -> Result<u8, String> {
    u8::try_from(scale).map_err(|_| FAULT_DECIMAL_SCALE.to_string())
}

/// Numeric, **scale-insensitive** ordering of two decimal operands (mixed `decimal`/`int` allowed):
/// align to the common scale and compare unscaled. `None` if the operands aren't decimal/int
/// (checker-unreachable) **or** an alignment overflow — in the overflow case the operands differ by
/// ≥10^Δ at i128 scale, so they are necessarily unequal; the caller's `< > <= >=` projection treats a
/// `None` like NaN (`false`), which is sound here because equality is `Some(Equal)` only. (M-NUM S1.)
pub fn decimal_cmp(a: &Value, b: &Value) -> Option<Ordering> {
    let (au, sa) = dec_parts(a)?;
    let (bu, sb) = dec_parts(b)?;
    let (x, y, _) = dec_align(au, sa, bu, sb)?;
    Some(x.cmp(&y))
}

/// Render `(unscaled, scale)` as a decimal string with **exactly `scale`** fractional digits — the
/// BCMath-padding form, single-sourced so both backends agree and the emitted PHP (a BCMath result
/// string) matches. `{1999, 2}` → `"19.99"`, `{1500, 3}` → `"1.500"`, `{100, 0}` → `"100"`,
/// `{15, 4}` → `"0.0015"`. Negative values carry a leading `-`; the value `0` (any scale) **never**
/// renders `-0` (M-NUM S1).
pub fn fmt_decimal(unscaled: i128, scale: u8) -> String {
    let neg = unscaled < 0;
    // Magnitude as a string of digits. `unsigned_abs` handles `i128::MIN` without overflow.
    let digits = unscaled.unsigned_abs().to_string();
    let s = scale as usize;
    let body = if s == 0 {
        digits
    } else if digits.len() > s {
        let dot = digits.len() - s;
        format!("{}.{}", &digits[..dot], &digits[dot..])
    } else {
        // Fewer integer digits than the scale → pad with leading zeros after `0.`.
        format!("0.{}{}", "0".repeat(s - digits.len()), digits)
    };
    // Never render `-0` / `-0.00`: only prefix `-` for a genuinely non-zero magnitude.
    if neg && unscaled != 0 {
        format!("-{body}")
    } else {
        body
    }
}

/// Parse the `decimal` literal grammar at runtime for `Decimal.of(string)` (M-NUM S1) — the SAME
/// grammar the tokenizer accepts for a `…d` literal, returning `(unscaled, scale)` or `None` on a
/// malformed string or i128 overflow (so `Decimal.of` is `decimal?`). Grammar: optional sign, then
/// digits with an optional single fractional part (`12`, `12.34`, `.5`, `-0.50`); NO exponent, NO
/// underscores (a runtime string is exact, unlike a source literal), NO surrounding whitespace. The
/// scale is the count of fractional digits (trailing zeros preserved). Shared by the interpreter, the
/// VM, and mirrored by the PHP `__phorj_dec_of` PCRE helper.
pub fn decimal_of(s: &str) -> Option<(i128, u8)> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let (neg, rest) = match bytes[0] {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    if rest.is_empty() {
        return None;
    }
    let (int_part, frac_part) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    // At least one digit overall; each part must be all ASCII digits (an empty integer part like `.5`
    // is allowed, but a trailing `12.` with an empty fractional part is not — matches the tokenizer, which
    // requires a digit after the dot to treat it as a fraction).
    if let Some(f) = frac_part {
        if f.is_empty() || !f.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    if !int_part.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if int_part.is_empty() && frac_part.is_none() {
        return None;
    }
    let frac = frac_part.unwrap_or("");
    let scale = u8::try_from(frac.len()).ok()?;
    let combined = format!("{int_part}{frac}");
    if combined.is_empty() {
        return None;
    }
    let magnitude: i128 = combined.parse().ok()?;
    let unscaled = if neg {
        magnitude.checked_neg()?
    } else {
        magnitude
    };
    Some((unscaled, scale))
}
/// Checked integer power `base ** exp` (Phase 1 operators slice). A negative exponent faults
/// ([`FAULT_NEGATIVE_EXPONENT`] — the result can't be a typed `int`); overflow (incl. an exponent
/// too large to fit `u32`) is a clean [`FAULT_INT_OVERFLOW`], never a panic (EV-7). Single-sourced:
/// both the interpreter's `**` arm and the `Core.Math.ipow` native call this, so `run`/`runvm`
/// compute and fault identically. PHP's `**`/`pow` return `int` for the same non-negative,
/// non-overflowing domain, keeping the transpiled output byte-identical there.
pub fn int_pow(base: i64, exp: i64) -> Result<i64, String> {
    if exp < 0 {
        return Err(FAULT_NEGATIVE_EXPONENT.to_string());
    }
    let e = u32::try_from(exp).map_err(|_| FAULT_INT_OVERFLOW.to_string())?;
    base.checked_pow(e)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// Float power `base ** exp` (Phase 1 operators slice). Floats never fault — NaN/inf are valid
/// `f64`. `powf` is C `pow` (matching PHP's `**`/`pow` on floats). Single-sourced with `Core.Math.pow`.
pub fn float_pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}
