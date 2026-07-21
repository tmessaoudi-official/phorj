//! UNBOXED enum arms — the ZERO-ALLOCATION enum vertical. A [`Kind::EnumInt`] value (an enum
//! with at most one `Int` payload) is a register PAIR: payload in `vars[d]`, variant tag (its
//! `enum_descs` index) in `evars[d]`. Construction is two register defs, `MatchTag` one compare,
//! `GetEnumField(0)` the payload word already in hand — no arena, no helper, no free. Tag-index
//! equality ≡ the VM's variant-name equality (descriptors are deduped per (type, variant) and
//! the checker only matches a scrutinee against its own enum's variants); every fault path
//! (the `Fault` backstop, handled in `mod.rs`) funnels to code 5 = redo-on-VM.

use super::*;

/// `Op::MakeEnum(idx)` — arity 1: re-tag the popped `Int` payload as the pair's payload word and
/// def the tag word; arity 0: filler payload + tag. (Arity > 1 / non-int payloads were rejected
/// by collect/analyze.)
pub(super) fn arm_make_enum(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    evars: &[Variable],
    kinds: &mut Vec<Kind>,
    tag: i64,
    arity: usize,
) -> Result<(), JitError> {
    let payload = if arity == 1 {
        let (pv, pk) = ub_pop(b, vars, fvars, kinds)?;
        if pk != Kind::Int {
            return Err(JitError::Unsupported(format!(
                "unboxed MakeEnum payload kind {pk:?}"
            )));
        }
        pv
    } else {
        b.ins().iconst(types::I64, 0)
    };
    // Dest depth = the pair's cell = current top (kinds.len() BEFORE the push).
    let d = kinds.len();
    let evar = *evars
        .get(d)
        .ok_or_else(|| JitError::Codegen(format!("unboxed MakeEnum: tag depth {d} exceeds max")))?;
    let tv = b.ins().iconst(types::I64, tag);
    b.def_var(evar, tv);
    ub_push(b, vars, fvars, kinds, payload, Kind::EnumInt)
}

/// `Op::MatchTag(idx)` — pop the scrutinee copy, compare its tag word against `tag`, push the
/// bool. (The VM compares variant NAMES; index equality is equivalent — see the module docs.)
pub(super) fn arm_match_tag(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    evars: &[Variable],
    kinds: &mut Vec<Kind>,
    tag: i64,
) -> Result<(), JitError> {
    let (_payload, k) = ub_pop(b, vars, fvars, kinds)?;
    if k != Kind::EnumInt {
        return Err(JitError::Unsupported(format!(
            "unboxed MatchTag operand kind {k:?}"
        )));
    }
    // The popped cell's tag word (source depth = kinds.len() AFTER the pop).
    let tv = b.use_var(evars[kinds.len()]);
    let r = b.ins().icmp_imm_s(IntCC::Equal, tv, tag);
    let r64 = b.ins().uextend(types::I64, r);
    ub_push(b, vars, fvars, kinds, r64, Kind::Bool)
}

/// `Op::GetEnumField(0)` — pop the enum pair, push its payload word as `Int`. (Index > 0 was
/// rejected by collect/analyze — the subset is single-payload variants.)
pub(super) fn arm_get_enum_field(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (payload, k) = ub_pop(b, vars, fvars, kinds)?;
    if k != Kind::EnumInt {
        return Err(JitError::Unsupported(format!(
            "unboxed GetEnumField operand kind {k:?}"
        )));
    }
    ub_push(b, vars, fvars, kinds, payload, Kind::Int)
}
