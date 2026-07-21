//! Arithmetic kernels + canonical fault strings (single-sourced; fault bodies are
//! parity-affecting — see docs/INVARIANTS.md).

/// Canonical fault body for integer `x / 0`. Single-sourced so `run` ≡ `runvm` in the fault path.
pub const FAULT_DIV_ZERO: &str = "division by zero";
/// Canonical fault body for integer `x % 0`.
pub const FAULT_MOD_ZERO: &str = "modulo by zero";
/// Canonical fault body for any integer op whose result leaves `i64` range
/// (`MAX + 1`, `MIN - 1`, `MIN * -1`, `MIN / -1`, `MIN % -1`, `-MIN`).
pub const FAULT_INT_OVERFLOW: &str = "integer overflow";
/// Canonical fault body for a bitwise shift by a negative count (PHP throws `ArithmeticError`).
pub const FAULT_NEGATIVE_SHIFT: &str = "bit shift by negative number";
/// Canonical fault body for a `decimal` `+ - *` (or scale-alignment) whose exact result leaves
/// `i128` range (M-NUM S1). Byte-identical across both Rust backends AND the emitted BCMath PHP (the
/// `__phorj_dec_*` helper bounds-checks its result against i128 range and throws the same body).
pub const FAULT_DECIMAL_OVERFLOW: &str = "decimal overflow";
/// Canonical fault body for `Decimal.div` with a zero divisor (M-NUM S2). Distinct from the integer
/// `FAULT_DIV_ZERO` body so the message is decimal-specific, but it still *contains* the substring
/// `"division by zero"`, so the differential harness classifies it as `FaultKind::DivZero` (run≡runvm
/// parity); the emitted PHP `__phorj_dec_div` helper throws the same body.
pub const FAULT_DECIMAL_DIV_ZERO: &str = "decimal division by zero";
/// Canonical fault body for a negative `scale` argument to `Decimal.div`/`Decimal.round` (M-NUM S2).
/// A scale is the count of fractional digits, so it must be `>= 0`; the PHP helpers throw the same.
pub const FAULT_DECIMAL_SCALE: &str = "decimal scale out of range";
/// Canonical fault body for a bare `decimal % 0` (the exact-remainder operator, 2026-06-27). Contains
/// `"modulo by zero"` so the differential harness classifies it as the same `FaultKind` as int `%0`.
pub const FAULT_DECIMAL_MOD_ZERO: &str = "decimal modulo by zero";
/// Canonical fault body for a bare `decimal / decimal` whose quotient does not terminate
/// (2026-06-27 exact-or-fault `/`): the fraction in lowest terms has a denominator with a prime
/// factor other than 2 or 5 (e.g. `1d / 3d`). Use `Decimal.div(a, b, scale, mode)` for a rounded
/// quotient instead. The emitted PHP `__phorj_dec_div_exact` throws the same body.
pub const FAULT_DECIMAL_NONTERMINATING: &str = "decimal division is not exact";
/// Canonical fault body for `int ** int` with a negative exponent. A negative exponent yields a
/// fractional result, which cannot be the typed `int` the `**` operator promises — so it faults
/// rather than silently widening to `float` (PHP's `2 ** -1 == 0.5`). Use `float**float` for that.
pub const FAULT_NEGATIVE_EXPONENT: &str = "negative exponent";

/// Checked integer addition; overflow is a clean fault, never a panic (EV-7).
pub fn int_add(a: i64, b: i64) -> Result<i64, String> {
    a.checked_add(b)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// Checked integer subtraction.
pub fn int_sub(a: i64, b: i64) -> Result<i64, String> {
    a.checked_sub(b)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// Checked integer multiplication.
pub fn int_mul(a: i64, b: i64) -> Result<i64, String> {
    a.checked_mul(b)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// Checked integer division. `b == 0` is `FAULT_DIV_ZERO`; `i64::MIN / -1` overflows.
pub fn int_div(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        return Err(FAULT_DIV_ZERO.to_string());
    }
    a.checked_div(b)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// Checked integer remainder. `b == 0` is `FAULT_MOD_ZERO`; `i64::MIN % -1` overflows.
pub fn int_rem(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        return Err(FAULT_MOD_ZERO.to_string());
    }
    a.checked_rem(b)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// Checked integer negation. `-i64::MIN` overflows (the exact Wave 0 P0 case).
pub fn int_neg(n: i64) -> Result<i64, String> {
    n.checked_neg()
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}

// --- `#[UncheckedOverflow]` wrapping kernels (perf-wave): two's-complement wrapping arithmetic for a function
// marked `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow). Overflow WRAPS (never faults) — the opt-in escape hatch.
// Single-sourced like the checked kernels (Inv-4): interp, VM, and JIT all call these for an unchecked fn,
// so the wrapping result is byte-identical across backends by construction. Scope = `+ - *` and unary `-`
// only; Div/Rem stay CHECKED even in an unchecked fn (div-by-zero must always fault), and `**`/`Pow`
// lowers to a native call (out of the wrapping-op set by construction) so it also stays checked — all
// intended boundaries, not silent gaps. `#[UncheckedOverflow]` has no faithful PHP analog (PHP overflow→float),
// so a using function is `E-TRANSPILE-UNCHECKED` (§14 LADDER).
/// Wrapping integer addition (`#[UncheckedOverflow]`): `i64::MAX + 1` → `i64::MIN`, never faults.
pub fn int_wrapping_add(a: i64, b: i64) -> i64 {
    a.wrapping_add(b)
}
/// Wrapping integer subtraction (`#[UncheckedOverflow]`).
pub fn int_wrapping_sub(a: i64, b: i64) -> i64 {
    a.wrapping_sub(b)
}
/// Wrapping integer multiplication (`#[UncheckedOverflow]`).
pub fn int_wrapping_mul(a: i64, b: i64) -> i64 {
    a.wrapping_mul(b)
}
/// Wrapping integer negation (`#[UncheckedOverflow]`): `-i64::MIN` → `i64::MIN`, never faults.
pub fn int_wrapping_neg(n: i64) -> i64 {
    n.wrapping_neg()
}
/// `Math.integerDivide(a, b)` (M-NUM S3): integer division truncating toward zero (PHP `intdiv`). `b == 0`
/// is [`FAULT_DIV_ZERO`]; `i64::MIN / -1` overflows ([`FAULT_INT_OVERFLOW`]) — both clean faults, never
/// a panic (EV-7). Distinct from [`int_div`] only in name/intent (both truncate toward zero); kept
/// separate so the `intdiv` native and the `/`-on-int operator can diverge later without coupling.
pub fn int_intdiv(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        return Err(FAULT_DIV_ZERO.to_string());
    }
    a.checked_div(b)
        .ok_or_else(|| FAULT_INT_OVERFLOW.to_string())
}
/// `Convert.toInt(float)` (M-NUM S3): truncate a float toward zero to an `int`, or `None` on NaN,
/// ±∞, or a value outside the i64 range. The range guard uses the float bounds that **both** Rust and
/// PHP agree on at the i64 edge: `2^63` (9223372036854775808.0) is exactly f64-representable, but
/// `i64::MAX` (2^63 − 1) is not — so the in-range window is `[-2^63, 2^63)` (upper **exclusive**;
/// `i64::MIN` is exactly `-2^63`). A value `v` with `LOWER <= v.trunc() < UPPER` casts losslessly via
/// `as i64`; anything else (incl. NaN/±∞, which fail the comparisons) returns `None`. This avoids
/// PHP's surprising `(int)NAN == 0`. Single-sourced so `run`/`runvm` agree; mirrored by the PHP
/// `__phorj_float_to_int` helper (which uses the same `9.2233720368547758E18` literal).
pub fn float_to_int(v: f64) -> Option<i64> {
    const UPPER: f64 = 9_223_372_036_854_775_808.0; // 2^63 — exclusive upper bound
    const LOWER: f64 = -UPPER; // i64::MIN as f64 (exact)
    let t = v.trunc();
    if v.is_finite() && (LOWER..UPPER).contains(&t) {
        Some(t as i64)
    } else {
        None
    }
}

/// `float as int` (M4 as-matrix) — **exact-or-null**: `Some(i)` only when `v` is integral and in
/// i64 range (`3.0 → 3`, `3.9 → None`, NaN/±∞ → None). Unlike [`float_to_int`] (truncate, used by
/// `Convert.toInt`), this never drops a fraction silently — the `as` operator's "no silent loss"
/// rule. `v.fract() == 0.0` is false for NaN/∞, so the finite+range guard in [`float_to_int`] runs
/// only for a genuinely integral value. Mirrored by the PHP `__phorj_float_to_int_exact` helper.
pub fn float_to_int_exact(v: f64) -> Option<i64> {
    if v.fract() == 0.0 {
        float_to_int(v)
    } else {
        None
    }
}

/// `Math.numberFormat(value, decimals)` (M-NUM S4): a non-locale `number_format` — `value` rounded
/// half-away-from-zero to `decimals` places, grouped with `,` every three integer digits and a `.`
/// decimal point. **Digit-string rounding** (2026-06-27): it rounds the *shortest-round-trip decimal
/// string* of `value` (`format!("{value}")`, which the PHP `__phorj_float` helper reproduces
/// byte-for-byte) digit-by-digit with carry — NOT `(value * 10^d).round()`. That removes the previous
/// `.5`-boundary divergence (Rust `f64::round` had no pre-rounding, PHP `round` did): both legs now
/// round the *intended* decimal identically (`numberFormat(0.285, 2) == "0.29"` on all three backends).
/// A negative `decimals` is clamped to `0`. A non-finite `value` is outside the format domain and
/// falls back to its plain display.
pub fn number_format(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return format!("{value}");
    }
    let s = format!("{value}");
    let neg = s.starts_with('-');
    let s = s.strip_prefix('-').unwrap_or(&s);
    let (int_str, frac_str) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    let mut int_digits: Vec<u8> = int_str.bytes().collect();
    let mut frac_digits: Vec<u8> = frac_str.bytes().collect();
    // Round half-away-from-zero: round up iff the first dropped fractional digit is >= '5'.
    let round_up = frac_digits.get(decimals).is_some_and(|&d| d >= b'5');
    frac_digits.truncate(decimals);
    while frac_digits.len() < decimals {
        frac_digits.push(b'0');
    }
    if round_up {
        let mut carry = 1u8;
        for d in frac_digits.iter_mut().rev() {
            if carry == 0 {
                break;
            }
            let v = *d - b'0' + carry;
            *d = b'0' + v % 10;
            carry = v / 10;
        }
        for d in int_digits.iter_mut().rev() {
            if carry == 0 {
                break;
            }
            let v = *d - b'0' + carry;
            *d = b'0' + v % 10;
            carry = v / 10;
        }
        if carry > 0 {
            int_digits.insert(0, b'0' + carry);
        }
    }
    // Strip leading zeros from the integer part (keep at least one digit).
    while int_digits.len() > 1 && int_digits[0] == b'0' {
        int_digits.remove(0);
    }
    // A result that is entirely zero never carries a sign (no `-0`).
    let all_zero = int_digits.iter().all(|&d| d == b'0') && frac_digits.iter().all(|&d| d == b'0');
    let mut out = String::new();
    if neg && !all_zero {
        out.push('-');
    }
    let n = int_digits.len();
    for (i, b) in int_digits.iter().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    if decimals > 0 {
        out.push('.');
        for b in &frac_digits {
            out.push(*b as char);
        }
    }
    out
}

/// Bitwise AND / OR / XOR on `int` (never fault — total over `i64`). PHP-identical.
pub fn int_bitand(a: i64, b: i64) -> i64 {
    a & b
}
pub fn int_bitor(a: i64, b: i64) -> i64 {
    a | b
}
pub fn int_bitxor(a: i64, b: i64) -> i64 {
    a ^ b
}
/// Bitwise NOT — `~n == -n - 1`, total over `i64`.
pub fn int_bitnot(n: i64) -> i64 {
    !n
}
/// Shift-left, PHP semantics: a negative count faults (`ArithmeticError`); a count ≥ 64 yields 0;
/// otherwise the low 64 bits of the shifted value (`wrapping_shl` would mask the count, so the ≥ 64
/// case is handled explicitly — `1 << 64` is 0, not 1).
pub fn int_shl(a: i64, n: i64) -> Result<i64, String> {
    if n < 0 {
        return Err(FAULT_NEGATIVE_SHIFT.to_string());
    }
    if n >= 64 {
        return Ok(0);
    }
    Ok(a.wrapping_shl(n as u32))
}
/// Shift-right (arithmetic, sign-preserving — PHP semantics): a negative count faults; a count ≥ 64
/// fills with the sign bit (`8 >> 64 == 0`, `-8 >> 64 == -1`); otherwise an arithmetic right shift.
pub fn int_shr(a: i64, n: i64) -> Result<i64, String> {
    if n < 0 {
        return Err(FAULT_NEGATIVE_SHIFT.to_string());
    }
    let n = if n >= 64 { 63 } else { n as u32 };
    Ok(a >> n)
}

/// Float addition. Floats never fault — NaN/inf are valid `f64`.
pub fn float_add(a: f64, b: f64) -> f64 {
    a + b
}
/// Float subtraction.
pub fn float_sub(a: f64, b: f64) -> f64 {
    a - b
}
/// Float multiplication.
pub fn float_mul(a: f64, b: f64) -> f64 {
    a * b
}
/// Float division. A **zero divisor faults** (`FAULT_DIV_ZERO`) — matching int `/0` and PHP 8's
/// `DivisionByZeroError` on `$a / 0.0` — rather than yielding IEEE `inf`/`NaN` (the "any division by
/// zero throws" rule). `-0.0` counts as zero (`-0.0 == 0.0`). A finite-overflow-to-`inf` (huge `a`,
/// tiny non-zero `b`) is *not* a zero division and stays `inf`.
pub fn float_div(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        return Err(FAULT_DIV_ZERO.to_string());
    }
    Ok(a / b)
}
/// Float remainder. A **zero divisor faults** (`FAULT_MOD_ZERO`), like int `%0` (PHP `fmod` would
/// return `NAN`; the emitted PHP routes through `__phorj_rem`, which throws to agree).
pub fn float_rem(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        return Err(FAULT_MOD_ZERO.to_string());
    }
    Ok(a % b)
}

// --- Decimal (fixed-point) kernels (M-NUM S1; single-sourced — both backends + the example oracle
// agree, and the BCMath PHP helper mirrors them). value = `unscaled × 10^(-scale)`. ---
