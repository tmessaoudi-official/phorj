//! Bytecode VM — the OPERAND STACK and locals-window access (Invariant 13 split, DEC-442).
//!
//! Every function here is on the hottest path in the interpreter: DEC-441 measured the VM at 176 Ir per
//! bytecode op against php's 11, and **24.6% of that was these helpers being called OUT OF LINE**. So the
//! `#[inline]`/`#[inline(always)]` attributes below are load-bearing measurements, not decoration — see
//! [`expected`] for the one that mattered most.
//!
//! Split out of `mod.rs` when this slice would have grown that file past its frozen size baseline
//! (Invariant 13: "split it, do not grow it"). The cohesion is exact — this is every operation that
//! touches `self.stack` positionally, and nothing else.

use super::*;

impl<'a> Vm<'a> {
    #[inline(always)]
    pub(super) fn pop(&mut self) -> Value {
        self.stack.pop().expect("vm stack underflow (compiler bug)")
    }

    /// Start index for popping the top `n` values. Real work in every build (`len - n`); the
    /// debug-only guard turns a compiler-bug underflow (which would wrap and then panic with a
    /// bare `index out of bounds`) into a labelled stack-desync assert. The compiler guarantees
    /// `n <= stack.len()`.
    #[inline]
    pub(super) fn pop_n_start(&self, n: usize) -> usize {
        debug_assert!(
            n <= self.stack.len(),
            "vm stack underflow: need {n} values, stack has {} (func {})",
            self.stack.len(),
            self.frames.last().map_or(usize::MAX, |f| f.func)
        );
        self.stack.len() - n
    }

    /// Absolute stack index of local `slot` within the frame whose window opens at `base`. The
    /// debug-only guard catches a slot outside the live locals window — the desync most likely to
    /// be introduced once P4/P5 mutate the stack as a GC root set — before the raw index panics.
    #[inline]
    pub(super) fn frame_slot(&self, base: usize, slot: usize) -> usize {
        let idx = base + slot;
        debug_assert!(
            idx < self.stack.len(),
            "vm local out of range: base {base} + slot {slot} = {idx} >= stack len {} (func {})",
            self.stack.len(),
            self.frames.last().map_or(usize::MAX, |f| f.func)
        );
        idx
    }

    /// Pop the top `n` values, returning them in stack order (bottom-most first).
    /// The compiler guarantees `n <= stack.len()`.
    pub(super) fn split_off(&mut self, n: usize) -> Vec<Value> {
        let start = self.pop_n_start(n);
        self.stack.split_off(start)
    }

    /// Pop two ints in operand order: returns `(lhs, rhs)` for `lhs OP rhs`.
    #[inline(always)]
    pub(super) fn pop2_int(&mut self) -> Result<(i64, i64), String> {
        let b = self.pop_int()?;
        let a = self.pop_int()?;
        Ok((a, b))
    }

    #[inline(always)]
    pub(super) fn pop2_float(&mut self) -> Result<(f64, f64), String> {
        let b = self.pop_float()?;
        let a = self.pop_float()?;
        Ok((a, b))
    }

    /// Pop two raw values in operand order: returns `(lhs, rhs)` for `lhs OP rhs`. Used by the decimal
    /// ops (M-NUM S1), whose kernel coerces a mixed `Decimal`/`Int` pair itself (no per-type pop).
    #[inline]
    pub(super) fn pop2(&mut self) -> (Value, Value) {
        let b = self.pop();
        let a = self.pop();
        (a, b)
    }

    #[inline]
    pub(super) fn pop_int(&mut self) -> Result<i64, String> {
        match self.pop() {
            Value::Int(n) => Ok(n),
            v => Err(expected("int", &v)),
        }
    }

    #[inline]
    pub(super) fn pop_float(&mut self) -> Result<f64, String> {
        match self.pop() {
            Value::Float(x) => Ok(x),
            v => Err(expected("float", &v)),
        }
    }

    /// Push the result of a checked integer kernel, propagating its fault body (e.g.
    /// `"integer overflow"`) verbatim — the fault string is single-sourced in `value`.
    #[inline(always)]
    pub(super) fn push_i(&mut self, r: Result<i64, String>) -> Result<(), String> {
        self.stack.push(Value::Int(r?));
        Ok(())
    }
    /// Push a fallible `f64` result, propagating a zero-divisor fault (`float_div`/`float_rem`). The
    /// `?` turns the kernel's fault body into the VM fault, byte-identical to the interpreter.
    #[inline(always)]
    pub(super) fn push_f(&mut self, r: Result<f64, String>) -> Result<(), String> {
        self.stack.push(Value::Float(r?));
        Ok(())
    }
}

/// The operand-type fault body for the `pop_int`/`pop_float` family, OUTLINED (DEC-442).
///
/// This function exists for one reason and it is measured, not stylistic: with the `format!` inline in
/// `pop_int`'s body, that body carried the whole formatting machinery plus an allocation, so LLVM declined
/// to inline it and **every** integer pop in every hot loop became an out-of-line call — 24.6% of the VM's
/// loop instructions were the stack helpers as real calls (DEC-441's profile). `#[cold]` +
/// `#[inline(never)]` keeps the fault path out of the caller entirely, leaving `pop_int` small enough to
/// inline.
///
/// **The message text is byte-for-byte what it was** (`"expected int, found …"` / `"expected float, …"`) —
/// fault bodies are parity-affecting (Invariant 4), so this refactor must not touch them, and the full
/// suite is what proves it did not.
#[cold]
#[inline(never)]
fn expected(want: &str, got: &Value) -> String {
    format!("expected {want}, found {}", got.type_name())
}
#[cfg(test)]
mod tests {
    use super::*;

    /// The operand-type fault bodies are byte-for-byte what they were before [`expected`] outlined them
    /// (DEC-442). Pinned by a test rather than by inspection because fault bodies are PARITY-AFFECTING
    /// (Invariant 4): the `agree_err` oracle classifies backends by body, so a stray word here would be a
    /// silent Invariant-1 break.
    ///
    /// Nothing else covers these strings — the type checker proves operand types before the VM runs, so
    /// the paths are unreachable for any checked program and could only be driven by hand-built invalid
    /// bytecode. That is exactly why the refactor needed a pin: a message with no test is free to drift.
    #[test]
    fn operand_type_fault_bodies_are_unchanged_by_outlining() {
        assert_eq!(
            expected("int", &Value::Str("x".into())),
            "expected int, found string"
        );
        assert_eq!(
            expected("float", &Value::Int(1)),
            "expected float, found int"
        );
    }
}
