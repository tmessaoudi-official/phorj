//! Bytecode VM — closure (M-Decomp W4). See mod.rs for the struct + core + entry points.

use super::*;

impl<'a> Vm<'a> {
    /// Invoke a first-class closure VALUE re-entrantly and return its result. Unlike [`Op::CallValue`]
    /// (which pushes a frame and lets the main `run` loop drive it), this is called from *inside* a
    /// higher-order native (`Core.List.map`/`filter`/`reduce`) that needs the closure's result
    /// synchronously: it pushes the closure's frame, runs a nested loop until exactly that frame (and
    /// any frames it spawns) returns, then pops and returns the value left on the stack. The slot math
    /// mirrors `Op::CallValue`; execution shares `exec_op` with the main loop — one execution core, no
    /// second interpreter (the parity analogue of the tree-walker's `call_closure`). M-RT S7b-3.
    ///
    /// **Allocation-free per call, deliberately** (perf, 2026-08-01). This is the per-ELEMENT path for
    /// every higher-order native, so a heap allocation here is one allocation per list element / per
    /// file line. Two used to happen and both are gone: `captures.clone()` built a throwaway `Vec` on
    /// every call (now the captures are cloned straight onto the stack, element by element — an `Rc`
    /// bump each, no container), and `args` was a `Vec` the caller had to build (now a borrowed slice,
    /// so `&[x.clone()]` is a stack temporary). Measured on `forEachLine` over 40k lines, where
    /// malloc/free was 24% of all instructions retired.
    pub(super) fn call_closure_value(
        &mut self,
        callee: &Value,
        args: &[Value],
    ) -> Result<Value, String> {
        // Clone the `Rc` (a refcount bump) rather than borrowing through `callee`: the borrow would
        // have to stay live across the `self.stack` mutation below.
        let cd = match callee {
            Value::Closure(cd) => cd.clone(),
            v => return Err(format!("cannot call {} as a function", v.type_name())),
        };
        let (func_idx, captures): (usize, &[Value]) = match cd.as_ref() {
            crate::value::ClosureData::Byte { func, captures } => (*func, captures),
            _ => return Err("expected a bytecode closure".to_string()),
        };
        let func_arity = self.program.functions[func_idx].arity;
        let n_captures = self.program.functions[func_idx].n_captures;
        let n_params = func_arity - n_captures;
        if args.len() != n_params {
            return Err(format!(
                "wrong number of arguments: expected {n_params}, got {}",
                args.len()
            ));
        }
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(crate::value::faults::FAULT_STACK_OVERFLOW.to_string());
        }
        // Frame layout `[captures.., args..]` — identical to `Op::CallValue`.
        let slot_base = self.stack.len();
        self.stack.extend(captures.iter().cloned());
        self.stack.extend(args.iter().cloned());
        let target_depth = self.frames.len();
        self.frames.push(Frame {
            func: func_idx,
            ip: 0,
            slot_base,
        });
        self.run_until(target_depth)?;
        // The closure's `Return` (frames shrank back to `target_depth`, which is >= 1 so the caller
        // is non-empty) left its value on top of the stack via `do_return`.
        Ok(self.pop())
    }

    /// Run a first-class closure VALUE as this VM's ROOT frame, returning its result plus captured
    /// stdout — the closure twin of [`run_entry`](Vm::run_entry), used once per request by the
    /// DEC-331 D5 web serve path.
    ///
    /// **This is deliberately NOT [`call_closure_value`](Vm::call_closure_value) with an empty frame
    /// stack, and the difference is not stylistic.** That method ends in `self.pop()`, which is
    /// correct only because a re-entrant call always has a caller frame beneath it: `do_return`
    /// pushes the return value `if !self.frames.is_empty()`, so at depth 0 the value is never pushed
    /// and the `pop` would take an unrelated stack slot (or underflow). The bottom-frame contract is
    /// `exit_value` instead — the same one [`run_entry`](Vm::run_entry) and the coop scheduler's
    /// `run_task_function` use. Verified by
    /// `a_closure_called_as_the_root_frame_returns_its_value`, which fails on the `call_closure_value`
    /// spelling.
    pub fn run_closure_entry(
        mut self,
        closure: &Value,
        args: &[Value],
    ) -> Result<(Value, String), Diagnostic> {
        self.program.validate().map_err(Diagnostic::runtime)?;
        let cd = match closure {
            Value::Closure(cd) => cd.clone(),
            v => {
                return Err(Diagnostic::runtime(format!(
                    "cannot call {} as a function",
                    v.type_name()
                )))
            }
        };
        let (func_idx, captures): (usize, &[Value]) = match cd.as_ref() {
            crate::value::ClosureData::Byte { func, captures } => (*func, captures),
            _ => return Err(Diagnostic::runtime("expected a bytecode closure")),
        };
        let func_arity = self.program.functions[func_idx].arity;
        let n_captures = self.program.functions[func_idx].n_captures;
        let n_params = func_arity - n_captures;
        if args.len() != n_params {
            return Err(Diagnostic::runtime(format!(
                "wrong number of arguments: expected {n_params}, got {}",
                args.len()
            )));
        }
        // Frame layout `[captures.., args..]` at `slot_base = 0` — identical to
        // [`call_closure_value`]'s layout, but as the BOTTOM frame (the `run_entry` window).
        self.stack.extend(captures.iter().cloned());
        self.stack.extend(args.iter().cloned());
        self.frames.push(Frame {
            func: func_idx,
            ip: 0,
            slot_base: 0,
        });
        self.run_to_completion()?;
        Ok((self.exit_value, self.out))
    }

    /// Invoke a plain (capture-less) function by its table index re-entrantly and return its result —
    /// the `Op::SpawnCall` analogue of [`call_closure_value`] (S4.3). Used by the eager `spawn` path to
    /// run a free function inline, and by the cooperative driver to run a task's root call. Pushes the
    /// function's frame `[args..]` and drives a nested `run_until` to exactly that frame's return.
    pub(super) fn call_function_value(
        &mut self,
        func_idx: usize,
        args: Vec<Value>,
    ) -> Result<Value, String> {
        let func_arity = self.program.functions[func_idx].arity;
        if args.len() != func_arity {
            return Err(format!(
                "wrong number of arguments: expected {func_arity}, got {}",
                args.len()
            ));
        }
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(crate::value::faults::FAULT_STACK_OVERFLOW.to_string());
        }
        let slot_base = self.stack.len();
        self.stack.extend(args);
        let target_depth = self.frames.len();
        self.frames.push(Frame {
            func: func_idx,
            ip: 0,
            slot_base,
        });
        self.run_until(target_depth)?;
        Ok(self.pop())
    }

    /// Drive `exec_op` until the frame stack shrinks back to `target_depth` (the depth *before* the
    /// frame to run was pushed). Used only by [`Vm::call_closure_value`] for re-entrant native
    /// callbacks; the top-level `run` loop is the `target_depth == 0` analogue that additionally
    /// captures output and returns it. A fault propagates as a raw `String` — the outer `run` loop
    /// (still executing the `CallNative` op) attaches the source line, exactly as for any native fault.
    pub(super) fn run_until(&mut self, target_depth: usize) -> Result<(), String> {
        // See the main loop in `mod.rs`: copy the `&'a` program reference out of `self` so `op` can
        // be borrowed from it (not cloned) while `self` is free for the `&mut` `exec_op` call.
        let program = self.program;
        // Same dispatch cache + single-borrow frame read as the main loop (DEC-448) — `func → code` is
        // immutable, so caching the last slice is sound across frame churn. This loop is the PER-ELEMENT
        // path for every higher-order native, so its per-op cost is paid once per list element / file
        // line: DEC-434 measured `run_until` at 10.6% of `fsforeachline`'s whole per-line budget.
        let mut cached: Option<(usize, &'a [Op])> = None;
        while self.frames.len() > target_depth {
            let fr = self.frames.len() - 1;
            let (func, ip) = {
                let f = self
                    .frames
                    .last_mut()
                    .expect("vm frame stack empty (compiler bug)");
                let ip = f.ip;
                f.ip = ip + 1;
                (f.func, ip)
            };
            let code = match cached {
                Some((cf, c)) if cf == func => c,
                _ => {
                    let c = program.functions[func].chunk.code.as_slice();
                    cached = Some((func, c));
                    c
                }
            };
            if ip >= code.len() {
                // `ip` was pre-incremented above; `do_return` pops this frame, so it is discarded.
                self.do_return(Value::Unit);
                continue;
            }
            let op = &code[ip];
            match self.exec_op(op, code.get(ip + 1), fr, func) {
                Ok(Flow::Next) => {}
                // `Flow::Done` is only ever returned by `main`'s `Return`; at `target_depth >= 1`
                // (always, since a native runs inside at least `main`) it is unreachable, but exit
                // cleanly rather than spin if a future caller passes `target_depth == 0`.
                Ok(Flow::Done) => return Ok(()),
                Err(msg) => {
                    // A throw raised inside this re-entrant call: unwind to a handler owned by *this*
                    // call (frame_depth above `target_depth`, i.e. a `try` inside the closure). If
                    // none exists, the throw escapes the closure — propagate the sentinel (with
                    // `pending_throw` intact) so the native returns it and the outer `run` loop
                    // unwinds to the `try` surrounding the higher-order call (M-faults 2b).
                    if msg == crate::chunk::THROW_SENTINEL && self.unwind_throw(target_depth) {
                        continue;
                    }
                    return Err(msg);
                }
            }
        }
        Ok(())
    }
}
