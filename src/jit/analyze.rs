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
    /// A `List<int>` handle (P-2c rollout): flat all-int lists store the raw `i64` in each slot's
    /// bytes 0..8 (the flat-map VALUE-slot layout), so `Index` is an inline bounds check + one
    /// load; boxed fallbacks go through the two-return `rt_u_index_int` helper.
    IntList(Own),
    /// An enum value with AT MOST ONE `Int` payload (the enum vertical), realized as TWO i64
    /// register words: the payload in the I64 space (`vars[d]`, filler 0 for a zero-payload
    /// variant) and the VARIANT TAG (its `enum_descs` index) in the tag space (`evars[d]`).
    /// ZERO-allocation: construct = two register defs, `MatchTag` = one compare,
    /// `GetEnumField(0)` = the payload word already in hand. Scalar-like (not a handle, no
    /// ownership, copy is free). Tag-index equality is equivalent to the VM's variant-name
    /// equality because the compiler's pre-pass dedups descriptors per (type, variant) and the
    /// checker guarantees a scrutinee is matched only against its own enum's variants.
    /// Multi-payload / non-int-payload variants are default-denied (collect + analyze).
    EnumInt,
    /// A CAPTURE-FREE first-class function value (the closure vertical): the target function
    /// index is carried entirely in the compile-time kind, so `CallValue` lowers to a DIRECT
    /// native call — no closure object, no indirection, zero allocation. The runtime word is a
    /// never-read filler. Capturing closures are default-denied (collect + analyze); two
    /// different targets merging at a leader disagree on the kind → VM fallback (sound).
    Fn(usize),
    /// A ONE-INT-CAPTURE first-class function value (the hofpipe vertical): the target index
    /// rides the compile-time kind and the runtime word in the cell IS the single captured
    /// `Int` — `MakeClosure` pops one capture and pushes this at the SAME depth, so the value
    /// is already in place: no closure object, no aux space, zero allocation. A consumer
    /// (the HOF loop arms) direct-calls the target with the capture PREPENDED as arg 0,
    /// matching the VM's `[caps.., args..]` lambda frame layout. ≥ 2 captures / non-int
    /// captures stay default-denied (collect + analyze).
    FnCap1(usize),
    /// Lever-3 pointer-walk iteration (the for-in desugar): the END pointer of a FLAT int
    /// list being iterated — the desugar's elems cell, rewritten at the `IterElems; Const(0)`
    /// init site. `Len` on it is an identity re-push (the bound IS the pointer), `Lt` against
    /// the cursor is one unsigned compare. Scalar-like (no ownership).
    IterEnd,
    /// Lever-3 pointer-walk iteration: the element CURSOR (the desugar's j cell). `Index`
    /// with it is ONE load (`ptr[0..8)` — flat slots keep the raw i64 in bytes 0..8);
    /// `j + 1` (`Const(1); AddI`) strength-reduces to `ptr + 64` (the slot stride). The
    /// mutation guard in `collect_functions_unboxed` proves the iterated slot is never
    /// written, so the list is always a bump-pinned FLAT snapshot (never ACL/boxed at
    /// runtime — a boxed one faults to code 5, redo on VM). Scalar-like (no ownership).
    IterPtr,
    /// A UNION-typed value (W7 — the `string | int | float | bool` param shape): TWO register
    /// words — the PAYLOAD in the I64 space (`vars[d]`; float = its bits, str = a handle) and
    /// the runtime TAG in the enum-tag space (`evars[d]`: 0 = int, 1 = float-bits, 2 = bool,
    /// 3 = str-handle). Produced at the fixpoint's param joins when call sites GENUINELY
    /// disagree on a scalar family (the sound form of what a silent unification could not
    /// do); consumed by tag-dispatched helpers (list append) and the tag-gated release.
    /// ABI: a Dyn param crosses as TWO i64 args (payload, tag).
    /// Ownership: MOVE-ONLY (no borrowed-Dyn kind exists — a copy would alias the owned str
    /// payload). Consumers that take the pair (append helper, a Dyn callee param) release
    /// the tag-3 payload; a Dyn cell still live at unwind/return LEAKS its payload — safe
    /// (arena exhaustion ⇒ code 5, redo on VM — never wrong bytes), same doctrine as the
    /// no-frees frame teardown, and unreachable for the read-once union-param shape.
    Dyn,
    /// A `List<union>` handle (always runtime-BOXED — built only by Dyn-element appends; an
    /// empty literal starts as a flat-empty StrList and the list-family join refines it).
    /// Same ownership discipline as the other list kinds; consumers: length, append, field
    /// store/read, call-arg move, borrowed-return clone. `Index` stays denied (deferred).
    DynList(Own),
    /// An INSTANCE handle (the object vertical): an arena SLOT (always slot-tagged — instances
    /// exist only via `MakeInstance` here or an injected method `this`), fields stored FLAT at
    /// byte `8·layout_slot` (≤ 8 int fields; the class index rides in the compile-time kind, so
    /// `GetField`/`SetField` are ONE inline load/store with a static offset and `CallMethod` is
    /// a statically-dispatched direct call). Subset gates: every field ctor-initialized
    /// (`desc.fields.len() == layout.len()` — no `None` window, so `GetField` is total) and
    /// int-valued. Ownership mirrors the string handles (`Owned` freed by consumer/`Pop` via
    /// the inline recycle ladder; `GetLocal` copies are `Borrowed`); `SetLocal` of an instance
    /// stays denied (aliasing). Returning an instance = OWNERSHIP TRANSFER, allowed only under
    /// the ctor-shaped gate in the `Return` arm.
    Inst(usize, Own),
}

/// Compile-time ownership of a handle operand — see [`Kind::Str`]. Part of `Kind`'s equality, so the
/// leader-state consistency check also enforces ownership agreement across merge edges (a mismatch
/// falls back to the VM, never double-frees).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Own {
    Owned,
    /// A copy whose runtime OWNED bit MAY be set (a `GetLocal` copy of an owned local): its
    /// consumer must NOT free it, and it can never merge with an `Owned` edge (releasing it
    /// there would recycle the still-live original — the double-free shape).
    Borrowed,
    /// A copy whose runtime OWNED bit is PROVABLY CLEAR (a pinned string const and its
    /// copies): releasing it anywhere is a runtime no-op, so an `Owned ⊔ ConstBorrow` merge
    /// safely joins to `Owned` — the declaration-initialized accumulator pattern
    /// (`mutable string s = ""; … s = s + x;`) hinges on exactly this join.
    ConstBorrow,
}

impl Own {
    /// The ownership a `GetLocal` copy carries: a const's copy is still provably bit-clear;
    /// everything else downgrades to the unjoinable `Borrowed`.
    pub(super) fn borrow_of(self) -> Own {
        if self == Own::ConstBorrow {
            Own::ConstBorrow
        } else {
            Own::Borrowed
        }
    }
}

impl Kind {
    /// Is this operand a handle into the per-run [`UbCtx`] table?
    pub(super) fn is_handle(self) -> bool {
        matches!(
            self,
            Kind::Str(_)
                | Kind::StrList(_)
                | Kind::StrIntMap(_)
                | Kind::IntList(_)
                | Kind::DynList(_)
                | Kind::Inst(..)
        )
    }
    /// Is this operand an OWNED handle (must be freed by its consumer)?
    pub(super) fn is_owned_handle(self) -> bool {
        matches!(
            self,
            Kind::Str(Own::Owned)
                | Kind::StrList(Own::Owned)
                | Kind::StrIntMap(Own::Owned)
                | Kind::IntList(Own::Owned)
                | Kind::DynList(Own::Owned)
                | Kind::Inst(_, Own::Owned)
        )
    }
}

/// The kind a `GetLocal` pushes for a slot of kind `k`: a handle read is a BORROW (the slot keeps
/// ownership — the copy's consumer must not free it); every other kind copies verbatim.
pub(super) fn borrowed_copy(k: Kind) -> Kind {
    match k {
        Kind::Str(o) => Kind::Str(o.borrow_of()),
        Kind::StrList(o) => Kind::StrList(o.borrow_of()),
        Kind::IntList(o) => Kind::IntList(o.borrow_of()),
        Kind::StrIntMap(o) => Kind::StrIntMap(o.borrow_of()),
        Kind::Inst(c, o) => Kind::Inst(c, o.borrow_of()),
        Kind::DynList(o) => Kind::DynList(o.borrow_of()),
        other => other,
    }
}

/// The FINAL per-slot param kinds of `fi` (usage proofs merged with the fixpoint's call-site
/// overrides) — the single source both the SIGNATURE builder and every CALL SITE derive the
/// ABI from: a `Dyn` param crosses as TWO i64 words (payload, tag), everything else as one.
pub(super) fn abi_param_kinds(
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    fi: usize,
) -> Vec<Kind> {
    let proven = unboxed_proven_param_kinds(program, fi);
    info.param_kinds(
        fi,
        &proven,
        program.functions[fi].arity,
        &program.functions[fi].dyn_params,
    )
}

/// Can `k` be a member of a tagged Dyn cell (the scalar union families)?
fn is_dynable(k: &Kind) -> bool {
    matches!(k, Kind::Int | Kind::Float | Kind::Bool | Kind::Str(_))
}

/// Is `k` any list-family kind (for the DynList-refining join)?
fn is_list_kind(k: &Kind) -> bool {
    matches!(k, Kind::StrList(_) | Kind::IntList(_) | Kind::DynList(_))
}

/// The kind a FIELD READ pushes for a field of kind `fk` on a receiver whose compile-time
/// ownership is `receiver_owned`: Int loads a raw word; a HANDLE field (str/list/map) BORROWS
/// from a live receiver but TAKES the word from a dying OWNED temp (the kinded release skips
/// that field — else the returned handle would dangle). `None` = not a supported field kind.
pub(super) fn field_read_kind(fk: Kind, receiver_owned: bool) -> Option<Kind> {
    let own = if receiver_owned {
        Own::Owned
    } else {
        Own::Borrowed
    };
    match fk {
        Kind::Int => Some(Kind::Int),
        Kind::Str(_) => Some(Kind::Str(own)),
        Kind::StrList(_) => Some(Kind::StrList(own)),
        Kind::IntList(_) => Some(Kind::IntList(own)),
        Kind::StrIntMap(_) => Some(Kind::StrIntMap(own)),
        Kind::DynList(_) => Some(Kind::DynList(own)),
        _ => None,
    }
}

/// Element-wise kind-vector join with `Unknown` as BOTTOM (the fixpoint's first-pass
/// placeholder): `Unknown ⊔ K = K`, identical kinds join to themselves, two KNOWN kinds
/// disagreeing → `None` (a genuine conflict — no single static signature exists).
pub(super) fn join_unknown_bottom(a: &[Kind], b: &[Kind]) -> Option<Vec<Kind>> {
    if a.len() != b.len() {
        return None;
    }
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| match (x, y) {
            _ if x == y => Some(*x),
            (Kind::Unknown, k) => Some(*k),
            (k, Kind::Unknown) => Some(*k),
            // W7: a UNION-typed param's call sites GENUINELY disagree on a scalar family —
            // join to the TAGGED two-word Dyn (each site passes (payload, tag); the callee
            // dispatches at runtime). This is the sound form of what a silent unification
            // could not do. Dyn absorbs further scalar evidence idempotently.
            (Kind::Dyn, k) | (k, Kind::Dyn) if is_dynable(k) => Some(Kind::Dyn),
            (a2, b2) if is_dynable(a2) && is_dynable(b2) => Some(Kind::Dyn),
            // List-family join: mixed list evidence (an `[]` starts as flat-empty StrList;
            // a Dyn append produces DynList) refines toward the BOXED DynList — every list
            // consumer of a DynList word is boxed/kind-agnostic at runtime, so widening the
            // compile-time kind is sound.
            (Kind::DynList(_), k) | (k, Kind::DynList(_)) if is_list_kind(k) => {
                Some(Kind::DynList(Own::Owned))
            }
            _ => None,
        })
        .collect()
}

/// Join two operand kinds at a merge edge. Identical kinds join to themselves. The SAME handle
/// family differing only between `Owned` and `ConstBorrow` joins to `Owned` — safe because a
/// release is runtime-bit-gated (freeing a provably-bit-clear const word is a no-op), so the
/// `Owned` side's frees are correct on both edges. `Borrowed` (bit UNKNOWN — may alias a live
/// owned local) never joins with `Owned`; `Borrowed ⊔ ConstBorrow` joins to `Borrowed` (neither
/// side frees). Anything else → `None` (VM fallback).
pub(super) fn join_kind(a: Kind, b: Kind) -> Option<Kind> {
    if a == b {
        return Some(a);
    }
    fn join_own(x: Own, y: Own) -> Option<Own> {
        match (x, y) {
            (a, b) if a == b => Some(a),
            (Own::Owned, Own::ConstBorrow) | (Own::ConstBorrow, Own::Owned) => Some(Own::Owned),
            (Own::Borrowed, Own::ConstBorrow) | (Own::ConstBorrow, Own::Borrowed) => {
                Some(Own::Borrowed)
            }
            _ => None,
        }
    }
    match (a, b) {
        (Kind::Str(x), Kind::Str(y)) => join_own(x, y).map(Kind::Str),
        (Kind::StrList(x), Kind::StrList(y)) => join_own(x, y).map(Kind::StrList),
        (Kind::StrIntMap(x), Kind::StrIntMap(y)) => join_own(x, y).map(Kind::StrIntMap),
        (Kind::IntList(x), Kind::IntList(y)) => join_own(x, y).map(Kind::IntList),
        (Kind::Inst(c1, x), Kind::Inst(c2, y)) if c1 == c2 => {
            join_own(x, y).map(|o| Kind::Inst(c1, o))
        }
        _ => None,
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

/// Is native-registry entry `id` `Core.List.length` (the listappend vertical: inline for a
/// flat handle — count bits — or an ACL builder record; helper for a boxed list)?
pub(super) fn unboxed_native_is_list_len(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "length" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.append` (the listappend vertical: at a PROVEN
/// accumulator site the consumed lhs becomes/extends an ACL builder record — in-place push,
/// php's `$xs[] =`; any other use stays on the VM)?
pub(super) fn unboxed_native_is_list_append(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "append" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.map` (the hofpipe vertical: a STATIC-lambda map
/// lowers to a native loop — inline element loads, a direct call per element, an ACL builder
/// output — no closure object, no VM re-entry)?
pub(super) fn unboxed_native_is_list_map(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "map" && nf.pure)
}

/// Is native-registry entry `id` `Core.List.count` (the hofpipe vertical: a STATIC-predicate
/// count lowers to a native loop — inline element loads, a direct call per element, a running
/// register sum)?
pub(super) fn unboxed_native_is_list_count(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.List" && nf.name == "count" && nf.pure)
}

/// Is `id` one of the pure 2-arg natives routed through the GENERIC `rt_u_native2` bridge
/// (which calls the registered native itself — single-sourced semantics)? Cheap name gate for
/// the match guards; the shape table is [`unboxed_native_bridge2`].
pub(super) fn unboxed_native_is_bridge2(id: usize) -> bool {
    crate::native::registry().get(id).is_some_and(|nf| {
        nf.pure
            && matches!(
                (nf.module, nf.name),
                ("Core.String", "join" | "contains" | "splitOnce") | ("Core.List", "drop")
            )
    })
}

/// The bridge-2 shape table: given the native and the two COMPILE-TIME operand kinds
/// (a = pushed first, b = second), return the `rt_u_native2` meta base (arg/result reprs —
/// see the helper's bit layout) and the result kind. `None` = kind mismatch (fail closed).
pub(super) fn unboxed_native_bridge2(id: usize, a: &Kind, b: &Kind) -> Option<(i64, Kind)> {
    let nf = crate::native::registry().get(id)?;
    match ((nf.module, nf.name), a, b) {
        (("Core.String", "join"), Kind::StrList(_), Kind::Str(_)) => {
            Some((3 | (2 << 3) | (2 << 6), Kind::Str(Own::Owned)))
        }
        (("Core.String", "contains"), Kind::Str(_), Kind::Str(_)) => {
            Some((2 | (2 << 3), Kind::Bool))
        }
        (("Core.String", "splitOnce"), Kind::Str(_), Kind::Str(_)) => {
            Some((2 | (2 << 3) | (3 << 6), Kind::StrList(Own::Owned)))
        }
        (("Core.List", "drop"), Kind::StrList(_), Kind::Int) => {
            Some((3 | (3 << 6), Kind::StrList(Own::Owned)))
        }
        (("Core.List", "drop"), Kind::IntList(_), Kind::Int) => {
            Some((4 | (4 << 6), Kind::IntList(Own::Owned)))
        }
        _ => None,
    }
}

/// Is native-registry entry `id` `Core.Conversion.toString` (INT operand only in this subset —
/// routed to the same zero-alloc `rt_u_int_to_str` renderer interpolation uses, so the bytes
/// can never drift from the VM's `as_display`)?
pub(super) fn unboxed_native_is_to_string(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Conversion" && nf.name == "toString" && nf.pure)
}

/// Is native-registry entry `id` `Core.Conversion.toFloat` (P-2c: inline `fcvt_from_sint` — the
/// kernel is `n as f64`, the same IEEE round-to-nearest widening)?
pub(super) fn unboxed_native_is_to_float(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Conversion" && nf.name == "toFloat" && nf.pure)
}

/// Is native-registry entry `id` `Core.Conversion.truncate` (P-2c: inline trunc + range guard +
/// `fcvt_to_sint`, mirroring `value::float_to_int` exactly — out-of-range/NaN/±∞ → code 5, the
/// VM redo renders the canonical fault)?
pub(super) fn unboxed_native_is_truncate(id: usize) -> bool {
    crate::native::registry()
        .get(id)
        .is_some_and(|nf| nf.module == "Core.Conversion" && nf.name == "truncate" && nf.pure)
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
pub(super) fn unboxed_proven_param_kinds(
    program: &BytecodeProgram,
    func_idx: usize,
) -> Vec<Option<Kind>> {
    let func = &program.functions[func_idx];
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
            // Enum payloads stay INT-gated; ctor FIELD values may now be handles (W-slices
            // 3+4), so a bare param feeding `MakeInstance` is left UNPROVEN — the call-site
            // override (`param_over`) supplies its true kind, and stamping `Int` here poisoned
            // the fixpoint recordings (an Int-placeholder is indistinguishable from a genuine
            // Int at the merge, which is exactly the union-param hazard).
            Op::MakeInstance(cidx) => {
                for _ in 0..program.class_descs[*cidx].fields.len() {
                    stack.pop();
                }
                stack.push(Prov::Other);
            }
            Op::MakeEnum(idx) => {
                for _ in 0..program.enum_descs[*idx].arity {
                    let p = stack.pop().unwrap_or(Prov::Other);
                    mark(&mut proven, p, Kind::Int);
                }
                stack.push(Prov::Other);
            }
            // Method/closure calls: args may be handles now (W-slice 4) — leave bare params
            // UNPROVEN (call-site overrides supply the kinds); the receiver / callee cell is
            // popped unmarked (it is an instance / fn value, not an int).
            Op::CallMethod(_, argc) => {
                for _ in 0..*argc {
                    stack.pop();
                }
                stack.pop();
                stack.push(Prov::Other);
            }
            Op::CallValue(argc) => {
                for _ in 0..*argc {
                    stack.pop();
                }
                stack.pop();
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

    // P-2c: prove `RemI` by a POSITIVE power-of-two const with a provably NON-NEGATIVE dividend —
    // then `x % 2^m ≡ x & (2^m - 1)` EXACTLY (truncated rem of a non-negative by a positive), and
    // both fault conditions (mod-zero, MIN % -1) are impossible, so the emitter may use a single
    // `band` with no checks. Non-negativity proof for `GetLocal(s)`:
    //  - slot `s`'s entry-prefix initializer is a const int ≥ 0 (see `entry_prefix_const_inits`);
    //  - every reachable `SetLocal(s)` writer is a PROVEN induction increment (`proven[w-1]` — the
    //    AddI pass above), so `s` only ever grows and (per that proof) never overflows;
    //    zero writers is also fine (a constant slot).
    // Any miss degrades to the checked `srem` path (safe).
    let inits = entry_prefix_const_inits(func, &reach);
    for k in 0..n {
        if !reach[k] || !matches!(code[k], Op::RemI) || k < 2 {
            continue;
        }
        let s = match code[k - 2] {
            Op::GetLocal(s) => s,
            _ => continue,
        };
        let pow2 = matches!(code[k - 1], Op::Const(ci)
            if matches!(func.chunk.consts.get(ci), Some(Value::Int(c)) if *c > 0 && (c & (c - 1)) == 0));
        if !pow2 {
            continue;
        }
        // MSRV 1.74: `Option::is_none_or` is 1.82+ — use `matches!` for the "known ≥ 0" test.
        if !matches!(inits.get(s).copied().flatten(), Some(v) if v >= 0) {
            continue;
        }
        let writers_ok = code.iter().enumerate().all(|(ip, op)| {
            !(reach[ip] && matches!(op, Op::SetLocal(t) if *t == s))
                || (ip >= 1 && matches!(code[ip - 1], Op::AddI) && proven[ip - 1])
        });
        if writers_ok {
            proven[k] = true;
        }
    }
    proven
}

/// The provable CONST-INT value of each frame slot at the end of the function's straight-line
/// entry prefix (before the first block leader): slots are frame-stack positions, so the prefix's
/// stack simulation identifies each declaration's initializer. Params and anything non-const or
/// past an unmodeled op are `None` (sound: a missed init only under-proves). Only ops the unboxed
/// collector admits are modeled; any other op ends the scan.
fn entry_prefix_const_inits(func: &crate::chunk::Function, reach: &[bool]) -> Vec<Option<i64>> {
    let code = &func.chunk.code;
    let is_leader = leaders(code, reach);
    let mut st: Vec<Option<i64>> = vec![None; func.arity];
    for (ip, op) in code.iter().enumerate() {
        if ip > 0 && is_leader[ip] {
            break;
        }
        match op {
            Op::Const(ci) => st.push(match func.chunk.consts.get(*ci) {
                Some(Value::Int(v)) => Some(*v),
                _ => None,
            }),
            Op::GetLocal(s) => st.push(st.get(*s).copied().flatten()),
            Op::SetLocal(s) => {
                let v = st.pop().flatten();
                if let Some(slot) = st.get_mut(*s) {
                    *slot = v;
                }
            }
            Op::MakeList(n) => {
                st.truncate(st.len().saturating_sub(*n));
                st.push(None);
            }
            Op::MakeMap(n) => {
                st.truncate(st.len().saturating_sub(2 * n));
                st.push(None);
            }
            Op::Concat(n) => {
                st.truncate(st.len().saturating_sub(*n));
                st.push(None);
            }
            Op::AddI
            | Op::SubI
            | Op::MulI
            | Op::DivI
            | Op::RemI
            | Op::AddF
            | Op::SubF
            | Op::MulF
            | Op::DivF
            | Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Gt
            | Op::Le
            | Op::Ge
            | Op::Index => {
                st.truncate(st.len().saturating_sub(2));
                st.push(None);
            }
            Op::Neg | Op::Not => {
                st.pop();
                st.push(None);
            }
            Op::Pop => {
                st.pop();
            }
            _ => break, // unmodeled op: stop (later slots stay unproven — sound)
        }
    }
    st
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

/// Per-graph cross-function facts for the unboxed path, produced by [`resolve_unboxed_graph`]'s
/// fixpoint: each function's return KIND (`None` until computed — callers assume `Int`) and,
/// for method bodies, the receiver class injected as param 0 (`this`). Both are read by the
/// analyze pass AND by `build_body_unboxed` (which re-runs analyze on the stable facts).
#[derive(Clone, Debug, Default)]
pub(super) struct UbGraphInfo {
    pub(super) ret_kinds: Vec<Option<Kind>>,
    pub(super) this_inst: Vec<Option<usize>>,
    /// Per-CLASS field kinds (layout-slot order NOT — ctor push order; every `MakeInstance`
    /// site of a class must agree, ownership-normalized). `Int` fields are raw words; `Str`
    /// fields are handle words the instance OWNS (released with it, runtime-bit-gated).
    pub(super) field_kinds: Vec<Option<Vec<Kind>>>,
    /// Per-FUNCTION param-kind overrides recorded from CALL SITES that pass handle args
    /// (a string argument MOVES into the callee — normalized `Str(Owned)`); all sites must
    /// agree. `None` = no override (usage-proven kinds apply); `Unknown` cells = no override
    /// for that one param.
    pub(super) param_over: Vec<Option<Vec<Kind>>>,
    /// The graph-wide THROWN class (v1: a single throwable class per graph, else fallback) —
    /// types every catch pad's incoming value.
    pub(super) thrown_class: Option<usize>,
}

impl UbGraphInfo {
    pub(super) fn new(n: usize, n_classes: usize) -> Self {
        Self {
            ret_kinds: vec![None; n],
            this_inst: vec![None; n],
            field_kinds: vec![None; n_classes],
            param_over: vec![None; n],
            thrown_class: None,
        }
    }
    /// The kind a `GetField` of ctor-push-position `j` on class `c` yields (`None` = the
    /// class's signature is not yet recorded — the fixpoint retries).
    pub(super) fn field_kind(&self, c: usize, j: usize) -> Option<Kind> {
        self.field_kinds.get(c)?.as_ref()?.get(j).copied()
    }
    /// The kind a caller's stack receives from calling `callee` (`Int` until the fixpoint
    /// fills it — the pre-object behavior, so pure-int graphs converge in one pass).
    pub(super) fn ret_of(&self, callee: usize) -> Kind {
        self.ret_kinds
            .get(callee)
            .copied()
            .flatten()
            .unwrap_or(Kind::Int)
    }
    /// Effective param kinds for `func_idx`: the usage-proven seeds, with a method body's
    /// slot 0 overridden to its receiver class (`this` arrives as a BORROWED instance handle)
    /// and declared scalar-union params seeded `Dyn` (`dyn_params` — the compiler-stamped
    /// checker fact; slot-aligned, so it applies LAST and wins over both usage proofs and
    /// call-site overrides: the declaration is ground truth, and Dyn is the superset every
    /// dynable site kind crosses into).
    pub(super) fn param_kinds(
        &self,
        func_idx: usize,
        proven: &[Option<Kind>],
        arity: usize,
        dyn_params: &[bool],
    ) -> Vec<Kind> {
        let mut pk: Vec<Kind> = (0..arity)
            .map(|s| proven.get(s).copied().flatten().unwrap_or(Kind::Unknown))
            .collect();
        // Call-site-recorded overrides (handle args) beat usage proofs (a str param feeding
        // MakeInstance is usage-proven "Int" by the conservative pre-pass — the override wins).
        if let Some(Some(over)) = self.param_over.get(func_idx) {
            for (s, k) in over.iter().enumerate() {
                if *k != Kind::Unknown {
                    if let Some(slot) = pk.get_mut(s) {
                        *slot = *k;
                    }
                }
            }
        }
        if let Some(c) = self.this_inst.get(func_idx).copied().flatten() {
            if let Some(p0) = pk.get_mut(0) {
                *p0 = Kind::Inst(c, Own::Borrowed);
            }
        }
        // W7: the declared-union seed — WITHOUT it, a mid-chain method that both takes and
        // consumes the union param deadlocks the fixpoint (its walk can't finish until the
        // param is Dyn; the param can't join to Dyn until later chain sites are reached).
        for (s, is_dyn) in dyn_params.iter().enumerate() {
            if *is_dyn {
                if let Some(slot) = pk.get_mut(s) {
                    *slot = Kind::Dyn;
                }
            }
        }
        pk
    }
}

/// `unboxed_analyze`'s result: the per-leader states, the max stack depth, the function's
/// return kind (`None` when no reachable `Return` produced one), and the statically-resolved
/// `CallMethod` sites `(target fn, receiver class)` — the graph fixpoint's discovery feed.
/// `unboxed_analyze`'s result bundle.
pub(super) struct UbAnalysis {
    pub(super) leader_state: LeaderStates,
    pub(super) max_depth: usize,
    /// The function's return kind (`None` when no reachable `Return` produced one).
    pub(super) ret_kind: Option<Kind>,
    /// Operand-stack depth before each reachable op (`accumulator_site`'s positional input).
    pub(super) depth_at: Vec<Option<usize>>,
}

/// Cross-function facts DISCOVERED during an analyze walk, harvested through a `&mut` OUT
/// param so facts recorded BEFORE a held-`Unsupported` failure survive it — the fixpoint's
/// escape from chicken-egg deadlocks (a caller records its ctor call signature even though
/// its own analysis then fails on a not-yet-known method return; the ctor's next round then
/// has its param kinds). Every recorded fact was validated by the sound walk prefix; merge
/// rules are idempotent, so leader re-walk duplicates are harmless.
#[derive(Default)]
pub(super) struct UbDiscovery {
    /// Statically-resolved `CallMethod` sites `(target fn, receiver class)`.
    pub(super) method_calls: Vec<(usize, usize)>,
    /// Observed `MakeInstance` field-kind signatures `(class, kinds in ctor push order)`.
    pub(super) inst_fields: Vec<(usize, Vec<Kind>)>,
    /// Observed CALL argument signatures for calls passing handle args `(callee, arg kinds
    /// in declaration order, ownership-normalized)`.
    pub(super) call_sigs: Vec<(usize, Vec<Kind>)>,
    /// Classes observed at `Throw` sites — merged into the graph-wide thrown-class singleton.
    pub(super) throw_classes: Vec<usize>,
}

/// Detect the FUSED-ACCUMULATOR shape at a Concat(2) site: GetLocal(s); ..rhs..; Concat(2);
/// SetLocal(s) all inside ONE basic block, where the lhs cell is the untouched GetLocal(s)
/// copy and s is not rewritten in between. The peephole lets emit treat the borrow as
/// CONSUMED (s's old value dies at the immediately-following SetLocal) - which is what
/// unlocks the helper's in-place append. Purely positional: depth_at (the analyze walk's
/// per-ip depth) locates the pusher of the lhs cell; liveness (the cell never popped) and
/// interference (no SetLocal(s)) are re-checked op-by-op. Returns the slot s.
/// Per-PARAM "single-use move" eligibility: param `p` is read by exactly ONE reachable
/// `GetLocal(p)` and never rewritten (`SetLocal(p)` count 0). Such a read can transfer the
/// param's OWNERSHIP (kind pushed Owned, the slot dies to `Unknown`) — how an owned string
/// argument flows through a ctor into an instance field without borrow aliasing. Pure count —
/// analyze and emit recompute it identically.
/// The innermost ACTIVE handler pad for each ip — a LEXICAL scan over `PushHandler`/
/// `PopHandler` (compiler-emitted try/catch is block-structured: a push at try-entry, a pop
/// at try-exit, the pad after; nesting nests; control flow never jumps into a try from
/// outside). `None` = no handler covers the ip (a throw there leaves the function as code 6).
/// The pad itself is OUTSIDE its own range (the VM pops the handler when unwinding to it, so
/// a rethrow in the pad targets the OUTER handler) — the pop precedes the pad lexically.
pub(super) fn handler_ranges(code: &[Op]) -> Vec<Option<usize>> {
    let mut out = vec![None; code.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (ip, op) in code.iter().enumerate() {
        match op {
            Op::PushHandler(t) => {
                out[ip] = stack.last().copied();
                stack.push(*t);
            }
            Op::PopHandler => {
                stack.pop();
                out[ip] = stack.last().copied();
            }
            _ => out[ip] = stack.last().copied(),
        }
    }
    out
}

pub(super) fn single_use_params(func: &crate::chunk::Function) -> Vec<bool> {
    let code = &func.chunk.code;
    let reach = reachable(code);
    let mut gets = vec![0usize; func.arity];
    let mut sets = vec![0usize; func.arity];
    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        match op {
            Op::GetLocal(s) if *s < func.arity => gets[*s] += 1,
            Op::SetLocal(s) if *s < func.arity => sets[*s] += 1,
            _ => {}
        }
    }
    (0..func.arity)
        .map(|p| gets[p] == 1 && sets[p] == 0)
        .collect()
}

pub(super) fn accumulator_site(
    code: &[Op],
    depth_at: &[Option<usize>],
    is_leader: &[bool],
    ip: usize,
) -> Option<usize> {
    let Some(Op::SetLocal(s)) = code.get(ip + 1) else {
        return None;
    };
    // The SetLocal must be same-block (a jump target between the two would observe the
    // un-fused intermediate state).
    if is_leader.get(ip + 1).copied().unwrap_or(true) {
        return None;
    }
    let d = depth_at.get(ip).copied().flatten()?;
    if d < 2 {
        return None;
    }
    let lhs_index = d - 2;
    let mut j = ip;
    loop {
        if j == 0 {
            return None;
        }
        j -= 1;
        let dj = depth_at.get(j).copied().flatten()?;
        let dj1 = depth_at.get(j + 1).copied().flatten()?;
        if dj == lhs_index && dj1 == lhs_index + 1 {
            // code[j] pushed the lhs cell. It must be the untouched borrow of s.
            let Op::GetLocal(src) = code[j] else {
                return None;
            };
            if src != *s {
                return None;
            }
            // Interference: s must not be rewritten between the borrow and the SetLocal.
            for op in &code[j + 1..ip] {
                if matches!(op, Op::SetLocal(x) if x == s) {
                    return None;
                }
            }
            return Some(*s);
        }
        // The cell was popped (or never live) below this point - not the accumulator shape.
        if dj <= lhs_index || dj1 <= lhs_index {
            return None;
        }
        // Hit the block leader without finding the pusher.
        if is_leader[j] {
            return None;
        }
    }
}

pub(super) fn unboxed_analyze(
    program: &BytecodeProgram,
    func_idx: usize,
    param_kinds: &[Kind],
    info: &UbGraphInfo,
    disc: &mut UbDiscovery,
) -> Result<UbAnalysis, JitError> {
    let code = &program.functions[func_idx].chunk.code;
    let n = code.len();
    let reach = reachable(code);
    let is_leader = leaders(code, &reach);

    let mut leader_state: LeaderStates = vec![None; n];
    let mut max_depth = param_kinds.len();
    // The function's return kind (all reachable `Return`s must agree, instance returns
    // normalized to `Owned`) + the statically-resolved `CallMethod` sites discovered
    // (`(target fn, receiver class)`) — the fixpoint's discovery feed.
    let mut ret_kind: Option<Kind> = None;
    // A `PushHandler` hit before the graph's thrown class was known — the walk CONTINUES
    // (its discoveries must land this round or the fixpoint deadlocks) and the held error
    // fires at the end instead.
    let mut pad_deferred = false;
    // Single-use param moves (see `single_use_params`): a unique read of an Owned-handle
    // param TRANSFERS ownership and kills the slot.
    let single_use = single_use_params(&program.functions[func_idx]);
    // Operand-stack depth BEFORE each reachable op (accumulator_site's positional input).
    // A leader re-walk (an ownership join) records the same depths - only kinds widen.
    let mut depth_at: Vec<Option<usize>> = vec![None; n];
    if n == 0 {
        return Ok(UbAnalysis {
            leader_state,
            max_depth,
            ret_kind,
            depth_at,
        });
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
                // Element-wise JOIN (see `join_kind`): the declaration-initialized accumulator
                // pattern merges a ConstBorrow entry edge with an Owned back edge — joined to
                // Owned (safe: releases are runtime-bit-gated). A widened state re-enqueues the
                // leader (the lattice is 2 levels deep per cell — converges immediately).
                if existing.len() != out.len() {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: inconsistent operand stack depth at leader ip {target} ({existing:?} vs {out:?})"
                    )));
                }
                let joined: Option<Vec<Kind>> = existing
                    .iter()
                    .zip(out.iter())
                    .map(|(&a, &b)| join_kind(a, b))
                    .collect();
                let Some(joined) = joined else {
                    return Err(JitError::Unsupported(format!(
                        "unboxed: inconsistent operand stack at leader ip {target} ({existing:?} vs {out:?})"
                    )));
                };
                if joined.as_slice() != existing.as_slice() {
                    leader_state[target] = Some(joined);
                    work.push(target);
                }
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
            depth_at[ip] = Some(kinds.len());
            match &code[ip] {
                Op::Const(idx) => {
                    // Kind follows the const's type — MUST mirror build_body: Float for a float const,
                    // a BORROWED (pinned, never freed) handle for a string const, Int otherwise.
                    let k = match program.functions[func_idx].chunk.consts.get(*idx) {
                        Some(Value::Float(_)) => Kind::Float,
                        Some(Value::Str(_)) => Kind::Str(Own::ConstBorrow),
                        Some(Value::Bool(_)) => Kind::Bool,
                        _ => Kind::Int,
                    };
                    kinds.push(k);
                }
                // P-2a handle verticals — mirror build_body's stack effects exactly (default-deny on
                // any operand-kind mismatch: fall back to the VM, never mis-type a handle).
                Op::MakeList(n) => {
                    // Element kinds select the list flavor: all-`Str` → `StrList`, all-`Int` →
                    // `IntList` (P-2c); anything else (mixed, floats, nested) is default-denied.
                    let d = kinds.len();
                    if *n > d {
                        return Err(JitError::Codegen("unboxed MakeList underflow".to_string()));
                    }
                    let all_str = kinds[d - n..].iter().all(|k| matches!(k, Kind::Str(_)));
                    let all_int = *n > 0 && kinds[d - n..].iter().all(|k| *k == Kind::Int);
                    if !(all_str || all_int) {
                        return Err(JitError::Unsupported(format!(
                            "unboxed MakeList element kinds {:?}",
                            &kinds[d - n..]
                        )));
                    }
                    kinds.truncate(d - n);
                    kinds.push(if all_int {
                        Kind::IntList(Own::Owned)
                    } else {
                        Kind::StrList(Own::Owned)
                    });
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
                Op::Index if matches!(kinds.last(), Some(Kind::IterPtr)) => {
                    // Lever 3: `elems[j]` with a pointer cursor — ONE load (the loop guard is
                    // the bounds proof; the desugar never indexes past the cursor).
                    kinds.pop();
                    match kinds.pop() {
                        Some(Kind::IterEnd) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed: iter-cursor Index receiver {other:?} (desugar drift)"
                            )))
                        }
                    }
                    kinds.push(Kind::Int);
                }
                Op::Index => {
                    // The subscript kind selects the flavor: `Int` → list element (`Str` from a
                    // `StrList`, `Int` from an `IntList`), `Str` → string-keyed map value (`Int`).
                    // Mirrors build_body's dispatch exactly.
                    match kinds.pop() {
                        Some(Kind::Int) => match kinds.pop() {
                            Some(Kind::StrList(_)) => kinds.push(Kind::Str(Own::Owned)),
                            Some(Kind::IntList(_)) => kinds.push(Kind::Int),
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed Index receiver kind {other:?}"
                                )))
                            }
                        },
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
                Op::IterElems => {
                    // Lever-3 POINTER-WALK init (mirrors the emit arm exactly): an INT list at
                    // the desugar's `IterElems; Const(0)` site becomes (end, cursor) pointer
                    // cells — Len/Lt/Index/j+1 then strength-reduce per-op. The collect-walk's
                    // mutation guard proves the iterated slot is never written, so the list is
                    // a flat snapshot. Gate: the Const(0) is same-block (not a leader).
                    let ptr_walk = matches!(kinds.last(), Some(Kind::IntList(Own::Borrowed)))
                        && !is_leader.get(ip + 1).copied().unwrap_or(true)
                        && matches!(code.get(ip + 1), Some(Op::Const(c))
                        if matches!(
                            program.functions[func_idx].chunk.consts.get(*c),
                            Some(Value::Int(0))
                        ));
                    if ptr_walk {
                        kinds.pop();
                        kinds.push(Kind::IterEnd);
                        kinds.push(Kind::IterPtr);
                        ip += 1; // consume the Const(0) — the emit mirrors with skip_ip
                    } else {
                        // For-in lowering (B1): the iterable normalizes to its element list. A
                        // BORROWED flat-able Str list handle IS that snapshot (sealed lists
                        // are immutable within this subset) — identity. Owned/other iterables
                        // → VM fallback (fail closed).
                        match kinds.pop() {
                            Some(Kind::StrList(Own::Borrowed)) => {
                                kinds.push(Kind::StrList(Own::Borrowed))
                            }
                            Some(Kind::IntList(Own::Borrowed)) => {
                                kinds.push(Kind::IntList(Own::Borrowed))
                            }
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed IterElems operand kind {other:?}"
                                )))
                            }
                        }
                    }
                }
                Op::Len => {
                    // List length (the for-in inner-loop bound): BORROWED list → Int; an
                    // IterEnd bound re-pushes itself (the pointer IS the bound — lever 3).
                    match kinds.pop() {
                        Some(Kind::IterEnd) => kinds.push(Kind::IterEnd),
                        Some(Kind::StrList(Own::Borrowed)) | Some(Kind::IntList(Own::Borrowed)) => {
                            kinds.push(Kind::Int)
                        }
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed Len operand kind {other:?}"
                            )))
                        }
                    }
                }
                Op::Concat(n) if *n >= 2 => {
                    // Mixed interpolation: `Str` operands concatenate; an `Int` operand renders
                    // to its decimal string first (`rt_u_int_to_str` — always inline-short).
                    // Anything else (floats/bools/handles) → VM fallback.
                    for _ in 0..*n {
                        match kinds.pop() {
                            Some(Kind::Str(_)) | Some(Kind::Int) => {}
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
                Op::CallNative(id, 1) if unboxed_native_is_list_len(*id) => {
                    // Any list kind, any ownership — mirrors `arm_list_len` exactly. An
                    // OWNED operand dies here (measure-then-free at emit — the
                    // `List.length(q.params())` shape: length of a fresh clone).
                    match kinds.pop() {
                        Some(Kind::IntList(_))
                        | Some(Kind::StrList(_))
                        | Some(Kind::DynList(_)) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed List.length operand kind {other:?}"
                            )))
                        }
                    }
                    kinds.push(Kind::Int);
                }
                Op::CallNative(id, 2)
                    if unboxed_native_is_list_append(*id)
                        && (matches!(kinds.last(), Some(Kind::Dyn))
                            || matches!(
                                kinds.get(kinds.len().wrapping_sub(2)),
                                Some(Kind::DynList(_))
                            )) =>
                {
                    // W7: a DYN element (tag-dispatched) or a DynList receiver — the boxed
                    // tag-aware clone-append helper; result = a fresh Owned DynList.
                    let elem = kinds.pop();
                    let lhs = kinds.pop();
                    let elem_ok = matches!(
                        elem,
                        Some(
                            Kind::Dyn
                                | Kind::Int
                                | Kind::Float
                                | Kind::Bool
                                | Kind::Str(Own::Owned)
                                | Kind::Str(Own::ConstBorrow)
                        )
                    );
                    let lhs_ok = matches!(
                        lhs,
                        Some(Kind::StrList(_) | Kind::IntList(_) | Kind::DynList(_))
                    );
                    if !elem_ok || !lhs_ok {
                        return Err(JitError::Unsupported(format!(
                            "unboxed dyn List.append operand kinds ({lhs:?}, {elem:?})"
                        )));
                    }
                    kinds.push(Kind::DynList(Own::Owned));
                }
                Op::CallNative(id, 2)
                    if unboxed_native_is_list_append(*id)
                        && accumulator_site(code, &depth_at, &is_leader, ip).is_some()
                        && matches!(kinds.last(), Some(Kind::Int))
                        && matches!(
                            kinds.get(kinds.len().wrapping_sub(2)),
                            Some(Kind::IntList(_))
                        ) =>
                {
                    // The listappend vertical: ONLY at a proven accumulator site (the lhs is
                    // the dying borrow of the slot the following SetLocal rewrites) AND the
                    // int-list shape — the emit consumes it into an ACL builder record. A
                    // non-int accumulator site (a str-list `out = append(out, q)` loop)
                    // falls THROUGH to the general clone arm below (guard order mirrors
                    // emit exactly).
                    match (kinds.pop(), kinds.pop()) {
                        (Some(Kind::Int), Some(Kind::IntList(_))) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed List.append operand kinds {other:?}"
                            )))
                        }
                    }
                    kinds.push(Kind::IntList(Own::Owned));
                }
                Op::CallNative(id, 2) if unboxed_native_is_list_append(*id) => {
                    // The GENERAL (non-accumulator) `List.append` — full clone semantics via
                    // `rt_u_list_append_clone` (a fresh BOXED list; inputs untouched), exactly
                    // the VM native. Mirrors the emit arm (fail closed).
                    match (kinds.pop(), kinds.pop()) {
                        (Some(Kind::Int), Some(Kind::IntList(_))) => {
                            kinds.push(Kind::IntList(Own::Owned));
                        }
                        (Some(Kind::Str(_)), Some(Kind::StrList(_))) => {
                            kinds.push(Kind::StrList(Own::Owned));
                        }
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed List.append operand kinds {other:?}"
                            )))
                        }
                    }
                }
                Op::CallNative(id, 2)
                    if unboxed_native_is_list_map(*id) || unboxed_native_is_list_count(*id) =>
                {
                    // The hofpipe vertical: a STATIC-target lambda (capture-free `Fn` or
                    // one-int-capture `FnCap1`) over an int list lowers to a native loop with
                    // a direct call per element. `map` needs an Int-returning transform (the
                    // output is an ACL int-list builder); `count` a Bool/Int predicate.
                    // v1: HOF loops don't route thrown payloads out of the loop body — a
                    // throwing graph stays on the VM (fail closed).
                    if info.thrown_class.is_some() {
                        return Err(JitError::Unsupported(
                            "unboxed: List HOF in a throwing graph (deferred)".to_string(),
                        ));
                    }
                    let is_map = unboxed_native_is_list_map(*id);
                    let f = match kinds.pop() {
                        Some(Kind::Fn(f)) | Some(Kind::FnCap1(f)) => f,
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed List HOF callee kind {other:?} (deferred)"
                            )))
                        }
                    };
                    // `arity` folds captures in (frame = [caps.., args..]) — the HOF passes
                    // exactly one element arg, so declared params must be 1.
                    if program.functions[f].arity - program.functions[f].n_captures != 1 {
                        return Err(JitError::Unsupported(
                            "unboxed: List HOF lambda arity != 1 (VM renders any fault)"
                                .to_string(),
                        ));
                    }
                    let rk = info.ret_of(f);
                    // map: Int only; count: Bool or Int.
                    if rk != Kind::Int && (is_map || rk != Kind::Bool) {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: List HOF lambda return kind {rk:?} (deferred)"
                        )));
                    }
                    match kinds.pop() {
                        Some(Kind::IntList(_)) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed List HOF receiver kind {other:?}"
                            )))
                        }
                    }
                    kinds.push(if is_map {
                        Kind::IntList(Own::Owned)
                    } else {
                        Kind::Int
                    });
                }
                Op::CallNative(id, 2) if unboxed_native_is_bridge2(*id) => {
                    // The generic pure-native bridge (join/contains/splitOnce/drop) — the
                    // helper calls the registered native itself; kinds from the shape table.
                    let b2 = kinds.pop();
                    let a2 = kinds.pop();
                    match (a2, b2) {
                        (Some(ka), Some(kb)) => match unboxed_native_bridge2(*id, &ka, &kb) {
                            Some((_, out)) => kinds.push(out),
                            None => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed bridge2 native operand kinds ({ka:?}, {kb:?})"
                                )))
                            }
                        },
                        other => {
                            return Err(JitError::Codegen(format!(
                                "unboxed analyze: bridge2 underflow {other:?}"
                            )))
                        }
                    }
                }
                Op::CallNative(id, 1) if unboxed_native_is_to_string(*id) => {
                    // `Conversion.toString(int)` — the interpolation renderer's exact bytes.
                    match kinds.pop() {
                        Some(Kind::Int) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed toString operand kind {other:?} (deferred)"
                            )))
                        }
                    }
                    kinds.push(Kind::Str(Own::Owned));
                }
                Op::CallNative(id, 1) if unboxed_native_is_to_float(*id) => {
                    match kinds.pop() {
                        Some(Kind::Int) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed toFloat operand kind {other:?}"
                            )))
                        }
                    }
                    kinds.push(Kind::Float);
                }
                Op::CallNative(id, 1) if unboxed_native_is_truncate(*id) => {
                    match kinds.pop() {
                        Some(Kind::Float) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed truncate operand kind {other:?}"
                            )))
                        }
                    }
                    kinds.push(Kind::Int);
                }
                Op::Pop => {
                    // W7: a discarded Dyn would need a tag-gated payload release — no
                    // consumer produces the shape yet (union params are read, not discarded).
                    if matches!(kinds.last(), Some(Kind::Dyn)) {
                        return Err(JitError::Unsupported(
                            "unboxed: Pop of a union (Dyn) value (deferred)".to_string(),
                        ));
                    }
                    kinds.pop();
                }
                Op::AddI
                    if matches!(kinds.get(kinds.len().wrapping_sub(2)), Some(Kind::IterPtr)) =>
                {
                    // Lever 3: the desugar's `j + 1` (always `Const(1); AddI`) strength-reduces
                    // to `cursor + 64` (the flat slot stride) — mirror the emit gate exactly.
                    if !(ip >= 1
                        && matches!(code.get(ip - 1), Some(Op::Const(c))
                        if matches!(
                            program.functions[func_idx].chunk.consts.get(*c),
                            Some(Value::Int(1))
                        )))
                    {
                        return Err(JitError::Unsupported(
                            "unboxed: non-unit increment on an iter cursor".to_string(),
                        ));
                    }
                    kinds.pop();
                    kinds.pop();
                    kinds.push(Kind::IterPtr);
                }
                Op::AddI | Op::SubI | Op::MulI | Op::DivI | Op::RemI => {
                    let (b2, a2) = (kinds.pop(), kinds.pop());
                    if matches!(b2, Some(Kind::IterPtr | Kind::IterEnd))
                        || matches!(a2, Some(Kind::IterPtr | Kind::IterEnd))
                    {
                        return Err(JitError::Unsupported(
                            "unboxed: arithmetic on iter pointers (desugar drift)".to_string(),
                        ));
                    }
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
                Op::Lt
                    if matches!(kinds.last(), Some(Kind::IterEnd))
                        && matches!(
                            kinds.get(kinds.len().wrapping_sub(2)),
                            Some(Kind::IterPtr)
                        ) =>
                {
                    // Lever 3: the desugar's `j < Len(elems)` header — one unsigned compare.
                    kinds.pop();
                    kinds.pop();
                    kinds.push(Kind::Bool);
                }
                Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                    let (b2, a2) = (kinds.pop(), kinds.pop());
                    if matches!(b2, Some(Kind::IterPtr | Kind::IterEnd))
                        || matches!(a2, Some(Kind::IterPtr | Kind::IterEnd))
                    {
                        return Err(JitError::Unsupported(
                            "unboxed: comparison on iter pointers (desugar drift)".to_string(),
                        ));
                    }
                    kinds.push(Kind::Bool);
                }
                Op::GetLocal(slot) => {
                    let k = *kinds.get(*slot).ok_or_else(|| {
                        JitError::Codegen(format!(
                            "unboxed analyze: GetLocal slot {slot} underflow"
                        ))
                    })?;
                    // Single-use param MOVE: the unique read of an Owned-str param transfers
                    // the word's ownership to the stack and kills the slot (`Unknown`) — the
                    // ctor-argument flow. Everything else is a BORROW: the slot keeps
                    // ownership; the copy is never freed by its consumer.
                    // W7: a Dyn cell is MOVE-ONLY — `borrowed_copy(Dyn)` would alias the
                    // owned str payload (two cells, one word, consumer releases → double
                    // free), and no borrowed-Dyn kind exists to mark the copy inert.
                    let movable = matches!(
                        k,
                        Kind::Str(Own::Owned)
                            | Kind::StrList(Own::Owned)
                            | Kind::IntList(Own::Owned)
                            | Kind::StrIntMap(Own::Owned)
                            | Kind::DynList(Own::Owned)
                            | Kind::Dyn
                    );
                    if *slot < single_use.len() && single_use[*slot] && movable {
                        kinds[*slot] = Kind::Unknown;
                        kinds.push(k);
                    } else if k == Kind::Dyn {
                        return Err(JitError::Unsupported(
                            "unboxed: multi-use union (Dyn) param (deferred)".to_string(),
                        ));
                    } else {
                        kinds.push(borrowed_copy(k));
                    }
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
                    // Handle writes (the accumulator pattern `s = s + x` / the reset `s = ""`):
                    // the OLD value is freed at emit (runtime-bit-gated release ladder), the slot
                    // takes the incoming kind. Storing a `Borrowed` handle stays DENIED — it may
                    // alias a live owned local (`s2 = s1`), and the slot's later free would
                    // recycle the original (the double-free shape). Kind-changing writes over a
                    // live handle slot are fine (the old value is released either way).
                    if matches!(
                        k,
                        Kind::Str(Own::Borrowed)
                            | Kind::StrList(Own::Borrowed)
                            | Kind::StrIntMap(Own::Borrowed)
                            | Kind::IntList(Own::Borrowed)
                            | Kind::DynList(Own::Borrowed)
                            | Kind::Inst(_, Own::Borrowed)
                    ) {
                        return Err(JitError::Unsupported(
                            "unboxed: SetLocal of a borrowed handle (aliasing — deferred)"
                                .to_string(),
                        ));
                    }
                    // W7: storing a Dyn needs the pair-copy + a tag-gated release of any old
                    // Dyn in the slot — no consumer needs it yet (union params are read once).
                    if k == Kind::Dyn {
                        return Err(JitError::Unsupported(
                            "unboxed: SetLocal of a union (Dyn) value (deferred)".to_string(),
                        ));
                    }
                    kinds[*slot] = k;
                }
                Op::SetIndexLocal(slot) => {
                    // The mapinsert vertical (`m[k] = v`) — mirrors `arm_set_index_map_local`
                    // exactly (fail closed): an Int value over a borrowed/const string key, on
                    // a uniquely-OWNED map slot. Aliasing is impossible in the subset (SetLocal
                    // of a borrowed handle is denied above), so the in-place mutation matches
                    // the VM's `Rc::make_mut` refcount-1 COW path byte-for-byte.
                    match (kinds.pop(), kinds.pop()) {
                        (Some(Kind::Int), Some(Kind::Str(_))) => {}
                        other => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed SetIndexLocal operand kinds {other:?}"
                            )))
                        }
                    }
                    if !matches!(kinds.get(*slot), Some(Kind::StrIntMap(Own::Owned))) {
                        return Err(JitError::Unsupported(format!(
                            "unboxed SetIndexLocal map slot kind {:?}",
                            kinds.get(*slot)
                        )));
                    }
                }
                // Enum vertical: MakeEnum pops the payload(s), pushes the two-word register enum;
                // MatchTag pops the scrutinee copy, pushes the tag-compare bool; GetEnumField(0)
                // pops the enum, pushes its payload. Only ≤1-int-payload variants are in the
                // subset (mirrors `collect_functions_unboxed`); anything else → VM fallback.
                Op::MakeEnum(idx) => {
                    let arity = program.enum_descs[*idx].arity;
                    if arity > 1 {
                        return Err(JitError::Unsupported(
                            "unboxed: MakeEnum arity > 1 (deferred)".to_string(),
                        ));
                    }
                    for _ in 0..arity {
                        let k = kinds.pop().ok_or_else(|| {
                            JitError::Codegen("unboxed analyze: MakeEnum underflow".to_string())
                        })?;
                        if k != Kind::Int {
                            return Err(JitError::Unsupported(format!(
                                "unboxed: MakeEnum payload kind {k:?} (deferred)"
                            )));
                        }
                    }
                    kinds.push(Kind::EnumInt);
                }
                Op::MatchTag(_) => {
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: MatchTag underflow".to_string())
                    })?;
                    if k != Kind::EnumInt {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: MatchTag operand kind {k:?} (deferred)"
                        )));
                    }
                    kinds.push(Kind::Bool);
                }
                Op::GetEnumField(i) => {
                    if *i != 0 {
                        return Err(JitError::Unsupported(
                            "unboxed: GetEnumField index > 0 (deferred)".to_string(),
                        ));
                    }
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: GetEnumField underflow".to_string())
                    })?;
                    if k != Kind::EnumInt {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: GetEnumField operand kind {k:?} (deferred)"
                        )));
                    }
                    kinds.push(Kind::Int);
                }
                // A fixed runtime fault (match-exhaustiveness backstop) — a TERMINATOR: no
                // fall-through successor, no propagated edge (mirrors `reachable`).
                Op::Fault(_) => {
                    break;
                }
                // Native try/catch: `PushHandler(t)` establishes the HANDLER EDGE — the pad
                // `t` is entered (by a throw anywhere in the range) with the stack exactly as
                // it stands HERE plus the thrown value on top (the VM's unwind truncates to
                // this height and pushes the payload). The thrown value's class is the
                // graph-wide singleton (not yet known -> held, the fixpoint retries once a
                // `Throw` is discovered). Fall-through continues into the try body unchanged.
                Op::PushHandler(t) => {
                    match info.thrown_class {
                        Some(c) => {
                            let mut pad = kinds.clone();
                            pad.push(Kind::Inst(c, Own::Owned));
                            propagate(&mut leader_state, &mut work, *t, &pad)?;
                        }
                        // Not yet known — DEFER (don't fail here): keep walking the try
                        // body so its call sigs / method discoveries land THIS round. The
                        // only `Throw` sites may live in methods that exact walk discovers
                        // (the sqlbuild shape: `qualify` throws, reached only through the
                        // builder chain inside the try) — failing at the marker would
                        // deadlock the fixpoint. The held error at the walk's end keeps
                        // this function declined until the class is known; the pad block
                        // simply stays unreached this round.
                        None => pad_deferred = true,
                    }
                }
                Op::PopHandler => {}
                // `Throw` pops the payload (an owned instance) and transfers control — a
                // TERMINATOR (the emit arm unwinds to the active pad or returns code 6).
                Op::Throw => {
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: Throw underflow".to_string())
                    })?;
                    let Kind::Inst(c, _) = k else {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: Throw of {k:?} (deferred)"
                        )));
                    };
                    disc.throw_classes.push(c);
                    break;
                }
                // `value instanceof Name` on a STATICALLY-classed instance folds to a
                // constant (class name match or supertype membership — the same
                // `class_implements` oracle the VM consults).
                Op::IsInstance(_) => {
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: IsInstance underflow".to_string())
                    })?;
                    if !matches!(k, Kind::Inst(..)) {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: IsInstance on {k:?} (deferred)"
                        )));
                    }
                    kinds.push(Kind::Bool);
                }
                // Closure vertical: a capture-free `MakeClosure` is a STATIC target — the kind
                // carries the function index; `CallValue` on it is a direct call (models the
                // return `Int`, like `Call`). Captures / non-`Fn` callees / a static arity
                // mismatch (the VM renders that fault) → VM fallback.
                Op::MakeClosure(f) => match program.functions[*f].n_captures {
                    0 => kinds.push(Kind::Fn(*f)),
                    1 => {
                        // hofpipe: ONE int capture — the popped capture word stays in its
                        // cell, which becomes the FnCap1 value (same depth, zero moves).
                        match kinds.pop() {
                            Some(Kind::Int) => {}
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed: non-int closure capture {other:?} (deferred)"
                                )))
                            }
                        }
                        kinds.push(Kind::FnCap1(*f));
                    }
                    _ => {
                        return Err(JitError::Unsupported(
                            "unboxed: closure with 2+ captures (deferred)".to_string(),
                        ))
                    }
                },
                Op::CallValue(argc) => {
                    for _ in 0..*argc {
                        if kinds.pop().is_some_and(|k| {
                            k.is_handle()
                                || k == Kind::EnumInt
                                || matches!(k, Kind::Fn(_) | Kind::Dyn)
                        }) {
                            return Err(JitError::Unsupported(
                                "unboxed: handle/enum/fn argument to CallValue (deferred)"
                                    .to_string(),
                            ));
                        }
                    }
                    let callee = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: CallValue underflow".to_string())
                    })?;
                    let Kind::Fn(f) = callee else {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: CallValue on {callee:?} (deferred)"
                        )));
                    };
                    // Capture-free ⇒ n_params == arity; a mismatch is the VM's canonical
                    // "wrong number of arguments" fault — fall back so it renders there.
                    if program.functions[f].arity != *argc {
                        return Err(JitError::Unsupported(
                            "unboxed: CallValue arity mismatch (VM renders the fault)".to_string(),
                        ));
                    }
                    if info.this_inst.get(f).copied().flatten().is_some() {
                        return Err(JitError::Unsupported(
                            "unboxed: CallValue to a method body (deferred)".to_string(),
                        ));
                    }
                    kinds.push(info.ret_of(f));
                }
                Op::Call(callee) => {
                    if info.this_inst.get(*callee).copied().flatten().is_some() {
                        // A method body's slot 0 is an injected instance — a plain `Call` would
                        // pass an untyped int there. Only `CallMethod` may reach it.
                        return Err(JitError::Unsupported(
                            "unboxed: plain Call to a method body (deferred)".to_string(),
                        ));
                    }
                    let arity = program.functions[*callee].arity;
                    let mut sig: Vec<Kind> = Vec::with_capacity(arity);
                    for _ in 0..arity {
                        let k = kinds.pop().ok_or_else(|| {
                            JitError::Codegen("unboxed analyze: Call underflow".to_string())
                        })?;
                        match k {
                            // A string/list argument MOVES into the callee (its param is a
                            // single-use-moved Owned slot there; a const word's later frees
                            // no-op). A BORROWED word CLONES at the emit boundary (W9 — PHP
                            // value semantics: `this.field` forwarded into the next builder;
                            // passing the raw word would leave the owner and the callee both
                            // freeing it), so every str/list ownership is accepted here and
                            // the callee always receives an Owned word.
                            Kind::Str(_) => {
                                sig.push(Kind::Str(Own::Owned));
                            }
                            Kind::StrList(_) => {
                                sig.push(Kind::StrList(Own::Owned));
                            }
                            Kind::IntList(_) => {
                                sig.push(Kind::IntList(Own::Owned));
                            }
                            // No map clone repr yet — Owned moves only (Borrowed stays denied
                            // via the handle guard below).
                            Kind::StrIntMap(Own::Owned) => {
                                sig.push(Kind::StrIntMap(Own::Owned));
                            }
                            Kind::DynList(_) => {
                                sig.push(Kind::DynList(Own::Owned));
                            }
                            k if k.is_handle()
                                || k == Kind::EnumInt
                                || matches!(k, Kind::Fn(_)) =>
                            {
                                return Err(JitError::Unsupported(
                                    "unboxed: handle/enum/fn argument to Call (deferred)"
                                        .to_string(),
                                ));
                            }
                            other => sig.push(other),
                        }
                    }
                    // Record EVERY call sig (not just handle-arg ones): the usage pre-pass no
                    // longer stamps ctor/method-arg params, so int-only callees need their
                    // param proofs from here too (the un-recorded gap silently declined the
                    // whole objalloc/methodcall graphs — a 60-85x regression the ratchet
                    // caught).
                    sig.reverse(); // popped last-arg-first; record declaration order
                    disc.call_sigs.push((*callee, sig));
                    kinds.push(info.ret_of(*callee));
                }
                // Object vertical: flat arena instances + static field offsets + statically-
                // dispatched methods. Gates: every field ctor-initialized (no `None` window),
                // ≤ 8 int fields, int-only field values/args, non-overloaded methods.
                Op::MakeInstance(cidx) => {
                    let desc = &program.class_descs[*cidx];
                    let nf = desc.fields.len();
                    // ≤ 15 fields: 0..6 in slot A, A[7] = the B-slot index, 7..14 in B
                    // (the two-slot layout — SelectQuery's 11 fields drove the widening).
                    if desc.layout.len() != nf || nf > 15 {
                        return Err(JitError::Unsupported(
                            "unboxed: MakeInstance with non-ctor-initialized or >15 fields (deferred)"
                                .to_string(),
                        ));
                    }
                    // Fields are Int words or Str handle words the instance takes OWNERSHIP of
                    // (a Borrowed str operand would alias a live local - its later field-free
                    // double-frees - DENY; ConstBorrow words are runtime-bit-clear, safe).
                    // Record the ownership-normalized signature for the fixpoint's per-class
                    // field-kind table (drives GetField/SetField typing + kinded release).
                    let mut sig: Vec<Kind> = Vec::with_capacity(nf);
                    for _ in 0..nf {
                        let k = kinds.pop().ok_or_else(|| {
                            JitError::Codegen("unboxed analyze: MakeInstance underflow".to_string())
                        })?;
                        match k {
                            Kind::Int => sig.push(Kind::Int),
                            Kind::Str(Own::Owned) | Kind::Str(Own::ConstBorrow) => {
                                sig.push(Kind::Str(Own::Owned))
                            }
                            // W-slice: handle-list/map fields — the field takes the word
                            // (ownership moves in; a Borrowed arg would alias — DENY).
                            Kind::StrList(Own::Owned) => sig.push(Kind::StrList(Own::Owned)),
                            Kind::IntList(Own::Owned) => sig.push(Kind::IntList(Own::Owned)),
                            Kind::StrIntMap(Own::Owned) => sig.push(Kind::StrIntMap(Own::Owned)),
                            Kind::DynList(Own::Owned) => sig.push(Kind::DynList(Own::Owned)),
                            other => {
                                return Err(JitError::Unsupported(format!(
                                    "unboxed: MakeInstance {} field {} kind {other:?} (deferred)",
                                    program.class_descs[*cidx].class,
                                    // popped top-first ⇒ this is ctor-push position nf-1-len
                                    nf - 1 - sig.len(),
                                )));
                            }
                        }
                    }
                    // Operands popped top-first = REVERSE ctor push order; store push order.
                    sig.reverse();
                    disc.inst_fields.push((*cidx, sig));
                    kinds.push(Kind::Inst(*cidx, Own::Owned));
                }
                Op::GetField(nidx) => {
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: GetField underflow".to_string())
                    })?;
                    let Kind::Inst(c, _) = k else {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: GetField on {k:?} (deferred)"
                        )));
                    };
                    let desc = &program.class_descs[c];
                    if desc.layout.slot(&program.names[*nidx]).is_none() {
                        return Err(JitError::Unsupported(
                            "unboxed: GetField name not in layout (VM renders the fault)"
                                .to_string(),
                        ));
                    }
                    // The field's kind comes from the fixpoint's per-class table (ctor push
                    // order = desc.fields order). Not yet recorded -> held-Unsupported, the
                    // fixpoint retries next round. A Str field read is a BORROW: the instance
                    // keeps ownership of the handle word.
                    let j = desc
                        .fields
                        .iter()
                        .position(|f| f == &program.names[*nidx])
                        .ok_or_else(|| {
                            JitError::Unsupported(
                                "unboxed: GetField name not a ctor field (deferred)".to_string(),
                            )
                        })?;
                    // A handle read from a BORROWED receiver borrows (the instance keeps
                    // the word); from an OWNED receiver (a dying temp — `new C(..).f`) the
                    // result TAKES the field's word (`field_read_kind`).
                    match info.field_kind(c, j) {
                        Some(fk) => {
                            match field_read_kind(fk, matches!(k, Kind::Inst(_, Own::Owned))) {
                                Some(out) => kinds.push(out),
                                None => {
                                    return Err(JitError::Unsupported(format!(
                                        "unboxed: GetField field kind {fk:?} (deferred)"
                                    )));
                                }
                            }
                        }
                        None => {
                            return Err(JitError::Unsupported(
                                "unboxed: GetField before class signature is known (fixpoint retry)"
                                    .to_string(),
                            ));
                        }
                    }
                }
                Op::SetField(nidx) => {
                    let v = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: SetField underflow".to_string())
                    })?;
                    let k = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: SetField underflow".to_string())
                    })?;
                    let Kind::Inst(c, _) = k else {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: SetField on {k:?} (deferred)"
                        )));
                    };
                    let desc = &program.class_descs[c];
                    if desc.layout.slot(&program.names[*nidx]).is_none() {
                        return Err(JitError::Unsupported(
                            "unboxed: SetField name not in layout (VM no-op parity)".to_string(),
                        ));
                    }
                    let j = desc
                        .fields
                        .iter()
                        .position(|f| f == &program.names[*nidx])
                        .ok_or_else(|| {
                            JitError::Unsupported(
                                "unboxed: SetField name not a ctor field (deferred)".to_string(),
                            )
                        })?;
                    // The stored value must MATCH the class's field kind; a Str store takes
                    // ownership of the word (Borrowed would alias a live local - DENY).
                    match (info.field_kind(c, j), v) {
                        (Some(Kind::Int), Kind::Int) => {}
                        (
                            Some(Kind::Str(_)),
                            Kind::Str(Own::Owned) | Kind::Str(Own::ConstBorrow),
                        ) => {}
                        (Some(Kind::StrList(_)), Kind::StrList(Own::Owned)) => {}
                        (Some(Kind::IntList(_)), Kind::IntList(Own::Owned)) => {}
                        (Some(Kind::StrIntMap(_)), Kind::StrIntMap(Own::Owned)) => {}
                        (Some(Kind::DynList(_)), Kind::DynList(Own::Owned)) => {}
                        (None, _) => {
                            return Err(JitError::Unsupported(
                                "unboxed: SetField before class signature is known (fixpoint retry)"
                                    .to_string(),
                            ));
                        }
                        (Some(fk), vv) => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed: SetField value kind {vv:?} into {fk:?} field (deferred)"
                            )));
                        }
                    }
                }
                Op::CallMethod(nidx, argc) => {
                    // Args mirror the free-`Call` contract: scalars pass by word; a
                    // string/list/map argument MOVES (Owned into the callee's param slot);
                    // borrowed handles / enums / fns stay denied. The moved kinds are
                    // recorded as the method's param sig (slot 0 = `this`, injected by
                    // `this_inst` — Unknown here so the override leaves it alone).
                    let mut sig: Vec<Kind> = Vec::with_capacity(*argc + 1);
                    for _ in 0..*argc {
                        let k = kinds.pop().ok_or_else(|| {
                            JitError::Codegen("unboxed analyze: CallMethod underflow".to_string())
                        })?;
                        match k {
                            // W9: a BORROWED str/list arg clones at the emit boundary (PHP
                            // value semantics) — the callee always receives an Owned word.
                            Kind::Str(_) => {
                                sig.push(Kind::Str(Own::Owned));
                            }
                            Kind::StrList(_) => {
                                sig.push(Kind::StrList(Own::Owned));
                            }
                            Kind::IntList(_) => {
                                sig.push(Kind::IntList(Own::Owned));
                            }
                            // No map clone repr yet — Owned moves only.
                            Kind::StrIntMap(Own::Owned) => {
                                sig.push(Kind::StrIntMap(Own::Owned));
                            }
                            Kind::DynList(_) => {
                                sig.push(Kind::DynList(Own::Owned));
                            }
                            k if k.is_handle()
                                || k == Kind::EnumInt
                                || matches!(k, Kind::Fn(_) | Kind::FnCap1(_)) =>
                            {
                                return Err(JitError::Unsupported(
                                    "unboxed: handle/enum/fn argument to CallMethod (deferred)"
                                        .to_string(),
                                ));
                            }
                            other => sig.push(other),
                        }
                    }
                    let recv = kinds.pop().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: CallMethod underflow".to_string())
                    })?;
                    let Kind::Inst(c, _) = recv else {
                        return Err(JitError::Unsupported(format!(
                            "unboxed: CallMethod on {recv:?} (deferred)"
                        )));
                    };
                    let key = (
                        program.class_descs[c].class.to_string(),
                        program.names[*nidx].clone(),
                    );
                    if program.method_overloads.contains_key(&key) {
                        return Err(JitError::Unsupported(
                            "unboxed: overloaded method (deferred)".to_string(),
                        ));
                    }
                    let Some(&target) = program.methods.get(&key) else {
                        return Err(JitError::Unsupported(
                            "unboxed: unresolved method (VM renders the fault)".to_string(),
                        ));
                    };
                    // Receiver becomes the callee's slot 0 (`this`), args at 1..=argc.
                    if program.functions[target].arity != *argc + 1 {
                        return Err(JitError::Unsupported(
                            "unboxed: CallMethod arity mismatch (VM renders the fault)".to_string(),
                        ));
                    }
                    disc.method_calls.push((target, c));
                    // Record EVERY method sig (see the Call arm note — int-only methods need
                    // their param proofs from call sites now).
                    sig.push(Kind::Unknown); // slot 0 = `this` (this_inst injects it)
                    sig.reverse(); // popped last-arg-first; record declaration order
                    disc.call_sigs.push((target, sig));
                    kinds.push(info.ret_of(target));
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
                    let rk = *kinds.last().ok_or_else(|| {
                        JitError::Codegen("unboxed analyze: Return underflow".to_string())
                    })?;
                    let rk = match rk {
                        // OWNERSHIP-TRANSFER gate for an instance return (the synthesized-ctor
                        // shape): the escaping handle must be the function's OWN fresh
                        // allocation, and nothing else owned may be live (no leak at return —
                        // frame teardown emits no frees).
                        Kind::Inst(c, own) => {
                            // (a) no INSTANCE params — applies to the BORROWED-return lineage
                            //     only: a returned Inst borrow could alias a caller-owned
                            //     instance (double-free on the caller side). An OWNED Inst
                            //     provably comes from THIS function's own MakeInstance
                            //     (GetLocal copies are Borrowed and there is no Inst param
                            //     move), so instance params cannot be its lineage — the
                            //     builder-method shape (`this` in, fresh instance out).
                            if matches!(own, Own::Borrowed)
                                && param_kinds.iter().any(|k| matches!(k, Kind::Inst(..)))
                            {
                                return Err(JitError::Unsupported(
                                    "unboxed: borrowed instance return from a function with instance params (deferred)"
                                        .to_string(),
                                ));
                            }
                            // (b) owned-cell census below the returned top.
                            let below = &kinds[..kinds.len() - 1];
                            let owned: Vec<Kind> = below
                                .iter()
                                .copied()
                                .filter(|k| k.is_owned_handle())
                                .collect();
                            let transfer_ok = match own {
                                // Returned owned directly: any other owned cells are frame
                                // temps the emit's Return teardown now releases (W9) — an
                                // Owned Inst on the stack is unique (moves; GetLocal copies
                                // are Borrowed), so no below cell can back it.
                                Own::Owned => true,
                                // Returned a borrow of a local: exactly ONE owned cell — an
                                // instance of the SAME class (the borrowed lineage) — remains
                                // and TRANSFERS (the emit teardown releases nothing on this
                                // path, so the census must stay exact).
                                Own::Borrowed => {
                                    owned.len() == 1
                                        && matches!(owned[0], Kind::Inst(c2, Own::Owned) if c2 == c)
                                }
                                // Instances are never const-borrows (no instance consts exist);
                                // unreachable by construction — reject defensively.
                                Own::ConstBorrow => false,
                            };
                            if !transfer_ok {
                                return Err(JitError::Unsupported(
                                    "unboxed: instance return with ambiguous ownership (deferred)"
                                        .to_string(),
                                ));
                            }
                            // The caller receives ownership.
                            Kind::Inst(c, Own::Owned)
                        }
                        // A str/list return always reaches the caller as a fresh/transferred
                        // OWNED word (the emit clones a borrowed one) — mirror that here so
                        // `ret_of` hands callers the right ownership.
                        Kind::Str(_) => Kind::Str(Own::Owned),
                        Kind::StrList(_) => Kind::StrList(Own::Owned),
                        Kind::IntList(_) => Kind::IntList(Own::Owned),
                        Kind::DynList(_) => Kind::DynList(Own::Owned),
                        // W7: a Dyn return has no single-kind decode on the caller side —
                        // mirror the emit reject exactly (analyze accepts ⇒ emit accepts).
                        Kind::Dyn => {
                            return Err(JitError::Unsupported(
                                "unboxed: union (Dyn) return (deferred)".to_string(),
                            ));
                        }
                        other => other,
                    };
                    match &ret_kind {
                        None => ret_kind = Some(rk),
                        Some(prev) if *prev != rk => {
                            return Err(JitError::Unsupported(format!(
                                "unboxed: inconsistent return kinds ({prev:?} vs {rk:?})"
                            )));
                        }
                        Some(_) => {}
                    }
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
    if pad_deferred {
        // A `PushHandler` ran before any thrown class was known — every discovery above
        // still merged (that's the point of deferring), but this function cannot compile
        // until the fixpoint learns the class and re-walks it.
        return Err(JitError::Unsupported(
            "unboxed: try before any thrown class is known (fixpoint retry)".to_string(),
        ));
    }
    Ok(UbAnalysis {
        leader_state,
        max_depth,
        ret_kind,
        depth_at,
    })
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
            if !reach[ip] {
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
                Op::CallNative(id, 2) if unboxed_native_is_bridge2(*id) => uses_handles = true,
                // hofpipe: the HOF loop arms direct-call the compiled lambda per element.
                Op::CallNative(id, 2)
                    if unboxed_native_is_list_map(*id) || unboxed_native_is_list_count(*id) =>
                {
                    uses_handles = true;
                    has_call = true;
                }
                // P-2c numeric conversions: pure, handle-free, fully inline.
                Op::CallNative(id, 1)
                    if unboxed_native_is_to_float(*id) || unboxed_native_is_truncate(*id) =>
                {
                    has_float = true;
                }
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
