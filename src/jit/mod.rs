//! # JIT backend (Cranelift) — codegen slice 1(b3a)
//!
//! **Status: NATIVE→NATIVE CALLS + SELF-RECURSION, compile-once [`Compiled`] handle, honest fib
//! measurement — not yet wired into `phg run`.** 1(b2) extended 1(b1)'s single-function
//! memory-operand-stack codegen to a **multi-function module**: the call graph reachable from the entry
//! (via `Op::Call`) is compiled as one `JITModule`, each phorj function to its own Cranelift function,
//! so a `Call` lowers to a direct native call to the callee's `FuncId` (self-recursion included —
//! resolved at `finalize_definitions`); recursive `fib` JITs. 1(b3a) split *compile* from *run* into
//! the [`Compiled`] handle (compile once, run many — the seam the honest benchmark and the future
//! `phg run` hot-function cache both need) and added [`is_eligible`] + [`Compiled::run`]'s `start_depth`
//! (the VM's live frame count at the invocation site — see its doc for the under-fault hazard the b3b
//! wiring hangs on). The b3b VM `Op::Call` speculative hook + fault-fallback is next. See
//! `docs/plans/perf-wave.plan.md`, the locked 1(b) design + the b2/b3 execution entries.
//!
//! ## The shared value stack + per-frame `slot_base` (mirrors `vm::exec` exactly)
//!
//! The VM keeps ONE `stack: Vec<Value>` and a `Frame { func, ip, slot_base }` list; a local at `slot`
//! is `stack[slot_base + slot]` (`slot_base = 0` for the entry frame). A `Call(idx)` pops nothing —
//! the callee's `arity` args already sit on top, so its window opens at `slot_base = stack.len() -
//! arity`; `Return` pops the return value, truncates the stack back to `slot_base`, and pushes the one
//! return value (`vm::do_return`). The JIT mirrors this to the byte: every compiled function is
//! `extern "C" fn(*mut JitCtx, slot_base: i64) -> i64`; the shared stack lives in [`JitCtx`]; frame-
//! relative helpers (`rt_get_local`/`rt_set_local`/`rt_return`) take `slot_base`. A native call's net
//! stack effect is therefore (pop `arity` args, push one result) — identical to the VM's `Op::Call`.
//!
//! ## Depth cap → the byte-identical `"stack overflow"` fault
//!
//! Native recursion would exhaust the OS stack and abort the process; the VM instead caps at
//! [`MAX_CALL_DEPTH`] frames and faults cleanly with `"stack overflow"`. The JIT reproduces this with a
//! `depth` counter in [`JitCtx`] (seeded 1 = the entry frame): `rt_depth_check` faults iff
//! `depth >= MAX_CALL_DEPTH` (the check the VM makes BEFORE pushing a frame), else increments;
//! `rt_return` decrements. So the fault fires at exactly the VM's depth, with the VM's string (proven
//! against the VM oracle in the tests, not a hardcoded literal — the string is not yet single-sourced).
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
use crate::limits::MAX_CALL_DEPTH;
use crate::value::Value;
use cranelift::codegen::ir::{FuncRef, MemFlagsData, Signature, Type};
use cranelift::prelude::{
    types, AbiParam, Block, FloatCC, FunctionBuilder, FunctionBuilderContext, InstBuilder, IntCC,
    Value as ClValue, Variable,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use std::cmp::Ordering;

/// Why a function could not be JIT-run. Not a runtime fault (that is [`JitRun::Fault`]) — this means
/// the JIT declined or failed to *build* native code for the function.
#[derive(Debug)]
pub enum JitError {
    /// The function (or one transitively called by it) contains an `Op` / a `Const` type / a closure
    /// capture outside this slice's supported subset. The default-deny stance: anything not explicitly
    /// lowered is rejected, so callers fall back to the VM. Carries a human label of the offending
    /// shape.
    Unsupported(String),
    /// Cranelift module setup, verification, or finalization failed — a codegen bug, not user error.
    Codegen(String),
}

/// The outcome of *running* a JIT-compiled function: either its return [`Value`] or a clean runtime
/// fault (identical string to the VM, because it comes from the shared [`crate::value`] kernels — or,
/// for `"stack overflow"`, the same literal the VM uses, oracle-checked in the tests).
#[derive(Debug)]
pub enum JitRun {
    Value(Value),
    Fault(String),
}

/// The call context every compiled native function receives (as an opaque pointer — Cranelift never
/// dereferences it; only the `rt_*` bridge helpers do). Holds the unified runtime operand stack (the
/// memory-operand-stack design), the live call depth (for the `"stack overflow"` cap), and the
/// out-channel for a fault.
struct JitCtx {
    /// The unified operand stack — the whole point of the 1(b) design, and it also holds every frame's
    /// **locals**: in this VM locals are stack slots (`stack[slot_base + slot]`), NOT a separate array.
    /// The entry frame's window opens at `slot_base = 0`, seeded with the call arguments (slots
    /// `0..arity`); a callee's window opens at `stack.len() - arity` over the args the caller left on
    /// top. A local declaration's initializer push self-seeds its slot as it executes, and operands
    /// stack on top. Living in memory (not SSA values) is what lets the stack survive across Cranelift
    /// block boundaries so control flow needs no block params.
    stack: Vec<Value>,
    /// Live call-frame depth, seeded to 1 (the entry frame). `rt_depth_check` faults when it would
    /// exceed [`MAX_CALL_DEPTH`] (the check `vm::exec` makes before pushing a frame) and otherwise
    /// increments; `rt_return` decrements. Keeps the `"stack overflow"` fault at the VM's exact depth
    /// AND bounds native recursion so a runaway can't blow the OS stack. On a *fault* path the matching
    /// decrement is skipped (the compiled code branches straight to fault-exit) — intentionally
    /// harmless: `JitCtx` is per-run (the run aborts and it is dropped), and it stays per-run even once
    /// b3/JIT-3 caches the module, so a stale count can never leak into a later run. Do not "fix" it.
    depth: usize,
    /// A helper sets this and returns the fault status on a clean runtime fault; the compiled code
    /// then branches to its fault-exit block, which returns status 1. The fault propagates up through
    /// each caller's post-call status check to the entry, unchanged.
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

/// The VM's clean deep-recursion fault. The string is a bare literal in `vm::exec`/`vm::closure`/the
/// interpreter (not yet single-sourced in `value.rs` like the arithmetic faults), so it is duplicated
/// here — but the tests assert the JIT fault against the VM oracle's rendering, not this literal, so
/// any VM-side drift is caught.
const FAULT_STACK_OVERFLOW: &str = "stack overflow";

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

/// Push a clone of the local at frame slot `slot` (`stack[slot_base + slot]`). Bridge for
/// `Op::GetLocal`; mirrors `exec.rs` (locals are stack slots, offset by the frame's `slot_base`).
/// Fallible only defensively.
extern "C" fn rt_get_local(p: *mut JitCtx, slot_base: i64, slot: i64) -> i64 {
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
extern "C" fn rt_set_local(p: *mut JitCtx, slot_base: i64, slot: i64) -> i64 {
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

/// Check-and-enter a new call frame. Bridge emitted before every `Op::Call`. Mirrors the VM's
/// `if self.frames.len() >= MAX_CALL_DEPTH { return Err("stack overflow") }` guard, made BEFORE the
/// frame is pushed: faults iff `depth >= MAX_CALL_DEPTH`, otherwise increments `depth` (the frame the
/// callee is about to run in). The matching decrement is in `rt_return`.
extern "C" fn rt_depth_check(p: *mut JitCtx) -> i64 {
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
extern "C" fn rt_frame_base(p: *mut JitCtx, arity: i64) -> i64 {
    let c = cx(p);
    c.stack.len().saturating_sub(arity as usize) as i64
}

/// Return from the current frame. Bridge for `Op::Return`; mirrors `vm::do_return`: pop the return
/// value, decrement `depth`, truncate the stack back to this frame's `slot_base` (discarding its
/// locals + operands), then push the single return value so the caller sees it on top (net effect of a
/// call: pop `arity` args, push one result). For the entry frame this leaves `[rv]` on the stack,
/// which [`compile_and_run`] pops as the result.
extern "C" fn rt_return(p: *mut JitCtx, slot_base: i64) -> i64 {
    let c = cx(p);
    let Some(rv) = c.stack.pop() else {
        return fault(c, FAULT_UNDERFLOW.to_string());
    };
    c.depth = c.depth.saturating_sub(1);
    c.stack.truncate(slot_base as usize);
    c.stack.push(rv);
    STATUS_OK
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
/// params dominate every use, no SSA-dominance violation).
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

/// Basic-block leaders for the UNBOXED codegen: ip 0, every (reachable) branch target, and the
/// fall-through after every `JumpIfFalse`. This is the SINGLE definition of the block structure —
/// both `build_body_unboxed` (which creates one Cranelift block per leader) and `unboxed_slot_kinds`
/// (which clears its abstract operand stack at each leader, mirroring the empty-at-leaders invariant)
/// drive off it, so the two can never drift apart. NOTE: the fall-through after an unconditional
/// `Jump` / a `Return` is NOT a leader (it is only reachable via an explicit branch, which adds it as
/// a target if live) — matching `reachable`.
fn leaders(code: &[Op], reach: &[bool]) -> Vec<bool> {
    let n = code.len();
    let mut is_leader = vec![false; n];
    if n > 0 {
        is_leader[0] = true;
    }
    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        match op {
            Op::Jump(t) => {
                if *t < n {
                    is_leader[*t] = true;
                }
            }
            Op::JumpIfFalse(t) => {
                if *t < n {
                    is_leader[*t] = true;
                }
                if ip + 1 < n {
                    is_leader[ip + 1] = true;
                }
            }
            _ => {}
        }
    }
    is_leader
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

/// The imported bridge-helper `FuncId`s, declared once per module and re-referenced into every
/// function body. Grouped so `build_body` takes one argument instead of eleven.
struct Helpers {
    push_int: FuncId,
    push_unit: FuncId,
    get_local: FuncId,
    set_local: FuncId,
    arith: FuncId,
    neg: FuncId,
    not: FuncId,
    eqne: FuncId,
    cmp: FuncId,
    jif: FuncId,
    depth_check: FuncId,
    frame_base: FuncId,
    ret: FuncId,
}

/// The `FuncRef`s for the bridge helpers, resolved into one function body (a `FuncId` must be
/// re-declared per body before it can be `call`ed there).
struct HelperRefs {
    push_int: FuncRef,
    push_unit: FuncRef,
    get_local: FuncRef,
    set_local: FuncRef,
    arith: FuncRef,
    neg: FuncRef,
    not: FuncRef,
    eqne: FuncRef,
    cmp: FuncRef,
    jif: FuncRef,
    depth_check: FuncRef,
    frame_base: FuncRef,
    ret: FuncRef,
}

/// Collect the set of function indices to compile: the entry plus every function transitively
/// **reachably** called (via `Op::Call`) from it, in a deterministic discovery order. Along the way
/// enforce eligibility per function (default-deny): a closure capture, a non-int/unit `Const`, or any
/// op outside the supported subset makes the WHOLE compilation `Unsupported` (so the caller falls back
/// to the VM), because a native call needs its callee compiled in the same module. Only **reachable**
/// ops are inspected — a dead `Call` to an ineligible function must not sink an otherwise-eligible one.
fn collect_functions(program: &BytecodeProgram, entry_idx: usize) -> Result<Vec<usize>, JitError> {
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
fn build_body(
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

// ===========================================================================================
// Unboxed int codegen (slice u1) — the ~2×-over-php path. Operands are compile-time SSA `i64`
// values (NO boxed `Vec<Value>`, NO per-op `extern "C"` helper call); native `iadd`/`icmp`/etc. run
// in registers. The boxed path above stays as the byte-identity ORACLE (unboxed ≡ boxed ≡ VM).
// ===========================================================================================

/// The kind of a compile-time operand-stack entry. The bytecode is type-erased, so this is tracked to
/// map `Return` correctly WITHOUT a type source: `Const`/arithmetic/`Neg` → `Int`, comparisons/`Not`
/// → `Bool`, a bare local (param) read → `Unknown`. u1 accepts a function ONLY if every reachable
/// `Return` yields `Int` — so a `bool`-returning function (which would else be mis-mapped to
/// `Value::Int`) and a bare-param return (unprovable-`Int` without types) fall back to the VM/boxed
/// path. Bool *params* are fine: they arrive as `0/1` i64 and are only ever consumed in bool contexts
/// (`Not`, `JumpIfFalse`, comparison operands) natively. Types + bare-param returns (so `fib`'s
/// `return n` JITs) come in u2 with a real type source.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Int,
    /// A float operand, stored in a `vars` cell as its `f64` BITS (an `i64`); code `bitcast`s I64↔F64
    /// only at the float op that consumes/produces it, so the operand stack + local model stay
    /// uniformly `I64` and the ABI is unchanged (a float arg is passed as its bits, a float return
    /// decoded via [`Compiled::ret_kind`]). Float arithmetic never overflows (no sticky); only a
    /// zero-divisor `DivF` faults (→ code 5, redo on VM).
    Float,
    Bool,
    Unknown,
}

/// Provenance of an operand-stack entry for the provenance pre-pass ONLY (not codegen): `Param(slot)`
/// = a bare `GetLocal(slot)` result; `Other` = anything else (a `Const`, an arithmetic/comparison
/// result, a call result).
#[derive(Clone, Copy)]
enum Prov {
    Param(usize),
    Other,
}

/// Which param slots are provably numeric AND their kind — `Some(Int)` if consumed (while still a bare
/// `GetLocal`) by an int-arith op (`AddI`/`SubI`/`MulI`/`DivI`/`RemI`/`Neg`), `Some(Float)` if consumed
/// by a float-arith op (`AddF`/`SubF`/`MulF`/`DivF`); the compiler emits each family ONLY for its
/// operand type. `None` = unprovable (falls back to `Unknown`). This lets a bare-param `Return` (e.g.
/// `fib`'s base case `return n`, or a float leaf's `return x`) type WITHOUT a declared-type source. It
/// MUST be a separate pre-pass: the
/// base-case `return n` can PRECEDE the `n - 1` (`SubI`) that proves `n` int, so a single forward pass
/// would reject it. SOUND and one-directional — a slot is marked only on hard evidence; imprecision
/// (a missed mark) only over-rejects (falls back), never mis-accepts. The operand stack is cleared at
/// terminators so no provenance leaks across a basic-block boundary; `self.arity` args are popped for
/// a `Call` (u2a calls are self-recursive, so the callee arity equals this function's).
fn unboxed_proven_param_kinds(func: &crate::chunk::Function) -> Vec<Option<Kind>> {
    let code = &func.chunk.code;
    let reach = reachable(code);
    let mut proven: Vec<Option<Kind>> = vec![None; func.arity];
    let mark = |proven: &mut Vec<Option<Kind>>, p: Prov, k: Kind| {
        if let Prov::Param(slot) = p {
            if slot < proven.len() {
                // A param has exactly one static type (the checker), so int- and float-proof can never
                // conflict on the same slot; the assignment is unambiguous.
                proven[slot] = Some(k);
            }
        }
    };
    let mut stack: Vec<Prov> = Vec::new();
    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        match op {
            Op::GetLocal(slot) => stack.push(Prov::Param(*slot)),
            Op::Const(_) => stack.push(Prov::Other),
            Op::AddI | Op::SubI | Op::MulI | Op::DivI | Op::RemI => {
                for _ in 0..2 {
                    let p = stack.pop().unwrap_or(Prov::Other);
                    mark(&mut proven, p, Kind::Int);
                }
                stack.push(Prov::Other);
            }
            Op::AddF | Op::SubF | Op::MulF | Op::DivF => {
                // Float arith (the compiler emits these ONLY for float operands) proves a bare-param
                // operand `float` — so a float leaf's bare-param `return x` types as Float.
                for _ in 0..2 {
                    let p = stack.pop().unwrap_or(Prov::Other);
                    mark(&mut proven, p, Kind::Float);
                }
                stack.push(Prov::Other);
            }
            Op::Neg => {
                let p = stack.pop().unwrap_or(Prov::Other);
                mark(&mut proven, p, Kind::Int);
                stack.push(Prov::Other);
            }
            Op::Not => {
                stack.pop();
                stack.push(Prov::Other);
            }
            Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                stack.pop();
                stack.pop();
                stack.push(Prov::Other);
            }
            Op::Call(_) => {
                // A call consumes the callee's args (whose count we don't track here) and yields a
                // result. Clear conservatively: losing provenance for operands below the args only
                // over-rejects (a missed mark), never mis-marks — and the call result is `Other`.
                stack.clear();
                stack.push(Prov::Other);
            }
            Op::JumpIfFalse(_) => {
                stack.pop();
                stack.clear();
            }
            Op::Jump(_) | Op::Return => stack.clear(),
            _ => stack.clear(),
        }
    }
    proven
}

/// Range-analysis pre-pass (docs/plans/perf-wave.plan.md): which `AddI` ops are PROVABLY-no-overflow
/// induction-variable increments, so `build_body_unboxed` can emit a plain wrapping-free `iadd` (no
/// `sadd_overflow`, no sticky accumulation) for them — the lever that lets a counted-loop's counter
/// stop paying for an overflow guard the VM would never actually fault on. Returns a `Vec<bool>` indexed
/// by ip (`true` = proven safe). SOUND + CONSERVATIVE: an unprovable op stays `false` (keeps the guard);
/// imprecision (a missed mark) only over-keeps a guard (a perf miss), never mis-accepts (a miscompile).
///
/// An `AddI` at ip `k` is proven iff ALL of these hold (positive conjunction — any doubt fails closed):
///  1. **shape** `GetLocal(s); Const(Int 1); AddI; SetLocal(s)` at `[k-2 ..= k+1]` (step `+1`, same slot `s`);
///  2. **single writer** — slot `s` has EXACTLY ONE reachable `SetLocal(s)` in the function (this one), so
///     `s` cannot be mutated between the guard and the increment (its other def is the pre-loop init);
///  3. **guarded** — the increment's innermost enclosing loop's header `H` (target of a backward branch
///     at `e`, `H < k < e`) LEADS with the strict-`<` guard on `s`: `code[H]==GetLocal(s)`,
///     `code[H+1] ∈ {GetLocal, Const(Int)}`, `code[H+2]==Lt`, `code[H+3]==JumpIfFalse(x)` with `x > e`
///     (the loop exit is forward, past the back-edge);
///  4. **not nested** — the guarded body `[H, e]` contains exactly ONE backward branch (this one), so the
///     counter is re-checked every iteration (rules out the inner-loop-runs-unbounded-for-fixed-`s` trap).
///
/// SOUNDNESS: the header guard `s < V` (signed `Lt`, `s` the LEFT/deeper operand — condition 3 keys off
/// `code[H]==GetLocal(s)`, so ONLY that orientation is accepted, never `V < s`) gives `s ≤ V-1 ≤
/// i64::MAX-1` whenever the body runs; single-writer (condition 2) keeps `s` unchanged from the guard to
/// the increment ⇒ `s+1 ≤ i64::MAX`, no overflow. The bound `V` is irrelevant to the proof (any i64
/// works), so it is not analyzed. The one place a bug flips safe→unsound is the guard↔increment link
/// (conditions 3+4); everywhere else a bug degrades to a missed mark (safe).
fn range_proven_ops(func: &crate::chunk::Function) -> Vec<bool> {
    let code = &func.chunk.code;
    let n = code.len();
    let reach = reachable(code);
    let mut proven = vec![false; n];

    // All reachable backward branches as `(source e, target/header H)`, H < e.
    let backs: Vec<(usize, usize)> = code
        .iter()
        .enumerate()
        .filter(|&(ip, _)| reach[ip])
        .filter_map(|(ip, op)| match op {
            Op::Jump(t) | Op::JumpIfFalse(t) if *t < ip => Some((ip, *t)),
            _ => None,
        })
        .collect();

    for k in 0..n {
        if !reach[k] || !matches!(code[k], Op::AddI) || k < 2 || k + 1 >= n {
            continue;
        }
        // (1) shape `GetLocal(s); Const(Int 1); AddI; SetLocal(s)`.
        let s = match code[k - 2] {
            Op::GetLocal(s) => s,
            _ => continue,
        };
        let is_one = matches!(code[k - 1], Op::Const(ci)
            if matches!(func.chunk.consts.get(ci), Some(Value::Int(1))));
        if !is_one || !matches!(code[k + 1], Op::SetLocal(t) if t == s) {
            continue;
        }
        // (2) single writer: exactly one reachable SetLocal(s) (this one).
        let writers = code
            .iter()
            .enumerate()
            .filter(|&(ip, op)| reach[ip] && matches!(op, Op::SetLocal(t) if *t == s))
            .count();
        if writers != 1 {
            continue;
        }
        // Innermost enclosing loop: exactly one backward branch (e, H) with H < k < e. Zero → not in a
        // loop; more than one → nested loops around k (fail closed — this slice does not prove nested).
        let enclosing: Vec<(usize, usize)> = backs
            .iter()
            .copied()
            .filter(|&(e, h)| h < k && k < e)
            .collect();
        if enclosing.len() != 1 {
            continue;
        }
        let (e, h) = enclosing[0];
        // (4) not nested: the ONLY backward branch whose source lies in [H, e] is this one.
        if backs.iter().any(|&(e2, _)| e2 != e && h <= e2 && e2 <= e) {
            continue;
        }
        // (3) header H leads with the strict-`<` guard on `s`:
        //   GetLocal(s); {GetLocal(_) | Const(Int _)}; Lt; JumpIfFalse(x)  with x > e (forward exit).
        if h + 3 >= n {
            continue;
        }
        let head_slot_ok = matches!(code[h], Op::GetLocal(g) if g == s);
        let bound_ok = matches!(code[h + 1], Op::GetLocal(_))
            || matches!(code[h + 1], Op::Const(ci)
                if matches!(func.chunk.consts.get(ci), Some(Value::Int(_))));
        if !(head_slot_ok && bound_ok && matches!(code[h + 2], Op::Lt)) {
            continue;
        }
        if !matches!(code[h + 3], Op::JumpIfFalse(x) if x > e) {
            continue;
        }
        proven[k] = true;
    }
    proven
}

/// Push an SSA value + its kind onto the unboxed operand stack, which is realized as depth-indexed
/// Cranelift `Variable`s (`vars[depth]`): the value is stored with `def_var` (cranelift turns
/// within-block def/use into plain SSA and inserts phis at merges / loop back-edges), the kind is
/// tracked compile-time in `kinds` (whose length IS the current depth). Fails `Codegen` if the depth
/// exceeds the pre-declared `max_depth` (an abstract-interp miscount — the actual bug, never silent).
fn ub_push(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
    v: ClValue,
    k: Kind,
) -> Result<(), JitError> {
    let d = kinds.len();
    // Dual-space: a Float value lives in the F64 space (`fvars`) so a loop-carried float stays in an
    // XMM register across the back-edge — no per-iteration GPR↔XMM bitcast (the floatmul 4.5× root
    // cause, docs/plans/perf-wave.plan.md). Int/Bool/Unknown live in the I64 space (`vars`). The two
    // spaces share the depth index; `kinds` selects which is live at each depth (edge-consistency
    // enforced by `unboxed_analyze`, so a given depth is never both spaces at one program point).
    let space = if k == Kind::Float { fvars } else { vars };
    let var = *space.get(d).ok_or_else(|| {
        JitError::Codegen(format!(
            "unboxed: stack depth {d} exceeds max {}",
            space.len()
        ))
    })?;
    b.def_var(var, v);
    kinds.push(k);
    Ok(())
}

/// Pop the top of the depth-indexed operand stack, returning its SSA value (`use_var`) + tracked kind.
fn ub_pop(
    b: &mut FunctionBuilder,
    vars: &[Variable],
    fvars: &[Variable],
    kinds: &mut Vec<Kind>,
) -> Result<(ClValue, Kind), JitError> {
    let k = kinds
        .pop()
        .ok_or_else(|| JitError::Codegen("unboxed: operand stack underflow".to_string()))?;
    let d = kinds.len();
    // Dual-space (see `ub_push`): read from the space matching the popped entry's kind.
    let space = if k == Kind::Float { fvars } else { vars };
    Ok((b.use_var(space[d]), k))
}

/// Forward CFG pass computing the abstract operand-stack KINDS at every block leader for the unboxed
/// path, plus the maximum stack depth (for `Variable` pre-declaration). Mirrors codegen's per-op stack
/// effects EXACTLY (a `Call` pops the callee arity + pushes `Int`; `GetLocal(slot)` DUPs slot `slot`'s
/// kind; `SetLocal(slot)` writes it; comparisons/`Not` push `Bool`, arithmetic pushes `Int`).
///
/// This REPLACES the old "empty-at-leaders" invariant. Because a local is a frame-stack position (a
/// declaration leaves its initializer on the stack, no `SetLocal`), the stack is NOT empty at a leader
/// once any local is live; instead every edge into a leader must carry the SAME `(depth, kinds)`. The
/// pass records a leader's state on first arrival and ASSERTS a match on every later edge (the if/else
/// merge, the loop back-edge); a mismatch — or a stack underflow / write past the top — returns
/// `Unsupported` (VM fallback), never a miscompile. Only the compile-time kinds+depth are checked here;
/// the VALUES are carried by the depth-indexed Variables, whose phis Cranelift inserts on its own.
/// Per-ip abstract operand-stack KINDS at each block leader (`None` = not a leader / unreached).
type LeaderStates = Vec<Option<Vec<Kind>>>;

fn unboxed_analyze(
    program: &BytecodeProgram,
    func_idx: usize,
    param_kinds: &[Kind],
) -> Result<(LeaderStates, usize), JitError> {
    let code = &program.functions[func_idx].chunk.code;
    let n = code.len();
    let reach = reachable(code);
    let is_leader = leaders(code, &reach);

    let mut leader_state: LeaderStates = vec![None; n];
    let mut max_depth = param_kinds.len();
    if n == 0 {
        return Ok((leader_state, max_depth));
    }
    // ip 0 (the entry leader) starts with the params on the stack: slots 0..arity at the frame base.
    leader_state[0] = Some(param_kinds.to_vec());
    let mut work = vec![0usize];

    // Record/assert an edge carrying `out` into leader `target`.
    let propagate = |leader_state: &mut LeaderStates,
                     work: &mut Vec<usize>,
                     target: usize,
                     out: &[Kind]|
     -> Result<(), JitError> {
        match &leader_state[target] {
            None => {
                leader_state[target] = Some(out.to_vec());
                work.push(target);
            }
            Some(existing) if existing.as_slice() != out => {
                return Err(JitError::Unsupported(format!(
                    "unboxed: inconsistent operand stack at leader ip {target} ({existing:?} vs {out:?})"
                )));
            }
            Some(_) => {}
        }
        Ok(())
    };

    while let Some(l) = work.pop() {
        let mut kinds = leader_state[l]
            .clone()
            .expect("a queued leader always has a recorded state");
        let mut ip = l;
        loop {
            match &code[ip] {
                Op::Const(idx) => {
                    // Kind follows the const's type (Int vs Float) — MUST mirror build_body, which pushes
                    // Float for a float const so a downstream `AddF`/`Return` types correctly.
                    let k = match program.functions[func_idx].chunk.consts.get(*idx) {
                        Some(Value::Float(_)) => Kind::Float,
                        _ => Kind::Int,
                    };
                    kinds.push(k);
                }
                Op::AddI | Op::SubI | Op::MulI | Op::DivI | Op::RemI => {
                    kinds.pop();
                    kinds.pop();
                    kinds.push(Kind::Int);
                }
                Op::AddF | Op::SubF | Op::MulF | Op::DivF => {
                    kinds.pop();
                    kinds.pop();
                    kinds.push(Kind::Float);
                }
                Op::Neg => {
                    kinds.pop();
                    kinds.push(Kind::Int);
                }
                Op::Not => {
                    kinds.pop();
                    kinds.push(Kind::Bool);
                }
                Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                    kinds.pop();
                    kinds.pop();
                    kinds.push(Kind::Bool);
                }
                Op::GetLocal(slot) => {
                    let k = *kinds.get(*slot).ok_or_else(|| {
                        JitError::Codegen(format!(
                            "unboxed analyze: GetLocal slot {slot} underflow"
                        ))
                    })?;
                    kinds.push(k);
                }
                Op::SetLocal(slot) => {
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: SetLocal underflow".to_string())
                    })?;
                    if *slot >= kinds.len() {
                        return Err(JitError::Codegen(format!(
                            "unboxed analyze: SetLocal slot {slot} past top {}",
                            kinds.len()
                        )));
                    }
                    kinds[*slot] = k;
                }
                Op::Call(callee) => {
                    for _ in 0..program.functions[*callee].arity {
                        kinds.pop();
                    }
                    kinds.push(Kind::Int);
                }
                Op::Jump(t) => {
                    propagate(&mut leader_state, &mut work, *t, &kinds)?;
                    break;
                }
                Op::JumpIfFalse(t) => {
                    kinds.pop(); // the bool condition
                    propagate(&mut leader_state, &mut work, *t, &kinds)?;
                    propagate(&mut leader_state, &mut work, ip + 1, &kinds)?;
                    break;
                }
                Op::Return => {
                    break;
                }
                other => {
                    return Err(JitError::Unsupported(format!("unboxed analyze: {other:?}")));
                }
            }
            max_depth = max_depth.max(kinds.len());
            let next = ip + 1;
            if next >= n {
                break;
            }
            if is_leader[next] {
                propagate(&mut leader_state, &mut work, next, &kinds)?;
                break;
            }
            ip = next;
        }
    }
    Ok((leader_state, max_depth))
}

/// Collect the set of functions to compile for the UNBOXED path: the entry plus every function it
/// transitively (reachably) calls (via `Op::Call`), in discovery order. Enforces the unboxed op-subset
/// per function (default-deny): a closure capture, a non-int `Const`, a BACKWARD branch (a loop — a
/// temporary guard until the loops slice), or any op outside the subset makes the WHOLE compilation
/// `Unsupported` (so the caller falls back), because a native call needs its callee compiled in the
/// same module. Mutable locals — `GetLocal`/`SetLocal` of any slot, including declared locals `>= arity`
/// — ARE in the subset (a slot is a frame-stack position, realized as a depth-indexed Cranelift
/// Variable in `build_body_unboxed`). `Call` (self OR cross-function) is allowed — the whole reached
/// graph is collected. Only reachable ops are inspected. (The provably-int-`Return` check + the
/// operand-stack-shape validation stay in `unboxed_analyze`/`build_body_unboxed`; a non-int return or an
/// inconsistent-stack leader anywhere fails the build and thus the whole compile — the fixpoint's
/// "reject the whole graph if any function is ineligible".)
fn collect_functions_unboxed(
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
        // Float slice v1 is LEAF-only: the `Op::Call` arm models a callee's return as `Int`, so a float
        // value flowing through a call would mis-decode (a callee returning float, or a float arg). A
        // function that both touches floats AND calls is rejected (sound over-rejection; cross-function
        // float is a follow-up). Tracked per-function.
        let mut has_float = false;
        let mut has_call = false;
        for (ip, op) in code.iter().enumerate() {
            if !reach[ip] {
                continue;
            }
            match op {
                Op::Const(idx) => match func.chunk.consts.get(*idx) {
                    Some(Value::Int(_)) => {}
                    Some(Value::Float(_)) => has_float = true,
                    other => return Err(JitError::Unsupported(format!("unboxed Const {other:?}"))),
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
                | Op::Jump(_)
                | Op::JumpIfFalse(_)
                | Op::Return => {}
                // Float arith (v1): AddF/SubF/MulF/DivF. RemF is NOT included (no native Cranelift frem;
                // fmod libcall deferred) → default-denied by the `other` arm. Float COMPARISONS are
                // op-allowed above (Eq..Ge) but REJECTED at build time when the operands are float
                // (fcmp/NaN deferred) — a build-time fallback, sound.
                Op::AddF | Op::SubF | Op::MulF | Op::DivF => has_float = true,
                // Mutable locals: a read of any slot and a write (SetLocal) are both in the subset.
                // Slots are Cranelift Variables (widen-1 c1); their kind is proven by the analyze pass,
                // and a non-numeric-typed local reaching a `Return` fails the build (whole-graph fallback).
                Op::GetLocal(_) | Op::SetLocal(_) => {}
                Op::Call(callee) => {
                    has_call = true;
                    work.push(*callee);
                }
                other => return Err(JitError::Unsupported(format!("unboxed {other:?}"))),
            }
        }
        if has_float && has_call {
            return Err(JitError::Unsupported(
                "unboxed: float ops + Call in one function (v1 float subset is leaf-only)"
                    .to_string(),
            ));
        }
        order.push(fi);
    }
    Ok(order)
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
fn build_body_unboxed(
    module: &mut JITModule,
    cl_ctx: &mut cranelift::codegen::Context,
    program: &BytecodeProgram,
    func_idx: usize,
    func_ids: &[Option<FuncId>],
    proven: &[Option<Kind>],
    ret_kind_out: &mut Option<Kind>,
) -> Result<(), JitError> {
    let func = &program.functions[func_idx];
    let code = &func.chunk.code;
    let n = code.len();
    let reach = reachable(code);

    // Param slots read as `Int` iff proven int by usage (so a bare-param `Return`, e.g. fib's base case,
    // types correctly); otherwise `Unknown` → a bare return of one is rejected. These seed the entry
    // stack for the analysis, which then fixes every leader's (depth, kinds) and the max stack depth.
    let param_kinds: Vec<Kind> = (0..func.arity)
        .map(|s| proven.get(s).copied().flatten().unwrap_or(Kind::Unknown))
        .collect();
    let (leader_state, max_depth) = unboxed_analyze(program, func_idx, &param_kinds)?;

    // Range analysis (docs/plans/perf-wave.plan.md): `proven_ops[ip]` = an `AddI` that is a provably-
    // no-overflow induction-variable increment → emit a plain wrapping-free `iadd`, no sticky. From it:
    //   `needs_sticky` — is any reachable speculated overflow op (`AddI`/`SubI`/`MulI`/`Neg`) NOT proven?
    //     If NO, the speculation sticky flag + its back-edge/Return checks are dead → omit them entirely
    //     (Cranelift's baseline `opt_level=none` does NOT DCE the loop-carried sticky phi, so omitting is
    //     what actually turns a proven counted loop's PARITY into a WIN).
    //   `needs_fault_exit` — is there ANY path to the shared fault-exit (a sticky redo, OR a `DivI`/
    //     `RemI`/`Call` per-op fault branch)? If NO, don't create the block at all (an unreferenced,
    //     never-jumped-to block would be a dangling exit — avoid it).
    let proven_ops = range_proven_ops(func);
    let speculated = |op: &Op| matches!(op, Op::AddI | Op::SubI | Op::MulI | Op::Neg);
    let needs_sticky = code
        .iter()
        .enumerate()
        .any(|(ip, op)| reach[ip] && speculated(op) && !proven_ops[ip]);
    // `DivF` also branches to the fault-exit (a zero divisor → code 5), as do `DivI`/`RemI` (hardware
    // trap) and `Call` (depth guard + fault propagation) — every op that emits a `fault_if`/direct
    // `brif` to the shared exit must be counted here, or the block won't exist when it is needed.
    let needs_fault_exit = needs_sticky
        || code.iter().enumerate().any(|(ip, op)| {
            reach[ip] && matches!(op, Op::DivI | Op::RemI | Op::DivF | Op::Call(_))
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

    // Entry block: `[depth, a0, a1, …]`. `depth` is the live frame count at the call site (the caller
    // passes `depth + 1`; the top-level entry gets 1) — a `Call` checks `depth >= MAX_CALL_DEPTH`
    // BEFORE recursing to reproduce the VM's `"stack overflow"` at the exact threshold.
    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let entry_params: Vec<ClValue> = b.block_params(entry).to_vec();
    let depth = entry_params[0];
    let args: Vec<ClValue> = entry_params[1..].to_vec();
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
    let mut vars: Vec<Variable> = Vec::with_capacity(max_depth);
    let mut fvars: Vec<Variable> = Vec::with_capacity(max_depth);
    let i_zero = b.ins().iconst(types::I64, 0);
    let f_zero = b.ins().f64const(0.0);
    for s in 0..max_depth {
        let ivar = b.declare_var(types::I64);
        let fvar = b.declare_var(types::F64);
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
        vars.push(ivar);
        fvars.push(fvar);
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

    b.ins().jump(start, &[]);
    b.switch_to_block(start);
    let mut current: Option<Block> = Some(start);
    // Compile-time KIND stack; its length is the current stack depth. Reset from `leader_state` at every
    // block leader (the values are carried by the depth-indexed Variables). The entry block starts with
    // the params on the stack.
    let mut kinds: Vec<Kind> = param_kinds.clone();

    // Emit "if `flag` (i8, nonzero) then fault with `code` else continue in a fresh block". Only ever
    // called on a path that needs the fault-exit (div/rem/call/depth or a sticky redo), so `fault_exit`
    // is guaranteed `Some` here (`needs_fault_exit`).
    let fault_if = |b: &mut FunctionBuilder, flag: ClValue, code: i64| {
        let fx = fault_exit.expect("fault_if requires a fault-exit block (needs_fault_exit)");
        let cv = b.ins().iconst(types::I64, code);
        let cont = b.create_block();
        b.ins().brif(flag, fx, &[cv.into()], cont, &[]);
        b.switch_to_block(cont);
    };

    // ovf-spec: OR a boolean overflow `flag` (i8, 0/1 from `*_overflow` / an `is_min` compare) into the
    // sticky Variable — no branch, so the hot no-overflow path costs only the OR. Zero-extends to i64.
    // Only called for an UNPROVEN speculated op, so `sticky` is `Some` here (`needs_sticky`).
    let accumulate_sticky = |b: &mut FunctionBuilder, flag: ClValue| {
        let sv = sticky.expect("accumulate_sticky requires the sticky var (needs_sticky)");
        let cur = b.use_var(sv);
        let ext = b.ins().uextend(types::I64, flag);
        let next = b.ins().bor(cur, ext);
        b.def_var(sv, next);
    };

    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
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
                other => return Err(JitError::Unsupported(format!("unboxed Const {other:?}"))),
            },
            Op::AddI | Op::SubI | Op::MulI => {
                let (bv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if proven_ops[ip] {
                    // range-analysis: a provably-no-overflow induction increment (`AddI` only — see
                    // `range_proven_ops`). Emit a plain `iadd`: its two's-complement result equals
                    // `sadd_overflow`'s result[0] bit-for-bit (byte-identity ✓), but the overflow can
                    // never occur, so no carry to fold and no sticky. This drops the counter's guard.
                    let res = b.ins().iadd(av, bv);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
                } else {
                    // ovf-spec: WRAPPING result + OR the overflow carry into sticky — NO per-op branch (the
                    // per-op `*_overflow`+branch was the intadd perf loss). `sadd_overflow`'s result[0] IS
                    // the two's-complement wrapped value; push it, fold result[1] (the carry) into sticky.
                    let (res, overflow) = match op {
                        Op::AddI => b.ins().sadd_overflow(av, bv),
                        Op::SubI => b.ins().ssub_overflow(av, bv),
                        _ => b.ins().smul_overflow(av, bv),
                    };
                    accumulate_sticky(&mut b, overflow);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
                }
            }
            Op::DivI | Op::RemI => {
                let (bv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // ovf-spec: div/rem CANNOT be speculated — `sdiv`/`srem` hardware-trap (SIGFPE) on both
                // divide-by-zero AND i64::MIN / -1. So KEEP both as real per-op branches (rare → cheap),
                // but funnel them to code 5 (redo on VM) like every other fault; the VM redo renders the
                // exact div-zero / mod-zero / overflow string in correct order.
                let zero = b.ins().iconst(types::I64, 0);
                let is_zero = b.ins().icmp(IntCC::Equal, bv, zero);
                fault_if(&mut b, is_zero, 5);
                let imin = b.ins().iconst(types::I64, i64::MIN);
                let a_is_min = b.ins().icmp(IntCC::Equal, av, imin);
                let neg1 = b.ins().iconst(types::I64, -1);
                let b_is_neg1 = b.ins().icmp(IntCC::Equal, bv, neg1);
                let both = b.ins().band(a_is_min, b_is_neg1);
                fault_if(&mut b, both, 5);
                let res = if matches!(op, Op::DivI) {
                    b.ins().sdiv(av, bv)
                } else {
                    b.ins().srem(av, bv)
                };
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
            }
            Op::AddF | Op::SubF | Op::MulF => {
                // Dual-space float arith: operands arrive from the F64 space already as f64 (NO per-op
                // bitcast), op, push the f64 result to the F64 space. NO fault, NO sticky — IEEE arith is
                // total (overflow yields inf, not a fault), matching value::float_{add,sub,mul}. Same ops
                // in the same order ⇒ bit-identical to the VM oracle (Invariant #1). (`RemF` is NOT in the
                // subset: Cranelift has no native frem — fmod libcall deferred; `collect_functions_unboxed`
                // default-denies it, so it never reaches here.)
                let (bf, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (af, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let rf = match op {
                    Op::AddF => b.ins().fadd(af, bf),
                    Op::SubF => b.ins().fsub(af, bf),
                    _ => b.ins().fmul(af, bf),
                };
                ub_push(&mut b, &vars, &fvars, &mut kinds, rf, Kind::Float)?;
            }
            Op::DivF => {
                // Float division: a ZERO divisor faults (value::float_div: `b == 0.0`, incl. -0.0) — no
                // hardware trap, but a semantic fault → branch to code 5 (redo on VM renders FAULT_DIV_
                // ZERO). `fcmp Equal` is false for NaN, so a NaN/inf divisor does NOT fault → fdiv yields
                // NaN/inf, matching float_div's `Ok(a / b)`.
                let (bf, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (af, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let zero = b.ins().f64const(0.0);
                let is_zero = b.ins().fcmp(FloatCC::Equal, bf, zero);
                fault_if(&mut b, is_zero, 5);
                let rf = b.ins().fdiv(af, bf);
                ub_push(&mut b, &vars, &fvars, &mut kinds, rf, Kind::Float)?;
            }
            Op::Neg => {
                let (av, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // ovf-spec: -i64::MIN overflows on the VM, but `ineg` does NOT hardware-trap (it wraps
                // MIN→MIN) — unlike div — so we speculate: fold `av == MIN` into sticky (no branch) and
                // emit the wrapping `ineg`. A set sticky forces the redo at the next back-edge / Return.
                let imin = b.ins().iconst(types::I64, i64::MIN);
                let is_min = b.ins().icmp(IntCC::Equal, av, imin);
                accumulate_sticky(&mut b, is_min);
                let res = b.ins().ineg(av);
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
            }
            Op::Not => {
                let (av, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let r = b.ins().icmp_imm(IntCC::Equal, av, 0); // 1 iff false
                let r64 = b.ins().uextend(types::I64, r);
                ub_push(&mut b, &vars, &fvars, &mut kinds, r64, Kind::Bool)?;
            }
            Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                let (bv, bk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, ak) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // `icmp` is only correct on integer bit-patterns. Reject unless BOTH operands are safely
                // non-float. A known `Float` → reject. An `Unknown` operand is AMBIGUOUS (a float param
                // used only in comparisons is never proven Float — the trap this guards): require the
                // OTHER operand to be a KNOWN non-float (Int/Bool); the checker's homogeneous-comparison
                // rule then guarantees the Unknown is the same non-float type. Both-Unknown → reject (VM
                // fallback). Float comparisons (fcmp/NaN) are deferred to a later slice (INVARIANTS #13).
                let known_nonfloat = |k: Kind| matches!(k, Kind::Int | Kind::Bool);
                if ak == Kind::Float
                    || bk == Kind::Float
                    || !(known_nonfloat(ak) || known_nonfloat(bk))
                {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: float/ambiguous comparison operands ({ak:?}, {bk:?}) — deferred"
                    )));
                }
                let cc = match op {
                    Op::Eq => IntCC::Equal,
                    Op::Ne => IntCC::NotEqual,
                    Op::Lt => IntCC::SignedLessThan,
                    Op::Gt => IntCC::SignedGreaterThan,
                    Op::Le => IntCC::SignedLessThanOrEqual,
                    _ => IntCC::SignedGreaterThanOrEqual,
                };
                let r = b.ins().icmp(cc, av, bv);
                let r64 = b.ins().uextend(types::I64, r);
                ub_push(&mut b, &vars, &fvars, &mut kinds, r64, Kind::Bool)?;
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
                ub_push(&mut b, &vars, &fvars, &mut kinds, v, kind)?;
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
                b.def_var(var, v);
                kinds[*slot] = k;
            }
            Op::Call(callee) => {
                // Self OR cross-function call. Reproduce the VM's pre-push depth guard, then a direct
                // native call to the callee's FuncId, passing `depth + 1` + the callee's `arity` args
                // already on the operand stack; propagate the callee's `(value, code)`.
                let callee_ref = fn_refs[*callee].ok_or_else(|| {
                    JitError::Codegen(format!("unboxed: call to uncompiled fn {callee}"))
                })?;
                let dmax = b.ins().iconst(types::I64, MAX_CALL_DEPTH as i64);
                let too_deep = b.ins().icmp(IntCC::SignedGreaterThanOrEqual, depth, dmax);
                fault_if(&mut b, too_deep, 5); // ovf-spec: stack-overflow → redo on VM (code 5)
                let d1 = b.ins().iadd_imm(depth, 1);
                // Pop the CALLEE's `arity` args (top is the last arg); rebuild in declaration order.
                let arity = program.functions[*callee].arity;
                let mut cargs: Vec<ClValue> = Vec::with_capacity(arity);
                for _ in 0..arity {
                    let (v, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                    cargs.push(v);
                }
                cargs.reverse();
                let mut call_args: Vec<ClValue> = Vec::with_capacity(arity + 1);
                call_args.push(d1);
                call_args.extend(cargs);
                let call = b.ins().call(callee_ref, &call_args);
                let results = b.inst_results(call);
                let (value, ccode) = (results[0], results[1]);
                // ovf-spec: a callee (also ovf-spec) returns code 0 or 5; code != 0 ⇒ propagate 5 to the
                // shared fault-exit → this whole graph redoes on the VM.
                let is_fault = b.ins().icmp_imm(IntCC::NotEqual, ccode, 0);
                let cont = b.create_block();
                // A `Call` is in the `needs_fault_exit` set, so `fault_exit` is `Some` here.
                let fx = fault_exit.expect("Call requires a fault-exit block (needs_fault_exit)");
                b.ins().brif(is_fault, fx, &[ccode.into()], cont, &[]);
                b.switch_to_block(cont);
                ub_push(&mut b, &vars, &fvars, &mut kinds, value, Kind::Int)?;
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
                        fault_if(&mut b, s, 5);
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
                        fault_if(&mut b, s, 5);
                    }
                }
                // cond nonzero (true) → fall through; zero (false) → take the jump.
                b.ins().brif(cond, fallb, &[], tb, &[]);
                current = None;
            }
            Op::Return => {
                let (v, kind) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if kind != Kind::Int && kind != Kind::Float {
                    // A bool/unknown return would be mis-decoded — reject to VM/boxed.
                    return Err(JitError::Unsupported(format!(
                        "unboxed: non-numeric return (kind {kind:?})"
                    )));
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

/// True iff `entry_idx` and every function it transitively (reachably) calls are in the JIT subset —
/// the cheap predicate a caller checks before committing to [`Compiled::compile`] (it runs the same
/// default-deny walk without building any code).
///
/// **INVARIANT — the whole speculative-execution model rests on this: every JIT-eligible op is
/// side-effect-free** (no output, no shared-state mutation). That is what makes the `phg run` fallback
/// sound: on a JIT fault (or an under-fault the VM would catch) the function is re-executed on the VM,
/// which would DOUBLE any side effect the JIT had already performed. Never add an op with observable
/// effects (a print, a global/field write, an allocation the caller can observe) to the subset in
/// `collect_functions` without redesigning the fallback contract.
pub fn is_eligible(program: &BytecodeProgram, entry_idx: usize) -> bool {
    collect_functions(program, entry_idx).is_ok()
}

/// A JIT-compiled function graph: the `entry` plus every function it transitively calls, all defined
/// and finalized in one [`JITModule`]. Separating *compile* from *run* is the seam the honest
/// benchmark (compile once, time many native runs) and the future `phg run` hot-function cache both
/// need — recompiling per call would dwarf the native speed the JIT exists to deliver.
pub struct Compiled {
    /// `Option` only so [`Drop`] can `take()` the module and hand it to `free_memory(self)`, which
    /// consumes it. Always `Some` between `compile` and drop.
    module: Option<JITModule>,
    /// The finalized entry code. It lives at a fixed address inside the module's executable mmap (NOT
    /// inside the `JITModule` struct), so moving the struct into this handle leaves the pointer valid;
    /// it stays valid for as long as `module` is alive (i.e. until this handle drops).
    entry: *const u8,
    /// Which codegen produced `entry`, selecting the run ABI: `false` = boxed ([`Compiled::run`],
    /// `fn(*mut JitCtx, i64)`); `true` = unboxed ([`Compiled::run_unboxed`], `fn(*mut i64, i64…)`).
    unboxed: bool,
    /// The entry's arity — needed only by the unboxed ABI (its args are native `i64` params, so the
    /// call site transmutes to the arity-specific function type). Unused for the boxed ABI.
    arity: usize,
    /// The entry's return kind (unboxed ABI only): `Int` → decode the returned i64 as `Value::Int`,
    /// `Float` → `Value::Float(f64::from_bits)`. Floats travel as their bits through the uniform i64
    /// ABI, so this is the sole signal telling `run_unboxed` how to decode. Ignored for the boxed ABI
    /// (which decodes via the boxed `Value` stack). Always `Int`/`Float` for unboxed (asserted at build).
    ret_kind: Kind,
}

impl Compiled {
    /// JIT-compile `entry_idx` and its transitive (reachable) call graph. Returns
    /// [`JitError::Unsupported`] if any function in that set contains an op / const / closure capture
    /// outside the int + control-flow + direct-call subset — the default-deny contract that keeps
    /// callers falling back to the VM.
    pub fn compile(program: &BytecodeProgram, entry_idx: usize) -> Result<Compiled, JitError> {
        // --- transitive eligibility + the set of functions to compile (default-deny, reachable-only) ---
        let order = collect_functions(program, entry_idx)?;

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
        builder.symbol("rt_depth_check", rt_depth_check as *const u8);
        builder.symbol("rt_frame_base", rt_frame_base as *const u8);
        builder.symbol("rt_return", rt_return as *const u8);
        let mut module = JITModule::new(builder);
        let ptr = module.target_config().pointer_type();

        // --- declare the imported bridge helpers ---
        let sig_push_int = make_sig(&module, &[ptr, types::I64], None); // rt_push_int
        let sig_void = make_sig(&module, &[ptr], None); // rt_push_unit
        let sig_local = make_sig(&module, &[ptr, types::I64, types::I64], Some(types::I64)); // get/set_local
        let sig_code = make_sig(&module, &[ptr, types::I64], Some(types::I64)); // arith/cmp/eqne/frame_base/ret
        let sig_status = make_sig(&module, &[ptr], Some(types::I64)); // neg/not/jump_if_false/depth_check
        let declare = |m: &mut JITModule, name: &str, sig: &Signature| {
            m.declare_function(name, Linkage::Import, sig)
                .map_err(|e| JitError::Codegen(format!("declare {name}: {e}")))
        };
        let helpers = Helpers {
            push_int: declare(&mut module, "rt_push_int", &sig_push_int)?,
            push_unit: declare(&mut module, "rt_push_unit", &sig_void)?,
            get_local: declare(&mut module, "rt_get_local", &sig_local)?,
            set_local: declare(&mut module, "rt_set_local", &sig_local)?,
            arith: declare(&mut module, "rt_arith", &sig_code)?,
            neg: declare(&mut module, "rt_neg", &sig_status)?,
            not: declare(&mut module, "rt_not", &sig_status)?,
            eqne: declare(&mut module, "rt_eqne", &sig_code)?,
            cmp: declare(&mut module, "rt_cmp", &sig_code)?,
            jif: declare(&mut module, "rt_jump_if_false", &sig_status)?,
            depth_check: declare(&mut module, "rt_depth_check", &sig_status)?,
            frame_base: declare(&mut module, "rt_frame_base", &sig_code)?,
            ret: declare(&mut module, "rt_return", &sig_code)?,
        };

        // --- declare a FuncId per phorj function (so bodies can cross-reference, incl. self) ---
        // Every compiled function has the signature `extern "C" fn(*mut JitCtx, slot_base: i64) -> i64`.
        let mut phorj_sig = module.make_signature();
        phorj_sig.params.push(AbiParam::new(ptr));
        phorj_sig.params.push(AbiParam::new(types::I64));
        phorj_sig.returns.push(AbiParam::new(types::I64));
        let mut func_ids: Vec<Option<FuncId>> = vec![None; program.functions.len()];
        for &fi in &order {
            let id = module
                .declare_function(&format!("phorj_fn_{fi}"), Linkage::Export, &phorj_sig)
                .map_err(|e| JitError::Codegen(format!("declare fn {fi}: {e}")))?;
            func_ids[fi] = Some(id);
        }

        // --- define every body ---
        for &fi in &order {
            let mut cl_ctx = module.make_context();
            cl_ctx.func.signature = phorj_sig.clone();
            build_body(&mut module, &mut cl_ctx, program, fi, &func_ids, &helpers)?;
            module
                .define_function(func_ids[fi].expect("declared above"), &mut cl_ctx)
                .map_err(|e| JitError::Codegen(format!("define fn {fi}: {e}")))?;
            module.clear_context(&mut cl_ctx);
        }
        module
            .finalize_definitions()
            .map_err(|e| JitError::Codegen(format!("finalize: {e}")))?;
        let entry =
            module.get_finalized_function(func_ids[entry_idx].expect("entry declared above"));

        Ok(Compiled {
            module: Some(module),
            entry,
            unboxed: false,
            arity: 0,
            ret_kind: Kind::Int, // unused by the boxed `run()` (decodes via the boxed Value stack)
        })
    }

    /// JIT-compile `entry_idx` (+ its transitive call graph) with the UNBOXED codegen (slice u2b): int
    /// functions that may be self- OR cross-recursive (no `SetLocal`, no local decls) whose every
    /// reachable `Return` yields a provably-`Int` operand (a param proven int by usage, an arithmetic
    /// result, or a call result). Returns [`JitError::Unsupported`] if any function in the reached graph
    /// is out-of-subset or has a non-int return (the whole graph falls back to the VM / boxed path). No
    /// `rt_*` helpers are registered: unboxed code is pure register arithmetic + native calls with
    /// inline fault checks; faults travel in the `(value, code)` multi-return, mapped to the
    /// single-sourced kernel strings in [`Compiled::run_unboxed`].
    pub fn compile_unboxed(
        program: &BytecodeProgram,
        entry_idx: usize,
    ) -> Result<Compiled, JitError> {
        // Transitive op-subset eligibility + the set of functions to compile (reachable-only).
        let order = collect_functions_unboxed(program, entry_idx)?;

        let builder = JITBuilder::new(default_libcall_names())
            .map_err(|e| JitError::Codegen(format!("JITBuilder: {e}")))?;
        let mut module = JITModule::new(builder);

        // Declare a FuncId per function: `extern "C" fn(depth: i64, a0..a_arity: i64) -> (i64, i64)`.
        // Per-function arity, so each has its own signature (declared BEFORE any body so calls — self
        // or cross — resolve at finalize).
        let mut func_ids: Vec<Option<FuncId>> = vec![None; program.functions.len()];
        for &fi in &order {
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(types::I64)); // depth
            for _ in 0..program.functions[fi].arity {
                sig.params.push(AbiParam::new(types::I64));
            }
            sig.returns.push(AbiParam::new(types::I64)); // value
            sig.returns.push(AbiParam::new(types::I64)); // fault code (0 = ok)
            let id = module
                .declare_function(&format!("phorj_unboxed_fn_{fi}"), Linkage::Export, &sig)
                .map_err(|e| JitError::Codegen(format!("declare unboxed fn {fi}: {e}")))?;
            func_ids[fi] = Some(id);
        }

        // Define every body. A non-numeric `Return` (the provably-Int/Float check in build_body) fails
        // the whole compile here — the fixpoint's "reject the whole graph if any function is ineligible".
        // Capture the ENTRY's return kind for `run_unboxed`'s Int-vs-Float decode.
        let mut entry_ret_kind: Option<Kind> = None;
        for &fi in &order {
            let proven = unboxed_proven_param_kinds(&program.functions[fi]);
            let mut ret_kind: Option<Kind> = None;
            let mut cl_ctx = module.make_context();
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            for _ in 0..program.functions[fi].arity {
                sig.params.push(AbiParam::new(types::I64));
            }
            sig.returns.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            cl_ctx.func.signature = sig;
            build_body_unboxed(
                &mut module,
                &mut cl_ctx,
                program,
                fi,
                &func_ids,
                &proven,
                &mut ret_kind,
            )?;
            module
                .define_function(func_ids[fi].expect("declared above"), &mut cl_ctx)
                .map_err(|e| JitError::Codegen(format!("define unboxed fn {fi}: {e}")))?;
            module.clear_context(&mut cl_ctx);
            if fi == entry_idx {
                entry_ret_kind = ret_kind;
            }
        }
        module
            .finalize_definitions()
            .map_err(|e| JitError::Codegen(format!("finalize unboxed: {e}")))?;
        let entry =
            module.get_finalized_function(func_ids[entry_idx].expect("entry declared above"));

        Ok(Compiled {
            module: Some(module),
            entry,
            unboxed: true,
            arity: program.functions[entry_idx].arity,
            // Every eligible function has ≥1 reachable Return (else no value is produced), so the entry's
            // kind is always set; default to Int defensively.
            ret_kind: entry_ret_kind.unwrap_or(Kind::Int),
        })
    }

    /// Run the compiled entry with `args`, seeding the operand stack as its slots `0..arity` at
    /// `slot_base = 0`. `start_depth` seeds the frame-depth counter that produces the `"stack
    /// overflow"` fault: it MUST equal the number of live frames at the invocation site so the fault
    /// fires at the VM's exact threshold. A top-level entry (tests / benchmark / `run_entry` parity)
    /// passes `start_depth = 1` (the VM's single entry frame); a mid-execution `phg run` hook (b3b)
    /// passes the VM's live `frames.len()`, so an eligible function reached at VM-depth D faults after
    /// `MAX_CALL_DEPTH - D` more frames — NOT `MAX_CALL_DEPTH`, which would under-fault (return a value
    /// where the VM faults, a happy-path disagreement the caller's fault-fallback cannot catch).
    pub fn run(&self, args: &[Value], start_depth: usize) -> JitRun {
        debug_assert!(
            !self.unboxed,
            "run() is the boxed ABI; use run_unboxed() for unboxed code"
        );
        // SAFETY: `self.entry` is the finalized machine code for a function compiled with exactly the
        // signature `extern "C" fn(*mut JitCtx, i64) -> i64` — the sole first-party `unsafe` this whole
        // effort exists to confine. `self.module` (which owns the executable memory) is alive for the
        // duration of the call (this handle is not dropped until after `run` returns). Every native
        // callee reached through it shares that same signature + the one `ctx` pointer.
        let entry: extern "C" fn(*mut JitCtx, i64) -> i64 = unsafe {
            std::mem::transmute::<*const u8, extern "C" fn(*mut JitCtx, i64) -> i64>(self.entry)
        };
        let mut call_ctx = JitCtx {
            stack: args.to_vec(),
            depth: start_depth,
            fault: None,
        };
        let status = entry(&mut call_ctx, 0);
        if status == 0 {
            // The entry's `rt_return` truncated to slot_base 0 and pushed the return value, so it is the
            // sole remaining stack element.
            JitRun::Value(call_ctx.stack.pop().unwrap_or(Value::Unit))
        } else {
            JitRun::Fault(
                call_ctx
                    .fault
                    .unwrap_or_else(|| "jit: unknown fault".to_string()),
            )
        }
    }

    /// Run an UNBOXED-compiled entry (from [`Compiled::compile_unboxed`]). The ABI is
    /// `extern "C" fn(depth: i64, a0…: i64) -> (i64 value, i64 code)`; args are passed as native `i64`
    /// (a bool arg is its `0/1`). On `code == 0` the returned `i64` is the (int) value; otherwise the
    /// code maps to the single-sourced `value::FAULT_*` string (or `"stack overflow"`, code 4) —
    /// byte-identical to the VM.
    ///
    /// `start_depth` seeds the frame-depth counter producing the `"stack overflow"` fault — the SAME
    /// contract as [`Compiled::run`]: a top-level entry (tests / benchmark / parity) passes `1` (the
    /// VM's single entry frame); a mid-execution `phg run` hook (b3b) passes `frames.len() + 1` (the
    /// caller frames still live, plus this not-yet-pushed callee), so an eligible function reached at
    /// VM-depth D faults after `MAX_CALL_DEPTH - D` more frames — NOT `MAX_CALL_DEPTH`, which would
    /// UNDER-fault (return a value where the VM faults — the one happy-path divergence the caller's
    /// fault-fallback cannot catch, because there is no fault to fall back on).
    pub fn run_unboxed(&self, args: &[Value], start_depth: usize) -> JitRun {
        debug_assert!(
            self.unboxed,
            "run_unboxed() requires unboxed code; use run()"
        );
        // The `#[repr(C)]` two-i64 return matching Cranelift's `returns = [i64, i64]`: on SysV
        // x86-64 both come back in rax:rdx, and a C struct of two eightbytes returns the same way (on
        // AArch64, x0:x1 likewise). The unit tests assert value AND fault against the VM oracle, so an
        // ABI mismatch would surface loudly rather than silently corrupt.
        #[repr(C)]
        struct UnboxedRet {
            value: i64,
            code: i64,
        }
        // Bool args are represented as 0/1 i64 (see `Kind` — bool params are only consumed in bool
        // contexts natively). A non-int/bool arg can't reach an eligible unboxed function.
        let ia: Vec<i64> = args
            .iter()
            .map(|v| match v {
                Value::Int(n) => *n,
                Value::Bool(b) => *b as i64,
                // A float arg travels as its f64 BITS through the uniform i64 ABI (decoded back at the
                // callee's float ops via bitcast). Matches the `Kind::Float` bits-in-I64 representation.
                Value::Float(f) => f.to_bits() as i64,
                _ => 0,
            })
            .collect();
        let d0: i64 = start_depth as i64; // live-frames-including-this-entry (see doc above)
                                          // SAFETY: `self.entry` is finalized machine code with signature
                                          // `extern "C" fn(i64 depth, i64… /* arity */) -> (i64, i64)`; we transmute to the arity-specific
                                          // type and pass depth + exactly `arity` i64 args. `self.module` owns the code, alive across the
                                          // call.
        let ret: UnboxedRet = unsafe {
            match self.arity {
                0 => {
                    let f: extern "C" fn(i64) -> UnboxedRet = std::mem::transmute(self.entry);
                    f(d0)
                }
                1 => {
                    let f: extern "C" fn(i64, i64) -> UnboxedRet = std::mem::transmute(self.entry);
                    f(d0, ia[0])
                }
                2 => {
                    let f: extern "C" fn(i64, i64, i64) -> UnboxedRet =
                        std::mem::transmute(self.entry);
                    f(d0, ia[0], ia[1])
                }
                3 => {
                    let f: extern "C" fn(i64, i64, i64, i64) -> UnboxedRet =
                        std::mem::transmute(self.entry);
                    f(d0, ia[0], ia[1], ia[2])
                }
                other => {
                    return JitRun::Fault(format!("jit: unboxed arity {other} unsupported"));
                }
            }
        };
        match ret.code {
            // Decode the returned i64 by the entry's return kind: Int verbatim, Float from its bits.
            0 => match self.ret_kind {
                Kind::Float => JitRun::Value(Value::Float(f64::from_bits(ret.value as u64))),
                _ => JitRun::Value(Value::Int(ret.value)),
            },
            // ovf-spec: EVERY unboxed fault now funnels to code 5 = "redo on VM" (codes 1/2/3/4 are no
            // longer emitted). The hook re-executes the callee on the VM, which renders the exact,
            // correctly-ordered fault string + source line. See [`REDO_ON_VM`].
            5 => JitRun::Fault(REDO_ON_VM.to_string()),
            other => JitRun::Fault(format!("jit: unboxed unknown fault code {other}")),
        }
    }
}

impl Drop for Compiled {
    fn drop(&mut self) {
        // `JITModule` has NO `Drop` impl (verified against cranelift-jit 0.133 `src/backend.rs`) —
        // merely dropping it LEAKS the code mmap; memory is reclaimed only by the explicit
        // `free_memory`, which consumes the module by value (hence the `Option::take`).
        if let Some(module) = self.module.take() {
            // SAFETY: this handle is being destroyed, so no `run` is in progress (each `run` borrows
            // `&self` and returns before drop) and `self.entry` is never used again. That satisfies
            // `free_memory`'s contract: no compiled function executing, no function pointer called
            // afterward.
            unsafe { module.free_memory() };
        }
    }
}

/// Compile the function at `entry_idx` (+ its transitive call graph) and run it once with `args`. A
/// convenience over [`Compiled::compile`] + [`Compiled::run`] for the common single-shot case (the
/// unit tests); the compiled module is freed when the temporary [`Compiled`] drops. `start_depth` is
/// 1 — a top-level entry, matching the VM's single entry frame.
pub fn compile_and_run(
    program: &BytecodeProgram,
    entry_idx: usize,
    args: &[Value],
) -> Result<JitRun, JitError> {
    Ok(Compiled::compile(program, entry_idx)?.run(args, 1))
}

#[cfg(test)]
mod tests;
