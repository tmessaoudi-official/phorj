//! String-SCAN emit arms (the DEC-332 stringcontains/isemail/isurl flips): thin call arms over
//! the dedicated zero-alloc helpers in `handles/strings_ext.rs` — the win over the generic
//! bridge2 route is skipping the two boxed `Value`/`PhStr` materializations per call; the
//! kernels themselves stay single-sourced in the natives. Sibling of `verticals.rs`
//! (Invariant 13).

use super::*;

/// Inline direct-mapped probe of the STRING-PREDICATE memo (entries 16..24 of the memo table —
/// `{w0, w1, result + 1}`, installed only for PINNED operand pairs by `handles/strings_ext.rs`)
/// followed by the helper call on a miss. Steady state on the bench shapes (const haystack ×
/// rotating flat-list needles) is ~8 ops, no call. Mixing MUST mirror `memo_str_install`
/// bit-for-bit. The helper's `-1` (defensive non-string) → code 5, VM redo.
fn emit_str_memo2(
    b: &mut FunctionBuilder,
    ec: &Ec,
    helper: FuncRef,
    w0: ClValue,
    w1: ClValue,
    extra: ClValue,
) -> ClValue {
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let chk1 = b.create_block();
    let chk2 = b.create_block();
    let slow = b.create_block();
    let memo = b.ins().load(types::I64, ec.stable, ec.ctx, 48);
    let w1s = b.ins().ishl_imm_s(w1, 1);
    let x = b.ins().bxor(w0, w1s);
    let mixed = b.ins().imul_imm_s(x, UB_SET_HASH_MULT);
    let ei = b.ins().ushr_imm_s(mixed, 61);
    let eoff = b.ins().imul_imm_s(ei, 24);
    let ebase = b.ins().iadd_imm_s(eoff, 384); // entries 16..24
    let eaddr = b.ins().iadd(memo, ebase);
    let m0 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 0);
    let eq0 = b.ins().icmp(IntCC::Equal, m0, w0);
    b.ins().brif(eq0, chk1, &[], slow, &[]);
    b.switch_to_block(chk1);
    let m1 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 8);
    let eq1 = b.ins().icmp(IntCC::Equal, m1, w1);
    b.ins().brif(eq1, chk2, &[], slow, &[]);
    b.switch_to_block(chk2);
    let r1 = b.ins().load(types::I64, MemFlagsData::new(), eaddr, 16);
    let hit = b.ins().iadd_imm_s(r1, -1); // stored result + 1; 0 = empty line
    b.ins().brif(r1, merge, &[hit.into()], slow, &[]);
    b.switch_to_block(slow);
    let call = b.ins().call(helper, &[ec.ctx, w0, w1, extra]);
    let r = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[r.into()]);
    b.switch_to_block(merge);
    b.block_params(merge)[0]
}

/// `Op::CallNative(Core.String.contains, 2)` — pop needle then hay (both `Str`), inline memo
/// probe + helper scan, push the 0/1 word as `Bool`.
pub(super) fn arm_str_contains(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (nv, nk) = ub_pop(b, vars, fvars, kinds)?;
    let (hv, hk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(hk, Kind::Str(_)) || !matches!(nk, Kind::Str(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed String.contains operand kinds ({hk:?}, {nk:?})"
        )));
    }
    // A memo hit can only key PINNED words (never installed otherwise), whose release is a
    // no-op — so skipping the helper's mask releases on the hit leg is exact.
    let mask = (hk.is_owned_handle() as i64) | ((nk.is_owned_handle() as i64) << 1);
    let maskv = b.ins().iconst(types::I64, mask);
    let r = emit_str_memo2(b, ec, h.str_contains, hv, nv, maskv);
    ub_push(b, vars, fvars, kinds, r, Kind::Bool)
}

/// `Op::CallNative(Core.Validation.{isEmail,isUrl}, 1)` — pop the `Str` operand; the memo pair
/// key is `(s, -(which + 1))` (a negative word is never a handle — no collision with contains
/// pairs); helper on a miss; push the 0/1 word as `Bool`.
pub(super) fn arm_validate(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    which: i64,
) -> Result<(), JitError> {
    let (sv, sk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(sk, Kind::Str(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Validation operand kind {sk:?}"
        )));
    }
    // The probe and the helper share the SAME second word: `key = -(which + 1)` (the helper
    // re-derives `which` from it), so the uniform memo2 emission covers both verticals.
    let keyv = b.ins().iconst(types::I64, -(which + 1));
    let freev = b.ins().iconst(types::I64, sk.is_owned_handle() as i64);
    let r = emit_str_memo2(b, ec, h.validate, sv, keyv, freev);
    ub_push(b, vars, fvars, kinds, r, Kind::Bool)
}
