//! BOXED codegen — the byte-identity ORACLE path: `JitCtx` (memory operand stack), the
//! `rt_*` bridge helpers mirroring `vm::exec_op`, the boxed op-subset collector, and `build_body`.

use super::*;

/// The call context every compiled native function receives (as an opaque pointer — Cranelift never
/// dereferences it; only the `rt_*` bridge helpers do). Holds the unified runtime operand stack (the
/// memory-operand-stack design), the live call depth (for the `"stack overflow"` cap), and the
/// out-channel for a fault.
pub(super) struct JitCtx {
    /// The unified operand stack — the whole point of the 1(b) design, and it also holds every frame's
    /// **locals**: in this VM locals are stack slots (`stack[slot_base + slot]`), NOT a separate array.
    /// The entry frame's window opens at `slot_base = 0`, seeded with the call arguments (slots
    /// `0..arity`); a callee's window opens at `stack.len() - arity` over the args the caller left on
    /// top. A local declaration's initializer push self-seeds its slot as it executes, and operands
    /// stack on top. Living in memory (not SSA values) is what lets the stack survive across Cranelift
    /// block boundaries so control flow needs no block params.
    pub(super) stack: Vec<Value>,
    /// Live call-frame depth, seeded to 1 (the entry frame). `rt_depth_check` faults when it would
    /// exceed [`MAX_CALL_DEPTH`] (the check `vm::exec` makes before pushing a frame) and otherwise
    /// increments; `rt_return` decrements. Keeps the `"stack overflow"` fault at the VM's exact depth
    /// AND bounds native recursion so a runaway can't blow the OS stack. On a *fault* path the matching
    /// decrement is skipped (the compiled code branches straight to fault-exit) — intentionally
    /// harmless: `JitCtx` is per-run (the run aborts and it is dropped), and it stays per-run even once
    /// b3/JIT-3 caches the module, so a stale count can never leak into a later run. Do not "fix" it.
    pub(super) depth: usize,
    /// A helper sets this and returns the fault status on a clean runtime fault; the compiled code
    /// then branches to its fault-exit block, which returns status 1. The fault propagates up through
    /// each caller's post-call status check to the entry, unchanged.
    pub(super) fault: Option<String>,
}

// --- helper status codes (returned as i64 from the fallible `rt_*` helpers) ---
/// The helper succeeded.
pub(super) const STATUS_OK: i64 = 0;
/// The helper recorded a fault in `ctx.fault`; the compiled code branches to the fault-exit block.
pub(super) const STATUS_FAULT: i64 = 1;
// `rt_jump_if_false` is 3-way: 0 = operand was true (fall through), 1 = false (take the jump),
// 2 = fault (non-bool operand / underflow).
pub(super) const JIF_TRUE: i64 = 0;
pub(super) const JIF_FALSE: i64 = 1;
pub(super) const JIF_FAULT: i64 = 2;

/// Canonical fault for the "can't happen" operand-stack underflow (validate guarantees balance). Not a
/// VM-parity string — an eligible function never hits it — but recorded rather than panicking, because
/// a panic through `extern "C"` aborts the process.
pub(super) const FAULT_UNDERFLOW: &str = "jit: operand stack underflow";

/// The VM's clean deep-recursion fault. The string is a bare literal in `vm::exec`/`vm::closure`/the
/// interpreter (not yet single-sourced in `value.rs` like the arithmetic faults), so it is duplicated
/// here — but the tests assert the JIT fault against the VM oracle's rendering, not this literal, so
/// any VM-side drift is caught.
pub(super) const FAULT_STACK_OVERFLOW: &str = "stack overflow";

/// ovf-spec code 5 marker. The unboxed codegen speculates (wrapping arith + a sticky flag), so it can
/// never render the true fault (which one fired, in what order) — it only signals "I overflowed
/// somewhere, redo on the VM". `run_unboxed`'s ONLY production caller is the b3b `Op::Call` hook
/// (`src/vm/exec.rs`), which treats ANY [`JitRun::Fault`] as "fall through and re-execute the callee on
/// the VM" — so this string is NEVER surfaced to a user; it exists only to make the direct unit tests
/// legible. The VM's per-op checked arithmetic is the single source of fault truth (Invariant 2).
pub(crate) const REDO_ON_VM: &str = "jit: speculation overflowed — redo on VM";

/// Reborrow the context pointer the compiled code threads to every helper.
///
/// SAFETY: `ctx` is the single `&mut JitCtx` that [`compile_and_run`] passes as the first argument to
/// the entry function; the compiled code forwards that exact pointer — non-null, unchanged — to every
/// `rt_*` helper and to every native callee (which forward it in turn), and never retains a helper's
/// borrow across another helper call. So a fresh `&mut` per helper invocation is sound.
#[inline]
pub(super) fn cx<'a>(p: *mut JitCtx) -> &'a mut JitCtx {
    unsafe { &mut *p }
}

/// Record `msg` as the pending fault and return the fault status (shared by every fallible helper).
#[inline]
pub(super) fn fault(c: &mut JitCtx, msg: String) -> i64 {
    c.fault = Some(msg);
    STATUS_FAULT
}

/// Push `Value::Int(n)`. Bridge for `Op::Const` of an int literal — infallible (only grows the stack).
pub(super) extern "C" fn rt_push_int(p: *mut JitCtx, n: i64) {
    cx(p).stack.push(Value::Int(n));
}

/// Push `Value::Unit`. Bridge for `Op::Const` of the unit literal (the compiler's synthesized
/// fall-through `return`) — infallible.
pub(super) extern "C" fn rt_push_unit(p: *mut JitCtx) {
    cx(p).stack.push(Value::Unit);
}

/// Push a clone of the local at frame slot `slot` (`stack[slot_base + slot]`). Bridge for
/// `Op::GetLocal`; mirrors `exec.rs` (locals are stack slots, offset by the frame's `slot_base`).
/// Fallible only defensively.
pub(super) extern "C" fn rt_get_local(p: *mut JitCtx, slot_base: i64, slot: i64) -> i64 {
    let c = cx(p);
    let idx = (slot_base + slot) as usize;
    let v = match c.stack.get(idx) {
        Some(v) => v.clone(),
        None => return fault(c, format!("jit: local slot {slot} out of range")),
    };
    c.stack.push(v);
    STATUS_OK
}

/// Pop the top of stack into the local at frame slot `slot` (set-and-pop, decision P2-4). Bridge for
/// `Op::SetLocal`; mirrors `exec.rs` (locals are stack slots, offset by the frame's `slot_base`).
pub(super) extern "C" fn rt_set_local(p: *mut JitCtx, slot_base: i64, slot: i64) -> i64 {
    let c = cx(p);
    let Some(v) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    let idx = (slot_base + slot) as usize;
    match c.stack.get_mut(idx) {
        Some(cell) => {
            *cell = v;
            STATUS_OK
        }
        None => fault(c, format!("jit: local slot {slot} out of range")),
    }
}

/// Shared int-arithmetic bridge: pop two operands (top is the RHS, mirroring the VM's `pop2` order),
/// dispatch through a single-sourced [`crate::value`] kernel, push the result. `code`: 0 add, 1 sub,
/// 2 mul, 3 div, 4 rem. On a kernel fault the canonical string is recorded — byte-identical to the VM.
pub(super) extern "C" fn rt_arith(p: *mut JitCtx, code: i64) -> i64 {
    let c = cx(p);
    let Some(bv) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    let Some(av) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    match (&av, &bv) {
        (Value::Int(x), Value::Int(y)) => {
            let kernel: fn(i64, i64) -> Result<i64, String> = match code {
                0 => crate::value::int_add,
                1 => crate::value::int_sub,
                2 => crate::value::int_mul,
                3 => crate::value::int_div,
                _ => crate::value::int_rem,
            };
            match kernel(*x, *y) {
                Ok(r) => {
                    c.stack.push(Value::Int(r));
                    STATUS_OK
                }
                Err(msg) => fault(c, msg),
            }
        }
        // Unreachable for an eligible (int-typed) function — the checker guarantees int operands.
        // Defensive: fault rather than misbehave.
        _ => fault(c, "jit: non-int arithmetic operand".to_string()),
    }
}

/// Negate the top of stack (int via the checked `int_neg` kernel; float direct). Bridge for `Op::Neg`;
/// mirrors `exec.rs` (negating `i64::MIN` is a clean `"integer overflow"`, never a panic).
pub(super) extern "C" fn rt_neg(p: *mut JitCtx) -> i64 {
    let c = cx(p);
    let Some(v) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    match v {
        Value::Int(n) => match crate::value::int_neg(n) {
            Ok(r) => {
                c.stack.push(Value::Int(r));
                STATUS_OK
            }
            Err(msg) => fault(c, msg),
        },
        Value::Float(x) => {
            c.stack.push(Value::Float(-x));
            STATUS_OK
        }
        other => fault(c, format!("cannot negate {}", other.type_name())),
    }
}

/// Logical not of a bool. Bridge for `Op::Not`; mirrors `exec.rs` ("cannot apply ! to {type}").
pub(super) extern "C" fn rt_not(p: *mut JitCtx) -> i64 {
    let c = cx(p);
    let Some(v) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    match v {
        Value::Bool(b) => {
            c.stack.push(Value::Bool(!b));
            STATUS_OK
        }
        other => fault(c, format!("cannot apply ! to {}", other.type_name())),
    }
}

/// Equality (`negate == 0`) or inequality (`negate != 0`). Pops two operands, pushes the `Bool`.
/// Bridge for `Op::Eq`/`Op::Ne`; uses the single-sourced [`Value::eq_val`] (infallible except on the
/// defensive underflow).
pub(super) extern "C" fn rt_eqne(p: *mut JitCtx, negate: i64) -> i64 {
    let c = cx(p);
    let Some(b) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    let Some(a) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    let eq = a.eq_val(&b);
    c.stack
        .push(Value::Bool(if negate != 0 { !eq } else { eq }));
    STATUS_OK
}

/// Ordering comparison. Pops two operands (top is RHS), pushes the `Bool`. `code`: 0 lt, 1 gt, 2 le,
/// 3 ge. Bridge for `Op::Lt`/`Gt`/`Le`/`Ge`; the ordering + comparability fault are single-sourced in
/// [`crate::value::compare_ord`] and the op→bool projection mirrors `vm::compare` exactly (NaN → false).
pub(super) extern "C" fn rt_cmp(p: *mut JitCtx, code: i64) -> i64 {
    let c = cx(p);
    let Some(b) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    let Some(a) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    match crate::value::compare_ord(&a, &b) {
        Ok(opt) => {
            let r = match opt {
                Some(o) => match code {
                    0 => o == Ordering::Less,
                    1 => o == Ordering::Greater,
                    2 => o != Ordering::Greater,
                    _ => o != Ordering::Less,
                },
                None => false, // NaN compares false — identical to `vm::compare`
            };
            c.stack.push(Value::Bool(r));
            STATUS_OK
        }
        Err(msg) => fault(c, msg),
    }
}

/// Pop a bool and report the branch decision. Bridge for `Op::JumpIfFalse`; mirrors `exec.rs`
/// ("expected bool, found {type}"). Returns [`JIF_TRUE`] (fall through), [`JIF_FALSE`] (take the
/// jump), or [`JIF_FAULT`] (non-bool operand / underflow — records `ctx.fault`).
pub(super) extern "C" fn rt_jump_if_false(p: *mut JitCtx) -> i64 {
    let c = cx(p);
    let Some(v) = c.stack.pop() else {
        c.fault = Some(FAULT_UNDERFLOW.to_string());
        return JIF_FAULT;
    };
    match v {
        Value::Bool(true) => JIF_TRUE,
        Value::Bool(false) => JIF_FALSE,
        other => {
            c.fault = Some(format!("expected bool, found {}", other.type_name()));
            JIF_FAULT
        }
    }
}

/// Check-and-enter a new call frame. Bridge emitted before every `Op::Call`. Mirrors the VM's
/// `if self.frames.len() >= MAX_CALL_DEPTH { return Err("stack overflow") }` guard, made BEFORE the
/// frame is pushed: faults iff `depth >= MAX_CALL_DEPTH`, otherwise increments `depth` (the frame the
/// callee is about to run in). The matching decrement is in `rt_return`.
pub(super) extern "C" fn rt_depth_check(p: *mut JitCtx) -> i64 {
    let c = cx(p);
    if c.depth >= MAX_CALL_DEPTH {
        return fault(c, FAULT_STACK_OVERFLOW.to_string());
    }
    c.depth += 1;
    STATUS_OK
}

/// Compute the callee's `slot_base`. Bridge emitted after `rt_depth_check`, before the native call.
/// The callee's `arity` args already sit on top of the stack (the caller pushed them), so its window
/// opens at `stack.len() - arity` — exactly `vm::pop_n_start`. Infallible: `validate` guarantees at
/// least `arity` values are present; the `saturating_sub` is a belt-and-braces guard against an
/// underflow wrap (which would only arise from a compiler bug) rather than a panic across `extern "C"`.
pub(super) extern "C" fn rt_frame_base(p: *mut JitCtx, arity: i64) -> i64 {
    let c = cx(p);
    c.stack.len().saturating_sub(arity as usize) as i64
}

/// Return from the current frame. Bridge for `Op::Return`; mirrors `vm::do_return`: pop the return
/// value, decrement `depth`, truncate the stack back to this frame's `slot_base` (discarding its
/// locals + operands), then push the single return value so the caller sees it on top (net effect of a
/// call: pop `arity` args, push one result). For the entry frame this leaves `[rv]` on the stack,
/// which [`compile_and_run`] pops as the result.
pub(super) extern "C" fn rt_return(p: *mut JitCtx, slot_base: i64) -> i64 {
    let c = cx(p);
    let Some(rv) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    c.depth = c.depth.saturating_sub(1);
    c.stack.truncate(slot_base as usize);
    c.stack.push(rv);
    STATUS_OK
}

/// The imported bridge-helper `FuncId`s, declared once per module and re-referenced into every
/// function body. Grouped so `build_body` takes one argument instead of eleven.
pub(super) struct Helpers {
    pub(super) push_int: FuncId,
    pub(super) push_unit: FuncId,
    pub(super) get_local: FuncId,
    pub(super) set_local: FuncId,
    pub(super) arith: FuncId,
    pub(super) neg: FuncId,
    pub(super) not: FuncId,
    pub(super) eqne: FuncId,
    pub(super) cmp: FuncId,
    pub(super) jif: FuncId,
    pub(super) depth_check: FuncId,
    pub(super) frame_base: FuncId,
    pub(super) ret: FuncId,
}

/// The `FuncRef`s for the bridge helpers, resolved into one function body (a `FuncId` must be
/// re-declared per body before it can be `call`ed there).
pub(super) struct HelperRefs {
    pub(super) push_int: FuncRef,
    pub(super) push_unit: FuncRef,
    pub(super) get_local: FuncRef,
    pub(super) set_local: FuncRef,
    pub(super) arith: FuncRef,
    pub(super) neg: FuncRef,
    pub(super) not: FuncRef,
    pub(super) eqne: FuncRef,
    pub(super) cmp: FuncRef,
    pub(super) jif: FuncRef,
    pub(super) depth_check: FuncRef,
    pub(super) frame_base: FuncRef,
    pub(super) ret: FuncRef,
}

/// Collect the set of function indices to compile: the entry plus every function transitively
/// **reachably** called (via `Op::Call`) from it, in a deterministic discovery order. Along the way
/// enforce eligibility per function (default-deny): a closure capture, a non-int/unit `Const`, or any
/// op outside the supported subset makes the WHOLE compilation `Unsupported` (so the caller falls back
/// to the VM), because a native call needs its callee compiled in the same module. Only **reachable**
/// ops are inspected — a dead `Call` to an ineligible function must not sink an otherwise-eligible one.
pub(super) fn collect_functions(
    program: &BytecodeProgram,
    entry_idx: usize,
) -> Result<Vec<usize>, JitError> {
    let mut order = Vec::new();
    let mut seen = vec![false; program.functions.len()];
    let mut work = vec![entry_idx];
    while let Some(fi) = work.pop() {
        if seen[fi] {
            continue;
        }
        seen[fi] = true;
        let func = &program.functions[fi];
        if func.n_captures != 0 {
            return Err(JitError::Unsupported("closure with captures".to_string()));
        }
        let code = &func.chunk.code;
        let reach = reachable(code);
        for (ip, op) in code.iter().enumerate() {
            if !reach[ip] {
                continue;
            }
            match op {
                Op::Const(idx) => match func.chunk.consts.get(*idx) {
                    Some(Value::Int(_)) | Some(Value::Unit) => {}
                    other => return Err(JitError::Unsupported(format!("Const {other:?}"))),
                },
                Op::AddI
                | Op::SubI
                | Op::MulI
                | Op::DivI
                | Op::RemI
                | Op::Neg
                | Op::Not
                | Op::Eq
                | Op::Ne
                | Op::Lt
                | Op::Gt
                | Op::Le
                | Op::Ge
                | Op::GetLocal(_)
                | Op::SetLocal(_)
                | Op::Jump(_)
                | Op::JumpIfFalse(_)
                | Op::Return => {}
                Op::Call(callee) => work.push(*callee),
                other => return Err(JitError::Unsupported(format!("{other:?}"))),
            }
        }
        order.push(fi);
    }
    Ok(order)
}

/// Emit the native code for one phorj function into `cl_ctx.func` (signature already set by the caller
/// to `fn(ptr, i64 slot_base) -> i64 status`). Mirrors `vm::exec_op` over the function's **reachable**
/// op stream, using the memory operand stack in [`JitCtx`]; a `Call` lowers to depth-check →
/// frame-base → direct native call → status-propagation.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_body(
    module: &mut JITModule,
    cl_ctx: &mut cranelift::codegen::Context,
    program: &BytecodeProgram,
    func_idx: usize,
    func_ids: &[Option<FuncId>],
    helpers: &Helpers,
) -> Result<(), JitError> {
    let func = &program.functions[func_idx];
    let code = &func.chunk.code;
    let n = code.len();
    let reach = reachable(code);

    let mut fbctx = FunctionBuilderContext::new();
    let mut b = FunctionBuilder::new(&mut cl_ctx.func, &mut fbctx);

    // Re-declare every helper + every compiled phorj function into this body (a `FuncId` is callable in
    // a body only after `declare_func_in_func`). Unused refs are harmless — Cranelift emits a
    // relocation only for a ref that is actually `call`ed.
    let h = HelperRefs {
        push_int: module.declare_func_in_func(helpers.push_int, b.func),
        push_unit: module.declare_func_in_func(helpers.push_unit, b.func),
        get_local: module.declare_func_in_func(helpers.get_local, b.func),
        set_local: module.declare_func_in_func(helpers.set_local, b.func),
        arith: module.declare_func_in_func(helpers.arith, b.func),
        neg: module.declare_func_in_func(helpers.neg, b.func),
        not: module.declare_func_in_func(helpers.not, b.func),
        eqne: module.declare_func_in_func(helpers.eqne, b.func),
        cmp: module.declare_func_in_func(helpers.cmp, b.func),
        jif: module.declare_func_in_func(helpers.jif, b.func),
        depth_check: module.declare_func_in_func(helpers.depth_check, b.func),
        frame_base: module.declare_func_in_func(helpers.frame_base, b.func),
        ret: module.declare_func_in_func(helpers.ret, b.func),
    };
    let mut fn_refs: Vec<Option<FuncRef>> = vec![None; func_ids.len()];
    for (i, id) in func_ids.iter().enumerate() {
        if let Some(fid) = id {
            fn_refs[i] = Some(module.declare_func_in_func(*fid, b.func));
        }
    }

    // A dedicated param-only entry block reads `ctx` + `slot_base` and unconditionally jumps to the
    // param-less `start` block for ip 0. This keeps the ip-0 block free of block params, so a back-edge
    // (`Jump(0)` — a `while` at the top of a function) can target it without passing block args. `ctx`
    // and `slot_base`, defined in the entry block which dominates the whole body, are usable in every
    // block (including across back-edges) without threading them as block params.
    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let ctx_val = b.block_params(entry)[0];
    let sb_val = b.block_params(entry)[1];

    // One Cranelift block per reachable *leader* (ip 0, every branch target, the fall-through after a
    // `JumpIfFalse`). The memory operand stack means blocks carry no params / phis.
    let mut blocks: Vec<Option<Block>> = vec![None; n];
    let start = b.create_block();
    blocks[0] = Some(start);
    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        match op {
            Op::Jump(t) => {
                if blocks[*t].is_none() {
                    blocks[*t] = Some(b.create_block());
                }
            }
            Op::JumpIfFalse(t) => {
                if blocks[*t].is_none() {
                    blocks[*t] = Some(b.create_block());
                }
                if ip + 1 < n && blocks[ip + 1].is_none() {
                    blocks[ip + 1] = Some(b.create_block());
                }
            }
            _ => {}
        }
    }

    b.ins().jump(start, &[]);
    b.switch_to_block(start);
    let mut fault_block: Option<Block> = None;
    // `current` is `Some(block)` while the fall-through position is live, `None` right after a
    // terminator (until the next leader is switched to). Dead ops are skipped via `reach`.
    let mut current: Option<Block> = Some(start);

    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        // Entering a leader (other than ip 0, already switched to `start`): if we fell through from a
        // live block, wire the fall-through edge; then switch into the leader block.
        if ip != 0 {
            if let Some(blk) = blocks[ip] {
                if current.is_some() {
                    b.ins().jump(blk, &[]);
                }
                b.switch_to_block(blk);
                current = Some(blk);
            }
        }
        // Unreachable landing after a terminator with no leader here: nothing to emit into.
        if current.is_none() {
            continue;
        }

        match op {
            Op::Const(idx) => match &func.chunk.consts[*idx] {
                Value::Int(k) => {
                    let kv = b.ins().iconst(types::I64, *k);
                    b.ins().call(h.push_int, &[ctx_val, kv]);
                }
                Value::Unit => {
                    b.ins().call(h.push_unit, &[ctx_val]);
                }
                // Eligibility already guaranteed Int/Unit.
                other => return Err(JitError::Unsupported(format!("Const {other:?}"))),
            },
            Op::AddI | Op::SubI | Op::MulI | Op::DivI | Op::RemI => {
                let code_id: i64 = match op {
                    Op::AddI => 0,
                    Op::SubI => 1,
                    Op::MulI => 2,
                    Op::DivI => 3,
                    _ => 4,
                };
                let cv = b.ins().iconst(types::I64, code_id);
                let call = b.ins().call(h.arith, &[ctx_val, cv]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::Neg => {
                let call = b.ins().call(h.neg, &[ctx_val]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::Not => {
                let call = b.ins().call(h.not, &[ctx_val]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::Eq | Op::Ne => {
                let negate: i64 = if matches!(op, Op::Ne) { 1 } else { 0 };
                let nv = b.ins().iconst(types::I64, negate);
                let call = b.ins().call(h.eqne, &[ctx_val, nv]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                let code_id: i64 = match op {
                    Op::Lt => 0,
                    Op::Gt => 1,
                    Op::Le => 2,
                    _ => 3,
                };
                let cv = b.ins().iconst(types::I64, code_id);
                let call = b.ins().call(h.cmp, &[ctx_val, cv]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::GetLocal(slot) => {
                let sv = b.ins().iconst(types::I64, *slot as i64);
                let call = b.ins().call(h.get_local, &[ctx_val, sb_val, sv]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::SetLocal(slot) => {
                let sv = b.ins().iconst(types::I64, *slot as i64);
                let call = b.ins().call(h.set_local, &[ctx_val, sb_val, sv]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
            }
            Op::Jump(t) => {
                let tb = blocks[*t]
                    .ok_or_else(|| JitError::Codegen(format!("jump to non-leader ip {t}")))?;
                b.ins().jump(tb, &[]);
                current = None;
            }
            Op::JumpIfFalse(t) => {
                let call = b.ins().call(h.jif, &[ctx_val]);
                let res = b.inst_results(call)[0];
                // Fault (status 2) → fault-exit; else test true/false.
                let fb = *fault_block.get_or_insert_with(|| b.create_block());
                let is_fault = b.ins().icmp_imm(IntCC::Equal, res, JIF_FAULT);
                let notfault = b.create_block();
                b.ins().brif(is_fault, fb, &[], notfault, &[]);
                b.switch_to_block(notfault);
                let tb = blocks[*t]
                    .ok_or_else(|| JitError::Codegen(format!("JumpIfFalse target ip {t}")))?;
                let fallb = blocks[ip + 1].ok_or_else(|| {
                    JitError::Codegen(format!("JumpIfFalse fall-through ip {}", ip + 1))
                })?;
                // status 1 (JIF_FALSE) → take the jump; status 0 (JIF_TRUE) → fall through.
                let is_false = b.ins().icmp_imm(IntCC::Equal, res, JIF_FALSE);
                b.ins().brif(is_false, tb, &[], fallb, &[]);
                current = None;
            }
            Op::Call(callee) => {
                let callee_ref = fn_refs[*callee]
                    .ok_or_else(|| JitError::Codegen(format!("call to uncompiled fn {callee}")))?;
                let arity = program.functions[*callee].arity as i64;
                // 1. depth check (+increment) — mirrors the VM's pre-push `frames.len() >= MAX` guard;
                //    a fault here is the byte-identical `"stack overflow"`.
                let dc = b.ins().call(h.depth_check, &[ctx_val]);
                let dc_res = b.inst_results(dc)[0];
                emit_fault_check(&mut b, dc_res, &mut fault_block);
                // 2. the callee's window opens at `stack.len() - arity`.
                let av = b.ins().iconst(types::I64, arity);
                let fbc = b.ins().call(h.frame_base, &[ctx_val, av]);
                let sb_new = b.inst_results(fbc)[0];
                // 3. the direct native call (self-recursion resolves at finalize); on return the args
                //    have been replaced by the single return value on top of the stack.
                let cc = b.ins().call(callee_ref, &[ctx_val, sb_new]);
                let cc_res = b.inst_results(cc)[0];
                emit_fault_check(&mut b, cc_res, &mut fault_block);
            }
            Op::Return => {
                let call = b.ins().call(h.ret, &[ctx_val, sb_val]);
                let res = b.inst_results(call)[0];
                emit_fault_check(&mut b, res, &mut fault_block);
                let zero = b.ins().iconst(types::I64, 0);
                b.ins().return_(&[zero]);
                current = None;
            }
            // Eligibility already rejected anything else.
            other => return Err(JitError::Unsupported(format!("{other:?}"))),
        }
    }

    // Fault-exit block (shared): return status 1.
    if let Some(fb) = fault_block {
        b.switch_to_block(fb);
        let one = b.ins().iconst(types::I64, 1);
        b.ins().return_(&[one]);
    }
    b.seal_all_blocks();
    b.finalize();
    Ok(())
}
