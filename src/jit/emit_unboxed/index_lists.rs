//! `Op::Index` over LIST kinds (M-Decomp from `verticals.rs`, Invariant 13): the int-list and
//! str-list element reads — inline flat loads, the inline ACL-record legs (a `Map.values` /
//! SHARED `Map.keys` result), and the boxed helper fallbacks. Bodies moved verbatim.

use super::*;

/// `Op::Index` with an `IntList` beneath the index — P-2c int-list element read
/// (`xs[i]` → Int, raw i64 at slot bytes 0..8): inline unsigned bounds check + ONE load
/// for a flat list; the two-return helper for a boxed one.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_index_int_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    proven: bool,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int || !matches!(lk, Kind::IntList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({lk:?}[{ik:?}])"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let flat_blk = b.create_block();
    let chk_acl = b.create_block();
    let acl_blk = b.create_block();
    let slow_blk = b.create_block();
    let flat_bit = b.ins().band_imm_s(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], chk_acl, &[]);
    // INLINE (ACL builder record — a `Map.values` result or a live int builder): bounds check
    // against the record's len word, then ONE load of the raw i64 at `ptr + idx·8`. The value
    // is a COPY, so record lifetime beyond this op is irrelevant; an OWNED record operand is
    // released after the load (runtime-bit-gated — a SHARED record no-ops).
    b.switch_to_block(chk_acl);
    let acl_bit = b.ins().band_imm_s(lv, UB_TAG_ACL);
    b.ins().brif(acl_bit, acl_blk, &[], slow_blk, &[]);
    b.switch_to_block(acl_blk);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let aidx = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let aroff = b.ins().imul_imm_s(aidx, 24);
    let aprec = b.ins().iadd(abase, aroff);
    let alen = b.ins().load(types::I64, MemFlagsData::new(), aprec, 8);
    let acnt = b.ins().ushr_imm_s(alen, 3);
    let aoob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, acnt);
    ec.fault_if(b, aoob, 5);
    let aptr = b.ins().load(types::I64, MemFlagsData::new(), aprec, 0);
    let awoff = b.ins().ishl_imm_s(iv, 3);
    let aaddr = b.ins().iadd(aptr, awoff);
    let ares = b.ins().load(types::I64, MemFlagsData::new(), aaddr, 0);
    if lk.is_owned_handle() {
        emit_release(b, ec, h, lv);
    }
    b.ins().jump(merge, &[ares.into()]);
    // INLINE (flat int list): unsigned bounds check, then ONE load of the raw i64 at
    // `buf[(base+idx)*64]`. Out-of-range → code 5 → the canonical fault on the VM.
    b.switch_to_block(flat_blk);
    // Task-9 v2: a range-PROVEN in-bounds index (interval ⊆ [0, len)) drops the bounds branch.
    if !proven {
        let cnt_raw = b.ins().ushr_imm_s(lv, 40);
        let cnt = b.ins().band_imm_s(cnt_raw, 0xFFFFF);
        let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
        ec.fault_if(b, oob, 5);
    }
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let base = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let slot = b.ins().iadd(base, iv);
    let soff = b.ins().ishl_imm_s(slot, 6);
    let addr = b.ins().iadd(buf, soff);
    let fres = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW (boxed int list): the two-return helper (value spans the full i64 range).
    b.switch_to_block(slow_blk);
    let freev = b.ins().iconst(types::I64, lk.is_owned_handle() as i64);
    let call = b.ins().call(h.index_int, &[ec.ctx, lv, iv, freev]);
    let sval = b.inst_results(call)[0];
    let scode = b.inst_results(call)[1];
    let bad = b.ins().icmp_imm_s(IntCC::NotEqual, scode, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sval.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Int)
}

/// Plain `Op::Index` — string-list element read: a flat list yields base+idx as a BORROWED
/// slot handle (zero copy, zero alloc); a boxed list goes through the helper.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_index_str_list(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    proven: bool,
) -> Result<(), JitError> {
    let (iv, ik) = ub_pop(b, vars, fvars, kinds)?;
    let (lv, lk) = ub_pop(b, vars, fvars, kinds)?;
    if ik != Kind::Int || !matches!(lk, Kind::StrList(_)) {
        return Err(JitError::Unsupported(format!(
            "unboxed Index operand kinds ({lk:?}[{ik:?}])"
        )));
    }
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let flat_blk = b.create_block();
    let chk_acl = b.create_block();
    let acl_blk = b.create_block();
    let slow_blk = b.create_block();
    let flat_bit = b.ins().band_imm_s(lv, UB_TAG_FLAT);
    b.ins().brif(flat_bit, flat_blk, &[], chk_acl, &[]);
    // INLINE (SHARED ACLS record — a `Map.keys` result): the record is MEMO-pinned (immortal
    // for the run) and its words are borrowed bump-pinned slot handles, so handing a word out
    // with OWNED stripped is a sound zero-copy borrow. Non-SHARED records keep the helper leg
    // (their owned words / lifetime need the clone discipline).
    b.switch_to_block(chk_acl);
    let acl_shared = b.ins().band_imm_s(lv, UB_TAG_ACL | UB_TAG_SHARED);
    let is_shared = b
        .ins()
        .icmp_imm_s(IntCC::Equal, acl_shared, UB_TAG_ACL | UB_TAG_SHARED);
    b.ins().brif(is_shared, acl_blk, &[], slow_blk, &[]);
    b.switch_to_block(acl_blk);
    let abase = b.ins().load(types::I64, ec.stable, ec.ctx, 40);
    let aidx = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let aroff = b.ins().imul_imm_s(aidx, 24);
    let aprec = b.ins().iadd(abase, aroff);
    let alen = b.ins().load(types::I64, MemFlagsData::new(), aprec, 8);
    let acnt = b.ins().ushr_imm_s(alen, 3);
    let aoob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, acnt);
    ec.fault_if(b, aoob, 5);
    let aptr = b.ins().load(types::I64, MemFlagsData::new(), aprec, 0);
    let awoff = b.ins().ishl_imm_s(iv, 3);
    let aaddr = b.ins().iadd(aptr, awoff);
    let aword = b.ins().load(types::I64, MemFlagsData::new(), aaddr, 0);
    let ares = b.ins().band_imm_s(aword, !UB_TAG_OWNED);
    b.ins().jump(merge, &[ares.into()]);
    // INLINE (flat list): unsigned bounds check (a negative idx is a huge u64 — same
    // reject as the VM's `usize::try_from`), then base+idx is a BORROWED slot handle —
    // zero copy, zero alloc. Out-of-range → code 5 → the VM redo renders the canonical
    // "list index out of range".
    b.switch_to_block(flat_blk);
    // Task-9 v2: a range-PROVEN in-bounds index (interval ⊆ [0, len)) drops the bounds branch.
    if !proven {
        let cnt_raw = b.ins().ushr_imm_s(lv, 40);
        let cnt = b.ins().band_imm_s(cnt_raw, 0xFFFFF);
        let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
        ec.fault_if(b, oob, 5);
    }
    let base = b.ins().band_imm_s(lv, UB_IDX_MASK);
    let slot = b.ins().iadd(base, iv);
    let fres = b.ins().bor_imm_s(slot, UB_TAG_SLOT);
    b.ins().jump(merge, &[fres.into()]);
    // SLOW (boxed list): the helper (element clone into a slot / untagged temp).
    b.switch_to_block(slow_blk);
    let freev = b.ins().iconst(types::I64, lk.is_owned_handle() as i64);
    let call = b.ins().call(h.index, &[ec.ctx, lv, iv, freev]);
    let sres = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, sres, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[sres.into()]);
    b.switch_to_block(merge);
    let res = b.block_params(merge)[0];
    ub_push(b, vars, fvars, kinds, res, Kind::Str(Own::Owned))
}
