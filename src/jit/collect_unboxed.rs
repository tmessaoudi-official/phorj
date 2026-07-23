//! JIT — the unboxed collection + graph-resolution passes (M-Decomp split from `analyze.rs`,
//! Invariant 13).

use super::analyze::*;
use super::*;

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
pub(super) fn collect_functions_unboxed(
    program: &BytecodeProgram,
    entry_idx: usize,
) -> Result<(Vec<usize>, bool), JitError> {
    let mut order = Vec::new();
    let mut seen = vec![false; program.functions.len()];
    let mut work = vec![entry_idx];
    // The ENTRY is seeded with exactly `arity` args by `run_unboxed` — a capturing lambda
    // can only enter the graph as a `MakeClosure` target (its captures arrive as prepended
    // call args), never as the entry itself.
    if program.functions[entry_idx].n_captures != 0 {
        return Err(JitError::Unsupported(
            "unboxed: capturing entry (deferred)".to_string(),
        ));
    }
    // Does the graph use the P-2a handle space (string consts / MakeList / Index / Concat /
    // `String.length`)? Drives the `UbCtx` setup + helper imports in `compile_unboxed`.
    let mut uses_handles = false;
    while let Some(fi) = work.pop() {
        if seen[fi] {
            continue;
        }
        seen[fi] = true;
        let func = &program.functions[fi];
        if func.n_captures > 1 {
            return Err(JitError::Unsupported(
                "closure with 2+ captures".to_string(),
            ));
        }
        let code = &func.chunk.code;
        let reach = reachable(code);
        // The ??-fused maxBy/minBy windows: their six desugar ops (incl. the otherwise
        // unsupported `Const(Null)`/`Eq`-on-null) are consumed by the fused vertical — skip
        // them here exactly as analyze (`ip += 6`) and emit (`skip_ip`) do.
        let mut fused = vec![false; code.len()];
        for ip in 0..code.len() {
            if reach[ip]
                && crate::jit::extreme_by_coalesce_window(code, &func.chunk.consts, ip).is_some()
            {
                fused[ip + 1..=ip + 6].fill(true);
            }
        }
        // Float slice v1 is LEAF-only: the `Op::Call` arm models a callee's return as `Int`, so a float
        // value flowing through a call would mis-decode (a callee returning float, or a float arg). A
        // function that both touches floats AND calls is rejected (sound over-rejection; cross-function
        // float is a follow-up). Tracked per-function.
        let mut has_float = false;
        let mut has_call = false;
        // MUTATION GUARD (lever 3 + the ACL builders): a slot that FEEDS an `IterElems`
        // anywhere in the function must never be WRITTEN in the function. The VM's for-in
        // iterates a SNAPSHOT; the JIT iterates the live flat buffer (and an ACL append /
        // reseed mutates or recycles the record IN PLACE under the walker). Disjointness
        // makes the snapshot free — any overlap → the whole function stays on the VM.
        let mut iter_srcs: Vec<usize> = Vec::new();
        let mut writes: Vec<usize> = Vec::new();
        for (ip, op) in code.iter().enumerate() {
            if !reach[ip] || fused[ip] {
                continue;
            }
            match op {
                Op::Const(idx) => match func.chunk.consts.get(*idx) {
                    Some(Value::Int(_) | Value::Bool(_)) => {}
                    Some(Value::Float(_)) => has_float = true,
                    Some(Value::Str(_)) => uses_handles = true,
                    other => return Err(JitError::Unsupported(format!("unboxed Const {other:?}"))),
                },
                // P-2a handle verticals. Operand-KIND validation lives in `unboxed_analyze` /
                // `build_body_unboxed` (this walk only gates the op set).
                Op::MakeList(_)
                | Op::MakeMap(_)
                | Op::Index
                | Op::SetIndexLocal(_)
                | Op::Pop
                | Op::Len => uses_handles = true,
                Op::IterElems => {
                    uses_handles = true;
                    // MUTATION GUARD source: the iterable is a GetLocal borrow (the only
                    // Borrowed producer in the subset) — record the slot it came from.
                    if ip >= 1 {
                        if let Some(Op::GetLocal(src)) = code.get(ip - 1) {
                            iter_srcs.push(*src);
                        }
                    }
                }
                Op::Concat(n) if *n >= 2 => uses_handles = true,
                Op::CallNative(id, 1) if unboxed_native_is_str_len(*id) => uses_handles = true,
                Op::CallNative(id, 1) if unboxed_native_is_list_len(*id) => uses_handles = true,
                Op::CallNative(id, 1) if unboxed_native_is_to_string(*id) => uses_handles = true,
                Op::CallNative(id, 2) if unboxed_native_is_list_append(*id) => uses_handles = true,
                Op::CallNative(id, 2) if unboxed_native_is_map_has(*id) => uses_handles = true,
                // Map materialization verticals (mapkeys/mapvalues/mapmerge/mapsize flips).
                Op::CallNative(id, 1)
                    if unboxed_native_is_map_keys(*id)
                        || unboxed_native_is_map_values(*id)
                        || unboxed_native_is_map_size(*id) =>
                {
                    uses_handles = true
                }
                Op::CallNative(id, 2) if unboxed_native_is_map_merge(*id) => uses_handles = true,
                Op::CallNative(id, 1) if unboxed_native_is_set_of(*id) => uses_handles = true,
                Op::CallNative(id, 2) if unboxed_native_is_set_contains(*id) => uses_handles = true,
                Op::CallNative(id, 2) if unboxed_native_is_list_contains(*id) => {
                    uses_handles = true
                }
                Op::CallNative(id, 2) if unboxed_native_is_bridge2(*id) => uses_handles = true,
                // Set-op verticals (setdifference/setunion flips).
                Op::CallNative(id, 2)
                    if unboxed_native_is_set_union(*id)
                        || unboxed_native_is_set_difference(*id) =>
                {
                    uses_handles = true
                }
                Op::CallNative(id, 1) if unboxed_native_is_set_size(*id) => uses_handles = true,
                // String-scan verticals (stringcontains/isemail/isurl flips).
                Op::CallNative(id, 2) if unboxed_native_is_str_contains(*id) => uses_handles = true,
                Op::CallNative(id, 1) if unboxed_native_validate_which(*id).is_some() => {
                    uses_handles = true
                }
                // hofpipe: the HOF loop arms direct-call the compiled lambda per element.
                Op::CallNative(id, 2)
                    if unboxed_native_is_list_map(*id)
                        || unboxed_native_is_list_count(*id)
                        || unboxed_native_is_list_sum_by(*id)
                        || unboxed_native_is_list_filter(*id) =>
                {
                    uses_handles = true;
                    has_call = true;
                }
                // Map HOFs (mapmap/mapfilter flips): inline pair walk + direct call per entry.
                Op::CallNative(id, 2)
                    if unboxed_native_is_map_map(*id) || unboxed_native_is_map_filter(*id) =>
                {
                    uses_handles = true;
                    has_call = true;
                }
                // The ??-fused maxBy/minBy fold (window-gated in analyze; a window-less use
                // fails analysis and the whole graph stays on the VM).
                Op::CallNative(id, 2)
                    if unboxed_native_is_list_max_by(*id) || unboxed_native_is_list_min_by(*id) =>
                {
                    uses_handles = true;
                    has_call = true;
                }
                // hofpipe fold: `List.reduce(xs, seed, f)` — arity-3, same inline loop + direct call.
                Op::CallNative(id, 3) if unboxed_native_is_list_reduce(*id) => {
                    uses_handles = true;
                    has_call = true;
                }
                // P-2c numeric conversions: pure, handle-free, fully inline.
                Op::CallNative(id, 1)
                    if unboxed_native_is_to_float(*id) || unboxed_native_is_truncate(*id) =>
                {
                    has_float = true;
                }
                // `Math.max(int, int): int` — pure scalar signed max, inline `smax`. No handles,
                // no float, no call: sets no eligibility flags (just avoids the `other` reject).
                Op::CallNative(id, 2) if unboxed_native_is_math_max(*id) => {}
                // `Math.min(int, int): int` — pure scalar signed min, inline `smin`. Mirror of
                // max: no handles, no float, no call.
                Op::CallNative(id, 2) if unboxed_native_is_math_min(*id) => {}
                // `Math.sign(int): int` — pure scalar sign (-1/0/1), inline branchless icmp pair.
                // No handles, no float, no fault, no call.
                Op::CallNative(id, 1) if unboxed_native_is_math_sign(*id) => {}
                // `Math.abs(int): int` — pure scalar abs with an `i64::MIN` fault guard (code 5).
                // No handles, no float. `CallNative(..)` already forces `needs_fault_exit`, so the
                // guard's `fault_if` has its block; sets no other eligibility flags.
                Op::CallNative(id, 1) if unboxed_native_is_math_abs(*id) => {}
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
                // Enum vertical: register-pair enums (≤1 int payload — the arity gate here; the
                // payload/operand KIND gates live in `unboxed_analyze`). `Fault` is the
                // match-exhaustiveness backstop terminator → shared fault-exit, code 5.
                Op::MakeEnum(idx) => {
                    if program.enum_descs.get(*idx).is_none_or(|d| d.arity > 1) {
                        return Err(JitError::Unsupported(
                            "unboxed: MakeEnum arity > 1 (deferred)".to_string(),
                        ));
                    }
                }
                Op::MatchTag(_) | Op::GetEnumField(0) | Op::Fault(_) => {}
                // DEC-329.3 `MatchTagName` (the duck-typed `?`): a single tag compare is only
                // name-equivalent when exactly ONE descriptor bears the variant name — decline
                // (fail-closed, VM fallback) when the program shares it across enums.
                Op::MatchTagName(idx) => {
                    let name = &program.enum_descs[*idx].variant;
                    if program
                        .enum_descs
                        .iter()
                        .filter(|d| d.variant == *name)
                        .count()
                        > 1
                    {
                        return Err(JitError::Unsupported(
                            "unboxed: MatchTagName over a shared variant name (deferred)"
                                .to_string(),
                        ));
                    }
                }
                // Native try/catch (handler ranges are compile-time; Throw needs the arena
                // for its instance payload — any thrower constructed one, so uses_handles is
                // already set by its MakeInstance; keep it explicit for safety).
                Op::PushHandler(_) | Op::PopHandler | Op::IsInstance(_) => {}
                Op::Throw => uses_handles = true,
                // Object vertical: flat arena instances (the arena is the UbCtx — handle space
                // required). Kind/layout gates live in `unboxed_analyze`; `CallMethod` targets
                // are discovered by `resolve_unboxed_graph`'s fixpoint (receiver kinds needed).
                Op::MakeInstance(_) | Op::GetField(_) | Op::SetField(_) => uses_handles = true,
                Op::CallMethod(..) => {
                    has_call = true;
                    uses_handles = true;
                }
                // Closure vertical: collect the capture-free target into the graph; `CallValue`
                // is a direct call at emit time (counts as a call for the float-leaf gate).
                Op::MakeClosure(f) => {
                    if program
                        .functions
                        .get(*f)
                        .is_none_or(|fun| fun.n_captures > 1)
                    {
                        return Err(JitError::Unsupported(
                            "unboxed: closure with 2+ captures (deferred)".to_string(),
                        ));
                    }
                    work.push(*f);
                }
                Op::CallValue(_) => has_call = true,
                // Mutable locals: a read of any slot and a write (SetLocal) are both in the subset.
                // Slots are Cranelift Variables (widen-1 c1); their kind is proven by the analyze pass,
                // and a non-numeric-typed local reaching a `Return` fails the build (whole-graph fallback).
                Op::GetLocal(_) => {}
                Op::SetLocal(s) => writes.push(*s),
                Op::Call(callee) => {
                    has_call = true;
                    work.push(*callee);
                }
                other => return Err(JitError::Unsupported(format!("unboxed {other:?}"))),
            }
        }
        if iter_srcs.iter().any(|s| writes.contains(s)) {
            return Err(JitError::Unsupported(
                "unboxed: iterated local is also written (snapshot semantics — VM fallback)"
                    .to_string(),
            ));
        }
        if has_float && has_call {
            return Err(JitError::Unsupported(
                "unboxed: float ops + Call in one function (v1 float subset is leaf-only)"
                    .to_string(),
            ));
        }
        order.push(fi);
    }
    Ok((order, uses_handles))
}

/// Resolve the FULL unboxed graph: the op-gated function set ([`collect_functions_unboxed`]),
/// then the cross-function FIXPOINT — per-function return kinds (a ctor returns an instance,
/// so its callers' stacks must see `Inst`, not the pre-object `Int` assumption), method-body
/// `this` injection, and `CallMethod` target DISCOVERY (a resolved method body is op-gated and
/// joins the set, then its own callees do). Facts refine monotonically (ret kinds flip at most
/// from the `Int` assumption to the computed kind as callee facts arrive); the iteration cap is
/// a defensive backstop — hitting it falls back to the VM, never miscompiles.
pub(super) fn resolve_unboxed_graph(
    program: &BytecodeProgram,
    entry_idx: usize,
) -> Result<(Vec<usize>, bool, UbGraphInfo), JitError> {
    let (mut order, mut uses_handles) = collect_functions_unboxed(program, entry_idx)?;
    let mut info = UbGraphInfo::new(program.functions.len(), program.class_descs.len());
    let cap = program.functions.len() + 3;
    for _round in 0..cap {
        let mut changed = false;
        // An `Unsupported` mid-fixpoint may be STALE-fact-induced (a caller analyzed before its
        // ctor's `Inst` return is recorded sees `Int` and rejects `CallMethod` on it) — hold the
        // error and retry next round; it is fatal only once the facts stop changing.
        let mut pending_err: Option<(usize, JitError)> = None;
        let mut idx = 0;
        while idx < order.len() {
            let fi = order[idx];
            idx += 1;
            let proven = unboxed_proven_param_kinds(program, fi);
            let pk = info.param_kinds(
                fi,
                &proven,
                program.functions[fi].arity,
                &program.functions[fi].dyn_params,
            );
            let mut disc = UbDiscovery::default();
            let rk = match unboxed_analyze(program, fi, &pk, &info, &mut disc) {
                Ok(a) => a.ret_kind,
                Err(e @ JitError::Unsupported(_)) => {
                    // Held: the walk prefix's DISCOVERIES below still merge (they were
                    // validated before the failure point) — that's what breaks the
                    // caller-needs-callee / callee-needs-caller deadlock.
                    pending_err = Some((fi, e));
                    None
                }
                Err(e) => return Err(e),
            };
            if let Some(rk) = rk {
                if info.ret_kinds[fi] != Some(rk) {
                    info.ret_kinds[fi] = Some(rk);
                    changed = true;
                }
            }
            for (class, sig) in disc.inst_fields {
                match &info.field_kinds[class] {
                    None => {
                        info.field_kinds[class] = Some(sig);
                        changed = true;
                    }
                    Some(prev) if prev != &sig => {
                        // Element-wise join with `Unknown` as BOTTOM: a first-fixpoint-pass
                        // site records `Unknown` where a param proof hasn't landed yet — a
                        // later pass refines it. Two KNOWN kinds disagreeing (Int here, Str
                        // there) stays a hard fallback (no single static signature).
                        let Some(joined) = join_unknown_bottom(prev, &sig) else {
                            return Err(JitError::Unsupported(format!(
                                "unboxed: conflicting MakeInstance field kinds ({prev:?} vs {sig:?})"
                            )));
                        };
                        if &joined != prev {
                            info.field_kinds[class] = Some(joined);
                            changed = true;
                        }
                    }
                    Some(_) => {}
                }
            }
            for (callee, sig) in disc.call_sigs {
                match &info.param_over[callee] {
                    None => {
                        info.param_over[callee] = Some(sig);
                        changed = true;
                    }
                    Some(prev) if prev != &sig => {
                        let Some(joined) = join_unknown_bottom(prev, &sig) else {
                            return Err(JitError::Unsupported(
                                "unboxed: conflicting call argument kinds (deferred)".to_string(),
                            ));
                        };
                        if &joined != prev {
                            info.param_over[callee] = Some(joined);
                            changed = true;
                        }
                    }
                    Some(_) => {}
                }
            }
            for c in disc.throw_classes {
                match info.thrown_class {
                    None => {
                        info.thrown_class = Some(c);
                        changed = true;
                    }
                    Some(prev) if prev != c => {
                        return Err(JitError::Unsupported(
                            "unboxed: multiple thrown classes in one graph (deferred)".to_string(),
                        ));
                    }
                    Some(_) => {}
                }
            }
            for (target, class) in disc.method_calls {
                match info.this_inst[target] {
                    None => {
                        info.this_inst[target] = Some(class);
                        changed = true;
                    }
                    Some(prev) if prev != class => {
                        // One method body reached with two receiver classes — cannot inject a
                        // single `this` kind (also structurally impossible today: the methods
                        // table is per (class, name)). Fall back.
                        return Err(JitError::Unsupported(
                            "unboxed: method body reached from two classes (deferred)".to_string(),
                        ));
                    }
                    Some(_) => {}
                }
                if !order.contains(&target) {
                    // Op-gate the discovered method body + its transitive plain callees.
                    let (sub_order, sub_handles) = collect_functions_unboxed(program, target)?;
                    for t in sub_order {
                        if !order.contains(&t) {
                            order.push(t);
                        }
                    }
                    uses_handles |= sub_handles;
                    changed = true;
                }
            }
        }
        if !changed {
            return match pending_err {
                None => Ok((order, uses_handles, info)),
                // Facts are stable and a function still rejects — a genuine fallback. Name
                // the function so a whole-graph decline is diagnosable from the message.
                Some((fi, JitError::Unsupported(msg))) => Err(JitError::Unsupported(format!(
                    "{msg} [in `{}`]",
                    program.functions[fi].name
                ))),
                Some((_, e)) => Err(e),
            };
        }
    }
    Err(JitError::Unsupported(
        "unboxed: graph resolution did not converge (deferred)".to_string(),
    ))
}
