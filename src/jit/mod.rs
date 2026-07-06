//! # JIT backend (Cranelift) — codegen slice 1 (JIT-1)
//!
//! **Status: MINIMAL CODEGEN — pure-int leaf functions, not yet wired into `phg run`.** This is the
//! first codegen slice of the ruled Cranelift path to the G-8 perf mandate (the bytecode VM is ~28×
//! slower than release-php+JIT on hot numeric loops; only native codegen closes that — see
//! `docs/plans/perf-wave.plan.md` and `docs/research/jit-aot-design-exploration.md`). It lowers a
//! **restricted, default-deny subset** of a compiled [`Function`](crate::chunk::Function)'s bytecode
//! — integer `Const`/`GetLocal`/`AddI`/`SubI`/`MulI`/`DivI`/`RemI`/`Return`, straight-line — to native
//! machine code, then runs it via the `finalize → transmute → call` path. Anything outside the subset
//! is rejected with [`JitError::Unsupported`] (the seed of the eligibility predicate the *wiring*
//! slice will formalize; control-flow branches/loops and the `phg run` cutover land there).
//!
//! ## Boxed-`Value`-via-kernels (the locked order: boxed first, unboxing last)
//!
//! Codegen never reimplements arithmetic. It threads **pointers to boxed [`Value`]s** and, for every
//! operation, `call`s a runtime bridge helper (`rt_*` below) that dispatches into the single-sourced
//! [`crate::value`] kernels (`int_add`, `int_mul`, …). So checked-overflow faults and their canonical
//! strings are **byte-identical to the VM by construction** (Invariant 4) — not re-derived. Unboxing
//! statically-int hot paths to raw `i64` is the deferred bonus (JIT-5), not this slice.
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

/// Why a function could not be JIT-run. Not a runtime fault (that is [`JitRun::Fault`]) — this means
/// the JIT declined or failed to *build* native code for the function.
#[derive(Debug)]
pub enum JitError {
    /// The function contains an `Op` (or a `Const` type) outside this slice's supported subset. The
    /// default-deny stance: anything not explicitly lowered is rejected, so callers fall back to the
    /// VM. Carries a human label of the offending shape.
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
/// dereferences it; only the `rt_*` bridge helpers do). Holds the argument slice, an arena keeping
/// every produced [`Value`] alive for the duration of the call, and the out-channels for the result
/// and a fault.
struct JitCtx {
    /// The caller's argument slice; `rt_get_local` reads `args[n]`. Valid for the whole call — the
    /// caller ([`compile_and_run`]) owns the backing `&[Value]`.
    args: *const Value,
    /// Every `Value` a helper produces is boxed and parked here so the raw pointer the helper returns
    /// stays valid until the call ends. `Box` (not a bare `Vec<Value>`) keeps each allocation stable
    /// across pushes — a `Vec<Value>` would reallocate its buffer and dangle every outstanding
    /// `*mut Value` the JIT is threading. This is exactly what makes `clippy::vec_box`'s "boxing is
    /// unnecessary" advice wrong here: the indirection is load-bearing for pointer stability.
    #[allow(clippy::vec_box)]
    arena: Vec<Box<Value>>,
    /// `rt_ret` writes the entry function's return value here.
    result: Value,
    /// A helper sets this and returns null on a clean runtime fault; the compiled code then branches
    /// to its fault-exit block, which returns status 1.
    fault: Option<String>,
}

/// Box `v`, park it in the arena, and return a stable raw pointer to it.
fn push_value(ctx: &mut JitCtx, v: Value) -> *mut Value {
    let mut boxed = Box::new(v);
    let ptr: *mut Value = &mut *boxed;
    ctx.arena.push(boxed);
    ptr
}

/// Push `Value::Int(n)` and return a pointer to it. Bridge for `Op::Const` of an int literal.
extern "C" fn rt_int(ctx: *mut JitCtx, n: i64) -> *mut Value {
    // SAFETY: `ctx` is the live `&mut JitCtx` the caller passed as the sole function argument; the
    // compiled code only ever passes that one pointer, unchanged, to every helper.
    let ctx = unsafe { &mut *ctx };
    push_value(ctx, Value::Int(n))
}

/// Clone `args[n]` into the arena and return a pointer to the clone. Bridge for `Op::GetLocal`.
extern "C" fn rt_get_local(ctx: *mut JitCtx, n: i64) -> *mut Value {
    // SAFETY: `ctx` is live (see `rt_int`); `args` points at the caller's slice, whose length the
    // eligibility subset guarantees exceeds every local index the compiled body reads.
    let ctx = unsafe { &mut *ctx };
    let v = unsafe { &*ctx.args.add(n as usize) }.clone();
    push_value(ctx, v)
}

/// Shared arithmetic bridge: dispatch two int operands through a single-sourced `value` kernel. On a
/// kernel fault, record its canonical string and return null (the compiled code branches to fault
/// exit). Byte-identity with the VM is guaranteed because `kernel` IS the VM's kernel.
fn arith(
    ctx: *mut JitCtx,
    a: *const Value,
    b: *const Value,
    kernel: fn(i64, i64) -> Result<i64, String>,
) -> *mut Value {
    // SAFETY: `ctx` is live; `a`/`b` are pointers this JIT threaded from `rt_int`/`rt_get_local`/a
    // prior `arith` result — all into `ctx.arena` (stable boxes) or the live `args` slice.
    let ctx = unsafe { &mut *ctx };
    let (av, bv) = unsafe { (&*a, &*b) };
    match (av, bv) {
        (Value::Int(x), Value::Int(y)) => match kernel(*x, *y) {
            Ok(r) => push_value(ctx, Value::Int(r)),
            Err(msg) => {
                ctx.fault = Some(msg);
                std::ptr::null_mut()
            }
        },
        // Unreachable for an eligible (int-typed) function — the checker guarantees int operands and
        // the subset only admits int-producing ops. Defensive: fault rather than misbehave.
        _ => {
            ctx.fault = Some("jit: non-int arithmetic operand".to_string());
            std::ptr::null_mut()
        }
    }
}

extern "C" fn rt_add(ctx: *mut JitCtx, a: *const Value, b: *const Value) -> *mut Value {
    arith(ctx, a, b, crate::value::int_add)
}
extern "C" fn rt_sub(ctx: *mut JitCtx, a: *const Value, b: *const Value) -> *mut Value {
    arith(ctx, a, b, crate::value::int_sub)
}
extern "C" fn rt_mul(ctx: *mut JitCtx, a: *const Value, b: *const Value) -> *mut Value {
    arith(ctx, a, b, crate::value::int_mul)
}
extern "C" fn rt_div(ctx: *mut JitCtx, a: *const Value, b: *const Value) -> *mut Value {
    arith(ctx, a, b, crate::value::int_div)
}
extern "C" fn rt_rem(ctx: *mut JitCtx, a: *const Value, b: *const Value) -> *mut Value {
    arith(ctx, a, b, crate::value::int_rem)
}

/// Store the function's return value into the context. Bridge for `Op::Return`.
extern "C" fn rt_ret(ctx: *mut JitCtx, v: *const Value) {
    // SAFETY: `ctx` is live; `v` is a pointer into `ctx.arena`/`args` (see `arith`).
    let ctx = unsafe { &mut *ctx };
    ctx.result = unsafe { &*v }.clone();
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

/// JIT-compile the function at `func_idx` in `program` (if its body is in the supported subset) and
/// run it with `args`, returning its return value or a clean runtime fault.
///
/// Returns [`JitError::Unsupported`] for any op/const outside the int-arith leaf subset — the
/// default-deny contract that keeps callers falling back to the VM for everything else.
pub fn compile_and_run(
    program: &BytecodeProgram,
    func_idx: usize,
    args: &[Value],
) -> Result<JitRun, JitError> {
    // --- module + host ISA, with the bridge helpers registered as symbols ---
    let mut builder = JITBuilder::new(default_libcall_names())
        .map_err(|e| JitError::Codegen(format!("JITBuilder: {e}")))?;
    builder.symbol("rt_int", rt_int as *const u8);
    builder.symbol("rt_get_local", rt_get_local as *const u8);
    builder.symbol("rt_add", rt_add as *const u8);
    builder.symbol("rt_sub", rt_sub as *const u8);
    builder.symbol("rt_mul", rt_mul as *const u8);
    builder.symbol("rt_div", rt_div as *const u8);
    builder.symbol("rt_rem", rt_rem as *const u8);
    builder.symbol("rt_ret", rt_ret as *const u8);
    let mut module = JITModule::new(builder);
    let ptr = module.target_config().pointer_type();

    // --- declare the imported bridge helpers ---
    let sig_unary = make_sig(&module, &[ptr, types::I64], Some(ptr)); // rt_int, rt_get_local
    let sig_bin = make_sig(&module, &[ptr, ptr, ptr], Some(ptr)); // rt_add/sub/mul/div/rem
    let sig_ret = make_sig(&module, &[ptr, ptr], None); // rt_ret
    let declare = |m: &mut JITModule, name: &str, sig: &Signature| {
        m.declare_function(name, Linkage::Import, sig)
            .map_err(|e| JitError::Codegen(format!("declare {name}: {e}")))
    };
    let rt_int_id = declare(&mut module, "rt_int", &sig_unary)?;
    let rt_getlocal_id = declare(&mut module, "rt_get_local", &sig_unary)?;
    let rt_add_id = declare(&mut module, "rt_add", &sig_bin)?;
    let rt_sub_id = declare(&mut module, "rt_sub", &sig_bin)?;
    let rt_mul_id = declare(&mut module, "rt_mul", &sig_bin)?;
    let rt_div_id = declare(&mut module, "rt_div", &sig_bin)?;
    let rt_rem_id = declare(&mut module, "rt_rem", &sig_bin)?;
    let rt_ret_id = declare(&mut module, "rt_ret", &sig_ret)?;

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

        let rt_int_ref = module.declare_func_in_func(rt_int_id, b.func);
        let rt_getlocal_ref = module.declare_func_in_func(rt_getlocal_id, b.func);
        let rt_add_ref = module.declare_func_in_func(rt_add_id, b.func);
        let rt_sub_ref = module.declare_func_in_func(rt_sub_id, b.func);
        let rt_mul_ref = module.declare_func_in_func(rt_mul_id, b.func);
        let rt_div_ref = module.declare_func_in_func(rt_div_id, b.func);
        let rt_rem_ref = module.declare_func_in_func(rt_rem_id, b.func);
        let rt_ret_ref = module.declare_func_in_func(rt_ret_id, b.func);

        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        let ctx_val = b.block_params(entry)[0];

        let mut stack: Vec<ClValue> = Vec::new();
        let mut fault_block: Option<Block> = None;
        let mut terminated = false;

        let func = &program.functions[func_idx];
        for op in &func.chunk.code {
            match op {
                Op::Const(idx) => match func.chunk.consts.get(*idx) {
                    Some(Value::Int(k)) => {
                        let kv = b.ins().iconst(types::I64, *k);
                        let call = b.ins().call(rt_int_ref, &[ctx_val, kv]);
                        stack.push(b.inst_results(call)[0]);
                    }
                    other => {
                        return Err(JitError::Unsupported(format!("Const {other:?}")));
                    }
                },
                Op::GetLocal(n) => {
                    let nv = b.ins().iconst(types::I64, *n as i64);
                    let call = b.ins().call(rt_getlocal_ref, &[ctx_val, nv]);
                    stack.push(b.inst_results(call)[0]);
                }
                Op::AddI | Op::SubI | Op::MulI | Op::DivI | Op::RemI => {
                    let rhs = stack
                        .pop()
                        .ok_or_else(|| JitError::Codegen("arith: stack underflow".to_string()))?;
                    let lhs = stack
                        .pop()
                        .ok_or_else(|| JitError::Codegen("arith: stack underflow".to_string()))?;
                    let fref = match op {
                        Op::AddI => rt_add_ref,
                        Op::SubI => rt_sub_ref,
                        Op::MulI => rt_mul_ref,
                        Op::DivI => rt_div_ref,
                        _ => rt_rem_ref,
                    };
                    let call = b.ins().call(fref, &[ctx_val, lhs, rhs]);
                    let r = b.inst_results(call)[0];
                    // Null result ⇒ the kernel faulted; branch to the shared fault-exit block.
                    let fb = *fault_block.get_or_insert_with(|| b.create_block());
                    let is_null = b.ins().icmp_imm(IntCC::Equal, r, 0);
                    let cont = b.create_block();
                    b.ins().brif(is_null, fb, &[], cont, &[]);
                    b.switch_to_block(cont);
                    stack.push(r);
                }
                Op::Return => {
                    let v = stack
                        .pop()
                        .ok_or_else(|| JitError::Codegen("return: empty stack".to_string()))?;
                    b.ins().call(rt_ret_ref, &[ctx_val, v]);
                    let zero = b.ins().iconst(types::I64, 0);
                    b.ins().return_(&[zero]);
                    terminated = true;
                    break; // first Return is the exit for a straight-line (no-jump) leaf function
                }
                other => {
                    return Err(JitError::Unsupported(format!("{other:?}")));
                }
            }
        }

        if !terminated {
            return Err(JitError::Unsupported(
                "no supported Return reached".to_string(),
            ));
        }
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
    let code = module.get_finalized_function(func_id);

    // --- run it ---
    // SAFETY: `code` is the finalized machine code for a function compiled with exactly the signature
    // `extern "C" fn(*mut JitCtx) -> i64` — the sole first-party `unsafe` this whole effort exists to
    // confine. `module` (which owns the executable memory) is kept alive across the call below.
    let entry: extern "C" fn(*mut JitCtx) -> i64 =
        unsafe { std::mem::transmute::<*const u8, extern "C" fn(*mut JitCtx) -> i64>(code) };
    let mut call_ctx = JitCtx {
        args: args.as_ptr(),
        arena: Vec::new(),
        result: Value::Unit,
        fault: None,
    };
    let status = entry(&mut call_ctx);
    // `module` owns the JIT memory `entry` points into; keep it alive until after the call, then drop.
    drop(module);

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
