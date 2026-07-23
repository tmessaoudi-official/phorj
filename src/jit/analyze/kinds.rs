//! The unboxed KIND LATTICE (M-Decomp from `analyze/mod.rs`, Invariant 13): the compile-time
//! operand kinds ([`Kind`]), handle ownership ([`Own`]), the `GetLocal` borrow rule
//! ([`borrowed_copy`]) and the merge-edge join ([`join_kind`]). Bodies moved verbatim (self-
//! contained — no imports needed).

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
                | Kind::DynList(_)
                | Kind::Inst(..)
        )
    }
    /// Is this operand an OWNED handle (must be freed by its consumer)?
    pub(in crate::jit) fn is_owned_handle(self) -> bool {
        matches!(
            self,
            Kind::Str(Own::Owned)
                | Kind::StrList(Own::Owned)
                | Kind::StrIntMap(Own::Owned)
                | Kind::IntList(Own::Owned)
                | Kind::IntSet(Own::Owned)
                | Kind::MapList(Own::Owned)
                | Kind::DynList(Own::Owned)
                | Kind::Inst(_, Own::Owned)
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
        Kind::StrIntMap(o) => Kind::StrIntMap(o.borrow_of()),
        Kind::Inst(c, o) => Kind::Inst(c, o.borrow_of()),
        Kind::DynList(o) => Kind::DynList(o.borrow_of()),
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
        (Kind::Inst(c1, x), Kind::Inst(c2, y)) if c1 == c2 => {
            join_own(x, y).map(|o| Kind::Inst(c1, o))
        }
        _ => None,
    }
}
