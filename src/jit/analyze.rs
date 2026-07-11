//! Kind-flow analysis for the UNBOXED path: compile-time operand kinds (`Kind`/`Own`),
//! provenance + range proofs, the depth-indexed stack model (`ub_push`/`ub_pop`), the per-leader
//! abstract-state pass (`unboxed_analyze`), and the unboxed op-subset collector.

use super::*;

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
pub(super) enum Kind {
    Int,
    /// A float operand, stored in a `vars` cell as its `f64` BITS (an `i64`); code `bitcast`s I64↔F64
    /// only at the float op that consumes/produces it, so the operand stack + local model stay
    /// uniformly `I64` and the ABI is unchanged (a float arg is passed as its bits, a float return
    /// decoded via [`Compiled::ret_kind`]). Float arithmetic never overflows (no sticky); only a
    /// zero-divisor `DivF` faults (→ code 5, redo on VM).
    Float,
    Bool,
    Unknown,
    /// A string HANDLE (P-2a helper-op vertical): an `i64` index into the per-run [`UbCtx`] handle
    /// table. Produced by a `Const(Str)` (a PINNED interned const — never freed), an `Index` into a
    /// `StrList`, or a `Concat` — the latter two allocate a fresh temp entry. Ownership is tracked at
    /// COMPILE time: an `Owned` operand is freed by the op that consumes it (or by `Pop`); a
    /// `Borrowed` one (a const, or a `GetLocal` copy of a slot's handle) is left alone — the slot /
    /// const table keeps it alive. Handle ops mutate ONLY the private per-run `UbCtx`, so the
    /// side-effect-free eligibility invariant (see [`is_eligible`]) holds: a fault-redo on the VM
    /// observes nothing.
    Str(Own),
    /// A `List<string>` handle (same table, same ownership discipline). Element kind is part of the
    /// variant (v1 verticals cover string lists only — a `MakeList` of anything else is rejected), so
    /// an `Index` result is provably `Str` without a type source.
    StrList(Own),
    /// A `Map<string, int>` handle (P-2b mapget vertical; same table, same ownership discipline).
    /// Key/value kinds are part of the variant — a `MakeMap` of anything else is rejected — so a
    /// string-subscripted `Index` result is provably `Int` without a type source. Runtime encoding:
    /// all-short-key maps seal FLAT (`UB_TAG_FLAT_MAP` — inline hash-probe lookup), the rest stay
    /// boxed `Value::Map` (helper lookup through the canonical `map_index` kernel).
    StrIntMap(Own),
}

/// Compile-time ownership of a handle operand — see [`Kind::Str`]. Part of `Kind`'s equality, so the
/// leader-state consistency check also enforces ownership agreement across merge edges (a mismatch
/// falls back to the VM, never double-frees).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Own {
    Owned,
    Borrowed,
}

impl Kind {
    /// Is this operand a handle into the per-run [`UbCtx`] table?
    pub(super) fn is_handle(self) -> bool {
        matches!(self, Kind::Str(_) | Kind::StrList(_) | Kind::StrIntMap(_))
    }
    /// Is this operand an OWNED handle (must be freed by its consumer)?
    pub(super) fn is_owned_handle(self) -> bool {
        matches!(
            self,
            Kind::Str(Own::Owned) | Kind::StrList(Own::Owned) | Kind::StrIntMap(Own::Owned)
        )
    }
}

/// The kind a `GetLocal` pushes for a slot of kind `k`: a handle read is a BORROW (the slot keeps
/// ownership — the copy's consumer must not free it); every other kind copies verbatim.
pub(super) fn borrowed_copy(k: Kind) -> Kind {
    match k {
        Kind::Str(_) => Kind::Str(Own::Borrowed),
        Kind::StrList(_) => Kind::StrList(Own::Borrowed),
        Kind::StrIntMap(_) => Kind::StrIntMap(Own::Borrowed),
        other => other,
    }
}

/// Is native-registry entry `id` the `Core.String.length` native (the sole `CallNative` in the P-2a
/// unboxed subset)? Matched by registry identity — the compiler emitted the id from the same
/// registry, so this can never alias another native.
pub(super) fn unboxed_native_is_str_len(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.String" && nf.name == "length" && nf.pure)
}

/// Provenance of an operand-stack entry for the provenance pre-pass ONLY (not codegen): `Param(slot)`
/// = a bare `GetLocal(slot)` result; `Other` = anything else (a `Const`, an arithmetic/comparison
/// result, a call result).
#[derive(Clone, Copy)]
pub(super) enum Prov {
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
pub(super) fn unboxed_proven_param_kinds(func: &crate::chunk::Function) -> Vec<Option<Kind>> {
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
pub(super) fn range_proven_ops(func: &crate::chunk::Function) -> Vec<bool> {
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
pub(super) fn ub_push(
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
pub(super) fn ub_pop(
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
pub(super) type LeaderStates = Vec<Option<Vec<Kind>>>;

pub(super) fn unboxed_analyze(
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
                    // Kind follows the const's type — MUST mirror build_body: Float for a float const,
                    // a BORROWED (pinned, never freed) handle for a string const, Int otherwise.
                    let k = match program.functions[func_idx].chunk.consts.get(*idx) {
                        Some(Value::Float(_)) => Kind::Float,
                        Some(Value::Str(_)) => Kind::Str(Own::Borrowed),
                        _ => Kind::Int,
                    };
                    kinds.push(k);
                }
                // P-2a handle verticals — mirror build_body's stack effects exactly (default-deny on
                // any operand-kind mismatch: fall back to the VM, never mis-type a handle).
                Op::MakeList(n) => {
                    for _ in 0..*n {
                        match kinds.pop() {
                            Some(Kind::Str(_)) => {}
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed MakeList element kind {other:?}"
                                )))
                            }
                        }
                    }
                    kinds.push(Kind::StrList(Own::Owned));
                }
                Op::MakeMap(n) => {
                    // The 2n operands are k1,v1,…,kn,vn (vn on top): pop value (Int) then key (Str),
                    // n times — anything else is default-denied (VM fallback).
                    for _ in 0..*n {
                        match kinds.pop() {
                            Some(Kind::Int) => {}
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed MakeMap value kind {other:?}"
                                )))
                            }
                        }
                        match kinds.pop() {
                            Some(Kind::Str(_)) => {}
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed MakeMap key kind {other:?}"
                                )))
                            }
                        }
                    }
                    kinds.push(Kind::StrIntMap(Own::Owned));
                }
                Op::Index => {
                    // The subscript kind selects the flavor: `Int` → string-list element (`Str`),
                    // `Str` → string-keyed map value (`Int`). Mirrors build_body's dispatch exactly.
                    match kinds.pop() {
                        Some(Kind::Int) => {
                            match kinds.pop() {
                                Some(Kind::StrList(_)) => {}
                                other => {
                                    return Err(JitError::Unsupported(format!(
                                        "unboxed Index receiver kind {other:?}"
                                    )))
                                }
                            }
                            kinds.push(Kind::Str(Own::Owned));
                        }
                        Some(Kind::Str(_)) => {
                            match kinds.pop() {
                                Some(Kind::StrIntMap(_)) => {}
                                other => {
                                    return Err(JitError::Unsupported(format!(
                                        "unboxed Index receiver kind {other:?}"
                                    )))
                                }
                            }
                            kinds.push(Kind::Int);
                        }
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed Index subscript kind {other:?}"
                            )))
                        }
                    }
                }
                Op::Concat(2) => {
                    for _ in 0..2 {
                        match kinds.pop() {
                            Some(Kind::Str(_)) => {}
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed Concat operand kind {other:?}"
                                )))
                            }
                        }
                    }
                    kinds.push(Kind::Str(Own::Owned));
                }
                Op::CallNative(id, 1) if unboxed_native_is_str_len(*id) => {
                    match kinds.pop() {
                        Some(Kind::Str(_)) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed String.length operand kind {other:?}"
                            )))
                        }
                    }
                    kinds.push(Kind::Int);
                }
                Op::Pop => {
                    kinds.pop();
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
                    // A handle read is a BORROW: the slot keeps ownership; the copy on the stack is
                    // never freed by its consumer (mirrors build_body's downgrade).
                    kinds.push(borrowed_copy(k));
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
                    // v1 default-deny: a handle write (or overwriting a slot that holds a handle)
                    // needs free-the-old-value semantics + alias analysis — rejected, VM fallback.
                    if k.is_handle() || kinds[*slot].is_handle() {
                        return Err(JitError::Unsupported(
                            "unboxed: SetLocal on a handle slot (deferred)".to_string(),
                        ));
                    }
                    kinds[*slot] = k;
                }
                Op::Call(callee) => {
                    for _ in 0..program.functions[*callee].arity {
                        // A handle arg would arrive at the callee as an untyped i64 param (proven-int
                        // usage could then do arithmetic on a handle INDEX) — reject, VM fallback.
                        if kinds.pop().is_some_and(Kind::is_handle) {
                            return Err(JitError::Unsupported(
                                "unboxed: handle argument to Call (deferred)".to_string(),
                            ));
                        }
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
pub(super) fn collect_functions_unboxed(
    program: &BytecodeProgram,
    entry_idx: usize,
) -> Result<(Vec<usize>, bool), JitError> {
    let mut order = Vec::new();
    let mut seen = vec![false; program.functions.len()];
    let mut work = vec![entry_idx];
    // Does the graph use the P-2a handle space (string consts / MakeList / Index / Concat /
    // `String.length`)? Drives the `UbCtx` setup + helper imports in `compile_unboxed`.
    let mut uses_handles = false;
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
                    Some(Value::Str(_)) => uses_handles = true,
                    other => return Err(JitError::Unsupported(format!("unboxed Const {other:?}"))),
                },
                // P-2a handle verticals. Operand-KIND validation lives in `unboxed_analyze` /
                // `build_body_unboxed` (this walk only gates the op set).
                Op::MakeList(_) | Op::MakeMap(_) | Op::Index | Op::Concat(2) | Op::Pop => {
                    uses_handles = true
                }
                Op::CallNative(id, 1) if unboxed_native_is_str_len(*id) => uses_handles = true,
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
    Ok((order, uses_handles))
}
