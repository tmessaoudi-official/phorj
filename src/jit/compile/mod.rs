//! `Compiled` — module lifecycle (compile boxed/unboxed, run, drop) and the VM-facing seam.

use super::*;

mod run;

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
    /// P-2a (unboxed ABI only): does the graph use handle ops? When true, `run_unboxed` builds a
    /// per-run [`UbCtx`] seeded from `const_handles` and passes its pointer; when false it passes null
    /// (nothing dereferences it).
    uses_handles: bool,
    /// The graph's interned string consts, in pinned-handle order (`UbCtx.handles[0..n]` per run).
    const_handles: Vec<Value>,
    /// The REUSED per-run handle context (built lazily on the first handle-using call, reset on
    /// every entry — see [`UbCtx::reset_for_run`]). Boxed so its interior pointers (arena base,
    /// free stack, record table — all into never-resized heap Vecs) stay stable across moves.
    ub_ctx_cache: std::cell::RefCell<Option<Box<UbCtx>>>,
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
            uses_handles: false,
            const_handles: Vec::new(),
            ub_ctx_cache: std::cell::RefCell::new(None),
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
        // Transitive op-subset eligibility + the set of functions to compile (reachable-only),
        // plus the cross-function fixpoint facts (ret kinds, method `this` injection).
        let (order, uses_handles, info) = resolve_unboxed_graph(program, entry_idx)?;
        // The ENTRY's return crosses back into the boxed world — `run_unboxed` decodes only
        // Int/Float. An instance-returning entry stays on the VM.
        if matches!(info.ret_of(entry_idx), Kind::Inst(..)) {
            return Err(JitError::Unsupported(
                "unboxed: entry returns an instance (deferred)".to_string(),
            ));
        }

        // `opt_level=speed` (P-2a): the default `none` leaves the register shuffles around the
        // handle-op helper calls and the loop-carried Variable phis unoptimized; `speed` is a pure
        // semantics-preserving Cranelift pass (byte-identity untouched — the same kernels run, in
        // the same order; the fault/sticky control flow is explicit IR, not droppable side effects).
        let mut builder =
            JITBuilder::with_flags(&[("opt_level", "speed")], default_libcall_names())
                .map_err(|e| JitError::Codegen(format!("JITBuilder: {e}")))?;
        if uses_handles {
            register_ub_symbols(&mut builder);
        }
        let mut module = JITModule::new(builder);
        let ptr = module.target_config().pointer_type();

        // P-2a: intern the graph's string consts (dedup by content — the P-1a chunk consts are
        // already `PhStr::literal` values, so a clone shares the Rc + cached hash). The COMPILE-TIME
        // handle for each const comes from `UbCtx::const_compile_handles` (a short const is a pinned
        // arena SLOT, a long one an untagged `handles` entry), and `UbCtx::new` seeds the per-run
        // state in the SAME deterministic order — the two walks must never diverge.
        let mut const_handles: Vec<Value> = Vec::new();
        let mut const_positions: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();
        if uses_handles {
            let mut by_content: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for &fi in &order {
                let func = &program.functions[fi];
                let reach = reachable(&func.chunk.code);
                for (ip, op) in func.chunk.code.iter().enumerate() {
                    if !reach[ip] {
                        continue;
                    }
                    if let Op::Const(idx) = op {
                        if let Some(Value::Str(s)) = func.chunk.consts.get(*idx) {
                            let pos =
                                *by_content.entry(s.as_str().to_string()).or_insert_with(|| {
                                    const_handles.push(Value::Str(s.clone()));
                                    const_handles.len() - 1
                                });
                            const_positions.insert((fi, *idx), pos);
                        }
                    }
                }
            }
        }
        let compile_handles = UbCtx::const_compile_handles(&const_handles);
        let const_map: std::collections::HashMap<(usize, usize), i64> = const_positions
            .into_iter()
            .map(|(k, pos)| (k, compile_handles[pos]))
            .collect();

        let ub_ids = if uses_handles {
            Some(declare_ub_helper_ids(&mut module, ptr)?)
        } else {
            None
        };

        // Declare a FuncId per function:
        // `extern "C" fn(ctx: *mut UbCtx, depth: i64, a0..a_arity: i64) -> (i64, i64)` — `ctx` is the
        // per-run handle table (null for a pure-numeric graph; only handle ops dereference it).
        // Per-function arity, so each has its own signature (declared BEFORE any body so calls — self
        // or cross — resolve at finalize).
        // W7: the ABI is KIND-driven — a `Dyn` param crosses as TWO i64 words (payload, tag).
        let make_fn_sig = |module: &JITModule, pks: &[Kind]| {
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(ptr)); // ctx
            sig.params.push(AbiParam::new(types::I64)); // depth
            for pk in pks {
                sig.params.push(AbiParam::new(types::I64));
                if *pk == Kind::Dyn {
                    sig.params.push(AbiParam::new(types::I64)); // the tag word
                }
            }
            sig.returns.push(AbiParam::new(types::I64)); // value
            sig.returns.push(AbiParam::new(types::I64)); // fault code (0 = ok)
            sig
        };
        // The VM hook seeds the ENTRY with one Value per arity slot — a Dyn entry param has
        // no tag source there (deferred; callees inside the graph are the Dyn consumers).
        if abi_param_kinds(program, &info, entry_idx).contains(&Kind::Dyn) {
            return Err(JitError::Unsupported(
                "unboxed: entry with a union (Dyn) param (deferred)".to_string(),
            ));
        }
        let mut func_ids: Vec<Option<FuncId>> = vec![None; program.functions.len()];
        for &fi in &order {
            // NB: a lambda's `arity` already folds its captures in (frame = [caps.., args..]),
            // so the sig covers the prepended capture args with no adjustment.
            let sig = make_fn_sig(&module, &abi_param_kinds(program, &info, fi));
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
            let proven = unboxed_proven_param_kinds(program, fi);
            let mut ret_kind: Option<Kind> = None;
            let mut cl_ctx = module.make_context();
            cl_ctx.func.signature = make_fn_sig(&module, &abi_param_kinds(program, &info, fi));
            build_body_unboxed(
                &mut module,
                &mut cl_ctx,
                program,
                fi,
                &func_ids,
                &proven,
                &mut ret_kind,
                ub_ids.as_ref(),
                &const_map,
                &info,
            )?;
            // Debug lens: `PHORJ_JIT_DISASM=1` prints each unboxed function's native disassembly
            // to stderr (the codegen-constant-factor investigations' ground truth — the floatloop/
            // floatmul near-ties are readable only at this level). Zero cost when unset.
            let want_disasm = std::env::var_os("PHORJ_JIT_DISASM").is_some_and(|v| v == "1");
            cl_ctx.set_disasm(want_disasm);
            module
                .define_function(func_ids[fi].expect("declared above"), &mut cl_ctx)
                .map_err(|e| JitError::Codegen(format!("define unboxed fn {fi}: {e}")))?;
            if want_disasm {
                if let Some(code) = cl_ctx.compiled_code() {
                    eprintln!(
                        "==== unboxed fn {fi} ({}) ====\n{}",
                        program.functions[fi].name,
                        code.vcode.as_deref().unwrap_or("<no vcode captured>")
                    );
                }
            }
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
            uses_handles,
            const_handles,
            ub_ctx_cache: std::cell::RefCell::new(None),
        })
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
