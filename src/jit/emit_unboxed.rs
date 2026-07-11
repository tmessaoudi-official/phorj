//! UNBOXED codegen: `build_body_unboxed` — depth-indexed Cranelift Variables, checked-arith
//! sticky faults, and the inline handle-op fast paths (concat / list index / map probe).

use super::*;

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
    // shared fault-exit (code 5, redo on VM).
    let needs_fault_exit = needs_sticky
        || code.iter().enumerate().any(|(ip, op)| {
            reach[ip]
                && matches!(
                    op,
                    Op::DivI
                        | Op::RemI
                        | Op::DivF
                        | Op::Call(_)
                        | Op::Index
                        | Op::Concat(_)
                        | Op::CallNative(..)
                        | Op::MakeList(_)
                        | Op::MakeMap(_)
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
    });
    let ub_ref = |what: &str| -> Result<&UbHelperRefs, JitError> {
        ub_refs.as_ref().ok_or_else(|| {
            JitError::Codegen(format!(
                "unboxed: {what} reached codegen without handle helpers (collect drift)"
            ))
        })
    };

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

    // Loads of RUN-INVARIANT `UbCtx` header fields — the arena base (offset 0), the free-stack
    // base (8), the capacity (32); nothing ever stores to them during a run. `notrap` + `can_move`
    // lets GVN/LICM collapse the per-op re-loads and hoist them out of hot loops (the mutable
    // header fields — `free_top` at 16, `bump` at 24 — keep the default flags).
    let stable = MemFlagsData::new().with_notrap().with_can_move();

    // P-2a-inline: push an owned arena slot's index onto the inline free stack (the caller has
    // already established `v` is slot-tagged with OWNED set). 5 memory ops, no call.
    let emit_slot_push = |b: &mut FunctionBuilder, v: ClValue| {
        let fsp = b.ins().load(types::I64, stable, ctx, 8);
        let ft = b.ins().load(types::I64, MemFlagsData::new(), ctx, 16);
        let slot = b.ins().band_imm(v, UB_IDX_MASK);
        let foff = b.ins().ishl_imm(ft, 2);
        let faddr = b.ins().iadd(fsp, foff);
        b.ins().istore32(MemFlagsData::new(), slot, faddr, 0);
        let ft1 = b.ins().iadd_imm(ft, 1);
        b.ins().store(MemFlagsData::new(), ft1, ctx, 16);
    };
    // P-2a-inline: recycle a slot-tagged operand IFF its runtime OWNED bit is set (a flat-list
    // element or pinned const is compile-time Owned but runtime-borrowed — the free is a no-op).
    // Used only where the operand is already known slot-tagged (the inline fast paths).
    let emit_slot_free_if_owned = |b: &mut FunctionBuilder, v: ClValue| {
        let owned_bit = b.ins().band_imm(v, UB_TAG_OWNED);
        let push_blk = b.create_block();
        let cont = b.create_block();
        b.ins().brif(owned_bit, push_blk, &[], cont, &[]);
        b.switch_to_block(push_blk);
        emit_slot_push(b, v);
        b.ins().jump(cont, &[]);
        b.switch_to_block(cont);
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
                        Kind::Str(Own::Borrowed),
                    )?;
                }
                other => return Err(JitError::Unsupported(format!("unboxed Const {other:?}"))),
            },
            // ---- P-2a handle verticals (inline fast paths over the UbCtx arena; helpers = slow
            // paths for untagged operands / >22-byte results / non-flat lists) --------------------
            Op::MakeList(n) => {
                let h = ub_ref("MakeList")?;
                let d = kinds.len();
                if *n > d {
                    return Err(JitError::Codegen("unboxed MakeList underflow".to_string()));
                }
                // Element kinds select the flavor (mirrors the analyze arm): all-`Str` →
                // `StrList` (handle pushes), all-`Int` → `IntList` (raw i64 pushes, P-2c).
                let all_str = kinds[d - n..].iter().all(|k| matches!(k, Kind::Str(_)));
                let all_int = *n > 0 && kinds[d - n..].iter().all(|k| *k == Kind::Int);
                if !(all_str || all_int) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed MakeList element kinds {:?}",
                        &kinds[d - n..]
                    )));
                }
                let capv = b.ins().iconst(types::I64, *n as i64);
                let call = b.ins().call(h.list_new, &[ctx, capv]);
                let list_h = b.inst_results(call)[0];
                // Push elements bottom-up straight from their depth-indexed Variables (no pops —
                // the kind stack is truncated once below). An OWNED element is consumed (moved).
                for j in 0..*n {
                    let depth_j = d - n + j;
                    let ev = b.use_var(vars[depth_j]);
                    let pc = if all_int {
                        b.ins().call(h.list_push_int, &[ctx, list_h, ev])
                    } else {
                        let freev = b
                            .ins()
                            .iconst(types::I64, kinds[depth_j].is_owned_handle() as i64);
                        b.ins().call(h.list_push, &[ctx, list_h, ev, freev])
                    };
                    let status = b.inst_results(pc)[0];
                    let bad = b.ins().icmp_imm(IntCC::NotEqual, status, 0);
                    fault_if(&mut b, bad, 5);
                }
                // Seal: all-short strings / all ints flatten into consecutive arena slots (a FLAT
                // handle) so `Index` runs fully inline; anything else keeps the boxed handle.
                let sc = b.ins().call(h.list_seal, &[ctx, list_h]);
                let sealed = b.inst_results(sc)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sealed, 0);
                fault_if(&mut b, bad, 5);
                kinds.truncate(d - n);
                ub_push(
                    &mut b,
                    &vars,
                    &fvars,
                    &mut kinds,
                    sealed,
                    if all_int {
                        Kind::IntList(Own::Owned)
                    } else {
                        Kind::StrList(Own::Owned)
                    },
                )?;
            }
            Op::MakeMap(n) => {
                let h = ub_ref("MakeMap")?;
                let d = kinds.len();
                if 2 * n > d {
                    return Err(JitError::Codegen("unboxed MakeMap underflow".to_string()));
                }
                // Validate the 2n-operand tail alternates key (Str) / value (Int) BEFORE emitting
                // (mirrors the analyze arm exactly).
                for j in 0..*n {
                    let (kk, vk) = (kinds[d - 2 * n + 2 * j], kinds[d - 2 * n + 2 * j + 1]);
                    if !matches!(kk, Kind::Str(_)) || vk != Kind::Int {
                        return Err(JitError::Unsupported(format!(
                            "unboxed MakeMap pair kinds ({kk:?} => {vk:?})"
                        )));
                    }
                }
                // Scratch: an untagged list accumulating k1,v1,…  (reuses the list allocator).
                let capv = b.ins().iconst(types::I64, 2 * *n as i64);
                let call = b.ins().call(h.list_new, &[ctx, capv]);
                let map_h = b.inst_results(call)[0];
                for j in 0..*n {
                    let kd = d - 2 * n + 2 * j;
                    let kv = b.use_var(vars[kd]);
                    let vv = b.use_var(vars[kd + 1]);
                    let freev = b
                        .ins()
                        .iconst(types::I64, kinds[kd].is_owned_handle() as i64);
                    let pc = b.ins().call(h.map_push_pair, &[ctx, map_h, kv, vv, freev]);
                    let status = b.inst_results(pc)[0];
                    let bad = b.ins().icmp_imm(IntCC::NotEqual, status, 0);
                    fault_if(&mut b, bad, 5);
                }
                // Seal: dedup through the canonical `build_map` kernel; an all-short-key int map
                // flattens into arena slot PAIRS (a `SLOT|FLAT` handle) so lookup runs fully inline.
                let sc = b.ins().call(h.map_seal, &[ctx, map_h]);
                let sealed = b.inst_results(sc)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sealed, 0);
                fault_if(&mut b, bad, 5);
                kinds.truncate(d - 2 * n);
                ub_push(
                    &mut b,
                    &vars,
                    &fvars,
                    &mut kinds,
                    sealed,
                    Kind::StrIntMap(Own::Owned),
                )?;
            }
            Op::Index if matches!(kinds.last(), Some(Kind::Str(_))) => {
                // ---- P-2b: string-keyed map lookup (`m[k]` → Int) --------------------------------
                let h = ub_ref("Index(map)")?;
                let (iv, ik) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (mv, mk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if !matches!(ik, Kind::Str(_)) || !matches!(mk, Kind::StrIntMap(_)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed Index operand kinds ({mk:?}[{ik:?}])"
                    )));
                }
                let merge = b.create_block();
                b.append_block_param(merge, types::I64);
                let fast_blk = b.create_block();
                let slow_blk = b.create_block();
                // INLINE iff the map sealed FLAT and the key is an arena slot; the probe needs the
                // key's CANON word, so a canon-0 slot (an inline-concat result, an unregistered
                // runtime string) also punts.
                let flat_bit = b.ins().band_imm(mv, UB_TAG_FLAT);
                let key_slot = b.ins().band_imm(iv, UB_TAG_SLOT);
                let flat_nz = b.ins().icmp_imm(IntCC::NotEqual, flat_bit, 0);
                let key_nz = b.ins().icmp_imm(IntCC::NotEqual, key_slot, 0);
                let both = b.ins().band(flat_nz, key_nz);
                b.ins().brif(both, fast_blk, &[], slow_blk, &[]);
                // INLINE: O(1) bucket walk. The key's CANON word (slot byte 32 — nonzero only when
                // assigned via the content registry, so canon equality ⇔ byte equality) indexes
                // nothing itself; the HASH picks the bucket (`hash & mask`), each bucket holds a
                // pair index (u32::MAX = empty), and ONE canon compare decides the pair. An empty
                // bucket is a genuine miss (the seal's load factor ≤ 1/2 guarantees termination) —
                // code 5, the VM redo renders the canonical `"map key not found"`. A canon-0 key
                // (inline-concat result, unregistered runtime string) punts to the helper.
                b.switch_to_block(fast_blk);
                let buf = b.ins().load(types::I64, stable, ctx, 0);
                let ki = b.ins().band_imm(iv, UB_IDX_MASK);
                let koff = b.ins().ishl_imm(ki, 6);
                let pk = b.ins().iadd(buf, koff);
                let khash =
                    b.ins()
                        .load(types::I64, MemFlagsData::new(), pk, UB_SLOT_HASH_OFF as i32);
                let kcanon = b.ins().load(
                    types::I64,
                    MemFlagsData::new(),
                    pk,
                    UB_SLOT_CANON_OFF as i32,
                );
                let probe_blk = b.create_block();
                b.ins().brif(kcanon, probe_blk, &[], slow_blk, &[]);
                b.switch_to_block(probe_blk);
                let cnt_raw = b.ins().ushr_imm(mv, UB_MAP_CNT_SHIFT);
                let cnt = b.ins().band_imm(cnt_raw, 0xFFF);
                let lg_raw = b.ins().ushr_imm(mv, UB_MAP_LOG_SHIFT);
                let lg = b.ins().band_imm(lg_raw, 0x1F);
                let base = b.ins().band_imm(mv, UB_IDX_MASK);
                let boff = b.ins().ishl_imm(base, 6);
                let pbase = b.ins().iadd(buf, boff);
                let tsoff = b.ins().ishl_imm(cnt, 7); // table starts after the 2·cnt pair slots
                let tbase = b.ins().iadd(pbase, tsoff);
                let one = b.ins().iconst(types::I64, 1);
                let tsize = b.ins().ishl(one, lg);
                let mask = b.ins().iadd_imm(tsize, -1);
                let t0 = b.ins().band(khash, mask);
                let head = b.create_block();
                b.append_block_param(head, types::I64); // bucket index
                let hit = b.create_block();
                b.append_block_param(hit, types::I64); // matched pair index
                let next = b.create_block();
                b.append_block_param(next, types::I64);
                b.ins().jump(head, &[t0.into()]);
                b.switch_to_block(head);
                let t = b.block_params(head)[0];
                let btoff = b.ins().ishl_imm(t, 2);
                let baddr = b.ins().iadd(tbase, btoff);
                let e = b.ins().uload32(MemFlagsData::new(), baddr, 0);
                let empty = b.ins().icmp_imm(IntCC::Equal, e, 0xFFFF_FFFF);
                fault_if(&mut b, empty, 5); // genuine miss → canonical fault on the VM
                let poff = b.ins().ishl_imm(e, 7); // pair stride = 2 slots = 128 bytes
                let ph = b.ins().iadd(pbase, poff);
                let pcanon = b.ins().load(
                    types::I64,
                    MemFlagsData::new(),
                    ph,
                    UB_SLOT_CANON_OFF as i32,
                );
                let ceq = b.ins().icmp(IntCC::Equal, pcanon, kcanon);
                b.ins().brif(ceq, hit, &[e.into()], next, &[t.into()]);
                b.switch_to_block(hit);
                let he = b.block_params(hit)[0];
                let hoff = b.ins().ishl_imm(he, 7);
                let hph = b.ins().iadd(pbase, hoff);
                let val = b.ins().load(types::I64, MemFlagsData::new(), hph, 64);
                // Consume the key (recycle iff runtime-OWNED); the flat map is bump-pinned
                // (runtime-borrowed always) — nothing to free.
                if ik.is_owned_handle() {
                    emit_slot_free_if_owned(&mut b, iv);
                }
                b.ins().jump(merge, &[val.into()]);
                b.switch_to_block(next);
                let nt = b.block_params(next)[0];
                let t1 = b.ins().iadd_imm(nt, 1);
                let tw = b.ins().band(t1, mask);
                b.ins().jump(head, &[tw.into()]);
                // SLOW: the helper (boxed map through the canonical kernel; hash-0 / untagged keys).
                b.switch_to_block(slow_blk);
                let mask = (ik.is_owned_handle() as i64) | ((mk.is_owned_handle() as i64) << 1);
                let maskv = b.ins().iconst(types::I64, mask);
                let call = b.ins().call(h.map_get, &[ctx, mv, iv, maskv]);
                let sval = b.inst_results(call)[0];
                let scode = b.inst_results(call)[1];
                let bad = b.ins().icmp_imm(IntCC::NotEqual, scode, 0);
                fault_if(&mut b, bad, 5);
                b.ins().jump(merge, &[sval.into()]);
                b.switch_to_block(merge);
                let res = b.block_params(merge)[0];
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
            }
            // ---- P-2c: int-list element read (`xs[i]` → Int, raw i64 at slot bytes 0..8) -------
            Op::Index
                if matches!(
                    kinds.get(kinds.len().wrapping_sub(2)),
                    Some(Kind::IntList(_))
                ) =>
            {
                let h = ub_ref("Index(int)")?;
                let (iv, ik) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (lv, lk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if ik != Kind::Int || !matches!(lk, Kind::IntList(_)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed Index operand kinds ({lk:?}[{ik:?}])"
                    )));
                }
                let merge = b.create_block();
                b.append_block_param(merge, types::I64);
                let flat_blk = b.create_block();
                let slow_blk = b.create_block();
                let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
                b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
                // INLINE (flat int list): unsigned bounds check, then ONE load of the raw i64 at
                // `buf[(base+idx)*64]`. Out-of-range → code 5 → the canonical fault on the VM.
                b.switch_to_block(flat_blk);
                let cnt_raw = b.ins().ushr_imm(lv, 40);
                let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
                let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
                fault_if(&mut b, oob, 5);
                let buf = b.ins().load(types::I64, stable, ctx, 0);
                let base = b.ins().band_imm(lv, UB_IDX_MASK);
                let slot = b.ins().iadd(base, iv);
                let soff = b.ins().ishl_imm(slot, 6);
                let addr = b.ins().iadd(buf, soff);
                let fres = b.ins().load(types::I64, MemFlagsData::new(), addr, 0);
                b.ins().jump(merge, &[fres.into()]);
                // SLOW (boxed int list): the two-return helper (value spans the full i64 range).
                b.switch_to_block(slow_blk);
                let freev = b.ins().iconst(types::I64, lk.is_owned_handle() as i64);
                let call = b.ins().call(h.index_int, &[ctx, lv, iv, freev]);
                let sval = b.inst_results(call)[0];
                let scode = b.inst_results(call)[1];
                let bad = b.ins().icmp_imm(IntCC::NotEqual, scode, 0);
                fault_if(&mut b, bad, 5);
                b.ins().jump(merge, &[sval.into()]);
                b.switch_to_block(merge);
                let res = b.block_params(merge)[0];
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
            }
            Op::Index => {
                let h = ub_ref("Index")?;
                let (iv, ik) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (lv, lk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if ik != Kind::Int || !matches!(lk, Kind::StrList(_)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed Index operand kinds ({lk:?}[{ik:?}])"
                    )));
                }
                let merge = b.create_block();
                b.append_block_param(merge, types::I64);
                let flat_blk = b.create_block();
                let slow_blk = b.create_block();
                let flat_bit = b.ins().band_imm(lv, UB_TAG_FLAT);
                b.ins().brif(flat_bit, flat_blk, &[], slow_blk, &[]);
                // INLINE (flat list): unsigned bounds check (a negative idx is a huge u64 — same
                // reject as the VM's `usize::try_from`), then base+idx is a BORROWED slot handle —
                // zero copy, zero alloc. Out-of-range → code 5 → the VM redo renders the canonical
                // "list index out of range".
                b.switch_to_block(flat_blk);
                let cnt_raw = b.ins().ushr_imm(lv, 40);
                let cnt = b.ins().band_imm(cnt_raw, 0xFFFFF);
                let oob = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, iv, cnt);
                fault_if(&mut b, oob, 5);
                let base = b.ins().band_imm(lv, UB_IDX_MASK);
                let slot = b.ins().iadd(base, iv);
                let fres = b.ins().bor_imm(slot, UB_TAG_SLOT);
                b.ins().jump(merge, &[fres.into()]);
                // SLOW (boxed list): the helper (element clone into a slot / untagged temp).
                b.switch_to_block(slow_blk);
                let freev = b.ins().iconst(types::I64, lk.is_owned_handle() as i64);
                let call = b.ins().call(h.index, &[ctx, lv, iv, freev]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                fault_if(&mut b, bad, 5);
                b.ins().jump(merge, &[sres.into()]);
                b.switch_to_block(merge);
                let res = b.block_params(merge)[0];
                ub_push(
                    &mut b,
                    &vars,
                    &fvars,
                    &mut kinds,
                    res,
                    Kind::Str(Own::Owned),
                )?;
            }
            Op::Concat(2) => {
                let h = ub_ref("Concat")?;
                let (bv, bk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, ak) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if !matches!(ak, Kind::Str(_)) || !matches!(bk, Kind::Str(_)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed Concat operand kinds ({ak:?}, {bk:?})"
                    )));
                }
                let merge = b.create_block();
                b.append_block_param(merge, types::I64);
                let fast1 = b.create_block();
                let slow_blk = b.create_block();
                // Fast iff BOTH operands are arena slots (a pinned short const, a flat element, or
                // an owned temp) — one AND + one branch.
                let both = b.ins().band(av, bv);
                let both_slot = b.ins().band_imm(both, UB_TAG_SLOT);
                b.ins().brif(both_slot, fast1, &[], slow_blk, &[]);
                // INLINE: load both lengths; a ≤22-byte result is built in a fresh slot with
                // bounded 3×8-byte over-copies (the 64-byte slot slack absorbs them — see
                // `UB_SLOT_SIZE`); the byte semantics are exactly `PhStr::concat`'s.
                b.switch_to_block(fast1);
                let buf = b.ins().load(types::I64, stable, ctx, 0);
                let ia = b.ins().band_imm(av, UB_IDX_MASK);
                let aoff = b.ins().ishl_imm(ia, 6);
                let pa = b.ins().iadd(buf, aoff);
                let ib = b.ins().band_imm(bv, UB_IDX_MASK);
                let boff = b.ins().ishl_imm(ib, 6);
                let pb = b.ins().iadd(buf, boff);
                let la = b.ins().uload8(types::I64, MemFlagsData::new(), pa, 0);
                let lb = b.ins().uload8(types::I64, MemFlagsData::new(), pb, 0);
                let tot = b.ins().iadd(la, lb);
                let big = b.ins().icmp_imm(
                    IntCC::UnsignedGreaterThan,
                    tot,
                    crate::phstr::INLINE_CAP as i64,
                );
                let fast2 = b.create_block();
                b.ins().brif(big, slow_blk, &[], fast2, &[]);
                // Allocate the result slot: pop the inline free stack, else bump (full → code 5,
                // redo on VM — exhaustion is a fallback, never a user-visible fault).
                b.switch_to_block(fast2);
                let alloc_done = b.create_block();
                b.append_block_param(alloc_done, types::I64);
                let pop_blk = b.create_block();
                let bump_blk = b.create_block();
                let ft = b.ins().load(types::I64, MemFlagsData::new(), ctx, 16);
                b.ins().brif(ft, pop_blk, &[], bump_blk, &[]);
                b.switch_to_block(pop_blk);
                let ft1 = b.ins().iadd_imm(ft, -1);
                b.ins().store(MemFlagsData::new(), ft1, ctx, 16);
                let fsp = b.ins().load(types::I64, stable, ctx, 8);
                let foff = b.ins().ishl_imm(ft1, 2);
                let faddr = b.ins().iadd(fsp, foff);
                let popped = b.ins().uload32(MemFlagsData::new(), faddr, 0);
                b.ins().jump(alloc_done, &[popped.into()]);
                b.switch_to_block(bump_blk);
                let bp = b.ins().load(types::I64, MemFlagsData::new(), ctx, 24);
                let cap = b.ins().load(types::I64, stable, ctx, 32);
                let full = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, bp, cap);
                fault_if(&mut b, full, 5);
                let bp1 = b.ins().iadd_imm(bp, 1);
                b.ins().store(MemFlagsData::new(), bp1, ctx, 24);
                b.ins().jump(alloc_done, &[bp.into()]);
                b.switch_to_block(alloc_done);
                let sidx = b.block_params(alloc_done)[0];
                let doff = b.ins().ishl_imm(sidx, 6);
                let pd = b.ins().iadd(buf, doff);
                b.ins().istore8(MemFlagsData::new(), tot, pd, 0);
                // Copy a → dst+1 (static offsets; over-copy is absorbed by the slot slack).
                for k in 0..3 {
                    let w = b.ins().load(types::I64, MemFlagsData::new(), pa, 1 + 8 * k);
                    b.ins().store(MemFlagsData::new(), w, pd, 1 + 8 * k);
                }
                // Copy b → dst+1+la (runtime offset).
                let la1 = b.ins().iadd_imm(la, 1);
                let pdb = b.ins().iadd(pd, la1);
                for k in 0..3 {
                    let w = b.ins().load(types::I64, MemFlagsData::new(), pb, 1 + 8 * k);
                    b.ins().store(MemFlagsData::new(), w, pdb, 8 * k);
                }
                // Zero the metadata words the over-copies just trashed: hash 0 + canon 0 = "punt
                // to the helper" — without this, stale/garbage canon could FALSE-MATCH in the
                // inline map probe (a byte-identity break, not just a slow path).
                let zmeta = b.ins().iconst(types::I64, 0);
                b.ins()
                    .store(MemFlagsData::new(), zmeta, pd, UB_SLOT_HASH_OFF as i32);
                b.ins()
                    .store(MemFlagsData::new(), zmeta, pd, UB_SLOT_CANON_OFF as i32);
                // Consume compile-time-OWNED operands: recycle iff the runtime OWNED bit is set
                // (a flat element / pinned const is compile-Owned but runtime-borrowed → no-op).
                for (v, k) in [(av, ak), (bv, bk)] {
                    if k.is_owned_handle() {
                        emit_slot_free_if_owned(&mut b, v);
                    }
                }
                let fres_raw = b.ins().bor_imm(sidx, UB_TAG_SLOT);
                let fres = b.ins().bor_imm(fres_raw, UB_TAG_OWNED);
                b.ins().jump(merge, &[fres.into()]);
                // SLOW: the helper handles every encoding + the >22-byte (heap) result.
                b.switch_to_block(slow_blk);
                let mask = (ak.is_owned_handle() as i64) | ((bk.is_owned_handle() as i64) << 1);
                let maskv = b.ins().iconst(types::I64, mask);
                let call = b.ins().call(h.concat, &[ctx, av, bv, maskv]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                fault_if(&mut b, bad, 5);
                b.ins().jump(merge, &[sres.into()]);
                b.switch_to_block(merge);
                let res = b.block_params(merge)[0];
                ub_push(
                    &mut b,
                    &vars,
                    &fvars,
                    &mut kinds,
                    res,
                    Kind::Str(Own::Owned),
                )?;
            }
            // ---- P-2c numeric conversions: fully inline, no helper, no handle space ------------
            Op::CallNative(id, 1) if unboxed_native_is_to_float(*id) => {
                // `Conversion.toFloat(int)` — the kernel is `n as f64`; `fcvt_from_sint` is the
                // same IEEE round-to-nearest widening. Total: no fault path.
                let (iv, ik) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if ik != Kind::Int {
                    return Err(JitError::Unsupported(format!(
                        "unboxed toFloat operand kind {ik:?}"
                    )));
                }
                let fv = b.ins().fcvt_from_sint(types::F64, iv);
                ub_push(&mut b, &vars, &fvars, &mut kinds, fv, Kind::Float)?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_truncate(*id) => {
                // `Conversion.truncate(float)` — mirrors `value::float_to_int` EXACTLY: trunc
                // toward zero, then require `LOWER <= t < UPPER` (NaN/±∞ fail the ordered
                // compares). In-range → `fcvt_to_sint` (cannot trap under the guard, and `t` is
                // already integral so the conversion is exact); out-of-range → code 5, the VM
                // redo renders the canonical "float is out of int range" fault.
                let (fv, fk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if fk != Kind::Float {
                    return Err(JitError::Unsupported(format!(
                        "unboxed truncate operand kind {fk:?}"
                    )));
                }
                let t = b.ins().trunc(fv);
                let lower = b.ins().f64const(-9_223_372_036_854_775_808.0);
                let upper = b.ins().f64const(9_223_372_036_854_775_808.0);
                let ge = b.ins().fcmp(FloatCC::GreaterThanOrEqual, t, lower);
                let lt = b.ins().fcmp(FloatCC::LessThan, t, upper);
                let ok = b.ins().band(ge, lt);
                let bad = b.ins().icmp_imm(IntCC::Equal, ok, 0);
                fault_if(&mut b, bad, 5);
                let res = b.ins().fcvt_to_sint(types::I64, t);
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
            }
            Op::CallNative(id, 1) if unboxed_native_is_str_len(*id) => {
                let h = ub_ref("String.length")?;
                let (sv, sk) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                if !matches!(sk, Kind::Str(_)) {
                    return Err(JitError::Unsupported(format!(
                        "unboxed String.length operand kind {sk:?}"
                    )));
                }
                let merge = b.create_block();
                b.append_block_param(merge, types::I64);
                let fast_blk = b.create_block();
                let slow_blk = b.create_block();
                let slot_bit = b.ins().band_imm(sv, UB_TAG_SLOT);
                b.ins().brif(slot_bit, fast_blk, &[], slow_blk, &[]);
                // INLINE: the length is the slot's leading byte.
                b.switch_to_block(fast_blk);
                let buf = b.ins().load(types::I64, stable, ctx, 0);
                let si = b.ins().band_imm(sv, UB_IDX_MASK);
                let soff = b.ins().ishl_imm(si, 6);
                let ps = b.ins().iadd(buf, soff);
                let n = b.ins().uload8(types::I64, MemFlagsData::new(), ps, 0);
                if sk.is_owned_handle() {
                    emit_slot_free_if_owned(&mut b, sv);
                }
                b.ins().jump(merge, &[n.into()]);
                b.switch_to_block(slow_blk);
                let freev = b.ins().iconst(types::I64, sk.is_owned_handle() as i64);
                let call = b.ins().call(h.str_len, &[ctx, sv, freev]);
                let sres = b.inst_results(call)[0];
                let bad = b.ins().icmp_imm(IntCC::SignedLessThan, sres, 0);
                fault_if(&mut b, bad, 5);
                b.ins().jump(merge, &[sres.into()]);
                b.switch_to_block(merge);
                let res = b.block_params(merge)[0];
                ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
            }
            Op::Pop => {
                let (v, k) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // An owned handle dies unconsumed here (a statement-expression string, a loop-body
                // temp) — release it so the arena/table stay at steady state. Scalars and borrows:
                // dropping the SSA value is the whole discard. Runtime dispatch: an owned slot
                // recycles inline; an untagged temp goes through the helper; a borrowed slot or a
                // flat list is a no-op (flat handled by the helper defensively).
                if k.is_owned_handle() {
                    let h = ub_ref("Pop(owned handle)")?;
                    let owned_bit = b.ins().band_imm(v, UB_TAG_OWNED);
                    let push_blk = b.create_block();
                    let not_owned = b.create_block();
                    let cont = b.create_block();
                    b.ins().brif(owned_bit, push_blk, &[], not_owned, &[]);
                    b.switch_to_block(push_blk);
                    emit_slot_push(&mut b, v);
                    b.ins().jump(cont, &[]);
                    b.switch_to_block(not_owned);
                    let slot_bit = b.ins().band_imm(v, UB_TAG_SLOT);
                    let helper_blk = b.create_block();
                    b.ins().brif(slot_bit, cont, &[], helper_blk, &[]);
                    b.switch_to_block(helper_blk);
                    b.ins().call(h.free, &[ctx, v]);
                    b.ins().jump(cont, &[]);
                    b.switch_to_block(cont);
                }
            }
            Op::AddI | Op::SubI | Op::MulI => {
                let (bv, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                let (av, _) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                // Plain (wrapping-free) when: `#[UncheckedOverflow]` (the whole fn wraps — all of AddI/SubI/MulI),
                // OR a range-proven induction `AddI`. The two's-complement `iadd`/`isub`/`imul` result is
                // bit-identical to `*_overflow`'s result[0] (byte-identity ✓); no fault, no sticky.
                if func.unchecked || (matches!(op, Op::AddI) && proven_ops[ip]) {
                    let res = match op {
                        Op::AddI => b.ins().iadd(av, bv),
                        Op::SubI => b.ins().isub(av, bv),
                        _ => b.ins().imul(av, bv),
                    };
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
                // P-2c: a range-PROVEN `RemI` (non-negative dividend, positive power-of-two const
                // divisor — see `range_proven_ops`) is a single `band`: exact same value (truncated
                // rem of a non-negative by a positive 2^m), and both fault conditions are impossible.
                if matches!(op, Op::RemI) && proven_ops[ip] {
                    let c = match &code[ip - 1] {
                        Op::Const(ci) => match func.chunk.consts.get(*ci) {
                            Some(Value::Int(c)) => *c,
                            other => {
                                return Err(JitError::Codegen(format!(
                                    "proven RemI divisor not an int const: {other:?}"
                                )))
                            }
                        },
                        other => {
                            return Err(JitError::Codegen(format!(
                                "proven RemI not preceded by Const: {other:?}"
                            )))
                        }
                    };
                    let res = b.ins().band_imm(av, c - 1);
                    ub_push(&mut b, &vars, &fvars, &mut kinds, res, Kind::Int)?;
                    continue;
                }
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
                // `#[UncheckedOverflow]`: `-i64::MIN` wraps to `i64::MIN` — plain `ineg`, no sticky. Else speculate
                // (fold `av == MIN` into sticky) as before.
                if !func.unchecked {
                    let imin = b.ins().iconst(types::I64, i64::MIN);
                    let is_min = b.ins().icmp(IntCC::Equal, av, imin);
                    accumulate_sticky(&mut b, is_min);
                }
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
                    || ak.is_handle()
                    || bk.is_handle()
                    || !(known_nonfloat(ak) || known_nonfloat(bk))
                {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: float/handle/ambiguous comparison operands ({ak:?}, {bk:?}) — deferred"
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
                // A handle read is a BORROW (the slot keeps ownership) — mirrors the analyze arm.
                ub_push(&mut b, &vars, &fvars, &mut kinds, v, borrowed_copy(kind))?;
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
                // v1 default-deny (mirrors the analyze arm): a handle write needs free-the-old-value
                // semantics + alias analysis — rejected, VM fallback.
                if k.is_handle() || kinds[*slot].is_handle() {
                    return Err(JitError::Unsupported(
                        "unboxed: SetLocal on a handle slot (deferred)".to_string(),
                    ));
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
                    let (v, k) = ub_pop(&mut b, &vars, &fvars, &mut kinds)?;
                    // A handle arg would arrive as an untyped i64 param (mirrors the analyze arm).
                    if k.is_handle() {
                        return Err(JitError::Unsupported(
                            "unboxed: handle argument to Call (deferred)".to_string(),
                        ));
                    }
                    cargs.push(v);
                }
                cargs.reverse();
                let mut call_args: Vec<ClValue> = Vec::with_capacity(arity + 2);
                call_args.push(ctx);
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
