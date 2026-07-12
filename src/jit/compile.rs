//! `Compiled` — module lifecycle (compile boxed/unboxed, run, drop) and the VM-facing seam.

use super::*;

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
            builder.symbol("rt_u_list_new", rt_u_list_new as *const u8);
            builder.symbol("rt_u_list_push", rt_u_list_push as *const u8);
            builder.symbol("rt_u_list_seal", rt_u_list_seal as *const u8);
            builder.symbol("rt_u_index", rt_u_index as *const u8);
            builder.symbol("rt_u_concat", rt_u_concat as *const u8);
            builder.symbol("rt_u_str_len", rt_u_str_len as *const u8);
            builder.symbol("rt_u_free", rt_u_free as *const u8);
            builder.symbol("rt_u_map_push_pair", rt_u_map_push_pair as *const u8);
            builder.symbol("rt_u_map_seal", rt_u_map_seal as *const u8);
            builder.symbol("rt_u_map_get", rt_u_map_get as *const u8);
            builder.symbol("rt_u_list_push_int", rt_u_list_push_int as *const u8);
            builder.symbol("rt_u_index_int", rt_u_index_int as *const u8);
            builder.symbol("rt_u_int_to_str", rt_u_int_to_str as *const u8);
            builder.symbol("rt_u_concat_mix", rt_u_concat_mix as *const u8);
            builder.symbol("rt_u_acc_append", rt_u_acc_append as *const u8);
            builder.symbol("rt_u_list_len", rt_u_list_len as *const u8);
            builder.symbol("rt_u_list_acc_append", rt_u_list_acc_append as *const u8);
            builder.symbol("rt_u_map_builder_set", rt_u_map_builder_set as *const u8);
            builder.symbol("rt_u_map_builder_seed", rt_u_map_builder_seed as *const u8);
            builder.symbol("rt_u_list_acc_reseed", rt_u_list_acc_reseed as *const u8);
            builder.symbol("rt_u_list_builder_new", rt_u_list_builder_new as *const u8);
            builder.symbol(
                "rt_u_list_append_clone",
                rt_u_list_append_clone as *const u8,
            );
            builder.symbol("rt_u_native2", rt_u_native2 as *const u8);
            builder.symbol("rt_u_str_eq", rt_u_str_eq as *const u8);
            builder.symbol("rt_u_clone_value", rt_u_clone_value as *const u8);
            builder.symbol("rt_u_list_append_dyn", rt_u_list_append_dyn as *const u8);
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

        // Declare the handle-op helper imports (only when used).
        let ub_ids = if uses_handles {
            let declare = |m: &mut JITModule, name: &str, sig: &Signature| {
                m.declare_function(name, Linkage::Import, sig)
                    .map_err(|e| JitError::Codegen(format!("declare {name}: {e}")))
            };
            let sig1 = make_sig(&module, &[ptr], Some(types::I64));
            let sig2 = make_sig(&module, &[ptr, types::I64], Some(types::I64));
            let sig3 = make_sig(&module, &[ptr, types::I64, types::I64], Some(types::I64));
            let sig4 = make_sig(
                &module,
                &[ptr, types::I64, types::I64, types::I64],
                Some(types::I64),
            );
            let sig_free = make_sig(&module, &[ptr, types::I64], None);
            let sig5 = make_sig(
                &module,
                &[ptr, types::I64, types::I64, types::I64, types::I64],
                Some(types::I64),
            );
            // Two-i64 return (value, code) — the same multi-return shape as the compiled
            // functions' own signatures (see [`UbMapGetRet`] for the ABI note).
            let mut sig_map_get = make_sig(
                &module,
                &[ptr, types::I64, types::I64, types::I64],
                Some(types::I64),
            );
            sig_map_get.returns.push(AbiParam::new(types::I64));
            Some(UbHelperIds {
                list_new: declare(&mut module, "rt_u_list_new", &sig2)?,
                list_push: declare(&mut module, "rt_u_list_push", &sig4)?,
                list_seal: declare(&mut module, "rt_u_list_seal", &sig2)?,
                index: declare(&mut module, "rt_u_index", &sig4)?,
                concat: declare(&mut module, "rt_u_concat", &sig4)?,
                str_len: declare(&mut module, "rt_u_str_len", &sig3)?,
                free: declare(&mut module, "rt_u_free", &sig_free)?,
                map_push_pair: declare(&mut module, "rt_u_map_push_pair", &sig5)?,
                map_seal: declare(&mut module, "rt_u_map_seal", &sig2)?,
                map_get: declare(&mut module, "rt_u_map_get", &sig_map_get)?,
                list_push_int: declare(&mut module, "rt_u_list_push_int", &sig3)?,
                index_int: {
                    let mut s = make_sig(
                        &module,
                        &[ptr, types::I64, types::I64, types::I64],
                        Some(types::I64),
                    );
                    s.returns.push(AbiParam::new(types::I64));
                    declare(&mut module, "rt_u_index_int", &s)?
                },
                int_to_str: declare(&mut module, "rt_u_int_to_str", &sig2)?,
                concat_mix: {
                    let s = make_sig(
                        &module,
                        &[
                            ptr,
                            types::I64,
                            types::I64,
                            types::I64,
                            types::I64,
                            types::I64,
                            types::I64,
                            types::I64,
                            types::I64,
                            types::I64,
                        ],
                        Some(types::I64),
                    );
                    declare(&mut module, "rt_u_concat_mix", &s)?
                },
                acc_append: declare(&mut module, "rt_u_acc_append", &sig4)?,
                list_len: declare(&mut module, "rt_u_list_len", &sig2)?,
                list_acc_append: declare(&mut module, "rt_u_list_acc_append", &sig3)?,
                map_builder_set: declare(&mut module, "rt_u_map_builder_set", &sig4)?,
                map_builder_seed: declare(&mut module, "rt_u_map_builder_seed", &sig4)?,
                list_acc_reseed: declare(&mut module, "rt_u_list_acc_reseed", &sig3)?,
                list_builder_new: declare(&mut module, "rt_u_list_builder_new", &sig1)?,
                list_append_clone: declare(&mut module, "rt_u_list_append_clone", &sig4)?,
                native2: {
                    let mut s = make_sig(
                        &module,
                        &[ptr, types::I64, types::I64, types::I64, types::I64],
                        Some(types::I64),
                    );
                    s.returns.push(AbiParam::new(types::I64));
                    declare(&mut module, "rt_u_native2", &s)?
                },
                str_eq: declare(&mut module, "rt_u_str_eq", &sig4)?,
                clone_value: declare(&mut module, "rt_u_clone_value", &sig3)?,
                list_append_dyn: {
                    let s = make_sig(
                        &module,
                        &[ptr, types::I64, types::I64, types::I64, types::I64],
                        Some(types::I64),
                    );
                    declare(&mut module, "rt_u_list_append_dyn", &s)?
                },
            })
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
            uses_handles,
            const_handles,
            ub_ctx_cache: std::cell::RefCell::new(None),
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
                Kind::Str(_) | Kind::StrList(_) | Kind::IntList(_) => {
                    let repr = match self.ret_kind {
                        Kind::Str(_) => 2,
                        Kind::StrList(_) => 3,
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
