//! `Compiled::run` / `run_unboxed` — the VM-facing execution seam (M-Decomp from `compile.rs`,
//! Invariant 13). The compile half (module build, boxed/unboxed codegen, Drop) stays in
//! `mod.rs`; this file is the run ABI (boxed `fn(*mut JitCtx, i64)` and the arity-dispatched
//! unboxed `fn(*mut UbCtx, i64, i64…) -> (i64, i64)`), the sole first-party `unsafe` transmute
//! island. Child of `compile`, so `impl Compiled` here reaches `Compiled`'s private fields.
use super::*;

impl Compiled {
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

        // P-2a: the per-run handle table — built iff the graph uses handle ops (its pinned prefix
        // is the interned string consts); a pure-numeric graph gets a null pointer nothing
        // dereferences. REUSED across calls (built lazily once, reset ON ENTRY — the ctx-reuse
        // lever: per-call construction made many-call handle graphs slower than `--no-jit`). The
        // entry reset also means a fault path leaks nothing into the VM redo.
        let mut cached: Option<Box<UbCtx>> = if self.uses_handles {
            let mut c = self
                .ub_ctx_cache
                .borrow_mut()
                .take()
                .unwrap_or_else(|| Box::new(UbCtx::new(&self.const_handles)));
            c.reset_for_run();
            Some(c)
        } else {
            None
        };
        let ub_ctx: *mut UbCtx = cached
            .as_deref_mut()
            .map_or(std::ptr::null_mut(), std::ptr::from_mut);
        // SAFETY: `self.entry` is finalized machine code with signature
        // `extern "C" fn(*mut UbCtx, i64 depth, i64… /* arity */) -> (i64, i64)`; we transmute to the
        // arity-specific type and pass ctx + depth + exactly `arity` i64 args. `self.module` owns the
        // code, alive across the call; `ub_ctx` (when non-null) outlives the call.
        let ret: UnboxedRet = unsafe {
            match self.arity {
                0 => {
                    let f: extern "C" fn(*mut UbCtx, i64) -> UnboxedRet =
                        std::mem::transmute(self.entry);
                    f(ub_ctx, d0)
                }
                1 => {
                    let f: extern "C" fn(*mut UbCtx, i64, i64) -> UnboxedRet =
                        std::mem::transmute(self.entry);
                    f(ub_ctx, d0, ia[0])
                }
                2 => {
                    let f: extern "C" fn(*mut UbCtx, i64, i64, i64) -> UnboxedRet =
                        std::mem::transmute(self.entry);
                    f(ub_ctx, d0, ia[0], ia[1])
                }
                3 => {
                    let f: extern "C" fn(*mut UbCtx, i64, i64, i64, i64) -> UnboxedRet =
                        std::mem::transmute(self.entry);
                    f(ub_ctx, d0, ia[0], ia[1], ia[2])
                }
                other => {
                    return JitRun::Fault(format!("jit: unboxed arity {other} unsupported"));
                }
            }
        };
        // Decode BEFORE stashing the ctx back — a returned str/list handle points into it.
        let decoded = match ret.code {
            // Decode the returned i64 by the entry's return kind: Int verbatim, Float from its
            // bits, Bool from 0/1; a STR/LIST return is a HANDLE into the per-run ctx and must
            // MATERIALIZE into a real `Value` here (a raw handle word printed as an int was
            // the conformance break this arm fixes).
            0 => match self.ret_kind {
                Kind::Float => JitRun::Value(Value::Float(f64::from_bits(ret.value as u64))),
                Kind::Bool => JitRun::Value(Value::Bool(ret.value != 0)),
                Kind::Str(_) | Kind::StrList(_) | Kind::IntList(_) | Kind::DynList(_) => {
                    let repr = match self.ret_kind {
                        Kind::Str(_) => 2,
                        Kind::StrList(_) => 3,
                        Kind::DynList(_) => 5,
                        _ => 4,
                    };
                    match cached.as_ref().and_then(|c| c.materialize(ret.value, repr)) {
                        Some(v) => JitRun::Value(v),
                        None => JitRun::Fault(REDO_ON_VM.to_string()),
                    }
                }
                _ => JitRun::Value(Value::Int(ret.value)),
            },
            // ovf-spec: EVERY unboxed fault now funnels to code 5 = "redo on VM" (codes 1/2/3/4 are no
            // longer emitted). The hook re-executes the callee on the VM, which renders the exact,
            // correctly-ordered fault string + source line. See [`REDO_ON_VM`].
            5 => JitRun::Fault(REDO_ON_VM.to_string()),
            other => JitRun::Fault(format!("jit: unboxed unknown fault code {other}")),
        };
        // Stash the reused ctx back for the next call (arena + record buffers keep their growth).
        if let Some(c) = cached.take() {
            *self.ub_ctx_cache.borrow_mut() = Some(c);
        }
        decoded
    }
}
