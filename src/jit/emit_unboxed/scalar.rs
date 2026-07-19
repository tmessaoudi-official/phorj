//! UNBOXED scalar arms — checked int arithmetic (speculation sticky), div/rem (incl. the
//! P-2c range-proven `RemI`-by-pow2 → `band`), IEEE float arithmetic, negation, boolean ops,
//! integer comparisons, and the P-2c fully-inline numeric conversions. Bodies moved verbatim
//! from the pre-decomposition `emit_unboxed.rs` (M-Decomp); shared emit state arrives via [`Ec`].

use super::*;

/// `Op::AddI | Op::SubI | Op::MulI`. `plain` (computed at dispatch) = `#[UncheckedOverflow]`
/// (the whole fn wraps — all of AddI/SubI/MulI) OR a range-proven induction `AddI`.
pub(super) fn arm_int_arith(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    op: &Op,
    plain: bool,
) -> Result<(), JitError> {
    let (bv, _) = ub_pop(b, vars, fvars, kinds)?;
    let (av, _) = ub_pop(b, vars, fvars, kinds)?;
    // Plain (wrapping-free) when: `#[UncheckedOverflow]` (the whole fn wraps — all of AddI/SubI/MulI),
    // OR a range-proven induction `AddI`. The two's-complement `iadd`/`isub`/`imul` result is
    // bit-identical to `*_overflow`'s result[0] (byte-identity ✓); no fault, no sticky.
    if plain {
        let res = match op {
            Op::AddI => b.ins().iadd(av, bv),
            Op::SubI => b.ins().isub(av, bv),
            _ => b.ins().imul(av, bv),
        };
        ub_push(b, vars, fvars, kinds, res, Kind::Int)
    } else {
        // ovf-spec: WRAPPING result + OR the overflow carry into sticky — NO per-op branch (the
        // per-op `*_overflow`+branch was the intadd perf loss). `sadd_overflow`'s result[0] IS
        // the two's-complement wrapped value; push it, fold result[1] (the carry) into sticky.
        let (res, overflow) = match op {
            Op::AddI => b.ins().sadd_overflow(av, bv),
            Op::SubI => b.ins().ssub_overflow(av, bv),
            _ => b.ins().smul_overflow(av, bv),
        };
        ec.accumulate_sticky(b, overflow);
        ub_push(b, vars, fvars, kinds, res, Kind::Int)
    }
}

/// `Op::DivI | Op::RemI` — a range-PROVEN `RemI` (non-negative dividend, positive
/// power-of-two const divisor) lowers to a single `band`; otherwise both fault conditions
/// stay real per-op branches funneled to code 5 (redo on VM).
#[allow(clippy::too_many_arguments)] // same shape as `build_body_unboxed` — emit plumbing
pub(super) fn arm_div_rem(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    func: &crate::chunk::Function,
    ip: usize,
    op: &Op,
    proven: bool,
) -> Result<(), JitError> {
    let (bv, _) = ub_pop(b, vars, fvars, kinds)?;
    let (av, _) = ub_pop(b, vars, fvars, kinds)?;
    // P-2c: a range-PROVEN `RemI` (non-negative dividend, positive power-of-two const
    // divisor — see `range_proven_ops`) is a single `band`: exact same value (truncated
    // rem of a non-negative by a positive 2^m), and both fault conditions are impossible.
    if matches!(op, Op::RemI) && proven {
        let c = match &func.chunk.code[ip - 1] {
            Op::Const(ci) => match func.chunk.consts.get(*ci) {
                Some(Value::Int(c)) => *c,
                other => {
                    return Err(JitError::Codegen(format!(
                        "proven RemI divisor not an int const: {other:?}"
                    )))
                }
            },
            other => {
                return Err(JitError::Codegen(format!(
                    "proven RemI not preceded by Const: {other:?}"
                )))
            }
        };
        let res = b.ins().band_imm(av, c - 1);
        return ub_push(b, vars, fvars, kinds, res, Kind::Int);
    }
    // ovf-spec: div/rem CANNOT be speculated — `sdiv`/`srem` hardware-trap (SIGFPE) on both
    // divide-by-zero AND i64::MIN / -1. So KEEP both as real per-op branches (rare → cheap),
    // but funnel them to code 5 (redo on VM) like every other fault; the VM redo renders the
    // exact div-zero / mod-zero / overflow string in correct order.
    let zero = b.ins().iconst(types::I64, 0);
    let is_zero = b.ins().icmp(IntCC::Equal, bv, zero);
    ec.fault_if(b, is_zero, 5);
    let imin = b.ins().iconst(types::I64, i64::MIN);
    let a_is_min = b.ins().icmp(IntCC::Equal, av, imin);
    let neg1 = b.ins().iconst(types::I64, -1);
    let b_is_neg1 = b.ins().icmp(IntCC::Equal, bv, neg1);
    let both = b.ins().band(a_is_min, b_is_neg1);
    ec.fault_if(b, both, 5);
    let res = if matches!(op, Op::DivI) {
        b.ins().sdiv(av, bv)
    } else {
        b.ins().srem(av, bv)
    };
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `Op::AddF | Op::SubF | Op::MulF` — dual-space float arith: operands arrive from the F64
/// space already as f64 (NO per-op bitcast), op, push the f64 result to the F64 space. NO
/// fault, NO sticky — IEEE arith is total (overflow yields inf, not a fault), matching
/// `value::float_{add,sub,mul}`. Same ops in the same order ⇒ bit-identical to the VM oracle
/// (Invariant #1). (`RemF` is NOT in the subset: Cranelift has no native frem — fmod libcall
/// deferred; `collect_functions_unboxed` default-denies it, so it never reaches here.)
pub(super) fn arm_float_arith(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    op: &Op,
) -> Result<(), JitError> {
    let (bf, _) = ub_pop(b, vars, fvars, kinds)?;
    let (af, _) = ub_pop(b, vars, fvars, kinds)?;
    let rf = match op {
        Op::AddF => b.ins().fadd(af, bf),
        Op::SubF => b.ins().fsub(af, bf),
        _ => b.ins().fmul(af, bf),
    };
    ub_push(b, vars, fvars, kinds, rf, Kind::Float)
}

/// `Op::DivF` — float division: a ZERO divisor faults (`value::float_div`: `b == 0.0`, incl.
/// -0.0) — no hardware trap, but a semantic fault → branch to code 5 (redo on VM renders
/// FAULT_DIV_ZERO). `fcmp Equal` is false for NaN, so a NaN/inf divisor does NOT fault →
/// fdiv yields NaN/inf, matching `float_div`'s `Ok(a / b)`.
pub(super) fn arm_div_f(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (bf, _) = ub_pop(b, vars, fvars, kinds)?;
    let (af, _) = ub_pop(b, vars, fvars, kinds)?;
    let zero = b.ins().f64const(0.0);
    let is_zero = b.ins().fcmp(FloatCC::Equal, bf, zero);
    ec.fault_if(b, is_zero, 5);
    let rf = b.ins().fdiv(af, bf);
    ub_push(b, vars, fvars, kinds, rf, Kind::Float)
}

/// `Op::Neg` — ovf-spec: -i64::MIN overflows on the VM, but `ineg` does NOT hardware-trap
/// (it wraps MIN→MIN) — unlike div — so we speculate: fold `av == MIN` into sticky (no
/// branch) and emit the wrapping `ineg`. A set sticky forces the redo at the next back-edge
/// / Return. `#[UncheckedOverflow]`: `-i64::MIN` wraps to `i64::MIN` — plain `ineg`, no sticky.
pub(super) fn arm_neg(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    unchecked: bool,
) -> Result<(), JitError> {
    let (av, _) = ub_pop(b, vars, fvars, kinds)?;
    if !unchecked {
        let imin = b.ins().iconst(types::I64, i64::MIN);
        let is_min = b.ins().icmp(IntCC::Equal, av, imin);
        ec.accumulate_sticky(b, is_min);
    }
    let res = b.ins().ineg(av);
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `Op::Not` — boolean negation (1 iff the operand is 0/false).
pub(super) fn arm_not(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (av, _) = ub_pop(b, vars, fvars, kinds)?;
    let r = b.ins().icmp_imm(IntCC::Equal, av, 0); // 1 iff false
    let r64 = b.ins().uextend(types::I64, r);
    ub_push(b, vars, fvars, kinds, r64, Kind::Bool)
}

/// `Op::Eq | Ne | Lt | Gt | Le | Ge` — integer comparisons only. `icmp` is only correct on
/// integer bit-patterns; float/handle/ambiguous operands are rejected (VM fallback).
pub(super) fn arm_cmp(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    op: &Op,
) -> Result<(), JitError> {
    let (bv, bk) = ub_pop(b, vars, fvars, kinds)?;
    let (av, ak) = ub_pop(b, vars, fvars, kinds)?;
    // FLOAT comparisons (both operands PROVEN Float — they live in the F64 space): the exact
    // `partial_cmp` projection of the shared kernels. `Lt/Gt/Le/Ge` = `value::compare_ord`'s
    // `Some(o)` arm with `None` (NaN) → false — Cranelift's ORDERED FloatCC codes are exactly
    // that. `Eq` = `eq_val`'s IEEE `==` (NaN ≠ NaN → ordered Equal); `Ne` = its negation
    // (NaN → true — FloatCC::NotEqual is unordered-or-unequal, the precise complement).
    if ak == Kind::Float && bk == Kind::Float {
        let cc = match op {
            Op::Eq => FloatCC::Equal,
            Op::Ne => FloatCC::NotEqual,
            Op::Lt => FloatCC::LessThan,
            Op::Gt => FloatCC::GreaterThan,
            Op::Le => FloatCC::LessThanOrEqual,
            _ => FloatCC::GreaterThanOrEqual,
        };
        let r = b.ins().fcmp(cc, av, bv);
        let r64 = b.ins().uextend(types::I64, r);
        return ub_push(b, vars, fvars, kinds, r64, Kind::Bool);
    }
    // `icmp` is only correct on integer bit-patterns. Reject unless BOTH operands are safely
    // non-float. A known-`Float`-vs-other pairing → reject. An `Unknown` operand is AMBIGUOUS
    // (a float param used only in comparisons is never proven Float — the trap this guards):
    // require the OTHER operand to be a KNOWN non-float (Int/Bool); the checker's homogeneous-
    // comparison rule then guarantees the Unknown is the same non-float type. Both-Unknown →
    // reject (VM fallback).
    let known_nonfloat = |k: Kind| matches!(k, Kind::Int | Kind::Bool);
    if ak == Kind::Float
        || bk == Kind::Float
        || ak.is_handle()
        || bk.is_handle()
        || !(known_nonfloat(ak) || known_nonfloat(bk))
    {
        return Err(JitError::Unsupported(format!(
            "unboxed: float/handle/ambiguous comparison operands ({ak:?}, {bk:?}) — deferred"
        )));
    }
    let cc = match op {
        Op::Eq => IntCC::Equal,
        Op::Ne => IntCC::NotEqual,
        Op::Lt => IntCC::SignedLessThan,
        Op::Gt => IntCC::SignedGreaterThan,
        Op::Le => IntCC::SignedLessThanOrEqual,
        _ => IntCC::SignedGreaterThanOrEqual,
    };
    let r = b.ins().icmp(cc, av, bv);
    let r64 = b.ins().uextend(types::I64, r);
    ub_push(b, vars, fvars, kinds, r64, Kind::Bool)
}

/// `Conversion.toFloat(int)` — the kernel is `n as f64`; `fcvt_from_sint` is the same IEEE
/// round-to-nearest widening. Total: no fault path.
pub(super) fn arm_to_float(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int {
        return Err(JitError::Unsupported(format!(
            "unboxed toFloat operand kind {ik:?}"
        )));
    }
    let fv = b.ins().fcvt_from_sint(types::F64, iv);
    ub_push(b, vars, fvars, kinds, fv, Kind::Float)
}

/// `Conversion.truncate(float)` — mirrors `value::float_to_int` EXACTLY: trunc toward zero,
/// then require `LOWER <= t < UPPER` (NaN/±∞ fail the ordered compares). In-range →
/// `fcvt_to_sint` (cannot trap under the guard, and `t` is already integral so the conversion
/// is exact); out-of-range → code 5, the VM redo renders the canonical "float is out of int
/// range" fault.
pub(super) fn arm_truncate(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    if fk != Kind::Float {
        return Err(JitError::Unsupported(format!(
            "unboxed truncate operand kind {fk:?}"
        )));
    }
    let t = b.ins().trunc(fv);
    let lower = b.ins().f64const(-9_223_372_036_854_775_808.0);
    let upper = b.ins().f64const(9_223_372_036_854_775_808.0);
    let ge = b.ins().fcmp(FloatCC::GreaterThanOrEqual, t, lower);
    let lt = b.ins().fcmp(FloatCC::LessThan, t, upper);
    let ok = b.ins().band(ge, lt);
    let bad = b.ins().icmp_imm(IntCC::Equal, ok, 0);
    ec.fault_if(b, bad, 5);
    let res = b.ins().fcvt_to_sint(types::I64, t);
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// `Math.max(int, int): int` — the kernel is `(*a).max(*b)` = `i64::max`; Cranelift's scalar
/// `smax` is the same SIGNED max, so interp ≡ VM ≡ JIT ≡ php by construction. Pure scalar, no
/// fault path, no handle space. Second arg is on top of the stack (`bv`, popped first).
pub(super) fn arm_math_max(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (bv, bk) = ub_pop(b, vars, fvars, kinds)?;
    let (av, ak) = ub_pop(b, vars, fvars, kinds)?;
    if ak != Kind::Int || bk != Kind::Int {
        return Err(JitError::Unsupported(format!(
            "unboxed Math.max operand kinds ({ak:?}, {bk:?})"
        )));
    }
    let res = b.ins().smax(av, bv);
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}
