//! Call-boundary PLUMBING (M-Decomp from `emit_unboxed/mod.rs`, Invariant 13): the W9
//! borrowed-argument clone, the call-args pop (kind-checked, clone-on-borrow), the unwind
//! releases, and the shared direct-call emission `emit_call_to` (`Op::Call`/`Op::CallValue`/every
//! HOF vertical's per-element call). Bodies moved verbatim; shared emit state arrives via [`Ec`]
//! and `use super::*` exactly as in the sibling arm files.

use super::*;

/// The W9 call-boundary CLONE of a compile-time-BORROWED str/list argument (PHP value
/// semantics: `this.field` forwarded into the next builder — passing the raw word would
/// leave the owner and the callee both freeing it). Returns the word the callee receives.
///
/// FAST PATH: a runtime-FLAT word passes through UN-cloned — flat snapshots are immutable,
/// bump-pinned for the whole run (never recycled mid-run, so no lifetime hazard wherever
/// the callee stores it), and every release of one is a no-op — "ownership" over a flat
/// word is vacuous. This is what keeps the immutable-threaded builder chain from paying a
/// boxed clone per UNCHANGED field per step (PHP pays a refcount bump there).
pub(super) fn emit_arg_clone(
    b: &mut FunctionBuilder,
    ec: &Ec,
    ub: Option<&UbHelperRefs>,
    v: ClValue,
    repr: i64,
) -> Result<ClValue, JitError> {
    let h = ub_ref(ub, "call-arg clone")?;
    let merge = b.create_block();
    b.append_block_param(merge, types::I64);
    let clone_blk = b.create_block();
    // Second pass-through: a runtime-BORROWED slot word (`x == SLOT` without OWNED) — the only
    // producers are const-interned words and bump-pinned flat elements, both pinned for the whole
    // run and release-no-op, so "ownership" over them is vacuous (the `emit_release` no-op leg). An
    // OWNED slot / untagged heap word falls through to the real clone — this spares the per-step
    // `this.tableName`/`this.tableAlias` const-string clones in a builder chain.
    let chk_slot = b.create_block();
    if repr != 2 {
        // FLAT is a list-word encoding only.
        let flat_bit = b.ins().band_imm_s(v, UB_TAG_FLAT);
        b.ins().brif(flat_bit, merge, &[v.into()], chk_slot, &[]);
    } else {
        b.ins().jump(chk_slot, &[]);
    }
    b.switch_to_block(chk_slot);
    let x = b.ins().band_imm_s(v, UB_TAG_SLOT | UB_TAG_OWNED);
    let borrowed_slot = b.ins().icmp_imm_s(IntCC::Equal, x, UB_TAG_SLOT);
    b.ins()
        .brif(borrowed_slot, merge, &[v.into()], clone_blk, &[]);
    b.switch_to_block(clone_blk);
    let reprv = b.ins().iconst(types::I64, repr);
    let call = b.ins().call(h.clone_value, &[ec.ctx, v, reprv]);
    let cv = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm_s(IntCC::SignedLessThan, cv, 0);
    ec.fault_if(b, bad, 5);
    b.ins().jump(merge, &[cv.into()]);
    b.switch_to_block(merge);
    Ok(b.block_params(merge)[0])
}

/// Pop `argc` args (top is the LAST arg) and build the callee's ABI word vector per its
/// FINAL param kinds (`pks` — slots aligned to the args; a method receiver is prepended by
/// the caller): a `Dyn` param takes TWO words (payload, tag) — a concrete argument gets a
/// constant tag (float args bitcast to their bits), a forwarded Dyn reads its tag from the
/// enum-tag space. Every other param takes the single-word move rules (an Owned/const
/// string or list MOVES into the callee — the word crosses as a plain i64; the callee's
/// single-use-moved param owns it; a BORROWED one CLONES first, W9 — mirrors the analyze
/// arms exactly), rejecting kinds that can't cross one-i64-per-arg (maps stay Owned-only —
/// no clone repr; instance handles, register-pair enums, static `Fn`s). Returns the words
/// in declaration order.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn pop_call_args(
    b: &mut FunctionBuilder,
    ec: &Ec,
    ub: Option<&UbHelperRefs>,
    vars: &[Variable],
    fvars: &[Variable],
    evars: &[Variable],
    kinds: &mut Vec<Kind>,
    argc: usize,
    pks: &[Kind],
) -> Result<Vec<ClValue>, JitError> {
    // Collected LAST-arg-first as (words…) per arg, then flattened in declaration order.
    let mut rev: Vec<Vec<ClValue>> = Vec::with_capacity(argc);
    for i in 0..argc {
        let pk = pks
            .get(argc - 1 - i)
            .copied()
            .ok_or_else(|| JitError::Codegen("unboxed: call pks underflow".to_string()))?;
        let (v, k) = ub_pop(b, vars, fvars, kinds)?;
        if pk == Kind::Dyn {
            let (payload, tag) = match k {
                Kind::Dyn => (v, b.use_var(evars[kinds.len()])),
                Kind::Int => (v, b.ins().iconst(types::I64, 0)),
                Kind::Float => {
                    let bits = b.ins().bitcast(types::I64, MemFlagsData::new(), v);
                    (bits, b.ins().iconst(types::I64, 1))
                }
                Kind::Bool => (v, b.ins().iconst(types::I64, 2)),
                // The Dyn takes the word (Owned moves in; a const word's frees no-op; a
                // Borrowed one clones first — W9).
                Kind::Str(Own::Owned) | Kind::Str(Own::ConstBorrow) => {
                    (v, b.ins().iconst(types::I64, 3))
                }
                Kind::Str(Own::Borrowed) => {
                    let cv = emit_arg_clone(b, ec, ub, v, 2)?;
                    (cv, b.ins().iconst(types::I64, 3))
                }
                other => {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: {other:?} argument into a Dyn param (deferred)"
                    )));
                }
            };
            rev.push(vec![payload, tag]);
            continue;
        }
        let v = match k {
            // Owned words MOVE; const words pass as-is (their frees no-op).
            Kind::Str(Own::Owned)
            | Kind::Str(Own::ConstBorrow)
            | Kind::StrList(Own::Owned)
            | Kind::StrList(Own::ConstBorrow)
            | Kind::IntList(Own::Owned)
            | Kind::IntList(Own::ConstBorrow)
            | Kind::StrIntMap(Own::Owned)
            | Kind::DynList(Own::Owned)
            | Kind::DynList(Own::ConstBorrow) => v,
            // W9: the borrowed word clones into a fresh Owned handle the callee may consume.
            Kind::Str(Own::Borrowed) => emit_arg_clone(b, ec, ub, v, 2)?,
            Kind::StrList(Own::Borrowed) => emit_arg_clone(b, ec, ub, v, 3)?,
            Kind::IntList(Own::Borrowed) => emit_arg_clone(b, ec, ub, v, 4)?,
            Kind::DynList(Own::Borrowed) => emit_arg_clone(b, ec, ub, v, 5)?,
            // A Dyn arg into a NON-Dyn param would cross as one word and silently drop its
            // tag — impossible post-fixpoint (Dyn absorbs at the join), reject defensively.
            Kind::Dyn => {
                return Err(JitError::Unsupported(
                    "unboxed: Dyn argument into a non-Dyn param (deferred)".to_string(),
                ));
            }
            k if k.is_handle() || k == Kind::EnumInt || matches!(k, Kind::Fn(_)) => {
                return Err(JitError::Unsupported(
                    "unboxed: handle/enum/fn argument to Call (deferred)".to_string(),
                ));
            }
            _ => v,
        };
        rev.push(vec![v]);
    }
    rev.reverse();
    Ok(rev.into_iter().flatten().collect())
}

/// Everything a call site / `Throw` needs to route a THROWN payload (code 6): the tables for
/// kind-directed unwind releases, the helpers, and the active catch pad (`None` = the throw
/// leaves this frame as `(payload, 6)`). `pad` = (pad block, pad stack height).
pub(super) struct ThrowSite<'a> {
    pub(super) program: &'a BytecodeProgram,
    pub(super) info: &'a UbGraphInfo,
    pub(super) h: &'a UbHelperRefs,
    pub(super) pad: Option<(Block, usize)>,
}

/// Release every OWNED cell in `kinds[from..]` (kind-directed — instances free their string
/// fields too): the VM's unwind/frame-teardown drops these values; the arena must recycle
/// them or leak. The cells' words are read from their depth-indexed Variables.
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn emit_unwind_releases(
    b: &mut FunctionBuilder,
    ec: &Ec,
    h: &UbHelperRefs,
    vars: &[Variable],
    kinds: &[Kind],
    from: usize,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
) {
    for (d, k) in kinds.iter().enumerate().skip(from) {
        if k.is_owned_handle() {
            let w = b.use_var(vars[d]);
            release_kinded(b, ec, h, w, *k, program, info, None);
        }
    }
}

/// The shared direct-call emission (`Op::Call` and `Op::CallValue` on a static `Fn`): reproduce
/// the VM's pre-push depth guard, native-call the callee's FuncId with `depth + 1` + the args,
/// propagate the callee's `(value, code)` — code != 0 ⇒ the whole graph redoes on the VM.
#[allow(clippy::too_many_arguments)] // emit plumbing, same shape as build_body_unboxed
pub(super) fn emit_call_to(
    b: &mut FunctionBuilder,
    ec: &Ec,
    fn_refs: &[Option<FuncRef>],
    ctx: ClValue,
    depth: ClValue,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    callee: usize,
    cargs: Vec<ClValue>,
    ret: Kind,
    throwing: Option<ThrowSite<'_>>,
) -> Result<(), JitError> {
    let callee_ref = fn_refs
        .get(callee)
        .copied()
        .flatten()
        .ok_or_else(|| JitError::Codegen(format!("unboxed: call to uncompiled fn {callee}")))?;
    let dmax = b.ins().iconst(types::I64, MAX_CALL_DEPTH as i64);
    let too_deep = b.ins().icmp(IntCC::SignedGreaterThanOrEqual, depth, dmax);
    ec.fault_if(b, too_deep, 5); // ovf-spec: stack-overflow → redo on VM (code 5)
    let d1 = b.ins().iadd_imm_s(depth, 1);
    let mut call_args: Vec<ClValue> = Vec::with_capacity(cargs.len() + 2);
    call_args.push(ctx);
    call_args.push(d1);
    call_args.extend(cargs);
    let call = b.ins().call(callee_ref, &call_args);
    let results = b.inst_results(call);
    let (value, ccode) = (results[0], results[1]);
    let cont = b.create_block();
    // A `Call`/`CallValue` is in the `needs_fault_exit` set, so `fault_exit` is `Some` here.
    let fx = ec
        .fault_exit
        .expect("Call requires a fault-exit block (needs_fault_exit)");
    match throwing {
        // Non-throwing graph: code 0 or 5 only — the 2-way fast dispatch.
        None => {
            let is_fault = b.ins().icmp_imm_s(IntCC::NotEqual, ccode, 0);
            b.ins().brif(is_fault, fx, &[ccode.into()], cont, &[]);
        }
        // Throwing graph: 0 → continue; 6 → route the thrown payload (unwind to the active
        // pad, or forward `(payload, 6)` out of this frame); else → fault-exit (redo on VM).
        Some(ts) => {
            let not_ok = b.create_block();
            let is_fault = b.ins().icmp_imm_s(IntCC::NotEqual, ccode, 0);
            b.ins().brif(is_fault, not_ok, &[], cont, &[]);
            b.switch_to_block(not_ok);
            let thrown_blk = b.create_block();
            let is_thrown = b.ins().icmp_imm_s(IntCC::Equal, ccode, 6);
            b.ins()
                .brif(is_thrown, thrown_blk, &[], fx, &[ccode.into()]);
            b.switch_to_block(thrown_blk);
            // Pending speculation: the VM-truth faulted at the earlier arith BEFORE this call
            // ever ran — redo wins over the throw (checked only on this cold path).
            if let Some(sv) = ec.sticky {
                let s = b.use_var(sv);
                ec.fault_if(b, s, 5);
            }
            match ts.pad {
                Some((pad_blk, pad_h)) => {
                    // The VM's unwind truncates to the handler height and pushes the payload.
                    emit_unwind_releases(b, ec, ts.h, vars, kinds, pad_h, ts.program, ts.info);
                    b.def_var(vars[pad_h], value);
                    b.ins().jump(pad_blk, &[]);
                }
                None => {
                    // Frame teardown: everything owned dies; the payload leaves as (value, 6).
                    emit_unwind_releases(b, ec, ts.h, vars, kinds, 0, ts.program, ts.info);
                    b.ins().return_(&[value, ccode]);
                }
            }
        }
    }
    b.switch_to_block(cont);
    // The callee's fixpoint return kind (Int for pure-int; an instance-returning ctor hands its OWNED
    // handle across). A Float return travels as i64 bits over the uniform ABI → bitcast back to F64.
    let value = if ret == Kind::Float {
        b.ins().bitcast(types::F64, MemFlagsData::new(), value)
    } else {
        value
    };
    ub_push(b, vars, fvars, kinds, value, ret)
}

/// `Op::CallValue` on a static `Fn` (the closurecall vertical's call site): peek the target
/// UNDER the args so the callee's ABI param kinds drive the arg pop (a Dyn param takes a
/// (payload, tag) pair even when this site passes a concrete scalar), then the shared direct
/// call. Body moved verbatim from the dispatch loop (M-Decomp, Invariant 13).
#[allow(clippy::too_many_arguments)] // emit plumbing
pub(super) fn arm_call_value(
    b: &mut FunctionBuilder,
    ec: &Ec,
    ub_refs: Option<&UbHelperRefs>,
    fn_refs: &[Option<FuncRef>],
    ctx: ClValue,
    depth: ClValue,
    vars: &[Variable],
    fvars: &[Variable],
    evars: &[Variable],
    kinds: &mut Vec<Kind>,
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    argc: usize,
    ts: Option<ThrowSite>,
) -> Result<(), JitError> {
    let fk_peek = *kinds
        .get(kinds.len().wrapping_sub(argc + 1))
        .ok_or_else(|| {
            JitError::Codegen("unboxed: CallValue underflow past analyze".to_string())
        })?;
    let Kind::Fn(f_peek) = fk_peek else {
        return Err(JitError::Unsupported(format!(
            "unboxed: CallValue on {fk_peek:?} (deferred)"
        )));
    };
    let pks = abi_param_kinds(program, info, f_peek);
    let cargs = pop_call_args(b, ec, ub_refs, vars, fvars, evars, kinds, argc, &pks)?;
    let (_fv, fk) = ub_pop(b, vars, fvars, kinds)?;
    let Kind::Fn(f) = fk else {
        return Err(JitError::Unsupported(format!(
            "unboxed: CallValue on {fk:?} (deferred)"
        )));
    };
    if program.functions[f].arity != argc {
        return Err(JitError::Codegen(
            "unboxed: CallValue arity mismatch past analyze".to_string(),
        ));
    }
    emit_call_to(
        b,
        ec,
        fn_refs,
        ctx,
        depth,
        vars,
        fvars,
        kinds,
        f,
        cargs,
        info.ret_of(f),
        ts,
    )
}
