//! Plan row 4b step 5b: the unboxed lowering of `Op::CloneInstance` (`with` on a receiver whose class
//! is not exact at compile time). The VM copies the runtime instance (`vm/runtime_class.rs`); the
//! unboxed tier knows that instance's EXACT class (`Kind::Inst`), so the copy is one slot allocation
//! plus one load/store per field — a scalar word copies as-is, a string/list handle word is CLONED
//! (`emit_arg_clone`, the W9 boundary clone: two instances must never own the same word). The
//! override writes that follow are ordinary `SetField`s on the copy.

use super::*;

/// `Op::CloneInstance` — `[.., instance]` → `[.., copy]`, the copy OWNED. A dying OWNED receiver is
/// released after the copy (its handle fields were cloned, never moved, so the release is total).
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_clone_instance(
    b: &mut FunctionBuilder,
    ec: &Ec,
    ub: Option<&UbHelperRefs>,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
) -> Result<(), JitError> {
    let h = ub_ref(ub, "CloneInstance")?;
    let (rv, rk) = ub_pop(b, vars, fvars, kinds)?;
    let Kind::Inst(c, _) = rk else {
        return Err(JitError::Unsupported(format!(
            "unboxed: CloneInstance on {rk:?} (deferred)"
        )));
    };
    let desc = &program.class_descs[c];
    let wide = is_wide(program, c);
    // The copy's slot(s), exactly as `arm_make_instance` allocates them.
    let sidx = ec.slot_alloc(b);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let pd = {
        let soff = b.ins().ishl_imm_s(sidx, 6);
        b.ins().iadd(buf, soff)
    };
    let pb = if wide {
        let bidx = ec.slot_alloc(b);
        b.ins().store(MemFlagsData::new(), bidx, pd, 56);
        let boff = b.ins().ishl_imm_s(bidx, 6);
        Some(b.ins().iadd(buf, boff))
    } else {
        None
    };
    let ps = {
        let ri = b.ins().band_imm_s(rv, UB_IDX_MASK);
        let roff = b.ins().ishl_imm_s(ri, 6);
        b.ins().iadd(buf, roff)
    };
    for (j, name) in desc.fields.iter().enumerate() {
        let slot = desc.layout.slot(name).ok_or_else(|| {
            JitError::Codegen("unboxed: CloneInstance field not in layout past analyze".to_string())
        })?;
        let fk = info.field_kind(c, j).ok_or_else(|| {
            JitError::Codegen("unboxed: CloneInstance before signature past analyze".to_string())
        })?;
        let (sp, soff) = field_addr(b, ec, ps, slot, wide);
        let w = b.ins().load(types::I64, MemFlagsData::new(), sp, soff);
        let w = match clone_repr(fk)? {
            None => w,
            Some(repr) => emit_arg_clone(b, ec, ub, w, repr)?,
        };
        let (dst, off) = if !wide || slot < 7 {
            (pd, (8 * slot) as i32)
        } else {
            (pb.expect("wide pair allocated"), (8 * (slot - 7)) as i32)
        };
        b.ins().store(MemFlagsData::new(), w, dst, off);
    }
    if rk.is_owned_handle() {
        release_kinded(b, ec, h, rv, rk, program, info, None);
    }
    let h_raw = b.ins().bor_imm_s(sidx, UB_TAG_SLOT);
    let hv = b.ins().bor_imm_s(h_raw, UB_TAG_OWNED);
    ub_push(b, vars, fvars, kinds, hv, Kind::Inst(c, Own::Owned))
}
