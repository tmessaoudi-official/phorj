//! The unboxed KIND LATTICE (M-Decomp from `analyze/mod.rs`, Invariant 13): the compile-time
//! operand kinds ([`Kind`]), handle ownership ([`Own`]), the `GetLocal` borrow rule
//! ([`borrowed_copy`]) and the merge-edge join ([`join_kind`]). Bodies moved verbatim (self-
//! contained except [`JitError`] for the `MakeList` admission).

use super::JitError;

/// The kind of a compile-time operand-stack entry. The bytecode is type-erased, so this is tracked to
/// map `Return` correctly WITHOUT a type source: `Const`/arithmetic/`Neg` → `Int`, comparisons/`Not`
/// → `Bool`, a bare local (param) read → `Unknown`. u1 accepts a function ONLY if every reachable
/// `Return` yields `Int` — so a `bool`-returning function (which would else be mis-mapped to
/// `Value::Int`) and a bare-param return (unprovable-`Int` without types) fall back to the VM/boxed
/// path. Bool *params* are fine: they arrive as `0/1` i64 and are only ever consumed in bool contexts
/// (`Not`, `JumpIfFalse`, comparison operands) natively. Types + bare-param returns (so `fib`'s
/// `return n` JITs) come in u2 with a real type source.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::jit) enum Kind {
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
    /// A `Set<int>` membership handle (the setcontains vertical — DEC-311 flip campaign). A
    /// NARROW kind: produced ONLY by `Core.Set.of` (which re-tags a fresh OWNED flat int-list
    /// handle — same `UB_TAG_FLAT | count<<40 | base` arena encoding as [`Kind::IntList`], raw
    /// `i64` per 64-byte slot at bytes 0..8), consumed ONLY by `Core.Set.contains` (an inline
    /// linear membership scan — byte-identical to the interpreter's own `Vec<HKey::Int>::contains`).
    /// It NEVER participates in any list op (a set is not a list), is never a param / call-arg /
    /// return (rejected in the `Return` arm so the entry-decode default is unreachable), and its
    /// release is generic (flat = a bump-pinned no-op via `emit_release`). Dedup is NOT applied at
    /// `Set.of` — irrelevant to the sole consumer (membership is dedup-invariant) and the narrow
    /// gating means no other op observes the store. Only requires an OWNED input (no live alias —
    /// the double-free gate); a borrowed / non-int-list input falls back to the VM.
    IntSet(Own),
    /// A `List<Map<string,int>>` handle (the mapkeys/mapvalues/mapmerge rotating-operand shape
    /// `maps[i % 3]`): a NARROW kind produced ONLY by `MakeList` over `StrIntMap` operands —
    /// runtime encoding is the ordinary sealed int list whose raw i64 "elements" are the MAP
    /// HANDLE WORDS. Consumed ONLY by `Index` (which pushes the loaded word as an OWNED
    /// `StrIntMap` after a runtime FLAT-map tag guard — a flat map is immutable + bump-pinned,
    /// so aliased "owned" copies are sound: releases no-op and a `SetIndexLocal` conversion
    /// COPIES). Never a param / call-arg / return (rejected like `IntSet`); a non-flat word at
    /// `Index` (a boxed map element) is code 5 — the byte-identical VM redo.
    MapList(Own),
    /// A `List<Set<int>>` handle (the setdifference/setunion rotating-operand shape
    /// `bs[i % 4]`): the exact [`Kind::MapList`] discipline over SET handle words — produced
    /// ONLY by `MakeList` over `IntSet` operands, consumed ONLY by `Index` (runtime FLAT_SET
    /// tag guard on the loaded word; a sealed flat set is immutable + bump-pinned, so an OWNED
    /// aliased copy is sound). Never a param / call-arg / return.
    SetList(Own),
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
    /// DEC-333 Json-ADT: a `Core.Json` enum value as a TWO-word register pair — payload in the I64
    /// space (`vars[d]`), RUNTIME tag in the enum-tag space (`evars[d]`): relative variant indices
    /// 0..6 (Null,Bool,Int,Float,String,Array,Object per the prelude order) + 7 = phorj null (a
    /// `Json?` absent). Payload by tag: 0/7 filler, 1 bool, 2 i64, 3 f64-bits, 4 str handle, 5
    /// `JList` handle, 6 `JMap` handle. [`JRef`] is the compile-time variant refinement (flow-
    /// sensitive, set by the `MatchTag` peephole on the matched edge). Release is TAG-GATED and
    /// mandatory (per-iteration values, unlike the read-once leak-ok `Dyn`): tags 4/5/6 free the
    /// payload word, others no-op. Only the injected `Core.Json` ADT (`canonical_json`) is ever
    /// typed `Json`; a user look-alike enum stays on `EnumInt`/VM.
    // DEC-333: constructed by the op arms in the next increment — the lattice/membership arms land
    // first (green, universally declined). `allow` removed when the analyze/emit arms construct these.
    #[allow(dead_code)]
    Json(JRef, Own),
    /// DEC-333: an untagged `UbCtx` handle to a boxed `Value::Map` (`Map<string,Json>`; values may
    /// be `Value::JsonLazy`). Same ownership discipline as the other handle kinds.
    #[allow(dead_code)]
    JMap(Own),
    /// DEC-333: an untagged `UbCtx` handle to a boxed `Value::List` (`List<Json>`).
    #[allow(dead_code)]
    JList(Own),
    /// DEC-333: a compile-time-only marker for `Const(Value::Null)`, admitted ONLY when the very
    /// next op is `Eq`/`Ne` (collect gate) so it is OPERAND-TRANSIENT — produced by `Const(Null)`,
    /// consumed by an immediate `Eq`/`Ne`-vs-`Json`, never stored/returned/merged (`SetLocal`/
    /// `Return` decline; `join_kind(NullMark, ·) → None`, checked before the `a==b` fast-path). The
    /// runtime word is filler `0` (never a handle — declining paths are wrong-bytes-safe, no free).
    #[allow(dead_code)]
    NullMark,
}

/// DEC-333: compile-time variant refinement of a [`Kind::Json`] cell — `Any` (unknown variant) or
/// `V(rel)` for a proven relative tag `0..=6`. Set on the matched (fall-through) edge of a
/// `GetLocal(s); MatchTag(t); JumpIfFalse` peephole; `GetEnumField(0)` needs a concrete `V(t)` (it
/// declines on `Any`). Joins widen to `Any` at any merge (see `join_jref`).
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(dead_code)] // DEC-333: constructed by the refinement peephole in the next increment
pub(in crate::jit) enum JRef {
    Any,
    V(u8),
}

/// Compile-time ownership of a handle operand — see [`Kind::Str`]. Part of `Kind`'s equality, so the
/// leader-state consistency check also enforces ownership agreement across merge edges (a mismatch
/// falls back to the VM, never double-frees).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::jit) enum Own {
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
    pub(in crate::jit) fn borrow_of(self) -> Own {
        if self == Own::ConstBorrow {
            Own::ConstBorrow
        } else {
            Own::Borrowed
        }
    }
}

impl Kind {
    /// Is this operand a handle into the per-run [`UbCtx`] table?
    pub(in crate::jit) fn is_handle(self) -> bool {
        matches!(
            self,
            Kind::Str(_)
                | Kind::StrList(_)
                | Kind::StrIntMap(_)
                | Kind::IntList(_)
                | Kind::IntSet(_)
                | Kind::MapList(_)
                | Kind::SetList(_)
                | Kind::DynList(_)
                | Kind::Inst(..)
                | Kind::JMap(_)
                | Kind::JList(_)
        )
    }
    /// Is this operand an OWNED handle (must be freed by its consumer)? DEC-333: a `Json(_, Owned)`
    /// pair counts (so teardown/`Pop`/`SetLocal`-overwrite visit it) — its release is TAG-GATED in
    /// `release_kinded` (only tags 4/5/6 free the payload word; the evars tag threads in there).
    pub(in crate::jit) fn is_owned_handle(self) -> bool {
        matches!(
            self,
            Kind::Str(Own::Owned)
                | Kind::StrList(Own::Owned)
                | Kind::StrIntMap(Own::Owned)
                | Kind::IntList(Own::Owned)
                | Kind::IntSet(Own::Owned)
                | Kind::MapList(Own::Owned)
                | Kind::SetList(Own::Owned)
                | Kind::DynList(Own::Owned)
                | Kind::Inst(_, Own::Owned)
                | Kind::JMap(Own::Owned)
                | Kind::JList(Own::Owned)
                | Kind::Json(_, Own::Owned)
        )
    }
}

/// The kind a `GetLocal` pushes for a slot of kind `k`: a handle read is a BORROW (the slot keeps
/// ownership — the copy's consumer must not free it); every other kind copies verbatim.
pub(in crate::jit) fn borrowed_copy(k: Kind) -> Kind {
    match k {
        Kind::Str(o) => Kind::Str(o.borrow_of()),
        Kind::StrList(o) => Kind::StrList(o.borrow_of()),
        Kind::IntList(o) => Kind::IntList(o.borrow_of()),
        Kind::IntSet(o) => Kind::IntSet(o.borrow_of()),
        Kind::MapList(o) => Kind::MapList(o.borrow_of()),
        Kind::SetList(o) => Kind::SetList(o.borrow_of()),
        Kind::StrIntMap(o) => Kind::StrIntMap(o.borrow_of()),
        Kind::Inst(c, o) => Kind::Inst(c, o.borrow_of()),
        Kind::DynList(o) => Kind::DynList(o.borrow_of()),
        // DEC-333: a GetLocal copy of a Json pair / JMap / JList borrows (the slot keeps
        // ownership). The catch-all `other => other` would return an Owned copy → double-free.
        Kind::Json(r, o) => Kind::Json(r, o.borrow_of()),
        Kind::JMap(o) => Kind::JMap(o.borrow_of()),
        Kind::JList(o) => Kind::JList(o.borrow_of()),
        other => other,
    }
}

/// Join two operand kinds at a merge edge. Identical kinds join to themselves. The SAME handle
/// family differing only between `Owned` and `ConstBorrow` joins to `Owned` — safe because a
/// release is runtime-bit-gated (freeing a provably-bit-clear const word is a no-op), so the
/// `Owned` side's frees are correct on both edges. `Borrowed` (bit UNKNOWN — may alias a live
/// owned local) never joins with `Owned`; `Borrowed ⊔ ConstBorrow` joins to `Borrowed` (neither
/// side frees). Anything else → `None` (VM fallback).
pub(in crate::jit) fn join_kind(a: Kind, b: Kind) -> Option<Kind> {
    // DEC-333 [R6-safety-1]: a NullMark must NEVER survive a merge — reject BEFORE the `a==b`
    // fast-path, else `join_kind(NullMark, NullMark)` returns `Some(NullMark)` and the mandated
    // `→None` breaks. NullMark is operand-transient (produced by Const(Null), consumed by an
    // immediate Eq/Ne), so it is never legitimately live at a leader; a merge ⇒ VM fallback.
    if matches!(a, Kind::NullMark) || matches!(b, Kind::NullMark) {
        return None;
    }
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
        (Kind::IntSet(x), Kind::IntSet(y)) => join_own(x, y).map(Kind::IntSet),
        (Kind::MapList(x), Kind::MapList(y)) => join_own(x, y).map(Kind::MapList),
        (Kind::SetList(x), Kind::SetList(y)) => join_own(x, y).map(Kind::SetList),
        (Kind::Inst(c1, x), Kind::Inst(c2, y)) if c1 == c2 => {
            join_own(x, y).map(|o| Kind::Inst(c1, o))
        }
        // DEC-333: two Json pairs join with a widened variant refinement (`V(a)⊔V(b)→Any` unless
        // identical — the identical case is caught by the `a==b` fast-path above); JMap/JList join
        // like the other handle families.
        (Kind::Json(rx, ox), Kind::Json(ry, oy)) => {
            join_own(ox, oy).map(|o| Kind::Json(join_jref(rx, ry), o))
        }
        (Kind::JMap(x), Kind::JMap(y)) => join_own(x, y).map(Kind::JMap),
        (Kind::JList(x), Kind::JList(y)) => join_own(x, y).map(Kind::JList),
        _ => None,
    }
}

/// DEC-333: join two [`Kind::Json`] variant refinements — equal concrete tags stay, anything else
/// widens to `Any` (the merge cannot prove one variant).
fn join_jref(a: JRef, b: JRef) -> JRef {
    match (a, b) {
        (JRef::V(x), JRef::V(y)) if x == y => JRef::V(x),
        _ => JRef::Any,
    }
}

/// Admit `MakeList(n)` into the unboxed subset: element kinds select the list flavor —
/// all-`Str` → `StrList`, all-`Int` → `IntList` (P-2c), all-`StrIntMap` → [`Kind::MapList`],
/// all-`IntSet` → [`Kind::SetList`]; anything else (mixed, floats, nested) is default-denied.
/// Mirrors `emit_unboxed/verticals.rs::arm_make_list`'s stack effects exactly.
pub(in crate::jit) fn admit_make_list(kinds: &mut Vec<Kind>, n: usize) -> Result<(), JitError> {
    let d = kinds.len();
    if n > d {
        return Err(JitError::Codegen("unboxed MakeList underflow".to_string()));
    }
    let all_str = kinds[d - n..].iter().all(|k| matches!(k, Kind::Str(_)));
    let all_int = n > 0 && kinds[d - n..].iter().all(|k| *k == Kind::Int);
    let all_map = n > 0
        && kinds[d - n..]
            .iter()
            .all(|k| matches!(k, Kind::StrIntMap(_)));
    let all_set = n > 0 && kinds[d - n..].iter().all(|k| matches!(k, Kind::IntSet(_)));
    if !(all_str || all_int || all_map || all_set) {
        return Err(JitError::Unsupported(format!(
            "unboxed MakeList element kinds {:?}",
            &kinds[d - n..]
        )));
    }
    kinds.truncate(d - n);
    kinds.push(if all_int {
        Kind::IntList(Own::Owned)
    } else if all_map {
        Kind::MapList(Own::Owned)
    } else if all_set {
        Kind::SetList(Own::Owned)
    } else {
        Kind::StrList(Own::Owned)
    });
    Ok(())
}
