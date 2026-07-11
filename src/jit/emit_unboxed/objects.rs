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
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_get_field(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    nidx: usize,
) -> Result<(), JitError> {
    let (rv, rk) = ub_pop(b, vars, fvars, kinds)?;
    let Kind::Inst(c, _) = rk else {
        return Err(JitError::Unsupported(format!(
            "unboxed: GetField on {rk:?} (deferred)"
        )));
    };
    let desc = &program.class_descs[c];
    let slot = desc.layout.slot(&program.names[nidx]).ok_or_else(|| {
        JitError::Codegen("unboxed: GetField name unresolved past analyze".to_string())
    })?;
    // Result kind mirrors the analyze arm: Int fields load a raw word; a Str field read
    // BORROWS from a live receiver, but TAKES the word from a dying OWNED temp (whose
    // kinded release below then skips that field).
    let j = desc
        .fields
        .iter()
        .position(|f| f == &program.names[nidx])
        .ok_or_else(|| {
            JitError::Codegen("unboxed: GetField field unresolved past analyze".to_string())
        })?;
    let fk = info.field_kind(c, j).ok_or_else(|| {
        JitError::Codegen("unboxed: GetField before signature past analyze".to_string())
    })?;
    let out = match fk {
        Kind::Int => Kind::Int,
        Kind::Str(_) => {
            if rk.is_owned_handle() {
                Kind::Str(Own::Owned)
            } else {
                Kind::Str(Own::Borrowed)
            }
        }
        other => {
            return Err(JitError::Codegen(format!(
                "unboxed: GetField field kind {other:?} past analyze"
            )));
        }
    };
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let ri = b.ins().band_imm(rv, UB_IDX_MASK);
    let roff = b.ins().ishl_imm(ri, 6);
    let pr = b.ins().iadd(buf, roff);
    let val = b
        .ins()
        .load(types::I64, MemFlagsData::new(), pr, (8 * slot) as i32);
    if rk.is_owned_handle() {
        // A dying temp receiver: free its OTHER str fields + its slot; the read field's
        // word (if Str) now belongs to the result.
        let exclude = if matches!(fk, Kind::Str(_)) {
            Some(slot)
        } else {
            None
        };
        release_kinded(b, ec, h, rv, rk, program, info, exclude);
    }
    ub_push(b, vars, fvars, kinds, val, out)
}

/// `Op::SetField(nidx)` — pop the value (top), then the instance; one inline store at the
/// static layout offset (shared-mutation semantics: every borrow of this handle sees it, same
/// as the VM's in-place `Rc<Instance>` write). An OWNED temp receiver dies here → recycle.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_set_field(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    nidx: usize,
) -> Result<(), JitError> {
    let (vv, vk) = ub_pop(b, vars, fvars, kinds)?;
    let (rv, rk) = ub_pop(b, vars, fvars, kinds)?;
    if !matches!(
        vk,
        Kind::Int | Kind::Str(Own::Owned) | Kind::Str(Own::ConstBorrow)
    ) {
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
    // A Str field overwrite releases the OLD word first (the instance owned it; the runtime
    // bit makes a const word's release a no-op).
    if matches!(vk, Kind::Str(_)) {
        let old = b
            .ins()
            .load(types::I64, MemFlagsData::new(), pr, (8 * slot) as i32);
        emit_release(b, ec, h, old);
    }
    b.ins()
        .store(MemFlagsData::new(), vv, pr, (8 * slot) as i32);
    if rk.is_owned_handle() {
        // Statement-temp receiver dies here — its fields (including the value just stored)
        // die with it, exactly like the VM's Rc drop.
        release_kinded(b, ec, h, rv, rk, program, info, None);
    }
    Ok(())
}

/// The LAYOUT slots of class `c`'s `Str` fields (from the fixpoint's per-class signature +
/// the desc's name→slot mapping). Empty for pure-int classes (the common fast case).
pub(super) fn str_field_layout_slots(
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    c: usize,
) -> Vec<usize> {
    let Some(sig) = info.field_kinds.get(c).and_then(|s| s.as_ref()) else {
        return Vec::new();
    };
    let desc = &program.class_descs[c];
    sig.iter()
        .enumerate()
        .filter(|(_, k)| matches!(k, Kind::Str(_)))
        .filter_map(|(j, _)| desc.fields.get(j).and_then(|n| desc.layout.slot(n)))
        .collect()
}

/// KIND-DIRECTED release: an instance with `Str` fields releases each owned field word
/// (runtime-bit-gated — const-word fields are no-ops) BEFORE recycling its own slot;
/// `exclude` skips one layout slot (the field a `GetField` on a dying temp just took
/// ownership of). Everything else routes to the plain fused ladder.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn release_kinded(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    v: ClValue,
    k: Kind,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    exclude: Option<usize>,
) {
    let str_slots: Vec<usize> = match k {
        Kind::Inst(c, _) => str_field_layout_slots(program, info, c)
            .into_iter()
            .filter(|s| Some(*s) != exclude)
            .collect(),
        _ => Vec::new(),
    };
    if str_slots.is_empty() {
        emit_release(b, ec, h, v);
        return;
    }
    // Instances are always slot-tagged: gate on the runtime OWNED bit, then free the str
    // field words (full ladder each — a long string field is an untagged heap handle) and
    // recycle the instance slot itself.
    let owned_bit = b.ins().band_imm(v, UB_TAG_OWNED);
    let free_blk = b.create_block();
    let cont = b.create_block();
    b.ins().brif(owned_bit, free_blk, &[], cont, &[]);
    b.switch_to_block(free_blk);
    let buf = b.ins().load(types::I64, ec.stable, ec.ctx, 0);
    let vi = b.ins().band_imm(v, UB_IDX_MASK);
    let voff = b.ins().ishl_imm(vi, 6);
    let pv = b.ins().iadd(buf, voff);
    for s in str_slots {
        let fw = b
            .ins()
            .load(types::I64, MemFlagsData::new(), pv, (8 * s) as i32);
        emit_release(b, ec, h, fw);
    }
    ec.slot_push(b, v);
    b.ins().jump(cont, &[]);
    b.switch_to_block(cont);
}
