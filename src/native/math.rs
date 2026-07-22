use super::*;
use crate::types::Ty;
use crate::value::Value;

fn math_sqrt(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.sqrt())),
        _ => Err("Math.sqrt expects (float)".into()),
    }
}
fn math_pow(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(b), Value::Float(e)] => Ok(Value::Float(crate::value::float_pow(*b, *e))),
        _ => Err("Math.pow expects (float, float)".into()),
    }
}
fn math_ipow(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // Single-sourced with the interpreter's `int ** int` arm via `value::int_pow`: a negative
        // exponent or overflow is a clean fault (EV-7), never a panic.
        [Value::Int(b), Value::Int(e)] => crate::value::int_pow(*b, *e).map(Value::Int),
        _ => Err("Math.integerPower expects (int, int)".into()),
    }
}
fn math_floor(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.floor())),
        _ => Err("Math.floor expects (float)".into()),
    }
}
fn math_ceil(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.ceil())),
        _ => Err("Math.ceil expects (float)".into()),
    }
}
fn math_abs(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // `i64::MIN.abs()` overflows; a clean fault keeps EV-7 (never panic on input).
        [Value::Int(n)] => n
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| "integer overflow in Math.abs".to_string()),
        _ => Err("Math.abs expects (int)".into()),
    }
}
fn math_min(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(a), Value::Int(b)] => Ok(Value::Int((*a).min(*b))),
        _ => Err("Math.min expects (int, int)".into()),
    }
}
fn math_max(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(a), Value::Int(b)] => Ok(Value::Int((*a).max(*b))),
        _ => Err("Math.max expects (int, int)".into()),
    }
}
// `Math.tryAdd/trySub/tryMul(int, int): int?` — CHECKED integer arithmetic that returns `null` on
// overflow instead of faulting (the type-driven recovery path; the fail-fast fault stays the default,
// `#[UncheckedOverflow]` is the wrap-instead escape hatch). Overflow → `null` maps the same i64 boundary the
// single-sourced `value::int_*` kernels detect, so the PHP leg (which returns float on overflow) agrees
// via an `is_int` guard — byte-identical across all three backends.
fn math_try_add(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(a), Value::Int(b)] => Ok(crate::value::int_add(*a, *b)
            .ok()
            .map_or(Value::Null, Value::Int)),
        _ => Err("Math.tryAdd expects (int, int)".into()),
    }
}
fn math_try_sub(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(a), Value::Int(b)] => Ok(crate::value::int_sub(*a, *b)
            .ok()
            .map_or(Value::Null, Value::Int)),
        _ => Err("Math.trySub expects (int, int)".into()),
    }
}
fn math_try_mul(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(a), Value::Int(b)] => Ok(crate::value::int_mul(*a, *b)
            .ok()
            .map_or(Value::Null, Value::Int)),
        _ => Err("Math.tryMul expects (int, int)".into()),
    }
}
fn math_round(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // PHP `round()` defaults to round-half-away-from-zero, matching Rust `f64::round`; the `(int)`
        // cast then truncates the already-integral result. Saturating `as i64` keeps EV-7 (no panic on
        // a huge magnitude); examples use small exact-representable values to stay byte-identical.
        [Value::Float(x)] => Ok(Value::Int(x.round() as i64)),
        _ => Err("Math.round expects (float)".into()),
    }
}

// --- Float predicates + special values (M-NUM S3). All PHP-core (`php -n`): `is_nan`/`is_finite`/
// `is_infinite`, `NAN`/`INF`. The predicates return `bool`, so they are byte-identical even for a
// non-representable float operand (the divergence is in float *display*, not in a `bool` result).
fn math_is_nan(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Bool(x.is_nan())),
        _ => Err("Math.isNaN expects (float)".into()),
    }
}
fn math_is_finite(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Bool(x.is_finite())),
        _ => Err("Math.isFinite expects (float)".into()),
    }
}
fn math_is_infinite(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Bool(x.is_infinite())),
        _ => Err("Math.isInfinite expects (float)".into()),
    }
}
fn math_nan(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [] => Ok(Value::Float(f64::NAN)),
        _ => Err("Math.nan expects ()".into()),
    }
}
fn math_infinity(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [] => Ok(Value::Float(f64::INFINITY)),
        _ => Err("Math.infinity expects ()".into()),
    }
}
fn math_neg_infinity(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [] => Ok(Value::Float(f64::NEG_INFINITY)),
        _ => Err("Math.negativeInfinity expects ()".into()),
    }
}
/// `Math.integerDivide(int, int) -> int` (M-NUM S3) — integer division truncating toward zero. Single-sourced
/// with `value::int_intdiv`: `b == 0` faults `"division by zero"`, `intdiv(i64::MIN, -1)` faults
/// `"integer overflow"` (both interp ≡ VM via FaultKind; PHP `intdiv` throws the matching class).
fn math_intdiv(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(a), Value::Int(b)] => crate::value::int_intdiv(*a, *b).map(Value::Int),
        _ => Err("Math.integerDivide expects (int, int)".into()),
    }
}

// --- Math breadth (M-NUM S4) -----------------------------------------------------------------
// Integer helpers (`sign`/`clamp`/`gcd`) return `int` → byte-identical regardless of float display.
// `gcd` has no PHP-core builtin (gmp is absent under `php -n`), so it erases to a `__phorj_gcd`
// helper. Transcendentals (`log`/`log10`/`exp`/`sin`/`cos`/`tan`/`pi`/`e`) erase to the libm builtins;
// a non-representable result diverges between Rust's shortest-round-trip and PHP, so examples exercise
// them at exact values or via `numberFormat`. `numberFormat` erases to a `__phorj_number_format`
// helper (identical string assembly both legs — see `value::number_format`).

fn math_sign(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // -1 / 0 / 1 — the sign of an int. Erases to PHP's `<=>` (spaceship), single-evaluating.
        [Value::Int(n)] => Ok(Value::Int(i64::from(*n > 0) - i64::from(*n < 0))),
        _ => Err("Math.sign expects (int)".into()),
    }
}
fn math_clamp(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // `clamp(v, lo, hi)` requires `lo <= hi` — the module's precondition convention, and what
        // Rust's own `Ord::clamp` demands. `lo > hi` is a caller bug, so it faults cleanly (UA-1.7)
        // rather than silently picking `lo`. The PHP leg's `__phorj_clamp` helper faults in kind.
        [Value::Int(_), Value::Int(lo), Value::Int(hi)] if lo > hi => {
            Err(format!("Math.clamp: min ({lo}) must not exceed max ({hi})"))
        }
        // Otherwise `max(lo, min(v, hi))` — identical to `(*v).min(*hi).max(*lo)`.
        [Value::Int(v), Value::Int(lo), Value::Int(hi)] => Ok(Value::Int((*v).min(*hi).max(*lo))),
        _ => Err("Math.clamp expects (int, int, int)".into()),
    }
}
fn math_gcd(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // Euclid over the magnitudes (`unsigned_abs` so `i64::MIN` doesn't overflow `abs`). The
        // result overflows `i64` only for `gcd(i64::MIN, i64::MIN)`/`gcd(i64::MIN, 0)` (= 2^63) →
        // a clean fault (EV-7), never a panic.
        [Value::Int(a), Value::Int(b)] => {
            let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            i64::try_from(a)
                .map(Value::Int)
                .map_err(|_| "integer overflow in Math.gcd".to_string())
        }
        _ => Err("Math.gcd expects (int, int)".into()),
    }
}
fn math_lcm(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // `lcm(a, b) = |a| / gcd(|a|, |b|) * |b|` over the magnitudes (`unsigned_abs` so `i64::MIN`
        // doesn't overflow `abs`). `lcm(_, 0) = 0` by convention. Division before multiplication keeps
        // the intermediate as small as possible; the final `u64` product and the `i64` narrowing are
        // both checked → a clean fault on overflow (EV-7), never a panic.
        [Value::Int(a), Value::Int(b)] => {
            if *a == 0 || *b == 0 {
                return Ok(Value::Int(0));
            }
            let (x, y) = (a.unsigned_abs(), b.unsigned_abs());
            let (mut ga, mut gb) = (x, y);
            while gb != 0 {
                let t = gb;
                gb = ga % gb;
                ga = t;
            }
            (x / ga)
                .checked_mul(y)
                .and_then(|l| i64::try_from(l).ok())
                .map(Value::Int)
                .ok_or_else(|| "integer overflow in Math.lcm".to_string())
        }
        _ => Err("Math.lcm expects (int, int)".into()),
    }
}
fn math_log(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.ln())),
        _ => Err("Math.log expects (float)".into()),
    }
}
fn math_log10(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.log10())),
        _ => Err("Math.log10 expects (float)".into()),
    }
}
fn math_exp(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.exp())),
        _ => Err("Math.exp expects (float)".into()),
    }
}
fn math_sin(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.sin())),
        _ => Err("Math.sin expects (float)".into()),
    }
}
fn math_cos(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.cos())),
        _ => Err("Math.cos expects (float)".into()),
    }
}
fn math_tan(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.tan())),
        _ => Err("Math.tan expects (float)".into()),
    }
}
// DEC-299/Wave-B: the trig/hyperbolic/log tail (G-math-breadth, FN-MATH GP). All pure `f64` kernels
// delegating to the platform libm — the same source PHP's `asin`/`atan2`/`hypot`/… use, so the three
// backends agree bit-for-bit (like the existing `sin`/`cos`). Naming is camelCase-consistent; the two
// angle conversions get the clearer `degToRad`/`radToDeg` (better than PHP's `deg2rad`/`rad2deg` —
// DEC-Wave-B-math AUTO), transpiled to the PHP names.
macro_rules! math_unary {
    ($name:ident, $method:ident, $label:literal) => {
        fn $name(args: &[Value], _: &mut String) -> Result<Value, String> {
            match args {
                [Value::Float(x)] => Ok(Value::Float(x.$method())),
                _ => Err(concat!("Math.", $label, " expects (float)").into()),
            }
        }
    };
}
math_unary!(math_asin, asin, "asin");
math_unary!(math_acos, acos, "acos");
math_unary!(math_atan, atan, "atan");
math_unary!(math_sinh, sinh, "sinh");
math_unary!(math_cosh, cosh, "cosh");
math_unary!(math_tanh, tanh, "tanh");
// Inverse hyperbolics — same platform libm as PHP's asinh/acosh/atanh, so bit-for-bit identical.
// Domain violations return NaN (acosh(x<1), atanh(|x|>1)) — rendered "NaN" byte-identically on all
// three legs (same as the shipped asin/acos out-of-domain path).
math_unary!(math_asinh, asinh, "asinh");
math_unary!(math_acosh, acosh, "acosh");
math_unary!(math_atanh, atanh, "atanh");
math_unary!(math_log1p, ln_1p, "log1p");
math_unary!(math_expm1, exp_m1, "expm1");
math_unary!(math_deg_to_rad, to_radians, "degToRad");
math_unary!(math_rad_to_deg, to_degrees, "radToDeg");
// `log2` uses `x.log(2.0)` (= ln(x)/ln(2)) — the SAME formula as the PHP emitter `log($x, 2)` — so the
// three backends agree bit-for-bit. `x.log2()` (a direct libm call) could differ by a ULP from PHP.
fn math_log2(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x)] => Ok(Value::Float(x.log(2.0))),
        _ => Err("Math.log2 expects (float)".into()),
    }
}
fn math_atan2(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(y), Value::Float(x)] => Ok(Value::Float(y.atan2(*x))),
        _ => Err("Math.atan2 expects (float, float)".into()),
    }
}
fn math_hypot(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(x), Value::Float(y)] => Ok(Value::Float(x.hypot(*y))),
        _ => Err("Math.hypot expects (float, float)".into()),
    }
}
fn math_pi(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [] => Ok(Value::Float(std::f64::consts::PI)),
        _ => Err("Math.pi expects ()".into()),
    }
}
fn math_e(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [] => Ok(Value::Float(std::f64::consts::E)),
        _ => Err("Math.e expects ()".into()),
    }
}
fn math_number_format(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // A negative `decimals` is clamped to 0 (matching the PHP helper), so this never faults.
        [Value::Float(v), Value::Int(d)] => Ok(Value::Str(
            crate::value::number_format(*v, (*d).max(0) as usize).into(),
        )),
        _ => Err("Math.numberFormat expects (float, int)".into()),
    }
}

/// The `Core.Math` registry entries (M3 Track B Wave 2; breadth added in M-NUM S4).
fn math_is_even(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(n)] => Ok(Value::Bool(n % 2 == 0)),
        _ => Err("Math.isEven expects (int)".into()),
    }
}

fn math_is_odd(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(n)] => Ok(Value::Bool(n % 2 != 0)),
        _ => Err("Math.isOdd expects (int)".into()),
    }
}

/// Build a `Core.Math` unary `float -> float` native (Wave-B tail helper) — shares the fixed shape so
/// each entry is one line (name + eval kernel + PHP emitter).
fn unary_float(
    name: &'static str,
    eval: fn(&[Value], &mut String) -> Result<Value, String>,
    lift_from: &'static [&'static str],
    php: fn(&[String]) -> String,
) -> NativeFn {
    NativeFn {
        module: "Core.Math",
        name,
        params: vec![Ty::Float],
        ret: Ty::Float,
        pure: true,
        eval: NativeEval::Pure(eval),
        lift_from,
        php,
    }
}

pub(crate) fn math_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.Math",
            name: "isEven",
            params: vec![Ty::Int],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(math_is_even),
            lift_from: &[],
            php: |a| format!("({}) % 2 === 0", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "isOdd",
            params: vec![Ty::Int],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(math_is_odd),
            lift_from: &[],
            php: |a| format!("({}) % 2 !== 0", parg(a, 0)),
        },
        // `Math.tryAdd/trySub/tryMul(int, int): int?` — checked arithmetic returning `null` on overflow
        // (the type-driven recovery for the fail-fast default). PHP int overflow yields a float, so the
        // IIFE returns the int only when it stayed an int (`is_int`), else `null` — the exact i64
        // boundary the Rust `value::int_*` kernels detect, so all three backends agree.
        NativeFn {
            module: "Core.Math",
            name: "tryAdd",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(math_try_add),
            lift_from: &[],
            php: |a| {
                format!(
                    "(function($x, $y) {{ $r = $x + $y; return is_int($r) ? $r : null; }})({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Math",
            name: "trySub",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(math_try_sub),
            lift_from: &[],
            php: |a| {
                format!(
                    "(function($x, $y) {{ $r = $x - $y; return is_int($r) ? $r : null; }})({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Math",
            name: "tryMul",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(math_try_mul),
            lift_from: &[],
            php: |a| {
                format!(
                    "(function($x, $y) {{ $r = $x * $y; return is_int($r) ? $r : null; }})({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Math",
            name: "sqrt",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_sqrt),
            lift_from: &["sqrt"],
            php: |a| format!("sqrt({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "pow",
            params: vec![Ty::Float, Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_pow),
            lift_from: &["pow"],
            php: |a| format!("pow({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            // `Math.integerPower(int, int) -> int` — integer power as a value (the `**` operator's named twin,
            // Phase 1 operators slice). PHP's `pow` returns an `int` for non-negative int args whose
            // result fits, matching the kernel's safe domain; the negative/overflow cases fault in
            // Phorj (never reached by a byte-identity example).
            module: "Core.Math",
            name: "integerPower",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_ipow),
            lift_from: &[],
            php: |a| format!("pow({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "floor",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_floor),
            lift_from: &["floor"],
            php: |a| format!("floor({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "ceil",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_ceil),
            lift_from: &["ceil"],
            php: |a| format!("ceil({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "abs",
            params: vec![Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_abs),
            lift_from: &["abs"],
            php: |a| format!("abs({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "min",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_min),
            lift_from: &["min"],
            php: |a| format!("min({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "max",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_max),
            lift_from: &["max"],
            php: |a| format!("max({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "round",
            params: vec![Ty::Float],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_round),
            lift_from: &[],
            php: |a| format!("(int)round({})", parg(a, 0)),
        },
        // --- Float predicates + special values + intdiv (M-NUM S3) ---
        NativeFn {
            module: "Core.Math",
            name: "isNaN",
            params: vec![Ty::Float],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(math_is_nan),
            lift_from: &["is_nan"],
            php: |a| format!("is_nan({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "isFinite",
            params: vec![Ty::Float],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(math_is_finite),
            lift_from: &["is_finite"],
            php: |a| format!("is_finite({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "isInfinite",
            params: vec![Ty::Float],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(math_is_infinite),
            lift_from: &["is_infinite"],
            php: |a| format!("is_infinite({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "nan",
            params: vec![],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_nan),
            lift_from: &[],
            php: |_| "NAN".to_string(),
        },
        NativeFn {
            module: "Core.Math",
            name: "infinity",
            params: vec![],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_infinity),
            lift_from: &[],
            php: |_| "INF".to_string(),
        },
        NativeFn {
            module: "Core.Math",
            name: "negativeInfinity",
            params: vec![],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_neg_infinity),
            lift_from: &[],
            php: |_| "-INF".to_string(),
        },
        NativeFn {
            module: "Core.Math",
            name: "integerDivide",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_intdiv),
            lift_from: &["intdiv"],
            php: |a| format!("intdiv({}, {})", parg(a, 0), parg(a, 1)),
        },
        // --- Math breadth (M-NUM S4) ---
        NativeFn {
            module: "Core.Math",
            name: "sign",
            params: vec![Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_sign),
            // PHP `<=>` yields -1/0/1 and evaluates its operand once (no double-emission).
            lift_from: &[],
            php: |a| format!("({} <=> 0)", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "clamp",
            params: vec![Ty::Int, Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_clamp),
            // Erases to a gated `__phorj_clamp` helper (UA-1.7): it must fault on `lo > hi` to match
            // the native, which the inline `max(min())` could not express.
            lift_from: &[],
            php: |a| {
                format!(
                    "__phorj_clamp({}, {}, {})",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2)
                )
            },
        },
        NativeFn {
            module: "Core.Math",
            name: "gcd",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_gcd),
            lift_from: &[],
            php: |a| format!("__phorj_gcd({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "lcm",
            params: vec![Ty::Int, Ty::Int],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(math_lcm),
            lift_from: &[],
            php: |a| format!("__phorj_lcm({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "log",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_log),
            lift_from: &["log"],
            php: |a| format!("log({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "log10",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_log10),
            lift_from: &["log10"],
            php: |a| format!("log10({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "exp",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_exp),
            lift_from: &["exp"],
            php: |a| format!("exp({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "sin",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_sin),
            lift_from: &["sin"],
            php: |a| format!("sin({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "cos",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_cos),
            lift_from: &["cos"],
            php: |a| format!("cos({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Math",
            name: "tan",
            params: vec![Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_tan),
            lift_from: &["tan"],
            php: |a| format!("tan({})", parg(a, 0)),
        },
        // Wave-B math tail (G-math-breadth). Unary f64 → f64 (phorj name == PHP name for these).
        unary_float("asin", math_asin, &["asin"], |a| {
            format!("asin({})", parg(a, 0))
        }),
        unary_float("acos", math_acos, &["acos"], |a| {
            format!("acos({})", parg(a, 0))
        }),
        unary_float("atan", math_atan, &["atan"], |a| {
            format!("atan({})", parg(a, 0))
        }),
        unary_float("sinh", math_sinh, &["sinh"], |a| {
            format!("sinh({})", parg(a, 0))
        }),
        unary_float("cosh", math_cosh, &["cosh"], |a| {
            format!("cosh({})", parg(a, 0))
        }),
        unary_float("tanh", math_tanh, &["tanh"], |a| {
            format!("tanh({})", parg(a, 0))
        }),
        unary_float("asinh", math_asinh, &["asinh"], |a| {
            format!("asinh({})", parg(a, 0))
        }),
        unary_float("acosh", math_acosh, &["acosh"], |a| {
            format!("acosh({})", parg(a, 0))
        }),
        unary_float("atanh", math_atanh, &["atanh"], |a| {
            format!("atanh({})", parg(a, 0))
        }),
        unary_float("log2", math_log2, &[], |a| {
            format!("log({}, 2)", parg(a, 0))
        }),
        unary_float("log1p", math_log1p, &["log1p"], |a| {
            format!("log1p({})", parg(a, 0))
        }),
        unary_float("expm1", math_expm1, &["expm1"], |a| {
            format!("expm1({})", parg(a, 0))
        }),
        // Angle conversion — clearer camelCase than PHP `deg2rad`/`rad2deg` (transpiled to those).
        unary_float("degToRad", math_deg_to_rad, &["deg2rad"], |a| {
            format!("deg2rad({})", parg(a, 0))
        }),
        unary_float("radToDeg", math_rad_to_deg, &["rad2deg"], |a| {
            format!("rad2deg({})", parg(a, 0))
        }),
        NativeFn {
            module: "Core.Math",
            name: "atan2",
            params: vec![Ty::Float, Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_atan2),
            lift_from: &["atan2"],
            php: |a| format!("atan2({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "hypot",
            params: vec![Ty::Float, Ty::Float],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_hypot),
            lift_from: &["hypot"],
            php: |a| format!("hypot({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Math",
            name: "pi",
            params: vec![],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_pi),
            lift_from: &[],
            php: |_| "M_PI".to_string(),
        },
        NativeFn {
            module: "Core.Math",
            name: "e",
            params: vec![],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(math_e),
            lift_from: &[],
            php: |_| "M_E".to_string(),
        },
        NativeFn {
            module: "Core.Math",
            name: "numberFormat",
            params: vec![Ty::Float, Ty::Int],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(math_number_format),
            lift_from: &[],
            php: |a| format!("__phorj_number_format({}, {})", parg(a, 0), parg(a, 1)),
        },
    ]
}

// ---- Core.Text ----------------------------------------------------------------------------------
// String natives, all concrete-typed. Each erases to a PHP string builtin (D-L9). ASCII-oriented to
// stay byte-identical with PHP: `len` is the *byte* length (PHP `strlen`), and `upper`/`lower` are
// ASCII-case (PHP `strtoupper`/`strtolower`), so multi-byte text could differ between the Rust
// backends and PHP — examples use ASCII. The interp↔VM spine is always byte-identical (both Rust).

#[cfg(test)]
#[path = "math_tests.rs"]
mod tests;
