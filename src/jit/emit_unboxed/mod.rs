//! UNBOXED codegen: `build_body_unboxed` — depth-indexed Cranelift Variables, checked-arith
//! sticky faults, and the inline handle-op fast paths (concat / list index / map probe).
//!
//! M-Decomp (P-2c tail): this `mod.rs` keeps the per-function SETUP (analysis, entry block,
//! dual-space Variables, sticky/fault-exit wiring) + the dispatch loop + the control-flow and
//! frame-slot arms; the op-arm BODIES live in [`scalar`] (int/float arithmetic, comparisons,
//! numeric conversions) and [`verticals`] (the handle-op inline fast paths). Shared emit state
//! crosses file boundaries via the Copy [`Ec`] context (which replaced the old captured closures).

use super::*;

mod concat;
mod enums;
mod objects;
mod scalar;
mod verticals;

use concat::*;
use enums::*;
use objects::*;
use scalar::*;
use verticals::*;

/// Copy-able per-function emit context shared by every op arm: the `UbCtx` pointer, the
/// stable-header memflags, and the (optional) shared fault-exit / speculation-sticky handles.
/// Replaces the closures the pre-decomposition monolith captured, so arms can live in sibling
/// files. All fields are Copy; methods take the builder explicitly.
#[derive(Clone, Copy)]
struct Ec {
    /// The per-run [`UbCtx`] pointer (entry param 0; null for a pure-numeric graph).
    ctx: ClValue,
    /// `notrap + can_move` flags for loads of RUN-INVARIANT `UbCtx` header fields — the arena
    /// base (offset 0), the free-stack base (8), the capacity (32); nothing ever stores to them
    /// during a run, so GVN/LICM collapses the per-op re-loads and hoists them out of hot loops
    /// (the mutable header fields — `free_top` at 16, `bump` at 24 — keep the default flags).
    stable: MemFlagsData,
    /// The shared fault-exit block (`Some` iff `needs_fault_exit`).
    fault_exit: Option<Block>,
    /// The speculation sticky Variable (`Some` iff `needs_sticky`).
    sticky: Option<Variable>,
}

impl Ec {
    /// Emit "if `flag` (i8, nonzero) then fault with `code` else continue in a fresh block".
    /// Only ever called on a path that needs the fault-exit (div/rem/call/depth or a sticky
    /// redo), so `fault_exit` is guaranteed `Some` here (`needs_fault_exit`).
    fn fault_if(&self, b: &mut FunctionBuilder, flag: ClValue, code: i64) {
        let fx = self
            .fault_exit
            .expect("fault_if requires a fault-exit block (needs_fault_exit)");
        let cv = b.ins().iconst(types::I64, code);
        let cont = b.create_block();
        b.ins().brif(flag, fx, &[cv.into()], cont, &[]);
        b.switch_to_block(cont);
    }

    /// ovf-spec: OR a boolean overflow `flag` (i8, 0/1 from `*_overflow` / an `is_min` compare)
    /// into the sticky Variable — no branch, so the hot no-overflow path costs only the OR.
    /// Zero-extends to i64. Only called for an UNPROVEN speculated op, so `sticky` is `Some`
    /// here (`needs_sticky`).
    fn accumulate_sticky(&self, b: &mut FunctionBuilder, flag: ClValue) {
        let sv = self
            .sticky
            .expect("accumulate_sticky requires the sticky var (needs_sticky)");
        let cur = b.use_var(sv);
        let ext = b.ins().uextend(types::I64, flag);
        let next = b.ins().bor(cur, ext);
        b.def_var(sv, next);
    }

    /// P-2a-inline: push an owned arena slot's index onto the inline free stack (the caller has
    /// already established `v` is slot-tagged with OWNED set). 5 memory ops, no call.
    fn slot_push(&self, b: &mut FunctionBuilder, v: ClValue) {
        let fsp = b.ins().load(types::I64, self.stable, self.ctx, 8);
        let ft = b.ins().load(types::I64, MemFlagsData::new(), self.ctx, 16);
        let slot = b.ins().band_imm(v, UB_IDX_MASK);
        let foff = b.ins().ishl_imm(ft, 2);
        let faddr = b.ins().iadd(fsp, foff);
        b.ins().istore32(MemFlagsData::new(), slot, faddr, 0);
        let ft1 = b.ins().iadd_imm(ft, 1);
        b.ins().store(MemFlagsData::new(), ft1, self.ctx, 16);
    }

    /// P-2a-inline: recycle a slot-tagged operand IFF its runtime OWNED bit is set (a flat-list
    /// element or pinned const is compile-time Owned but runtime-borrowed — the free is a no-op).
    /// Used only where the operand is already known slot-tagged (the inline fast paths).
    fn slot_free_if_owned(&self, b: &mut FunctionBuilder, v: ClValue) {
        let owned_bit = b.ins().band_imm(v, UB_TAG_OWNED);
        let push_blk = b.create_block();
        let cont = b.create_block();
        b.ins().brif(owned_bit, push_blk, &[], cont, &[]);
        b.switch_to_block(push_blk);
        self.slot_push(b, v);
        b.ins().jump(cont, &[]);
        b.switch_to_block(cont);
    }

    /// Allocate a fresh arena SLOT inline (the P-2a-inline concat ladder, shared by
    /// `MakeInstance`): pop the inline free stack if non-empty, else bump — a full arena is
    /// code 5 (redo on VM; exhaustion is a fallback, never a user-visible fault). Returns the
    /// slot INDEX (untagged).
    fn slot_alloc(&self, b: &mut FunctionBuilder) -> ClValue {
        let alloc_done = b.create_block();
        b.append_block_param(alloc_done, types::I64);
        let pop_blk = b.create_block();
        let bump_blk = b.create_block();
        let ft = b.ins().load(types::I64, MemFlagsData::new(), self.ctx, 16);
        b.ins().brif(ft, pop_blk, &[], bump_blk, &[]);
        b.switch_to_block(pop_blk);
        let ft1 = b.ins().iadd_imm(ft, -1);
        b.ins().store(MemFlagsData::new(), ft1, self.ctx, 16);
        let fsp = b.ins().load(types::I64, self.stable, self.ctx, 8);
        let foff = b.ins().ishl_imm(ft1, 2);
        let faddr = b.ins().iadd(fsp, foff);
        let popped = b.ins().uload32(MemFlagsData::new(), faddr, 0);
        b.ins().jump(alloc_done, &[popped.into()]);
        b.switch_to_block(bump_blk);
        let bp = b.ins().load(types::I64, MemFlagsData::new(), self.ctx, 24);
        let cap = b.ins().load(types::I64, self.stable, self.ctx, 32);
        let full = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, bp, cap);
        self.fault_if(b, full, 5);
        let bp1 = b.ins().iadd_imm(bp, 1);
        b.ins().store(MemFlagsData::new(), bp1, self.ctx, 24);
        b.ins().jump(alloc_done, &[bp.into()]);
        b.switch_to_block(alloc_done);
        b.block_params(alloc_done)[0]
    }
}

/// Resolve the handle-op helper refs, or fail with the canonical collect-drift diagnostic
/// (a handle op reached codegen although `collect_functions_unboxed` admitted no helpers).
fn ub_ref<'a>(ub: Option<&'a UbHelperRefs>, what: &str) -> Result<&'a UbHelperRefs, JitError> {
    ub.ok_or_else(|| {
        JitError::Codegen(format!(
            "unboxed: {what} reached codegen without handle helpers (collect drift)"
        ))
    })
}

/// The W9 call-boundary CLONE of a compile-time-BORROWED str/list argument (PHP value
/// semantics: `this.field` forwarded into the next builder — passing the raw word would
/// leave the owner and the callee both freeing it). Returns the fresh Owned word.
fn emit_arg_clone(
    b: &mut FunctionBuilder,
    ec: &Ec,
    ub: Option<&UbHelperRefs>,
    v: ClValue,
    repr: i64,
) -> Result<ClValue, JitError> {
    let h = ub_ref(ub, "call-arg clone")?;
    let reprv = b.ins().iconst(types::I64, repr);
    let call = b.ins().call(h.clone_value, &[ec.ctx, v, reprv]);
    let cv = b.inst_results(call)[0];
    let bad = b.ins().icmp_imm(IntCC::SignedLessThan, cv, 0);
    ec.fault_if(b, bad, 5);
    Ok(cv)
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
fn pop_call_args(
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
struct ThrowSite<'a> {
    program: &'a BytecodeProgram,
    info: &'a UbGraphInfo,
    h: &'a UbHelperRefs,
    pad: Option<(Block, usize)>,
}

/// Release every OWNED cell in `kinds[from..]` (kind-directed — instances free their string
/// fields too): the VM's unwind/frame-teardown drops these values; the arena must recycle
/// them or leak. The cells' words are read from their depth-indexed Variables.
#[allow(clippy::too_many_arguments)] // emit plumbing
fn emit_unwind_releases(
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
fn emit_call_to(
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
    let d1 = b.ins().iadd_imm(depth, 1);
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
            let is_fault = b.ins().icmp_imm(IntCC::NotEqual, ccode, 0);
            b.ins().brif(is_fault, fx, &[ccode.into()], cont, &[]);
        }
        // Throwing graph: 0 → continue; 6 → route the thrown payload (unwind to the active
        // pad, or forward `(payload, 6)` out of this frame); else → fault-exit (redo on VM).
        Some(ts) => {
            let not_ok = b.create_block();
            let is_fault = b.ins().icmp_imm(IntCC::NotEqual, ccode, 0);
            b.ins().brif(is_fault, not_ok, &[], cont, &[]);
            b.switch_to_block(not_ok);
            let thrown_blk = b.create_block();
            let is_thrown = b.ins().icmp_imm(IntCC::Equal, ccode, 6);
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
    // The callee's fixpoint-recorded return kind (Int for pure-int graphs; an instance-returning
    // ctor hands its OWNED arena handle across — the ownership-transfer contract). A Float return
    // travels as its i64 bits over the uniform ABI → bitcast back into the F64 space here.
    let value = if ret == Kind::Float {
        b.ins().bitcast(types::F64, MemFlagsData::new(), value)
    } else {
        value
    };
    ub_push(b, vars, fvars, kinds, value, ret)
}

/// Emit UNBOXED native code for one int function (self- or cross-recursive) into `cl_ctx.func`
/// (signature already `extern "C" fn(depth, a0..a_arity: i64) -> (i64 value, i64 code)` — a
/// multi-return, so no fault-cell pointer / no memory store on any path). Success returns `(value, 0)`;
/// a fault returns `(0, code)` (1 overflow / 2 div-zero / 3 mod-zero / 4 stack-overflow). Fault
/// CONDITIONS mirror the `value.rs` int kernels EXACTLY (div/rem check zero BEFORE `i64::MIN / -1`,
/// matching `int_div`/`int_rem`); the STRINGS are mapped from the code in [`Compiled::run_unboxed`] via
/// the single-sourced `value::FAULT_*` consts.
///
/// The frame value-stack (locals at the base — slots `0..arity` are the params — plus temporaries on
/// top) is realized as depth-indexed Cranelift `Variable`s (`vars[depth]`): a declaration leaves its
/// initializer on the stack with no `SetLocal`, so locals and temporaries are ONE stack, exactly as the
/// VM models it. `unboxed_analyze` fixes the compile-time depth+kinds at each leader (and validates
/// edge consistency); Cranelift + `seal_all_blocks` inserts the phis for merges and loop back-edges.
/// Returns `Unsupported` for a non-`Int` `Return` operand or an inconsistent-stack leader.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_body_unboxed(
    module: &mut JITModule,
    cl_ctx: &mut cranelift::codegen::Context,
    program: &BytecodeProgram,
    func_idx: usize,
    func_ids: &[Option<FuncId>],
    proven: &[Option<Kind>],
    ret_kind_out: &mut Option<Kind>,
    ub: Option<&UbHelperIds>,
    const_handles: &std::collections::HashMap<(usize, usize), i64>,
    info: &UbGraphInfo,
) -> Result<(), JitError> {
    let func = &program.functions[func_idx];
    let code = &func.chunk.code;
    let n = code.len();
    let reach = reachable(code);

    // Param slots read as `Int` iff proven int by usage (so a bare-param `Return`, e.g. fib's base case,
    // types correctly); otherwise `Unknown` → a bare return of one is rejected. A method body's slot 0
    // is the injected receiver (`this` — a BORROWED instance handle from the fixpoint facts). These seed
    // the entry stack for the analysis, which then fixes every leader's (depth, kinds) and the max depth.
    let param_kinds: Vec<Kind> = info.param_kinds(func_idx, proven, func.arity, &func.dyn_params);
    let single_use = single_use_params(func);
    // Innermost active catch pad per ip (lexical try ranges) — drives Throw + call-site
    // code-6 dispatch.
    let handler_at = handler_ranges(code);
    let mut scratch_disc = UbDiscovery::default();
    let ub_analysis = unboxed_analyze(program, func_idx, &param_kinds, info, &mut scratch_disc)?;
    let (leader_state, max_depth, depth_at) = (
        ub_analysis.leader_state,
        ub_analysis.max_depth,
        ub_analysis.depth_at,
    );

    // Range analysis (docs/plans/perf-wave.plan.md): `proven_ops[ip]` = an `AddI` that is a provably-
    // no-overflow induction-variable increment → emit a plain wrapping-free `iadd`, no sticky. From it:
    //   `needs_sticky` — is any reachable speculated overflow op (`AddI`/`SubI`/`MulI`/`Neg`) NOT proven?
    //     If NO, the speculation sticky flag + its back-edge/Return checks are dead → omit them entirely
    //     (Cranelift's baseline `opt_level=none` does NOT DCE the loop-carried sticky phi, so omitting is
    //     what actually turns a proven counted loop's PARITY into a WIN).
    //   `needs_fault_exit` — is there ANY path to the shared fault-exit (a sticky redo, OR a `DivI`/
    //     `RemI`/`Call` per-op fault branch)? If NO, don't create the block at all (an unreferenced,
    //     never-jumped-to block would be a dangling exit — avoid it).
    let mut proven_ops = range_proven_ops(func);
    // Task 9: the accumulator interval pass may prove MORE ops (bounded accumulator adds,
    // counter-affine SubI/MulI, expression-dividend RemI-by-pow2) and may require ENTRY
    // GUARDS (`param > G` ⇒ code-5 decline to the VM — the specialization precondition).
    let entry_guards: Vec<(usize, i64)> = match accumulator_elision(func, &proven_ops) {
        Some(acc) => {
            for (ip, p) in acc.proven.iter().enumerate() {
                if *p {
                    proven_ops[ip] = true;
                }
            }
            acc.guards
        }
        None => Vec::new(),
    };
    let proven_ops = proven_ops; // freeze
    let speculated = |op: &Op| matches!(op, Op::AddI | Op::SubI | Op::MulI | Op::Neg);
    // `#[UncheckedOverflow]` (Core.Runtime.Integer.UncheckedOverflow): the whole function's int arith WRAPS — every `AddI`/`SubI`/`MulI`/
    // `Neg` becomes a plain wrapping op (no overflow check, no sticky), exactly like a range-proven
    // counter but function-wide. So nothing speculates → `needs_sticky` is false, and the guard/back-edge/
    // Return-select machinery is omitted entirely (the same WIN the range-analysis lever produces, but
    // guaranteed here rather than proven). Div/Rem stay checked (div-zero must always fault).
    let needs_sticky = !func.unchecked
        && code
            .iter()
            .enumerate()
            .any(|(ip, op)| reach[ip] && speculated(op) && !proven_ops[ip]);
    // `DivF` also branches to the fault-exit (a zero divisor → code 5), as do `DivI`/`RemI` (hardware
    // trap) and `Call` (depth guard + fault propagation) — every op that emits a `fault_if`/direct
    // `brif` to the shared exit must be counted here, or the block won't exist when it is needed.
    // P-2a: `Index` (bounds), `Concat` and `String.length` (defensive bad-handle) also branch to the
    // shared fault-exit (code 5, redo on VM). `Fault` (the match-exhaustiveness backstop) jumps to
    // it unconditionally. `GetField` is counted because a BORROWED field word returned from the
    // function CLONES at `Return` (fault on a defensive mismatch) — the `return this.field;`
    // shape has no other counted op (every other borrowed-returnable value traces to one).
    let needs_fault_exit = needs_sticky
        || !entry_guards.is_empty()
        || code.iter().enumerate().any(|(ip, op)| {
            reach[ip]
                && matches!(
                    op,
                    Op::DivI
                        | Op::RemI
                        | Op::DivF
                        | Op::Call(_)
                        | Op::CallValue(_)
                        | Op::CallMethod(..)
                        | Op::Index
                        | Op::Len
                        | Op::Concat(_)
                        | Op::CallNative(..)
                        | Op::MakeList(_)
                        | Op::MakeMap(_)
                        | Op::MakeInstance(_)
                        | Op::GetField(_)
                        | Op::Fault(_)
                )
        });

    let mut fbctx = FunctionBuilderContext::new();
    let mut b = FunctionBuilder::new(&mut cl_ctx.func, &mut fbctx);
    // A `Call` (self OR cross-function) lowers to a native call to the callee's FuncId (resolved at
    // finalize). Pre-declare every compiled function's ref into this body (unused refs are harmless —
    // a relocation is emitted only for a ref actually `call`ed).
    let mut fn_refs: Vec<Option<FuncRef>> = vec![None; func_ids.len()];
    for (i, id) in func_ids.iter().enumerate() {
        if let Some(fid) = id {
            fn_refs[i] = Some(module.declare_func_in_func(*fid, b.func));
        }
    }
    // P-2a: the handle-op helper refs (declared into this body only when the graph uses handles).
    let ub_refs = ub.map(|ids| UbHelperRefs {
        list_new: module.declare_func_in_func(ids.list_new, b.func),
        list_push: module.declare_func_in_func(ids.list_push, b.func),
        list_seal: module.declare_func_in_func(ids.list_seal, b.func),
        index: module.declare_func_in_func(ids.index, b.func),
        concat: module.declare_func_in_func(ids.concat, b.func),
        str_len: module.declare_func_in_func(ids.str_len, b.func),
        free: module.declare_func_in_func(ids.free, b.func),
        map_push_pair: module.declare_func_in_func(ids.map_push_pair, b.func),
        map_seal: module.declare_func_in_func(ids.map_seal, b.func),
        map_get: module.declare_func_in_func(ids.map_get, b.func),
        list_push_int: module.declare_func_in_func(ids.list_push_int, b.func),
        index_int: module.declare_func_in_func(ids.index_int, b.func),
        int_to_str: module.declare_func_in_func(ids.int_to_str, b.func),
        concat_mix: module.declare_func_in_func(ids.concat_mix, b.func),
        acc_append: module.declare_func_in_func(ids.acc_append, b.func),
        list_len: module.declare_func_in_func(ids.list_len, b.func),
        list_acc_append: module.declare_func_in_func(ids.list_acc_append, b.func),
        map_builder_set: module.declare_func_in_func(ids.map_builder_set, b.func),
        map_builder_seed: module.declare_func_in_func(ids.map_builder_seed, b.func),
        list_acc_reseed: module.declare_func_in_func(ids.list_acc_reseed, b.func),
        list_builder_new: module.declare_func_in_func(ids.list_builder_new, b.func),
        list_append_clone: module.declare_func_in_func(ids.list_append_clone, b.func),
        native2: module.declare_func_in_func(ids.native2, b.func),
        str_eq: module.declare_func_in_func(ids.str_eq, b.func),
        clone_value: module.declare_func_in_func(ids.clone_value, b.func),
        list_append_dyn: module.declare_func_in_func(ids.list_append_dyn, b.func),
    });
    // Entry block: `[ctx, depth, a0, a1, …]`. `ctx` is the per-run [`UbCtx`] pointer (null for a
    // pure-numeric graph — only handle ops dereference it, and they exist only when it is real).
    // `depth` is the live frame count at the call site (the caller passes `depth + 1`; the top-level
    // entry gets 1) — a `Call` checks `depth >= MAX_CALL_DEPTH` BEFORE recursing to reproduce the
    // VM's `"stack overflow"` at the exact threshold.
    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let entry_params: Vec<ClValue> = b.block_params(entry).to_vec();
    let ctx = entry_params[0];
    let depth = entry_params[1];
    // W7: map ABI arg slots -> frame slots per the FINAL param kinds (a Dyn param consumes
    // TWO incoming words: payload into the I64 space, tag into the enum-tag space).
    let abi_args: Vec<ClValue> = entry_params[2..].to_vec();
    let mut args: Vec<ClValue> = Vec::with_capacity(param_kinds.len());
    let mut arg_tags: Vec<Option<ClValue>> = Vec::with_capacity(param_kinds.len());
    {
        let mut ai = 0usize;
        for pk in &param_kinds {
            let payload = abi_args
                .get(ai)
                .copied()
                .ok_or_else(|| JitError::Codegen("unboxed: ABI arg underflow".to_string()))?;
            ai += 1;
            if *pk == Kind::Dyn {
                let tag = abi_args.get(ai).copied().ok_or_else(|| {
                    JitError::Codegen("unboxed: ABI Dyn tag underflow".to_string())
                })?;
                ai += 1;
                arg_tags.push(Some(tag));
            } else {
                arg_tags.push(None);
            }
            args.push(payload);
        }
    }
    // Every stack cell is a Cranelift `Variable` (`vars[d]` = stack depth d), all DECLARED AND DEFINED
    // in the entry block — which dominates the whole body — so every `use_var`, including a loop-header
    // read reached via a back-edge, is dominated by a definition; Cranelift's SSA construction +
    // `seal_all_blocks` then inserts the phis. The bottom `arity` cells are seeded from the incoming
    // args (the frame's slots 0..arity); the rest get a filler `0` that is always overwritten before it
    // is read (structured control flow + definite-assignment; same argument as the boxed `Value::Unit`
    // filler). Within a block, def/use of these Variables optimizes to plain SSA — no memory traffic.
    // Dual-space stack cells: `vars[d]` = the I64 space (ints/bools/bits), `fvars[d]` = the F64 space
    // (floats stay in XMM, so a loop-carried float phi never round-trips through a GPR). Both are
    // declared+seeded in the entry block (which dominates the body). A float PARAM arrives as its i64
    // bits (uniform i64 ABI) → bitcast to F64 ONCE here, not per-op. The space NOT matching a slot's
    // initial kind gets a type-correct filler that definite-assignment guarantees is overwritten before
    // read, but must exist to dominate any (dead-then-DCE'd) use.
    // TRI-space stack cells: `vars[d]` = the I64 space, `fvars[d]` = the F64 space, and `evars[d]`
    // = the ENUM-TAG space (the second word of a register-pair [`Kind::EnumInt`] — the payload
    // rides in `vars[d]`). A cell in an unused space carries its type-correct filler; unused
    // Variables cost nothing after SSA + DCE.
    let mut vars: Vec<Variable> = Vec::with_capacity(max_depth);
    let mut fvars: Vec<Variable> = Vec::with_capacity(max_depth);
    let mut evars: Vec<Variable> = Vec::with_capacity(max_depth);
    let i_zero = b.ins().iconst(types::I64, 0);
    let f_zero = b.ins().f64const(0.0);
    for s in 0..max_depth {
        let ivar = b.declare_var(types::I64);
        let fvar = b.declare_var(types::F64);
        let evar = b.declare_var(types::I64);
        if s < args.len() && matches!(param_kinds.get(s), Some(Kind::Float)) {
            let fbits = b.ins().bitcast(types::F64, MemFlagsData::new(), args[s]);
            b.def_var(fvar, fbits);
            b.def_var(ivar, i_zero);
        } else if s < args.len() {
            b.def_var(ivar, args[s]);
            b.def_var(fvar, f_zero);
        } else {
            b.def_var(ivar, i_zero);
            b.def_var(fvar, f_zero);
        }
        // W7: a Dyn param's SECOND incoming word (the runtime tag) seeds its enum-tag cell;
        // every other slot gets the filler.
        match arg_tags.get(s).copied().flatten() {
            Some(tag) => b.def_var(evar, tag),
            None => b.def_var(evar, i_zero),
        }
        vars.push(ivar);
        fvars.push(fvar);
        evars.push(evar);
    }

    // ovf-spec: the speculation sticky flag. A Cranelift `Variable` (NOT an SSA value) so a loop
    // back-edge phis it at the loop header — the same reason the stack cells are Variables. Declared
    // AND seeded to 0 in the entry block (which dominates the whole body). Each speculatively-wrapped
    // op ORs its overflow carry in (no per-op branch); at every loop back-edge AND every `Return`,
    // `sticky != 0` ⇒ exit code 5 = "redo on VM", where the VM's per-op CHECKED arithmetic reproduces
    // the true first fault in the correct order (the single source of fault truth — Invariant 2).
    // Only declared when at least one unproven speculated op needs it (else the whole sticky chain is
    // dead — and Cranelift baseline won't DCE the loop-carried phi, so omitting is the actual win).
    let sticky = if needs_sticky {
        let v = b.declare_var(types::I64);
        let sticky_seed = b.ins().iconst(types::I64, 0);
        b.def_var(v, sticky_seed);
        Some(v)
    } else {
        None
    };

    // One Cranelift block per reachable leader — the SAME leader set `unboxed_analyze` used, so the two
    // views of the block structure can never drift.
    let is_leader = leaders(code, &reach);
    let mut blocks: Vec<Option<Block>> = vec![None; n];
    for ip in 0..n {
        if reach[ip] && is_leader[ip] {
            blocks[ip] = Some(b.create_block());
        }
    }
    let start = blocks[0].expect("ip 0 is always a leader");

    // Shared fault-exit: takes the fault code as a block param, returns (0, code). Created only when a
    // fault path exists (a sticky redo, or a div/rem/call per-op branch); otherwise there is nothing to
    // jump to it and creating it would leave a dangling block.
    let fault_exit = if needs_fault_exit {
        let fx = b.create_block();
        b.append_block_param(fx, types::I64);
        Some(fx)
    } else {
        None
    };

    // Task 9 entry guards: `param > G` fails the elision precondition — decline the whole
    // call with code 5 (redo on the VM: correct, just unspecialized for out-of-range bounds).
    for &(slot, gmax) in &entry_guards {
        let fx = fault_exit.expect("entry guards imply needs_fault_exit");
        let v = b.use_var(vars[slot]);
        let over = b.ins().icmp_imm(IntCC::SignedGreaterThan, v, gmax);
        let cv = b.ins().iconst(types::I64, 5);
        let cont = b.create_block();
        b.ins().brif(over, fx, &[cv.into()], cont, &[]);
        b.switch_to_block(cont);
    }
    b.ins().jump(start, &[]);
    b.switch_to_block(start);
    let mut current: Option<Block> = Some(start);
    // Compile-time KIND stack; its length is the current stack depth. Reset from `leader_state` at every
    // block leader (the values are carried by the depth-indexed Variables). The entry block starts with
    // the params on the stack.
    let mut kinds: Vec<Kind> = param_kinds.clone();

    // The shared emit context handed to every op arm (see [`Ec`]): the `UbCtx` pointer, the
    // stable-header memflags (`notrap + can_move` — run-invariant loads GVN/LICM can hoist),
    // and the optional fault-exit / sticky handles.
    let ec = Ec {
        ctx,
        stable: MemFlagsData::new().with_notrap().with_can_move(),
        fault_exit,
        sticky,
    };

    // Per-call-site throw routing (Some only in a throwing graph): the active pad's block +
    // stack height, resolved per ip from the lexical handler ranges.
    let throw_site = |ip: usize,
                      blocks: &[Option<Block>],
                      leader_state: &LeaderStates|
     -> Result<Option<(Block, usize)>, JitError> {
        match handler_at[ip] {
            None => Ok(None),
            Some(pad_ip) => {
                let blk = blocks[pad_ip].ok_or_else(|| {
                    JitError::Codegen("unboxed: catch pad has no block".to_string())
                })?;
                let h = leader_state[pad_ip]
                    .as_ref()
                    .ok_or_else(|| JitError::Codegen("unboxed: catch pad unanalyzed".to_string()))?
                    .len()
                    - 1;
                Ok(Some((blk, h)))
            }
        }
    };
    let mut skip_ip: Option<usize> = None;
    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        // A fused two-op arm (the accumulator peephole) already emitted this op's effect.
        if skip_ip == Some(ip) {
            skip_ip = None;
            continue;
        }
        if ip != 0 {
            if let Some(blk) = blocks[ip] {
                if current.is_some() {
                    b.ins().jump(blk, &[]); // fall-through edge into this leader
                }
                b.switch_to_block(blk);
                current = Some(blk);
                // The values are carried by the depth-indexed Variables (Cranelift phis them); reset the
                // compile-time KIND stack to this leader's recorded shape (validated by `unboxed_analyze`).
                kinds = leader_state[ip].clone().ok_or_else(|| {
                    JitError::Codegen(format!(
                        "unboxed: block leader ip {ip} has no analyzed state"
                    ))
                })?;
            }
        }
        if current.is_none() {
            continue;
        }

        match op {
            Op::Const(idx) => match func.chunk.consts.get(*idx) {
                Some(Value::Int(k)) => {
                    let v = b.ins().iconst(types::I64, *k);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, v, Kind::Int)?;
                }
                Some(Value::Float(f)) => {
                    // Dual-space: push the native f64 into the F64 space (no bits-in-i64 round-trip).
                    let fv = b.ins().f64const(*f);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, fv, Kind::Float)?;
                }
                Some(Value::Bool(bv)) => {
                    let v = b.ins().iconst(types::I64, *bv as i64);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, v, Kind::Bool)?;
                }
                Some(Value::Str(_)) => {
                    // P-2a: a string const is a PINNED handle (interned per graph, seeded into the
                    // per-run `UbCtx` prefix) — the push is a plain `iconst` of its index, zero calls.
                    let h = *const_handles.get(&(func_idx, *idx)).ok_or_else(|| {
                        JitError::Codegen(format!(
                            "unboxed: Str const {idx} in fn {func_idx} has no pinned handle"
                        ))
                    })?;
                    let v = b.ins().iconst(types::I64, h);
                    ub_push(
                        &mut b,
                        &vars,
                        &fvars,
                        &mut kinds,
                        v,
                        Kind::Str(Own::ConstBorrow),
                    )?;
                }
                other => return Err(JitError::Unsupported(format!("unboxed Const {other:?}"))),
            },
            // ---- P-2a/P-2b/P-2c handle verticals (inline fast paths over the UbCtx arena;
            // helpers = slow paths) — arm bodies live in `verticals.rs` ---------------------------
            Op::MakeList(n) => {
                // BUILDER-RESEED peephole (`xs = [v]` over a live builder slot): a literal
                // reset in a builder loop must NOT bump-seal a fresh flat list each cycle —
                // bump slots never recycle, so a long run walks off the arena (the 1M-iter
                // cliff: exhaustion → boxed → permanent code-5 VM redo). Reuse a record via
                // the reseed helper and skip the SetLocal. Kind-gated to an
                // already-`IntList(Owned)` slot, so the INITIAL binding seals normally.
                if *n == 1
                    && !is_leader.get(ip + 1).copied().unwrap_or(true)
                    && matches!(kinds.last(), Some(Kind::Int))
                {
                    if let Some(Op::SetLocal(s)) = code.get(ip + 1) {
                        if matches!(kinds.get(*s), Some(Kind::IntList(Own::Owned))) {
                            let h = ub_ref(ub_refs.as_ref(), "MakeList(reseed)")?;
                            let (vv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                            let old = b.use_var(vars[*s]);
                            let call = b.ins().call(h.list_acc_reseed, &[ec.ctx, old, vv]);
                            let sres = b.inst_results(call)[0];
                            let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                            ec.fault_if(&mut b, bad, 5);
                            b.def_var(vars[*s], sres);
                            kinds[*s] = Kind::IntList(Own::Owned);
                            skip_ip = Some(ip + 1);
                            continue;
                        }
                    }
                }
                let h = ub_ref(ub_refs.as_ref(), "MakeList")?;
                arm_make_list(&mut b, &ec, h, &vars, &fvars, &mut kinds, *n)?;
            }
            Op::MakeMap(n) => {
                // BUILDER-RESEED peephole (`m = [k => v]` over a live builder slot) — the
                // map twin of the MakeList reseed above (same arena-exhaustion cliff).
                if *n == 1 && !is_leader.get(ip + 1).copied().unwrap_or(true) {
                    let d = kinds.len();
                    let pair_ok = d >= 2
                        && matches!(kinds[d - 1], Kind::Int)
                        && matches!(kinds[d - 2], Kind::Str(_));
                    if pair_ok {
                        if let Some(Op::SetLocal(s)) = code.get(ip + 1) {
                            if matches!(kinds.get(*s), Some(Kind::StrIntMap(Own::Owned))) {
                                let h = ub_ref(ub_refs.as_ref(), "MakeMap(reseed)")?;
                                let (vv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                                let (iv, ik) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                                let old = b.use_var(vars[*s]);
                                let call = b.ins().call(h.map_builder_seed, &[ec.ctx, old, iv, vv]);
                                let sres = b.inst_results(call)[0];
                                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                                ec.fault_if(&mut b, bad, 5);
                                b.def_var(vars[*s], sres);
                                kinds[*s] = Kind::StrIntMap(Own::Owned);
                                if ik.is_owned_handle() {
                                    emit_release(&mut b, &ec, h, iv);
                                }
                                skip_ip = Some(ip + 1);
                                continue;
                            }
                        }
                    }
                }
                let h = ub_ref(ub_refs.as_ref(), "MakeMap")?;
                arm_make_map(&mut b, &ec, h, &vars, &fvars, &mut kinds, *n)?;
            }
            Op::Index if matches!(kinds.last(), Some(Kind::IterPtr)) => {
                // Lever 3: `elems[j]` with a pointer cursor — ONE load; the loop guard is the
                // bounds proof (the desugar never indexes past the cursor).
                let (pv, _pk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (_ev, ek) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if ek != Kind::IterEnd {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: iter-cursor Index receiver {ek:?} (desugar drift)"
                    )));
                }
                let x = b.ins().load(types::I64, MemFlagsData::new(), pv, 0);
                ub_push(&mut b, &vars, &fvars, &mut kinds, x, Kind::Int)?;
            }
            Op::Index if matches!(kinds.last(), Some(Kind::Str(_))) => {
                // P-2b: string-keyed map lookup (`m[k]` → Int) — the inline bucket probe.
                let h = ub_ref(ub_refs.as_ref(), "Index(map)")?;
                arm_index_map(&mut b, &ec, h, &vars, &fvars, &mut kinds)?;
            }
            // P-2c: int-list element read (`xs[i]` → Int, raw i64 at slot bytes 0..8).
            Op::Index
                if matches!(
                    kinds.get(kinds.len().wrapping_sub(2)),
                    Some(Kind::IntList(_))
                ) =>
            {
                let h = ub_ref(ub_refs.as_ref(), "Index(int)")?;
                arm_index_int_list(&mut b, &ec, h, &vars, &fvars, &mut kinds, proven_ops[ip])?;
            }
            Op::Index => {
                let h = ub_ref(ub_refs.as_ref(), "Index")?;
                arm_index_str_list(&mut b, &ec, h, &vars, &fvars, &mut kinds, proven_ops[ip])?;
            }
            Op::IterElems
                if matches!(kinds.last(), Some(Kind::IntList(Own::Borrowed)))
                    && !is_leader.get(ip + 1).copied().unwrap_or(true)
                    && matches!(code.get(ip + 1), Some(Op::Const(c))
                        if matches!(func.chunk.consts.get(*c), Some(Value::Int(0)))) =>
            {
                // Lever-3 POINTER-WALK init (mirrors the analyze arm): the desugar's
                // `IterElems; Const(0)` becomes (end, cursor) pointer cells; Len/Lt/Index/j+1
                // then strength-reduce per-op. The mutation guard proved the iterated slot is
                // never written, so the list is a stable flat snapshot (boxed → code-5 redo).
                let h = ub_ref(ub_refs.as_ref(), "IterElems(ptr-walk)")?;
                arm_iter_ptr_init(&mut b, &ec, h, &vars, &fvars, &mut kinds)?;
                skip_ip = Some(ip + 1);
            }
            Op::IterElems => {
                arm_iter_elems(&mut b, &vars, &fvars, &mut kinds)?;
            }
            Op::Len if matches!(kinds.last(), Some(Kind::IterEnd)) => {
                // Lever 3: the bound IS the end pointer — identity re-push, zero instructions.
                let (v, k) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                ub_push(&mut b, &vars, &fvars, &mut kinds, v, k)?;
            }
            Op::Len => {
                let h = ub_ref(ub_refs.as_ref(), "Len")?;
                arm_list_len(&mut b, &ec, h, &vars, &fvars, &mut kinds)?;
            }
            Op::Concat(cn) if *cn >= 2 => {
                let h = ub_ref(ub_refs.as_ref(), "Concat")?;
                // FUSED-ACCUMULATOR peephole (s = s + x): the lhs is the untouched borrow of
                // the very slot the following SetLocal rewrites - treat it as CONSUMED (the old
                // value dies here), emit ONE pair merge whose helper path appends IN PLACE on a
                // uniquely-owned heap lhs (amortized O(1) - the classic accumulator), store the
                // result straight into the slot, and skip the SetLocal.
                if *cn == 2 {
                    if let Some(s) = accumulator_site(code, &depth_at, &is_leader, ip) {
                        let dl = kinds.len();
                        if dl >= 2
                            && matches!(kinds[dl - 1], Kind::Str(_))
                            && matches!(kinds[dl - 2], Kind::Str(_))
                        {
                            let (bv, bk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                            let (av, _ak) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                            // The strbuild vertical: an ACC-record in-place append (inline
                            // cap-checked copy; helper converts/grows) — see `concat_acc`.
                            let res = concat_acc(&mut b, &ec, h, av, bv, bk)?;
                            b.def_var(vars[s], res);
                            kinds[s] = Kind::Str(Own::Owned);
                            skip_ip = Some(ip + 1);
                            continue;
                        }
                    }
                }
                arm_concat(&mut b, &ec, h, &vars, &fvars, &mut kinds, *cn)?;
            }
            // ---- P-2c numeric conversions: fully inline, no helper, no handle space ------------
            Op::CallNative(id, 2) if unboxed_native_is_bridge2(*id) => {
                // The generic pure-native bridge — mirrors the analyze arm; the helper calls
                // the REGISTERED native (single-sourced kernel), so semantics cannot drift.
                let h = ub_ref(ub_refs.as_ref(), "native bridge2")?;
                let (bv, bk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, ak) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let Some((meta_base, out_kind)) = unboxed_native_bridge2(*id, &ak, &bk) else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed bridge2 native operand kinds ({ak:?}, {bk:?})"
                    )));
                };
                let meta = meta_base
                    | ((ak.is_owned_handle() as i64) << 9)
                    | ((bk.is_owned_handle() as i64) << 10);
                let idv = b.ins().iconst(types::I64, *id as i64);
                let metav = b.ins().iconst(types::I64, meta);
                let call = b.ins().call(h.native2, &[ec.ctx, idv, av, bv, metav]);
                let sval = b.inst_results(call)[0];
                let scode = b.inst_results(call)[1];
                let bad = b.ins().icmp_imm(IntCC::NotEqual, scode, 0);
                ec.fault_if(&mut b, bad, 5);
                ub_push(&mut b, &vars, &fvars, &mut kinds, sval, out_kind)?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_to_float(*id) => {
                arm_to_float(&mut b, &vars, &fvars, &mut kinds)?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_truncate(*id) => {
                arm_truncate(&mut b, &ec, &vars, &fvars, &mut kinds)?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_str_len(*id) => {
                let h = ub_ref(ub_refs.as_ref(), "String.length")?;
                arm_str_len(&mut b, &ec, h, &vars, &fvars, &mut kinds)?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_to_string(*id) => {
                // `Conversion.toString(int)` — the SAME zero-alloc renderer interpolation uses.
                let h = ub_ref(ub_refs.as_ref(), "Conversion.toString")?;
                let (nv, nk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if nk != Kind::Int {
                    return Err(JitError::Unsupported(format!(
                        "unboxed toString operand kind {nk:?} (deferred)"
                    )));
                }
                let call = b.ins().call(h.int_to_str, &[ec.ctx, nv]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                ec.fault_if(&mut b, bad, 5);
                ub_push(
                    &mut b,
                    &vars,
                    &fvars,
                    &mut kinds,
                    sres,
                    Kind::Str(Own::Owned),
                )?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_list_len(*id) => {
                // `List.length` — same lowering as `Op::Len` (flat count bits / ACL len
                // word inline; helper for a boxed list).
                let h = ub_ref(ub_refs.as_ref(), "List.length")?;
                arm_list_len(&mut b, &ec, h, &vars, &fvars, &mut kinds)?;
            }
            Op::CallNative(id, 2)
                if unboxed_native_is_list_map(*id) || unboxed_native_is_list_count(*id) =>
            {
                // The hofpipe vertical: a STATIC-lambda `List.map`/`List.count` lowers to a
                // native loop (inline element loads over flat/ACL, a direct call per element,
                // an ACL builder output for map / a register sum for count).
                let h = ub_ref(ub_refs.as_ref(), "List HOF")?;
                arm_list_hof(
                    &mut b,
                    &ec,
                    h,
                    &fn_refs,
                    ctx,
                    depth,
                    &vars,
                    &fvars,
                    &mut kinds,
                    info,
                    unboxed_native_is_list_map(*id),
                )?;
            }
            Op::CallNative(id, 2)
                if unboxed_native_is_list_append(*id)
                    && (matches!(kinds.last(), Some(Kind::Dyn))
                        || matches!(
                            kinds.get(kinds.len().wrapping_sub(2)),
                            Some(Kind::DynList(_))
                        )) =>
            {
                // W7: a DYN element (tag-dispatched) or a DynList receiver — the boxed
                // tag-aware clone-append helper; result = a fresh Owned DynList. Guard order
                // mirrors analyze exactly (this arm BEFORE the accumulator arm — a Dyn
                // element at an accumulator site takes the tag-aware path there too).
                let h = ub_ref(ub_refs.as_ref(), "List.append(dyn)")?;
                let (ev, ek) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // The popped element's enum-tag cell (a forwarded Dyn's runtime tag lives
                // there; kinds.len() is the popped slot's index AFTER the pop).
                let elem_tag_cell = evars[kinds.len()];
                let (lv, lk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (payload, tag) = match ek {
                    Kind::Dyn => (ev, b.use_var(elem_tag_cell)),
                    Kind::Int => (ev, b.ins().iconst(types::I64, 0)),
                    Kind::Float => {
                        let bits = b.ins().bitcast(types::I64, MemFlagsData::new(), ev);
                        (bits, b.ins().iconst(types::I64, 1))
                    }
                    Kind::Bool => (ev, b.ins().iconst(types::I64, 2)),
                    // The Dyn takes the word (Owned moves in; a const word's frees no-op).
                    Kind::Str(Own::Owned) | Kind::Str(Own::ConstBorrow) => {
                        (ev, b.ins().iconst(types::I64, 3))
                    }
                    other => {
                        return Err(JitError::Unsupported(format!(
                            "unboxed dyn List.append element kind {other:?}"
                        )));
                    }
                };
                let lhs_repr: i64 = match lk {
                    Kind::StrList(_) => 3,
                    Kind::IntList(_) => 4,
                    Kind::DynList(_) => 5,
                    other => {
                        return Err(JitError::Unsupported(format!(
                            "unboxed dyn List.append lhs kind {other:?}"
                        )));
                    }
                };
                let meta = lhs_repr | ((lk.is_owned_handle() as i64) << 3);
                let metav = b.ins().iconst(types::I64, meta);
                let call = b
                    .ins()
                    .call(h.list_append_dyn, &[ec.ctx, lv, payload, tag, metav]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                ec.fault_if(&mut b, bad, 5);
                ub_push(
                    &mut b,
                    &vars,
                    &fvars,
                    &mut kinds,
                    sres,
                    Kind::DynList(Own::Owned),
                )?;
            }
            Op::CallNative(id, 2)
                if unboxed_native_is_list_append(*id)
                    && accumulator_site(code, &depth_at, &is_leader, ip).is_some()
                    && matches!(kinds.last(), Some(Kind::Int))
                    && matches!(
                        kinds.get(kinds.len().wrapping_sub(2)),
                        Some(Kind::IntList(_))
                    ) =>
            {
                // The listappend vertical (mirrors the Concat accumulator peephole): ONLY
                // at a proven accumulator site — the lhs is the dying borrow of the very
                // slot the following SetLocal rewrites; consume it into an ACL builder
                // record (in-place push), store the result straight into the slot, and
                // skip the SetLocal. Non-int accumulator shapes fall THROUGH to the general
                // clone arm (guard order mirrors analyze exactly).
                let s = accumulator_site(code, &depth_at, &is_leader, ip).ok_or_else(|| {
                    JitError::Unsupported(
                        "unboxed List.append outside an accumulator site".to_string(),
                    )
                })?;
                let h = ub_ref(ub_refs.as_ref(), "List.append")?;
                let (vv, vk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, ak) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if vk != Kind::Int || !matches!(ak, Kind::IntList(_)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed List.append operand kinds ({ak:?}, {vk:?})"
                    )));
                }
                let res = list_append_acc(&mut b, &ec, h, av, vv)?;
                b.def_var(vars[s], res);
                kinds[s] = Kind::IntList(Own::Owned);
                skip_ip = Some(ip + 1);
            }
            Op::CallNative(id, 2) if unboxed_native_is_list_append(*id) => {
                // The GENERAL (non-accumulator) `List.append` — full clone semantics via the
                // helper: a fresh BOXED list, inputs untouched unless compile-time-OWNED
                // (mirrors the analyze arm exactly).
                let h = ub_ref(ub_refs.as_ref(), "List.append(clone)")?;
                let (ev, ek) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (lv, lk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let out_kind = match (ek, lk) {
                    (Kind::Int, Kind::IntList(_)) => Kind::IntList(Own::Owned),
                    (Kind::Str(_), Kind::StrList(_)) => Kind::StrList(Own::Owned),
                    other => {
                        return Err(JitError::Unsupported(format!(
                            "unboxed List.append operand kinds {other:?}"
                        )))
                    }
                };
                let meta = (matches!(ek, Kind::Str(_)) as i64)
                    | ((ek.is_owned_handle() as i64) << 1)
                    | ((lk.is_owned_handle() as i64) << 2);
                let metav = b.ins().iconst(types::I64, meta);
                let call = b.ins().call(h.list_append_clone, &[ec.ctx, lv, ev, metav]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                ec.fault_if(&mut b, bad, 5);
                ub_push(&mut b, &vars, &fvars, &mut kinds, sres, out_kind)?;
            }
            Op::Pop => {
                arm_pop(
                    &mut b,
                    &ec,
                    ub_refs.as_ref(),
                    &vars,
                    &fvars,
                    &mut kinds,
                    program,
                    info,
                )?;
            }
            Op::AddI if matches!(kinds.get(kinds.len().wrapping_sub(2)), Some(Kind::IterPtr)) => {
                // Lever 3: the desugar's `j + 1` — `cursor + 64` (the flat slot stride); the
                // analyze mirror verified the increment literal is exactly 1.
                if !(ip >= 1
                    && matches!(code.get(ip - 1), Some(Op::Const(c))
                        if matches!(func.chunk.consts.get(*c), Some(Value::Int(1)))))
                {
                    return Err(JitError::Unsupported(
                        "unboxed: non-unit increment on an iter cursor".to_string(),
                    ));
                }
                let (_one, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (pv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let bumped = b.ins().iadd_imm(pv, 64);
                ub_push(&mut b, &vars, &fvars, &mut kinds, bumped, Kind::IterPtr)?;
            }
            Op::AddI | Op::SubI | Op::MulI => {
                // Plain (wrapping-free) when `#[UncheckedOverflow]` or a range-proven induction
                // `AddI` — computed here (dispatch owns `proven_ops`), lowered in `scalar.rs`.
                // Task 9: the accumulator interval pass proves SubI/MulI too (counter-affine
                // terms, bounded-accumulator sites) — `proven_ops[ip]` is op-agnostic here.
                let plain = func.unchecked || proven_ops[ip];
                arm_int_arith(&mut b, &ec, &vars, &fvars, &mut kinds, op, plain)?;
            }
            Op::DivI | Op::RemI => {
                arm_div_rem(
                    &mut b,
                    &ec,
                    &vars,
                    &fvars,
                    &mut kinds,
                    func,
                    ip,
                    op,
                    proven_ops[ip],
                )?;
            }
            Op::AddF | Op::SubF | Op::MulF => {
                arm_float_arith(&mut b, &vars, &fvars, &mut kinds, op)?;
            }
            Op::DivF => {
                arm_div_f(&mut b, &ec, &vars, &fvars, &mut kinds)?;
            }
            Op::Neg => {
                arm_neg(&mut b, &ec, &vars, &fvars, &mut kinds, func.unchecked)?;
            }
            Op::Not => {
                arm_not(&mut b, &vars, &fvars, &mut kinds)?;
            }
            Op::Lt
                if matches!(kinds.last(), Some(Kind::IterEnd))
                    && matches!(kinds.get(kinds.len().wrapping_sub(2)), Some(Kind::IterPtr)) =>
            {
                // Lever 3: the desugar's loop header — ONE unsigned pointer compare.
                let (ev, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (pv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let r = b.ins().icmp(IntCC::UnsignedLessThan, pv, ev);
                let r64 = b.ins().uextend(types::I64, r);
                ub_push(&mut b, &vars, &fvars, &mut kinds, r64, Kind::Bool)?;
            }
            Op::Eq | Op::Ne
                if matches!(kinds.last(), Some(Kind::Str(_)))
                    && matches!(kinds.get(kinds.len().wrapping_sub(2)), Some(Kind::Str(_))) =>
            {
                // String equality — the shared `Value::eq_val` kernel via the helper (the
                // `col == \"*\"` / `next == \"_\"` prelude shapes).
                let h = ub_ref(ub_refs.as_ref(), "Eq(str)")?;
                let (bv, bk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, ak) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let meta = (ak.is_owned_handle() as i64) | ((bk.is_owned_handle() as i64) << 1);
                let metav = b.ins().iconst(types::I64, meta);
                let call = b.ins().call(h.str_eq, &[ec.ctx, av, bv, metav]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                ec.fault_if(&mut b, bad, 5);
                let res = if matches!(op, Op::Ne) {
                    b.ins().bxor_imm(sres, 1)
                } else {
                    sres
                };
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Bool)?;
            }
            Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                arm_cmp(&mut b, &vars, &fvars, &mut kinds, op)?;
            }
            Op::GetLocal(slot) => {
                // DUP: read the frame-stack cell at `slot` and push a copy on top, carrying that cell's
                // CURRENT kind (a proven-numeric param, or whatever was last stored there). Dual-space:
                // read from the space matching that kind (a Float local from `fvars`).
                let kind = *kinds.get(*slot).ok_or_else(|| {
                    JitError::Codegen(format!("unboxed GetLocal slot {slot} above stack top"))
                })?;
                let space = if kind == Kind::Float { &fvars } else { &vars };
                let var = *space.get(*slot).ok_or_else(|| {
                    JitError::Codegen(format!(
                        "unboxed GetLocal slot {slot} out of range (max_depth {})",
                        space.len()
                    ))
                })?;
                let v = b.use_var(var);
                // A register-pair enum — and a W7 Dyn cell — copies BOTH words: the tag into
                // the copy's evars cell (dest depth = current top, i.e. kinds.len() BEFORE
                // the push).
                if matches!(kind, Kind::EnumInt | Kind::Dyn) {
                    let tv = b.use_var(evars[*slot]);
                    b.def_var(evars[kinds.len()], tv);
                }
                // Single-use param MOVE / plain BORROW — mirrors the analyze arm exactly.
                // W7: a Dyn cell is MOVE-ONLY (a borrowed copy would alias the owned str
                // payload — double free); multi-use was declined by analyze.
                let movable = matches!(
                    kind,
                    Kind::Str(Own::Owned)
                        | Kind::StrList(Own::Owned)
                        | Kind::IntList(Own::Owned)
                        | Kind::StrIntMap(Own::Owned)
                        | Kind::DynList(Own::Owned)
                        | Kind::Dyn
                );
                if *slot < single_use.len() && single_use[*slot] && movable {
                    kinds[*slot] = Kind::Unknown;
                    ub_push(&mut b, &vars, &fvars, &mut kinds, v, kind)?;
                } else if kind == Kind::Dyn {
                    return Err(JitError::Unsupported(
                        "unboxed: multi-use union (Dyn) param (deferred)".to_string(),
                    ));
                } else {
                    ub_push(&mut b, &vars, &fvars, &mut kinds, v, borrowed_copy(kind))?;
                }
            }
            Op::SetIndexLocal(slot) => {
                // The mapinsert vertical: `m[k] = v` on a uniquely-owned map local — inline
                // AMB-builder overwrite (packed-table probe + one store); conversion / insert
                // / growth via the ONE helper. The result handle def_vars back into the slot.
                let h = ub_ref(ub_refs.as_ref(), "SetIndexLocal(map)")?;
                arm_set_index_map_local(&mut b, &ec, h, &vars, &fvars, &mut kinds, *slot)?;
            }
            Op::SetLocal(slot) => {
                // Pop the top and store it into the frame-stack cell at `slot`, updating that cell's
                // tracked kind. A back-edge assignment feeds Cranelift's loop-header phi via `def_var`.
                // Dual-space: store into the space matching the popped value's kind (a Float feeds the
                // F64 phi → the loop-carried float stays in XMM across the back-edge).
                let (v, k) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let space = if k == Kind::Float { &fvars } else { &vars };
                let var = *space.get(*slot).ok_or_else(|| {
                    JitError::Codegen(format!(
                        "unboxed SetLocal slot {slot} out of range (max_depth {})",
                        space.len()
                    ))
                })?;
                if *slot >= kinds.len() {
                    return Err(JitError::Codegen(format!(
                        "unboxed SetLocal slot {slot} above stack top {}",
                        kinds.len()
                    )));
                }
                // Storing a BORROWED handle stays denied (aliasing — mirrors the analyze arm).
                if matches!(
                    k,
                    Kind::Str(Own::Borrowed)
                        | Kind::StrList(Own::Borrowed)
                        | Kind::StrIntMap(Own::Borrowed)
                        | Kind::IntList(Own::Borrowed)
                        | Kind::DynList(Own::Borrowed)
                        | Kind::Inst(_, Own::Borrowed)
                ) {
                    return Err(JitError::Unsupported(
                        "unboxed: SetLocal of a borrowed handle (aliasing — deferred)".to_string(),
                    ));
                }
                // W7: mirrors the analyze deny (pair-copy + old-Dyn release not wired yet).
                if k == Kind::Dyn {
                    return Err(JitError::Unsupported(
                        "unboxed: SetLocal of a union (Dyn) value (deferred)".to_string(),
                    ));
                }
                // Overwriting a live OWNED handle releases the old value first (the accumulator
                // pattern `s = s + x` — the old string dies here; runtime-bit-gated, so a
                // joined-to-Owned const edge is a no-op release).
                if kinds[*slot].is_owned_handle() {
                    let h = ub_ref(ub_refs.as_ref(), "SetLocal(handle overwrite)")?;
                    let old = b.use_var(vars[*slot]);
                    release_kinded(&mut b, &ec, h, old, kinds[*slot], program, info, None);
                }
                // A register-pair enum stores BOTH words: the tag from the popped cell's evars
                // slot (source depth = kinds.len() AFTER the pop) into the frame cell's.
                if k == Kind::EnumInt {
                    let tv = b.use_var(evars[kinds.len()]);
                    b.def_var(evars[*slot], tv);
                }
                b.def_var(var, v);
                kinds[*slot] = k;
            }
            Op::Call(callee) => {
                // Self OR cross-function call: pop the callee's `arity` args per its FINAL
                // ABI param kinds (a Dyn param takes TWO words), then the shared direct-call
                // emission (depth guard + native call + fault propagation).
                let arity = program.functions[*callee].arity;
                let pks = abi_param_kinds(program, info, *callee);
                let cargs = pop_call_args(
                    &mut b,
                    &ec,
                    ub_refs.as_ref(),
                    &vars,
                    &fvars,
                    &evars,
                    &mut kinds,
                    arity,
                    &pks,
                )?;
                emit_call_to(
                    &mut b,
                    &ec,
                    &fn_refs,
                    ctx,
                    depth,
                    &vars,
                    &fvars,
                    &mut kinds,
                    *callee,
                    cargs,
                    info.ret_of(*callee),
                    if info.thrown_class.is_some() {
                        Some(ThrowSite {
                            program,
                            info,
                            h: ub_ref(ub_refs.as_ref(), "throw dispatch")?,
                            pad: throw_site(ip, &blocks, &leader_state)?,
                        })
                    } else {
                        None
                    },
                )?;
            }
            // Closure vertical: a capture-free `MakeClosure` is fully STATIC — the target rides
            // in the compile-time kind (`Fn(f)`), the runtime word is a never-read filler. A
            // ONE-int-capture closure (hofpipe) re-tags the capture word IN PLACE (`FnCap1`):
            // pop + re-push at the same depth is zero moves.
            Op::MakeClosure(f) => match program.functions[*f].n_captures {
                0 => {
                    let filler = b.ins().iconst(types::I64, 0);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, filler, Kind::Fn(*f))?;
                }
                1 => {
                    let (cv, ck) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                    if ck != Kind::Int {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: non-int closure capture {ck:?} (deferred)"
                        )));
                    }
                    ub_push(&mut b, &vars, &fvars, &mut kinds, cv, Kind::FnCap1(*f))?;
                }
                _ => {
                    return Err(JitError::Unsupported(
                        "unboxed: closure with 2+ captures (deferred)".to_string(),
                    ))
                }
            },
            // `CallValue` on a static `Fn(f)`: pop the args, pop the (filler) callee word, then
            // the SAME direct-call emission as `Op::Call` — no indirection, no closure object.
            Op::CallValue(argc) => {
                // The static `Fn` target is UNDER the args — peek it so the callee's ABI
                // param kinds drive the arg pop (a Dyn param takes a (payload, tag) pair
                // even when THIS site passes a concrete scalar).
                let fk_peek = *kinds
                    .get(kinds.len().wrapping_sub(*argc + 1))
                    .ok_or_else(|| {
                        JitError::Codegen("unboxed: CallValue underflow past analyze".to_string())
                    })?;
                let Kind::Fn(f_peek) = fk_peek else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: CallValue on {fk_peek:?} (deferred)"
                    )));
                };
                let pks = abi_param_kinds(program, info, f_peek);
                let cargs = pop_call_args(
                    &mut b,
                    &ec,
                    ub_refs.as_ref(),
                    &vars,
                    &fvars,
                    &evars,
                    &mut kinds,
                    *argc,
                    &pks,
                )?;
                let (_fv, fk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let Kind::Fn(f) = fk else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: CallValue on {fk:?} (deferred)"
                    )));
                };
                if program.functions[f].arity != *argc {
                    return Err(JitError::Codegen(
                        "unboxed: CallValue arity mismatch past analyze".to_string(),
                    ));
                }
                emit_call_to(
                    &mut b,
                    &ec,
                    &fn_refs,
                    ctx,
                    depth,
                    &vars,
                    &fvars,
                    &mut kinds,
                    f,
                    cargs,
                    info.ret_of(f),
                    if info.thrown_class.is_some() {
                        Some(ThrowSite {
                            program,
                            info,
                            h: ub_ref(ub_refs.as_ref(), "throw dispatch")?,
                            pad: throw_site(ip, &blocks, &leader_state)?,
                        })
                    } else {
                        None
                    },
                )?;
            }
            // Object vertical (flat arena instances) — arm bodies live in `objects.rs`.
            Op::MakeInstance(cidx) => {
                let desc = &program.class_descs[*cidx];
                // Field j (ctor push order) → its layout slot (static permutation — mirrors the
                // VM's `layout.slot(name)` mapping exactly).
                let perm: Vec<usize> = desc
                    .fields
                    .iter()
                    .map(|n| {
                        desc.layout.slot(n).ok_or_else(|| {
                            JitError::Codegen(format!("unboxed: ctor field `{n}` not in layout"))
                        })
                    })
                    .collect::<Result<_, _>>()?;
                arm_make_instance(&mut b, &ec, &vars, &fvars, &mut kinds, *cidx, &perm)?;
            }
            Op::GetField(nidx) => {
                let h = ub_ref(ub_refs.as_ref(), "GetField")?;
                arm_get_field(
                    &mut b, &ec, h, &vars, &fvars, &mut kinds, program, info, *nidx,
                )?;
            }
            Op::SetField(nidx) => {
                let h = ub_ref(ub_refs.as_ref(), "SetField")?;
                arm_set_field(
                    &mut b, &ec, h, &vars, &fvars, &mut kinds, program, info, *nidx,
                )?;
            }
            // Statically-dispatched method call: the receiver's class is in the compile-time
            // kind → the target is a methods-table lookup → the SAME direct-call emission, with
            // the receiver handle prepended as the callee's slot 0 (`this`). The VM's frame
            // teardown drops the receiver — an OWNED temp receiver is freed after the call.
            Op::CallMethod(nidx, argc) => {
                // The receiver is UNDER the args — peek its class to resolve the target so
                // the callee's ABI param kinds (slot 0 = `this`, args at 1..) can drive the
                // arg pop (a Dyn param takes a (payload, tag) pair).
                let rk_peek = *kinds
                    .get(kinds.len().wrapping_sub(*argc + 1))
                    .ok_or_else(|| {
                        JitError::Codegen("unboxed: CallMethod underflow past analyze".to_string())
                    })?;
                let Kind::Inst(c_peek, _) = rk_peek else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: CallMethod on {rk_peek:?} (deferred)"
                    )));
                };
                let key = (
                    program.class_descs[c_peek].class.to_string(),
                    program.names[*nidx].clone(),
                );
                let Some(&target) = program.methods.get(&key) else {
                    return Err(JitError::Codegen(
                        "unboxed: CallMethod unresolved past analyze".to_string(),
                    ));
                };
                let pks = abi_param_kinds(program, info, target);
                let cargs = pop_call_args(
                    &mut b,
                    &ec,
                    ub_refs.as_ref(),
                    &vars,
                    &fvars,
                    &evars,
                    &mut kinds,
                    *argc,
                    &pks[1..],
                )?;
                let (rv, rk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if !matches!(rk, Kind::Inst(c2, _) if c2 == c_peek) {
                    return Err(JitError::Codegen(
                        "unboxed: CallMethod receiver kind drifted past analyze".to_string(),
                    ));
                }
                let mut full_args: Vec<ClValue> = Vec::with_capacity(cargs.len() + 1);
                full_args.push(rv);
                full_args.extend(cargs);
                emit_call_to(
                    &mut b,
                    &ec,
                    &fn_refs,
                    ctx,
                    depth,
                    &vars,
                    &fvars,
                    &mut kinds,
                    target,
                    full_args,
                    info.ret_of(target),
                    if info.thrown_class.is_some() {
                        Some(ThrowSite {
                            program,
                            info,
                            h: ub_ref(ub_refs.as_ref(), "throw dispatch")?,
                            pad: throw_site(ip, &blocks, &leader_state)?,
                        })
                    } else {
                        None
                    },
                )?;
                if rk.is_owned_handle() {
                    let h = ub_ref(ub_refs.as_ref(), "CallMethod(receiver free)")?;
                    release_kinded(&mut b, &ec, h, rv, rk, program, info, None);
                }
            }
            // Enum vertical (zero-alloc register pairs) — arm bodies live in `enums.rs`.
            Op::MakeEnum(idx) => {
                let arity = program.enum_descs[*idx].arity;
                arm_make_enum(
                    &mut b,
                    &vars,
                    &fvars,
                    &evars,
                    &mut kinds,
                    *idx as i64,
                    arity,
                )?;
            }
            Op::MatchTag(idx) => {
                arm_match_tag(&mut b, &vars, &fvars, &evars, &mut kinds, *idx as i64)?;
            }
            Op::GetEnumField(0) => {
                arm_get_enum_field(&mut b, &vars, &fvars, &mut kinds)?;
            }
            // Native try/catch: handler bookkeeping is COMPILE-TIME (the lexical ranges) —
            // no runtime code at the markers.
            Op::PushHandler(_) | Op::PopHandler => {}
            // `Throw`: release everything the VM's unwind would drop, then jump to the active
            // pad with the payload (or leave the frame as `(payload, 6)`). Pending overflow
            // speculation wins first (VM-truth faulted before reaching the throw).
            Op::Throw => {
                let (pv, pk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if !matches!(pk, Kind::Inst(..)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: Throw of {pk:?} (deferred)"
                    )));
                }
                if let Some(sv) = sticky {
                    let s = b.use_var(sv);
                    ec.fault_if(&mut b, s, 5);
                }
                let h = ub_ref(ub_refs.as_ref(), "Throw")?;
                match handler_at[ip] {
                    Some(pad_ip) => {
                        let pad_blk = blocks[pad_ip].ok_or_else(|| {
                            JitError::Codegen("unboxed: catch pad has no block".to_string())
                        })?;
                        let pad_h = leader_state[pad_ip]
                            .as_ref()
                            .ok_or_else(|| {
                                JitError::Codegen("unboxed: catch pad unanalyzed".to_string())
                            })?
                            .len()
                            - 1;
                        emit_unwind_releases(&mut b, &ec, h, &vars, &kinds, pad_h, program, info);
                        b.def_var(vars[pad_h], pv);
                        b.ins().jump(pad_blk, &[]);
                    }
                    None => {
                        emit_unwind_releases(&mut b, &ec, h, &vars, &kinds, 0, program, info);
                        let six = b.ins().iconst(types::I64, 6);
                        b.ins().return_(&[pv, six]);
                    }
                }
                current = None;
            }
            // `instanceof` on a statically-classed instance: a compile-time constant (name
            // match or supertype membership — the same `class_implements` oracle as the VM).
            Op::IsInstance(name) => {
                let (_v, k) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let Kind::Inst(c, _) = k else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: IsInstance on {k:?} (deferred)"
                    )));
                };
                let cls = &program.class_descs[c].class;
                let is = cls.as_ref() == name.as_str()
                    || program
                        .class_implements
                        .get(&**cls)
                        .is_some_and(|ifaces| ifaces.contains(name));
                let r = b.ins().iconst(types::I64, is as i64);
                ub_push(&mut b, &vars, &fvars, &mut kinds, r, Kind::Bool)?;
            }
            // A fixed runtime fault (the match-exhaustiveness backstop): unconditionally funnel
            // to the shared fault-exit as code 5 — the VM redo reproduces the exact canonical
            // message (Invariant 2). A terminator: no fall-through.
            Op::Fault(_) => {
                let fx = fault_exit.expect("Fault requires a fault-exit block (needs_fault_exit)");
                let five = b.ins().iconst(types::I64, 5);
                b.ins().jump(fx, &[five.into()]);
                current = None;
            }
            Op::Jump(t) => {
                let tb = blocks[*t].ok_or_else(|| {
                    JitError::Codegen(format!("unboxed jump to non-leader ip {t}"))
                })?;
                // ovf-spec back-edge guard: a `Jump` to an earlier ip closes a loop. If speculation
                // overflowed, bail to the VM redo HERE (≤1 partial iteration past the overflow) rather
                // than loop on wrapped values, which can diverge from the VM's fault — e.g.
                // `while (i != 0) { i = i * 3; }`: `3^k mod 2^64` is always odd, never 0, so wrapping
                // loops forever while the VM faults overflow in ~40 iters (a byte-identity spine
                // violation, not a slowdown). Forward jumps can't extend execution past a fault → no guard.
                if *t <= ip {
                    if let Some(sv) = sticky {
                        let s = b.use_var(sv);
                        ec.fault_if(&mut b, s, 5);
                    }
                }
                b.ins().jump(tb, &[]);
                current = None;
            }
            Op::JumpIfFalse(t) => {
                let (cond, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let tb = blocks[*t].ok_or_else(|| {
                    JitError::Codegen(format!("unboxed JumpIfFalse target ip {t}"))
                })?;
                let fallb = blocks[ip + 1].ok_or_else(|| {
                    JitError::Codegen(format!("unboxed JumpIfFalse fall-through ip {}", ip + 1))
                })?;
                // ovf-spec back-edge guard (see `Op::Jump`): if this conditional can branch backward
                // (a loop back-edge), redo on the VM when speculation overflowed. Conservatively guards
                // both edges when the taken-target is backward — redo is always sound; the common
                // while-loop uses a forward `JumpIfFalse` (exit) + a backward `Jump`, so this rarely fires.
                if *t <= ip {
                    if let Some(sv) = sticky {
                        let s = b.use_var(sv);
                        ec.fault_if(&mut b, s, 5);
                    }
                }
                // cond nonzero (true) → fall through; zero (false) → take the jump.
                b.ins().brif(cond, fallb, &[], tb, &[]);
                current = None;
            }
            Op::Return => {
                let (v, popped_kind) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let kind = popped_kind;
                // Int/Float return to the caller's world; an INSTANCE return is the ownership-
                // transfer contract (validated by the analyze pass's transfer gate — the entry
                // function returning an instance was rejected in `compile_unboxed`). Normalize
                // the instance's compile-time ownership to Owned: the caller receives it.
                // A STR/LIST return transfers Owned/const words as-is; a BORROWED one (a field
                // read, a flat element) is CLONED first — PHP value semantics, and the owner
                // and the caller can never double-free one word.
                let (v, kind) = match kind {
                    Kind::Int | Kind::Float | Kind::Bool => (v, kind),
                    Kind::Inst(c, _) => (v, Kind::Inst(c, Own::Owned)),
                    Kind::Str(own)
                    | Kind::StrList(own)
                    | Kind::IntList(own)
                    | Kind::DynList(own)
                        if own == Own::Borrowed =>
                    {
                        let h = ub_ref(ub_refs.as_ref(), "Return(borrowed handle clone)")?;
                        let repr = match kind {
                            Kind::Str(_) => 2,
                            Kind::StrList(_) => 3,
                            Kind::DynList(_) => 5,
                            _ => 4,
                        };
                        let reprv = b.ins().iconst(types::I64, repr);
                        let call = b.ins().call(h.clone_value, &[ec.ctx, v, reprv]);
                        let cv = b.inst_results(call)[0];
                        let bad = b.ins().icmp_imm(IntCC::SignedLessThan, cv, 0);
                        ec.fault_if(&mut b, bad, 5);
                        let owned = match kind {
                            Kind::Str(_) => Kind::Str(Own::Owned),
                            Kind::StrList(_) => Kind::StrList(Own::Owned),
                            Kind::DynList(_) => Kind::DynList(Own::Owned),
                            _ => Kind::IntList(Own::Owned),
                        };
                        (cv, owned)
                    }
                    Kind::Str(_) => (v, Kind::Str(Own::Owned)),
                    Kind::StrList(_) => (v, Kind::StrList(Own::Owned)),
                    Kind::IntList(_) => (v, Kind::IntList(Own::Owned)),
                    Kind::DynList(_) => (v, Kind::DynList(Own::Owned)),
                    other => {
                        // An unknown/enum/map return would be mis-decoded — reject to VM/boxed.
                        return Err(JitError::Unsupported(format!(
                            "unboxed: non-numeric return (kind {other:?})"
                        )));
                    }
                };
                // W9 frame teardown: release every OWNED cell left below (the VM drops the
                // frame; a leaked temp — e.g. an owned frag local after its borrowed copy
                // went into an append — would exhaust the arena in a hot loop). The return
                // value is SECURED first (borrowed returns cloned above; an Owned handle on
                // the stack is unique, so no below cell can back it). EXCEPTION: a BORROWED
                // instance return transfers its single backing cell (the analyze census
                // guarantees it is the only owned cell) — nothing is released on that path.
                let transfers_backing = matches!(popped_kind, Kind::Inst(_, Own::Borrowed));
                if !transfers_backing && kinds.iter().any(|k| k.is_owned_handle()) {
                    let h = ub_ref(ub_refs.as_ref(), "Return(frame teardown)")?;
                    emit_unwind_releases(&mut b, &ec, h, &vars, &kinds, 0, program, info);
                }
                // Record the return kind for `run_unboxed`'s Int-vs-Float decode; ASSERT every reachable
                // Return in THIS function agrees — a mixed Int/Float would decode the i64 return-bits
                // wrong (advisor 3C). The value operand is the same i64 bits either way; only the decode
                // differs, so consistency here is load-bearing.
                match ret_kind_out {
                    None => *ret_kind_out = Some(kind),
                    Some(prev) if *prev != kind => {
                        return Err(JitError::Codegen(format!(
                            "unboxed: inconsistent return kind ({prev:?} vs {kind:?})"
                        )));
                    }
                    Some(_) => {}
                }
                // Dual-space: a float return is a native f64 → bitcast to its i64 bits for the uniform
                // i64 ABI (`run_unboxed` decodes back via `ret_kind`). Int/Bool are already i64.
                let vbits = if kind == Kind::Float {
                    b.ins().bitcast(types::I64, MemFlagsData::new(), v)
                } else {
                    v
                };
                // ovf-spec: if speculation overflowed anywhere on this path, return code 5 (redo on VM)
                // instead of the wrapped value; else (value, 0). `select` keeps the hot no-overflow path
                // branchless. The value operand is ignored by `run_unboxed` when code != 0. When there is
                // no sticky flag (every speculated op proven / none present), the code is a constant 0 —
                // no phi, no select — which is what lets a proven counted loop return with zero overhead.
                let code = if let Some(sv) = sticky {
                    let s = b.use_var(sv);
                    let five = b.ins().iconst(types::I64, 5);
                    let zero = b.ins().iconst(types::I64, 0);
                    b.ins().select(s, five, zero)
                } else {
                    b.ins().iconst(types::I64, 0)
                };
                b.ins().return_(&[vbits, code]);
                current = None;
            }
            // Everything else falls back to the VM/boxed path.
            other => return Err(JitError::Unsupported(format!("unboxed {other:?}"))),
        }
    }

    // Fault-exit (shared): return (0, code). Only emitted when a fault path actually targets it (else it
    // was never created — see `needs_fault_exit`).
    if let Some(fx) = fault_exit {
        b.switch_to_block(fx);
        let code_param = b.block_params(fx)[0];
        let zero = b.ins().iconst(types::I64, 0);
        b.ins().return_(&[zero, code_param]);
    }

    b.seal_all_blocks();
    b.finalize();
    Ok(())
}
