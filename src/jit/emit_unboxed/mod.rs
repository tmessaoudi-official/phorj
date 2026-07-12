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

/// Pop `argc` int-representable args off the operand stack (top is the LAST arg), rejecting
/// kinds that can't cross the one-i64-per-arg ABI (handles, register-pair enums, static `Fn`s).
/// Returns the args in declaration order.
fn pop_int_args(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    argc: usize,
) -> Result<Vec<ClValue>, JitError> {
    let mut cargs: Vec<ClValue> = Vec::with_capacity(argc);
    for _ in 0..argc {
        let (v, k) = ub_pop(b, vars, fvars, kinds)?;
        match k {
            // A string argument MOVES into the callee (mirrors the analyze arm) — the word
            // crosses as a plain i64; the callee's single-use-moved param owns it.
            Kind::Str(Own::Owned) | Kind::Str(Own::ConstBorrow) => {}
            // Any other handle would arrive as an untyped i64 param; a two-word enum can't
            // cross the ABI; a `Fn`'s static target would be lost.
            k if k.is_handle() || k == Kind::EnumInt || matches!(k, Kind::Fn(_)) => {
                return Err(JitError::Unsupported(
                    "unboxed: handle/enum/fn argument to Call (deferred)".to_string(),
                ));
            }
            _ => {}
        }
        cargs.push(v);
    }
    cargs.reverse();
    Ok(cargs)
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
    let param_kinds: Vec<Kind> = info.param_kinds(func_idx, proven, func.arity);
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
    // it unconditionally.
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
    let args: Vec<ClValue> = entry_params[2..].to_vec();
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
        b.def_var(evar, i_zero);
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
                let h = ub_ref(ub_refs.as_ref(), "MakeList")?;
                arm_make_list(&mut b, &ec, h, &vars, &fvars, &mut kinds, *n)?;
            }
            Op::MakeMap(n) => {
                let h = ub_ref(ub_refs.as_ref(), "MakeMap")?;
                arm_make_map(&mut b, &ec, h, &vars, &fvars, &mut kinds, *n)?;
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
            Op::IterElems => {
                arm_iter_elems(&mut b, &vars, &fvars, &mut kinds)?;
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
                // A register-pair enum copies BOTH words: the tag into the copy's evars cell
                // (dest depth = current top, i.e. kinds.len() BEFORE the push).
                if kind == Kind::EnumInt {
                    let tv = b.use_var(evars[*slot]);
                    b.def_var(evars[kinds.len()], tv);
                }
                // Single-use param MOVE / plain BORROW — mirrors the analyze arm exactly.
                if *slot < single_use.len() && single_use[*slot] && kind == Kind::Str(Own::Owned) {
                    kinds[*slot] = Kind::Unknown;
                    ub_push(&mut b, &vars, &fvars, &mut kinds, v, Kind::Str(Own::Owned))?;
                } else {
                    ub_push(&mut b, &vars, &fvars, &mut kinds, v, borrowed_copy(kind))?;
                }
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
                        | Kind::Inst(_, Own::Borrowed)
                ) {
                    return Err(JitError::Unsupported(
                        "unboxed: SetLocal of a borrowed handle (aliasing — deferred)".to_string(),
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
                // Self OR cross-function call: pop the callee's `arity` args, then the shared
                // direct-call emission (depth guard + native call + fault propagation).
                let arity = program.functions[*callee].arity;
                let cargs = pop_int_args(&mut b, &vars, &fvars, &mut kinds, arity)?;
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
            // in the compile-time kind (`Fn(f)`), the runtime word is a never-read filler.
            Op::MakeClosure(f) => {
                let filler = b.ins().iconst(types::I64, 0);
                ub_push(&mut b, &vars, &fvars, &mut kinds, filler, Kind::Fn(*f))?;
            }
            // `CallValue` on a static `Fn(f)`: pop the args, pop the (filler) callee word, then
            // the SAME direct-call emission as `Op::Call` — no indirection, no closure object.
            Op::CallValue(argc) => {
                let cargs = pop_int_args(&mut b, &vars, &fvars, &mut kinds, *argc)?;
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
                let cargs = pop_int_args(&mut b, &vars, &fvars, &mut kinds, *argc)?;
                let (rv, rk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let Kind::Inst(c, _) = rk else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: CallMethod on {rk:?} (deferred)"
                    )));
                };
                let key = (
                    program.class_descs[c].class.to_string(),
                    program.names[*nidx].clone(),
                );
                let Some(&target) = program.methods.get(&key) else {
                    return Err(JitError::Codegen(
                        "unboxed: CallMethod unresolved past analyze".to_string(),
                    ));
                };
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
                let (v, kind) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // Int/Float return to the caller's world; an INSTANCE return is the ownership-
                // transfer contract (validated by the analyze pass's transfer gate — the entry
                // function returning an instance was rejected in `compile_unboxed`). Normalize
                // the instance's compile-time ownership to Owned: the caller receives it.
                let kind = match kind {
                    Kind::Int | Kind::Float => kind,
                    Kind::Inst(c, _) => Kind::Inst(c, Own::Owned),
                    other => {
                        // A bool/unknown return would be mis-decoded — reject to VM/boxed.
                        return Err(JitError::Unsupported(format!(
                            "unboxed: non-numeric return (kind {other:?})"
                        )));
                    }
                };
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
