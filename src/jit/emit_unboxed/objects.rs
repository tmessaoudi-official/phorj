//! UNBOXED object arms — the FLAT-ARENA object vertical. A [`Kind::Inst`] value is an arena
//! SLOT handle (`SLOT|OWNED` tagged) whose fields live flat at byte `8·layout_slot` inside the
//! 64-byte slot (≤ 8 int fields; the class rides in the compile-time kind). `MakeInstance` =
//! one inline slot alloc + `n` stores; `GetField`/`SetField` = ONE load/store at a static
//! offset (the subset gate guarantees every field is ctor-initialized, so a read can never see
//! the VM's `None` window — `GetField` is total). No helper, no boxed fallback: instances exist
//! only inside the JIT'd graph (an instance-returning ENTRY is rejected), and every fault path
//! (arena exhaustion) is code 5 = redo-on-VM. Ownership mirrors the string handles: `Owned` is
//! recycled by its consumer / `Pop`; `GetLocal` copies are `Borrowed`; a temp receiver dying at
//! `GetField`/`SetField`/`CallMethod` is freed by the consuming arm (the VM's `Rc` drop).

use super::*;

/// `Op::MakeInstance(cidx)` — allocate a slot, store the ctor field values (stack order =
/// `desc.fields` order) each at its layout slot via the static permutation `perm`, push the
/// `SLOT|OWNED` handle.
pub(super) fn arm_make_instance(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    cidx: usize,
    perm: &[usize],
) -> Result<(), JitError> {
    let nf = perm.len();
    let d = kinds.len();
    if nf > d {
        return Err(JitError::Codegen(
            "unboxed MakeInstance underflow".to_string(),
        ));
    }
    let sidx = ec.slot_alloc(b);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let soff = b.ins().ishl_imm(sidx, 6);
    let pd = b.ins().iadd(buf, soff);
    // Field j (push order) → byte offset 8·perm[j]; values read straight from their
    // depth-indexed Variables (no pops — the kind stack is truncated once below).
    for (j, &slot) in perm.iter().enumerate() {
        let v = b.use_var(vars[d - nf + j]);
        b.ins().store(MemFlagsData::new(), v, pd, (8 * slot) as i32);
    }
    kinds.truncate(d - nf);
    let h_raw = b.ins().bor_imm(sidx, UB_TAG_SLOT);
    let h = b.ins().bor_imm(h_raw, UB_TAG_OWNED);
    ub_push(b, vars, fvars, kinds, h, Kind::Inst(cidx, Own::Owned))
}

/// `Op::GetField(nidx)` — one inline load at the static layout offset; an OWNED temp receiver
/// (`new C(..).f`) dies here → recycle it after the read (the VM's `Rc` drop).
pub(super) fn arm_get_field(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    nidx: usize,
) -> Result<(), JitError> {
    let (rv, rk) = ub_pop(b, vars, fvars, kinds)?;
    let Kind::Inst(c, _) = rk else {
        return Err(JitError::Unsupported(format!(
            "unboxed: GetField on {rk:?} (deferred)"
        )));
    };
    let slot = program.class_descs[c]
        .layout
        .slot(&program.names[nidx])
        .ok_or_else(|| {
            JitError::Codegen("unboxed: GetField name unresolved past analyze".to_string())
        })?;
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let ri = b.ins().band_imm(rv, UB_IDX_MASK);
    let roff = b.ins().ishl_imm(ri, 6);
    let pr = b.ins().iadd(buf, roff);
    let val = b
        .ins()
        .load(types::I64, MemFlagsData::new(), pr, (8 * slot) as i32);
    if rk.is_owned_handle() {
        ec.slot_free_if_owned(b, rv);
    }
    ub_push(b, vars, fvars, kinds, val, Kind::Int)
}

/// `Op::SetField(nidx)` — pop the value (top), then the instance; one inline store at the
/// static layout offset (shared-mutation semantics: every borrow of this handle sees it, same
/// as the VM's in-place `Rc<Instance>` write). An OWNED temp receiver dies here → recycle.
pub(super) fn arm_set_field(
    b: &mut FunctionBuilder,
    ec: &Ec,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    nidx: usize,
) -> Result<(), JitError> {
    let (vv, vk) = ub_pop(b, vars, fvars, kinds)?;
    let (rv, rk) = ub_pop(b, vars, fvars, kinds)?;
    if vk != Kind::Int {
        return Err(JitError::Unsupported(format!(
            "unboxed: SetField value kind {vk:?} (deferred)"
        )));
    }
    let Kind::Inst(c, _) = rk else {
        return Err(JitError::Unsupported(format!(
            "unboxed: SetField on {rk:?} (deferred)"
        )));
    };
    let slot = program.class_descs[c]
        .layout
        .slot(&program.names[nidx])
        .ok_or_else(|| {
            JitError::Codegen("unboxed: SetField name unresolved past analyze".to_string())
        })?;
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let ri = b.ins().band_imm(rv, UB_IDX_MASK);
    let roff = b.ins().ishl_imm(ri, 6);
    let pr = b.ins().iadd(buf, roff);
    b.ins()
        .store(MemFlagsData::new(), vv, pr, (8 * slot) as i32);
    if rk.is_owned_handle() {
        ec.slot_free_if_owned(b, rv);
    }
    Ok(())
}
