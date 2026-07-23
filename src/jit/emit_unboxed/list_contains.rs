//! `Core.List.contains` unboxed vertical — split out of the grandfathered `verticals.rs`
//! (Invariant 13). Future flat-int-list membership/scan verticals belong here beside it.

use super::*;

/// `Core.List.contains(xs, needle)` — the listcontains vertical: an inline LINEAR scan of the flat
/// int block (`count<<40 | base`, 64-byte slot stride — the same element layout `arm_index_int_list`
/// reads), byte-identical to the interpreter's `list_contains` over `Vec<Value::Int>`. A HIT yields
/// `1` (true), an exhausted scan a clean `0` (false), never a fault. A non-flat (boxed) int list —
/// which, unlike an always-flat `IntSet`, CAN occur — punts to a code-5 VM redo (byte-identical).
pub(super) fn arm_listcontains(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let (needle, nk) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if nk != Kind::Int || !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed List.contains operand kinds ({lk:?}, {nk:?})"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64); // 1 = present, 0 = absent
    let flat_blk = b.create_block();
    let slow_blk = b.create_block();
    let flat_bit = b.ins().band_imm_s(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
    // FLAT int list: scan `cnt` raw i64 slots at `buf[(base+i)*64]` (the `arm_index_int_list` load).
    b.switch_to_block(flat_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let cnt_raw = b.ins().ushr_imm_s(lv, 40);
    let cnt = b.ins().band_imm_s(cnt_raw, 0xFFFFF);
    let head = b.create_block();
    b.append_block_param(head, types::I64); // loop index i
    let body = b.create_block();
    let step = b.create_block();
    let zero = b.ins().iconst(types::I64, 0);
    b.ins().jump(head, &[zero.into()]);
    // HEAD: i >= cnt -> exhausted (clean false); else load + compare.
    b.switch_to_block(head);
    let i = b.block_params(head)[0];
    let done = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, i, cnt);
    let absent = b.ins().iconst(types::I64, 0);
    b.ins().brif(done, merge, &[absent.into()], body, &[]);
    b.switch_to_block(body);
    let slot = b.ins().iadd(base, i);
    let soff = b.ins().ishl_imm_s(slot, 6);
    let addr = b.ins().iadd(buf, soff);
    let elem = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    let hit = b.ins().icmp(IntCC::Equal, elem, needle);
    let present = b.ins().iconst(types::I64, 1);
    b.ins().brif(hit, merge, &[present.into()], step, &[]);
    b.switch_to_block(step);
    let i1 = b.ins().iadd_imm_s(i, 1);
    b.ins().jump(head, &[i1.into()]);
    // SLOW (boxed int list): redo the whole call on the VM byte-identically (code 5).
    b.switch_to_block(slow_blk);
    let always = b.ins().iconst(types::I64, 1);
    ec.fault_if(b, always, 5);
    let unreach = b.ins().iconst(types::I64, 0);
    b.ins().jump(merge, &[unreach.into()]); // unreachable terminator (fault_if diverges)
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Bool)
}
