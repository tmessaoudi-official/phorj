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

mod analyze;
mod boxed;
mod compile;
mod emit_unboxed;
mod handles;

pub use self::compile::Compiled;

use self::analyze::*;
use self::boxed::*;
use self::emit_unboxed::*;
use self::handles::*;

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
