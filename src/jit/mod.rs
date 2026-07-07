//! # JIT backend (Cranelift) — codegen slice 1(b1)
//!
//! **Status: MEMORY-OPERAND-STACK CODEGEN — int leaf functions with control flow, not yet wired into
//! `phg run`.** This extends the 1(a) pure-int straight-line spike to comparisons / `Neg` / `Not` /
//! `SetLocal` and, crucially, **branches and loops**, by switching the codegen model from a
//! compile-time SSA-pointer stack to a **runtime memory operand stack** living in [`JitCtx`] (see
//! `docs/plans/perf-wave.plan.md`, the locked 1(b) design). Native calls + recursion are 1(b2);
//! wiring into `phg run` + honest fib measurement is 1(b3).
//!
//! ## Why a memory operand stack (not SSA values)
//!
//! 1(a) threaded `*mut Value` pointers as Cranelift SSA values through a *compile-time* `Vec<ClValue>`.
//! That works for straight-line code, but branches merge divergent stack states — the classic SSA
//! phi / block-parameter reconciliation problem, plus the short-circuit "stack depth at a block
//! boundary" hazard. Spilling the operand stack into a Rust-side `Vec<Value>` inside the call context
//! sidesteps all of it: the stack lives in memory across block boundaries, so Cranelift blocks need
//! **no block params and no phis** — control flow becomes plain native branches. Byte-identity is
//! preserved by construction (SSA-register operands + unboxing are the deferred JIT-5 bonus).
//!
//! ## Boxed-`Value`-via-kernels (the locked order: boxed first, unboxing last)
//!
//! Codegen never reimplements arithmetic/comparison. Every operation is a `call` to a runtime bridge
//! helper (`rt_*` below) that operates on the operand stack and dispatches into the single-sourced
//! [`crate::value`] kernels (`int_add`, `int_neg`, `compare_ord`, `eq_val`, …) — mirroring the VM's
//! `exec.rs` arms exactly, so checked-overflow faults and their canonical strings are **byte-identical
//! to the VM** (Invariant 4), not re-derived.
//!
//! ## No panics across the `extern "C"` boundary
//!
//! `BytecodeProgram::validate` guarantees operand-stack balance, so underflow "can't happen" — but a
//! panic unwinding through the `extern "C"` ABI aborts the whole `phg` process. So every popping
//! helper is defensive: on the impossible underflow (or a defensively-checked out-of-range local) it
//! records a fault and returns the fault status, exactly like the arith arm's `_ => fault` case —
//! never `.unwrap()`.
//!
//! ## Why in-tree (not a separate crate)
//!
//! The JIT is a 4th backend intimately coupled to `Op`/`Value`/chunk (invariants #3/#4/#6), all of
//! which live in this single `phorj` lib crate; the CLI dispatch (`cli::mod`) and the
//! bench/disassemble/playground compile paths are lib code too. A separate `phorj-jit` crate would
//! force those internals `pub` and create a `phorj → phorj-jit → phorj` dependency cycle whose
//! cleanest fix is a vtable in the very hot path the JIT exists to eliminate. So the JIT lives here.
//!
//! ## The `unsafe` island
//!
//! A JIT's call path (`finalize → transmute(buf → fn ptr) → call`) is `unsafe` in phorj's own code —
//! phorj's FIRST first-party `unsafe` (the four other external deps confine their unsafe to
//! third-party crates). Per the ruling (`docs/specs/UNIFIED-SPEC.md` §"External dependency policy",
//! 2026-07-06 amendment): the crate roots relax from `#![forbid(unsafe_code)]` to
//! `#![deny(unsafe_code)]` and this module — and ONLY this module — carries the audited
//! `#![allow(unsafe_code)]` island below. `deny` (unlike `forbid`) permits that scoped `allow`, and CI
//! (`.github/workflows/ci.yml`, the `unsafe-island` gate) fails the build if an `allow(unsafe_code)`
//! escape hatch appears anywhere outside `src/jit/`, machine-enforcing "first-party unsafe lives only
//! in the JIT."
#![allow(unsafe_code)]

use crate::chunk::{BytecodeProgram, Op};
use crate::value::Value;
use cranelift::codegen::ir::{Signature, Type};
use cranelift::prelude::{
    types, AbiParam, Block, FunctionBuilder, FunctionBuilderContext, InstBuilder, IntCC,
    Value as ClValue,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, Linkage, Module};
use std::cmp::Ordering;

/// Why a function could not be JIT-run. Not a runtime fault (that is [`JitRun::Fault`]) — this means
/// the JIT declined or failed to *build* native code for the function.
#[derive(Debug)]
pub enum JitError {
    /// The function contains an `Op` (or a `Const` type / a closure capture) outside this slice's
    /// supported subset. The default-deny stance: anything not explicitly lowered is rejected, so
    /// callers fall back to the VM. Carries a human label of the offending shape.
    Unsupported(String),
    /// Cranelift module setup, verification, or finalization failed — a codegen bug, not user error.
    Codegen(String),
}

/// The outcome of *running* a JIT-compiled function: either its return [`Value`] or a clean runtime
/// fault (identical string to the VM, because it comes from the shared [`crate::value`] kernels).
#[derive(Debug)]
pub enum JitRun {
    Value(Value),
    Fault(String),
}

/// The call context the compiled native function receives (as an opaque pointer — Cranelift never
/// dereferences it; only the `rt_*` bridge helpers do). Holds the runtime operand stack (the
/// memory-operand-stack design) plus the out-channels for the result and a fault.
struct JitCtx {
    /// The unified operand stack — the whole point of the 1(b) design, and it also holds the frame's
    /// **locals**: in this VM locals are stack slots (`stack[slot_base + slot]`, `slot_base = 0` for a
    /// leaf frame), NOT a separate array. It is seeded with the call arguments (slots `0..arity`); a
    /// local declaration's initializer push self-seeds its slot as it executes, and operands stack on
    /// top. Living in memory (not SSA values) is what lets the stack survive across Cranelift block
    /// boundaries so control flow needs no block params.
    stack: Vec<Value>,
    /// `rt_ret` writes the entry function's return value here.
    result: Value,
    /// A helper sets this and returns the fault status on a clean runtime fault; the compiled code
    /// then branches to its fault-exit block, which returns status 1.
    fault: Option<String>,
}

// --- helper status codes (returned as i64 from the fallible `rt_*` helpers) ---
/// The helper succeeded.
const STATUS_OK: i64 = 0;
/// The helper recorded a fault in `ctx.fault`; the compiled code branches to the fault-exit block.
const STATUS_FAULT: i64 = 1;
// `rt_jump_if_false` is 3-way: 0 = operand was true (fall through), 1 = false (take the jump),
// 2 = fault (non-bool operand / underflow).
const JIF_TRUE: i64 = 0;
const JIF_FALSE: i64 = 1;
const JIF_FAULT: i64 = 2;

/// Canonical fault for the "can't happen" operand-stack underflow (validate guarantees balance). Not a
/// VM-parity string — an eligible function never hits it — but recorded rather than panicking, because
/// a panic through `extern "C"` aborts the process.
const FAULT_UNDERFLOW: &str = "jit: operand stack underflow";

/// Reborrow the context pointer the compiled code threads to every helper.
///
/// SAFETY: `ctx` is the single `&mut JitCtx` that [`compile_and_run`] passes as the sole argument to
/// the entry function; the compiled code forwards that exact pointer — non-null, unchanged — to every
/// `rt_*` helper, and never retains a helper's borrow across another helper call. So a fresh `&mut`
/// per helper invocation is sound.
#[inline]
fn cx<'a>(p: *mut JitCtx) -> &'a mut JitCtx {
    unsafe { &mut *p }
}

/// Record `msg` as the pending fault and return the fault status (shared by every fallible helper).
#[inline]
fn fault(c: &mut JitCtx, msg: String) -> i64 {
    c.fault = Some(msg);
    STATUS_FAULT
}

/// Push `Value::Int(n)`. Bridge for `Op::Const` of an int literal — infallible (only grows the stack).
extern "C" fn rt_push_int(p: *mut JitCtx, n: i64) {
    cx(p).stack.push(Value::Int(n));
}

/// Push `Value::Unit`. Bridge for `Op::Const` of the unit literal (the compiler's synthesized
/// fall-through `return`) — infallible.
extern "C" fn rt_push_unit(p: *mut JitCtx) {
    cx(p).stack.push(Value::Unit);
}

/// Push a clone of the local at stack slot `slot` (`stack[slot]`, slot_base = 0). Bridge for
/// `Op::GetLocal`; mirrors `exec.rs` (locals are stack slots). Fallible only defensively.
extern "C" fn rt_get_local(p: *mut JitCtx, slot: i64) -> i64 {
    let c = cx(p);
    let v = match c.stack.get(slot as usize) {
        Some(v) => v.clone(),
        None => return fault(c, format!("jit: local slot {slot} out of range")),
    };
    c.stack.push(v);
    STATUS_OK
}

/// Pop the top of stack into the local at stack slot `slot` (set-and-pop, decision P2-4). Bridge for
/// `Op::SetLocal`; mirrors `exec.rs` (locals are stack slots).
extern "C" fn rt_set_local(p: *mut JitCtx, slot: i64) -> i64 {
    let c = cx(p);
    let Some(v) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    match c.stack.get_mut(slot as usize) {
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
extern "C" fn rt_arith(p: *mut JitCtx, code: i64) -> i64 {
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
extern "C" fn rt_neg(p: *mut JitCtx) -> i64 {
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
extern "C" fn rt_not(p: *mut JitCtx) -> i64 {
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
extern "C" fn rt_eqne(p: *mut JitCtx, negate: i64) -> i64 {
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
extern "C" fn rt_cmp(p: *mut JitCtx, code: i64) -> i64 {
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
extern "C" fn rt_jump_if_false(p: *mut JitCtx) -> i64 {
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

/// Pop the return value into the context. Bridge for `Op::Return`.
extern "C" fn rt_ret(p: *mut JitCtx) -> i64 {
    let c = cx(p);
    match c.stack.pop() {
        Some(v) => {
            c.result = v;
            STATUS_OK
        }
        None => fault(c, FAULT_UNDERFLOW.to_string()),
    }
}

/// Build a Cranelift signature with the given parameter/return machine types.
fn make_sig(module: &JITModule, params: &[Type], ret: Option<Type>) -> Signature {
    let mut sig = module.make_signature();
    for p in params {
        sig.params.push(AbiParam::new(*p));
    }
    if let Some(r) = ret {
        sig.returns.push(AbiParam::new(r));
    }
    sig
}

/// Mark every instruction index reachable from ip 0, following branch targets and non-terminator
/// fall-through. Unconditional `Jump`/`Return` have no fall-through successor; `JumpIfFalse` has both
/// its target and its fall-through (`ip + 1`). Dead ops (e.g. the compiler's fall-through
/// `Const(Unit); Return` tail after a real `return`) stay unmarked so codegen never materializes them
/// — which is also what keeps every emitted Cranelift block reachable-from-entry (so the entry-block
/// `ctx` param dominates every use, no SSA-dominance violation).
fn reachable(code: &[Op]) -> Vec<bool> {
    let n = code.len();
    let mut reach = vec![false; n];
    let mut work = vec![0usize];
    while let Some(ip) = work.pop() {
        if ip >= n || reach[ip] {
            continue;
        }
        reach[ip] = true;
        match &code[ip] {
            Op::Return => {}
            Op::Jump(t) => work.push(*t),
            Op::JumpIfFalse(t) => {
                work.push(*t);
                work.push(ip + 1);
            }
            _ => work.push(ip + 1),
        }
    }
    reach
}

/// After a call whose result `res` is a `0 = ok / 1 = fault` status, branch to the shared fault-exit
/// block on fault and continue in a fresh block (the sequential-execution successor).
fn emit_fault_check(b: &mut FunctionBuilder, res: ClValue, fault_block: &mut Option<Block>) {
    let fb = *fault_block.get_or_insert_with(|| b.create_block());
    let is_fault = b.ins().icmp_imm(IntCC::NotEqual, res, 0);
    let cont = b.create_block();
    b.ins().brif(is_fault, fb, &[], cont, &[]);
    b.switch_to_block(cont);
}

/// JIT-compile the function at `func_idx` in `program` (if its body is in the supported subset) and
/// run it with `args`, returning its return value or a clean runtime fault.
///
/// Returns [`JitError::Unsupported`] for any op / const / closure capture outside the int-leaf +
/// control-flow subset — the default-deny contract that keeps callers falling back to the VM.
pub fn compile_and_run(
    program: &BytecodeProgram,
    func_idx: usize,
    args: &[Value],
) -> Result<JitRun, JitError> {
    let func = &program.functions[func_idx];
    if func.n_captures != 0 {
        return Err(JitError::Unsupported("closure with captures".to_string()));
    }
    let code = &func.chunk.code;
    let n = code.len();
    let reach = reachable(code);

    // --- eligibility (default-deny), over the REACHABLE ops only ---
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
            other => return Err(JitError::Unsupported(format!("{other:?}"))),
        }
    }

    // --- module + host ISA, with the bridge helpers registered as symbols ---
    let mut builder = JITBuilder::new(default_libcall_names())
        .map_err(|e| JitError::Codegen(format!("JITBuilder: {e}")))?;
    builder.symbol("rt_push_int", rt_push_int as *const u8);
    builder.symbol("rt_push_unit", rt_push_unit as *const u8);
    builder.symbol("rt_get_local", rt_get_local as *const u8);
    builder.symbol("rt_set_local", rt_set_local as *const u8);
    builder.symbol("rt_arith", rt_arith as *const u8);
    builder.symbol("rt_neg", rt_neg as *const u8);
    builder.symbol("rt_not", rt_not as *const u8);
    builder.symbol("rt_eqne", rt_eqne as *const u8);
    builder.symbol("rt_cmp", rt_cmp as *const u8);
    builder.symbol("rt_jump_if_false", rt_jump_if_false as *const u8);
    builder.symbol("rt_ret", rt_ret as *const u8);
    let mut module = JITModule::new(builder);
    let ptr = module.target_config().pointer_type();

    // --- declare the imported bridge helpers ---
    let sig_push_int = make_sig(&module, &[ptr, types::I64], None); // rt_push_int
    let sig_void = make_sig(&module, &[ptr], None); // rt_push_unit
    let sig_slot = make_sig(&module, &[ptr, types::I64], Some(types::I64)); // rt_get/set_local
    let sig_code = make_sig(&module, &[ptr, types::I64], Some(types::I64)); // rt_arith/cmp/eqne
    let sig_status = make_sig(&module, &[ptr], Some(types::I64)); // rt_pop/neg/not/jump_if_false/ret
    let declare = |m: &mut JITModule, name: &str, sig: &Signature| {
        m.declare_function(name, Linkage::Import, sig)
            .map_err(|e| JitError::Codegen(format!("declare {name}: {e}")))
    };
    let id_push_int = declare(&mut module, "rt_push_int", &sig_push_int)?;
    let id_push_unit = declare(&mut module, "rt_push_unit", &sig_void)?;
    let id_get_local = declare(&mut module, "rt_get_local", &sig_slot)?;
    let id_set_local = declare(&mut module, "rt_set_local", &sig_slot)?;
    let id_arith = declare(&mut module, "rt_arith", &sig_code)?;
    let id_neg = declare(&mut module, "rt_neg", &sig_status)?;
    let id_not = declare(&mut module, "rt_not", &sig_status)?;
    let id_eqne = declare(&mut module, "rt_eqne", &sig_code)?;
    let id_cmp = declare(&mut module, "rt_cmp", &sig_code)?;
    let id_jif = declare(&mut module, "rt_jump_if_false", &sig_status)?;
    let id_ret = declare(&mut module, "rt_ret", &sig_status)?;

    // --- the entry function: extern "C" fn(*mut JitCtx) -> i64 (status: 0 ok, 1 fault) ---
    let mut ctx = module.make_context();
    ctx.func.signature.params.push(AbiParam::new(ptr));
    ctx.func.signature.returns.push(AbiParam::new(types::I64));
    let func_id = module
        .declare_function("phorj_jit_entry", Linkage::Export, &ctx.func.signature)
        .map_err(|e| JitError::Codegen(format!("declare entry: {e}")))?;

    {
        let mut fbctx = FunctionBuilderContext::new();
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fbctx);

        let r_push_int = module.declare_func_in_func(id_push_int, b.func);
        let r_push_unit = module.declare_func_in_func(id_push_unit, b.func);
        let r_get_local = module.declare_func_in_func(id_get_local, b.func);
        let r_set_local = module.declare_func_in_func(id_set_local, b.func);
        let r_arith = module.declare_func_in_func(id_arith, b.func);
        let r_neg = module.declare_func_in_func(id_neg, b.func);
        let r_not = module.declare_func_in_func(id_not, b.func);
        let r_eqne = module.declare_func_in_func(id_eqne, b.func);
        let r_cmp = module.declare_func_in_func(id_cmp, b.func);
        let r_jif = module.declare_func_in_func(id_jif, b.func);
        let r_ret = module.declare_func_in_func(id_ret, b.func);

        // A dedicated param-only entry block reads `ctx` and unconditionally jumps to the param-less
        // `start` block for ip 0. This keeps the ip-0 block free of block params, so a back-edge
        // (`Jump(0)` — a `while` at the top of a function) can target it without passing block args.
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        let ctx_val = b.block_params(entry)[0];

        // One Cranelift block per reachable *leader* (ip 0, every branch target, the fall-through
        // after a `JumpIfFalse`). The memory operand stack means blocks carry no params / phis.
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
            // Entering a leader (other than ip 0, already switched to `start`): if we fell through
            // from a live block, wire the fall-through edge; then switch into the leader block.
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
                        b.ins().call(r_push_int, &[ctx_val, kv]);
                    }
                    Value::Unit => {
                        b.ins().call(r_push_unit, &[ctx_val]);
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
                    let call = b.ins().call(r_arith, &[ctx_val, cv]);
                    let res = b.inst_results(call)[0];
                    emit_fault_check(&mut b, res, &mut fault_block);
                }
                Op::Neg => {
                    let call = b.ins().call(r_neg, &[ctx_val]);
                    let res = b.inst_results(call)[0];
                    emit_fault_check(&mut b, res, &mut fault_block);
                }
                Op::Not => {
                    let call = b.ins().call(r_not, &[ctx_val]);
                    let res = b.inst_results(call)[0];
                    emit_fault_check(&mut b, res, &mut fault_block);
                }
                Op::Eq | Op::Ne => {
                    let negate: i64 = if matches!(op, Op::Ne) { 1 } else { 0 };
                    let nv = b.ins().iconst(types::I64, negate);
                    let call = b.ins().call(r_eqne, &[ctx_val, nv]);
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
                    let call = b.ins().call(r_cmp, &[ctx_val, cv]);
                    let res = b.inst_results(call)[0];
                    emit_fault_check(&mut b, res, &mut fault_block);
                }
                Op::GetLocal(slot) => {
                    let sv = b.ins().iconst(types::I64, *slot as i64);
                    let call = b.ins().call(r_get_local, &[ctx_val, sv]);
                    let res = b.inst_results(call)[0];
                    emit_fault_check(&mut b, res, &mut fault_block);
                }
                Op::SetLocal(slot) => {
                    let sv = b.ins().iconst(types::I64, *slot as i64);
                    let call = b.ins().call(r_set_local, &[ctx_val, sv]);
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
                    let call = b.ins().call(r_jif, &[ctx_val]);
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
                Op::Return => {
                    let call = b.ins().call(r_ret, &[ctx_val]);
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
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| JitError::Codegen(format!("define: {e}")))?;
    module.clear_context(&mut ctx);
    module
        .finalize_definitions()
        .map_err(|e| JitError::Codegen(format!("finalize: {e}")))?;
    let entry_code = module.get_finalized_function(func_id);

    // --- run it ---
    // SAFETY: `entry_code` is the finalized machine code for a function compiled with exactly the
    // signature `extern "C" fn(*mut JitCtx) -> i64` — the sole first-party `unsafe` this whole effort
    // exists to confine. `module` (which owns the executable memory) is kept alive across the call.
    let entry: extern "C" fn(*mut JitCtx) -> i64 =
        unsafe { std::mem::transmute::<*const u8, extern "C" fn(*mut JitCtx) -> i64>(entry_code) };

    // Seed the unified stack with the arguments (slots `0..arity`); local declarations self-seed
    // their slots as they execute, operands stack on top.
    let mut call_ctx = JitCtx {
        stack: args.to_vec(),
        result: Value::Unit,
        fault: None,
    };
    let status = entry(&mut call_ctx);
    // `JITModule` has NO `Drop` impl (verified against cranelift-jit 0.133 `src/backend.rs`) — merely
    // dropping it LEAKS the code mmap; memory is reclaimed only by the explicit `free_memory`. `entry`
    // has already returned and its pointer is never used again, so freeing now satisfies the method's
    // contract (no compiled fn executing, no fn-ptr called afterward). `call_ctx` is independent Rust
    // heap, unaffected. (When the wiring slice caches compiled functions, the module instead lives for
    // the program's lifetime and frees once at the end.)
    // SAFETY: no outstanding use of any pointer into `module` past this point — see above.
    unsafe { module.free_memory() };

    if status == 0 {
        Ok(JitRun::Value(call_ctx.result))
    } else {
        Ok(JitRun::Fault(
            call_ctx
                .fault
                .unwrap_or_else(|| "jit: unknown fault".to_string()),
        ))
    }
}

#[cfg(test)]
mod tests;
