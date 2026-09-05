//! `Op::Concat` — string concatenation and interpolation, split out of `exec.rs` (Invariant 13) when
//! the DEC-431 B accumulator peephole made the arm the largest in the dispatch match.

use super::Vm;
use crate::chunk::Op;
use crate::value::Value;

impl Vm<'_> {
    /// The body of the `Op::Concat(n)` arm. `next` is the op that follows in the same chunk — read
    /// only by the `s = s + x` peephole below.
    pub(super) fn exec_concat(
        &mut self,
        n: usize,
        next: Option<&Op>,
        fr: usize,
    ) -> Result<(), String> {
        let mut parts = self.split_off(n);
        // DEC-431 B — the ACCUMULATOR peephole, `s = s + x`. The bytecode is
        // `GetLocal(k); <x>; Concat(2); SetLocal(k)`, so at this op the accumulator's
        // `Rc` is aliased twice — the slot AND the stack copy — and `PhStr::concat` has to
        // copy the whole string every iteration: quadratic off the JIT (492 ms at 20k lines
        // against the JIT's 2.3 ms, measured in KNOWN_ISSUES). When the NEXT op stores back
        // into the very slot the left operand came from, the slot's reference is about to be
        // overwritten anyway, so it can be released NOW: take the slot (leave `Unit`), which
        // makes the stack copy unique, and append in place — amortised O(1), the same
        // uniqueness rule PHP's `.=` (refcount 1) and the JIT's arena slot rely on.
        //
        // Safe by construction: (1) the RHS is already evaluated and on the stack, so a
        // self-reference (`s = s + s`) simply holds a third `Rc` and `append_in_place`
        // declines — it falls through to the copying path with the right answer; (2) nothing
        // executes between this op and the `SetLocal` that refills the slot, so the `Unit`
        // placeholder is never observable — a two-`Str` concat cannot fault. A new `Op` was
        // the ruled shape (`TakeLocal`); this needs none, so Invariant 3's three exhaustive
        // matches are untouched. DEC-463 classes the fix as an implementation choice.
        if n == 2 {
            if let (Some(&Op::SetLocal(slot)), [Value::Str(a), Value::Str(b)]) =
                (next, parts.as_slice())
            {
                let base = self.frames[fr].slot_base;
                let idx = self.frame_slot(base, slot);
                let same = matches!(
                    (&self.stack[idx], a),
                    (Value::Str(crate::phstr::PhStr::Heap(x)), crate::phstr::PhStr::Heap(y))
                        if std::rc::Rc::ptr_eq(x, y)
                );
                if same {
                    let bytes: Vec<u8> = b.as_bytes().to_vec();
                    self.stack[idx] = Value::Unit;
                    if let Some(Value::Str(acc)) = parts.first_mut() {
                        if acc.append_in_place(&bytes) {
                            self.concat_in_place += 1;
                            let acc = parts.swap_remove(0);
                            self.stack.push(acc);
                            return Ok(());
                        }
                    }
                }
            }
        }
        // Two-`Str` fast path (`a + b`, the dominant concat shape): the single-sourced
        // `PhStr::concat` kernel — short results stay inline, no display round-trip.
        if let [Value::Str(a), Value::Str(b)] = parts.as_slice() {
            let joined = crate::phstr::PhStr::concat(a, b);
            self.stack.push(Value::Str(joined));
        } else {
            let mut s = String::new();
            for v in &parts {
                match v.as_display() {
                    Some(t) => s.push_str(&t),
                    None => {
                        return Err(format!(
                            "cannot interpolate {} into a string",
                            v.type_name()
                        ))
                    }
                }
            }
            self.stack.push(Value::Str(s.into()));
        }
        Ok(())
    }
}
