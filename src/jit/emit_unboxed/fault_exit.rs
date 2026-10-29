//! Which ops branch to the shared fault-exit block (moved out of `emit_unboxed/mod.rs` with plan
//! row 4b step 5, which added the runtime-class ops to the list).

use super::*;

/// `DivF` also branches to the fault-exit (a zero divisor → code 5), as do `DivI`/`RemI` (hardware
/// trap) and `Call` (depth guard + fault propagation) — every op that emits a `fault_if`/direct
/// `brif` to the shared exit must be counted here, or the block won't exist when it is needed.
/// P-2a: `Index` (bounds), `Concat` and `String.length` (defensive bad-handle) also branch to the
/// shared fault-exit (code 5, redo on VM). `Fault` (the match-exhaustiveness backstop) jumps to
/// it unconditionally. `GetField` is counted because a BORROWED field word returned from the
/// function CLONES at `Return` (fault on a defensive mismatch) — the `return this.field;`
/// shape has no other counted op (every other borrowed-returnable value traces to one).
pub(super) fn counts_fault_exit(op: &Op) -> bool {
    matches!(
        op,
        Op::DivI
        | Op::RemI
        | Op::DivF
        | Op::Call(_)
        | Op::CallValue(_)
        | Op::CallMethod(..)
        | Op::Index
        | Op::Len
        | Op::Concat(_)
        | Op::CallNative(..)
        | Op::MakeList(_)
        | Op::MakeMap(_)
        | Op::MakeInstance(_)
        | Op::GetField(_)
        | Op::GetFieldOrHook(..) // resolves to a field op or `CallMethod` (row 4b)
        | Op::SetFieldOrHook(..)
        | Op::CloneInstance // its handle-field clones fault to code 5 (row 4b step 5b)
        | Op::Fault(_)
        // STRING `Op::Eq`/`Op::Ne` emit a defensive bad-handle `fault_if` (the
        // `Eq(str)` helper arm, :1058); int/float/bool Eq/Ne go to `arm_cmp` and
        // emit no fault, so for those this over-counts a fault-exit block that is
        // unreferenced (terminated + DCE'd by opt_level=speed — same tolerance the
        // CallNative/GetField/MakeList entries above already rely on). Counting it
        // here is REQUIRED once a str operand can reach an `Eq`: DEC-333's entry
        // str-param seed makes `match (s) { "1" => … }` (firstOne) compile, and a
        // string-Eq-only function has NO other counted op — the missing block was
        // a `fault_if`-without-fault-exit panic.
        | Op::Eq
        | Op::Ne
    )
}
