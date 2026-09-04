//! VM — running `Runtime.onShutdown` handlers (DEC-204, shape DEC-497).
//!
//! Split out of `mod.rs`, which is grandfathered under Invariant 13 and must not grow.
//!
//! The whole reason this is not two lines inside `run_main`: a handler runs AFTER `main`, so the
//! frame stack is EMPTY, and the re-entrant `call_closure_value` is only correct with a caller frame
//! beneath it (it ends in `self.pop()`, and `do_return` pushes the return value only
//! `if !self.frames.is_empty()`). The first version used it and panicked with
//! `vm stack underflow (compiler bug)` — through the no-crash contract. The bottom-frame contract is
//! `run_to_completion` + `exit_value`, the same one `run_entry` and `run_closure_entry` use.

use super::*;

impl Vm<'_> {
    /// Run and clear this thread's `Runtime.onShutdown` handlers (DEC-204, shape DEC-497).
    ///
    /// A handler's fault is REPORTED and does not become the program's exit status: it runs after
    /// `main`'s result is decided, so letting it overwrite that result would let a cleanup routine
    /// turn a successful run into a failing one, or replace a real diagnosis with a later one. Every
    /// handler runs even if an earlier faulted. Must stay byte-identical with the interpreter's
    /// `run_shutdown_handlers` — the parity spine covers this path like any other.
    pub(super) fn run_shutdown_handlers(&mut self) {
        for h in crate::shutdown::take_handlers() {
            if let Err(msg) = self.run_shutdown_handler(&h) {
                eprintln!("phg: a Runtime.onShutdown handler faulted ({msg}); continuing shutdown");
            }
        }
    }

    /// Run ONE handler as a ROOT frame.
    ///
    /// This deliberately does not use `call_closure_value`, and the difference is not stylistic —
    /// that method ends in `self.pop()`, which is only correct with a caller frame beneath it,
    /// because `do_return` pushes the return value `if !self.frames.is_empty()`. `main` has already
    /// returned by the time a handler runs, so the frame stack is EMPTY and the pop underflowed:
    /// `vm stack underflow (compiler bug)`, a panic, straight through the no-crash contract. The
    /// bottom-frame contract is `run_to_completion` + `exit_value`, the same one `run_entry` and
    /// `run_closure_entry` use. `shutdown_handlers_run_after_main_in_registration_order_on_every_leg` pins it.
    fn run_shutdown_handler(&mut self, handler: &Value) -> Result<(), String> {
        let cd = match handler {
            Value::Closure(cd) => cd.clone(),
            v => return Err(format!("cannot call {} as a function", v.type_name())),
        };
        let (func_idx, captures): (usize, &[Value]) = match cd.as_ref() {
            crate::value::ClosureData::Byte { func, captures } => (*func, captures),
            _ => return Err("expected a bytecode closure".to_string()),
        };
        let f = &self.program.functions[func_idx];
        if f.arity != f.n_captures {
            return Err(format!(
                "a Runtime.onShutdown handler takes no parameters, but this one takes {}",
                f.arity - f.n_captures
            ));
        }
        let slot_base = self.stack.len();
        self.stack.extend(captures.iter().cloned());
        self.frames.push(Frame {
            func: func_idx,
            ip: 0,
            slot_base,
        });
        self.run_to_completion().map_err(|d| d.message)?;
        // Leave the operand stack as we found it: a handler runs after `main`, so anything it leaves
        // behind is pure residue, and the next handler's `slot_base` must not drift upward.
        self.stack.truncate(slot_base);
        Ok(())
    }
}
