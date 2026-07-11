//! Statement compilation — loops, break/continue, try/finally.

use super::*;

impl Compiler<'_> {
    /// `for (T name in iter)` desugars to a counter loop over an inline list
    /// (decision P2-7). Hidden locals `$for_list` and `$for_idx` bracket `name`.
    pub(in crate::compiler) fn compile_for(
        &mut self,
        name: &str,
        elem_ty: CTy,
        kv: Option<(&str, CTy)>,
        iter: &Expr,
        body: &[Stmt],
        line: u32,
    ) -> Result<(), String> {
        self.begin_scope();
        self.expr(iter)?; // [iterable]
                          // B1 iteration protocol: normalize the source collection (`List`/`Set`) to a plain element
                          // list once, so the loop body below indexes a list regardless of the source kind. Shared with
                          // the interpreter via `value::iter_elements` ⇒ byte-identical iteration order.
        self.emit(Op::IterElems, line); // [list]
        let s_list = self.add_local("$for_list", CTy::Other);
        self.emit_const(Value::Int(0), line); // [list, 0]
        let s_idx = self.add_local("$for_idx", CTy::Int);

        // break/continue pop down to here (just `$for_list`+`$for_idx` live): a `break` lands at the
        // exit (where `end_scope` drops those two), a `continue` at the index-increment — both after
        // the loop variable has been dropped, so the pop count covers the loop var + any body locals.
        let body_base = self.locals.len();
        let loop_start = self.here();
        self.emit(Op::GetLocal(s_idx), line);
        self.emit(Op::GetLocal(s_list), line);
        self.emit(Op::Len, line); // [idx, len]
        self.emit(Op::Lt, line); // [idx < len]
        let exit_jump = self.emit_jump(Op::JumpIfFalse(0), line);

        // B1: for the two-binding Map form, `IterElems` yields a list of `[key, value]` 2-lists, so
        // each `elem` is destructured into two loop vars (re-fetching the pair for each — cheap, no dup
        // op). The single-binding form binds the element directly.
        if let Some((vname, vcty)) = kv.clone() {
            self.emit(Op::GetLocal(s_list), line);
            self.emit(Op::GetLocal(s_idx), line);
            self.emit(Op::Index, line); // [pair]
            self.emit_const(Value::Int(0), line);
            self.emit(Op::Index, line); // [key]
            self.add_local(name, elem_ty); // key loop var
            self.emit(Op::GetLocal(s_list), line);
            self.emit(Op::GetLocal(s_idx), line);
            self.emit(Op::Index, line); // [pair]
            self.emit_const(Value::Int(1), line);
            self.emit(Op::Index, line); // [value]
            self.add_local(vname, vcty); // value loop var
        } else {
            self.emit(Op::GetLocal(s_list), line);
            self.emit(Op::GetLocal(s_idx), line);
            self.emit(Op::Index, line); // [elem]
            self.add_local(name, elem_ty); // elem becomes the loop variable
        }

        self.loop_frames.push(LoopFrame {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            body_base,
        });
        self.begin_scope(); // body's own locals get cleaned each iteration
        for s in body {
            self.stmt(s)?;
        }
        self.end_scope(line);
        let frame = self.loop_frames.pop().expect("for loop frame");

        // Drop the loop variable(s): two for the Map two-binding form (value then key, LIFO), one
        // otherwise.
        if kv.is_some() {
            self.emit(Op::Pop, line);
            self.locals.pop();
        }
        self.emit(Op::Pop, line); // drop the loop variable
        self.locals.pop(); // unregister `name`

        // idx = idx + 1 — also the `continue` target (loop var already dropped above).
        let cont_target = self.here();
        for j in &frame.continue_jumps {
            self.patch_jump_to(*j, cont_target);
        }
        self.emit(Op::GetLocal(s_idx), line);
        self.emit_const(Value::Int(1), line);
        self.emit(Op::AddI, line);
        self.emit(Op::SetLocal(s_idx), line);
        self.emit(Op::Jump(loop_start), line);

        self.patch_jump(exit_jump);
        for j in &frame.break_jumps {
            self.patch_jump(*j);
        }
        self.end_scope(line); // pops $for_idx, $for_list
        Ok(())
    }

    /// `while (cond) { .. }` / `do { .. } while (cond);` (M-mut.3). Lowers to a `JumpIfFalse`-guarded
    /// back-edge — no new loop opcode (F5). A loop frame collects `break`/`continue` jumps; `break`
    /// targets the exit, `continue` the condition re-test (the loop top for `while`, the bottom test
    /// for `do-while`). `body_base` is the locals depth both forms pop down to.
    pub(in crate::compiler) fn compile_while(
        &mut self,
        cond: &Expr,
        body: &[Stmt],
        post_cond: bool,
        line: u32,
    ) -> Result<(), String> {
        self.begin_scope();
        let body_base = self.locals.len();
        if post_cond {
            // do-while: body first, condition (and the continue target) at the bottom.
            let loop_start = self.here();
            self.loop_frames.push(LoopFrame {
                break_jumps: Vec::new(),
                continue_jumps: Vec::new(),
                body_base,
            });
            self.begin_scope();
            for s in body {
                self.stmt(s)?;
            }
            self.end_scope(line);
            let frame = self.loop_frames.pop().expect("do-while loop frame");

            let cont_target = self.here();
            for j in &frame.continue_jumps {
                self.patch_jump_to(*j, cont_target);
            }
            self.expr(cond)?; // [cond]
            let exit_jump = self.emit_jump(Op::JumpIfFalse(0), line); // false → exit
            self.emit(Op::Jump(loop_start), line); // true → loop again
            self.patch_jump(exit_jump);
            for j in &frame.break_jumps {
                self.patch_jump(*j);
            }
        } else {
            // while: condition (and the continue target) at the top.
            let loop_start = self.here();
            self.expr(cond)?; // [cond]
            let exit_jump = self.emit_jump(Op::JumpIfFalse(0), line); // pops cond
            self.loop_frames.push(LoopFrame {
                break_jumps: Vec::new(),
                continue_jumps: Vec::new(),
                body_base,
            });
            self.begin_scope();
            for s in body {
                self.stmt(s)?;
            }
            self.end_scope(line);
            let frame = self.loop_frames.pop().expect("while loop frame");

            self.emit(Op::Jump(loop_start), line);
            self.patch_jump(exit_jump);
            for j in &frame.break_jumps {
                self.patch_jump(*j);
            }
            for j in &frame.continue_jumps {
                self.patch_jump_to(*j, loop_start);
            }
        }
        self.end_scope(line);
        Ok(())
    }

    /// C-style `for (init; cond; step) { body }` (M-mut.3). `init`'s binding lives in the loop's own
    /// scope; `continue` jumps to `step`, `break` to the exit. Same jump back-edge as `compile_while`.
    pub(in crate::compiler) fn compile_cfor(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        step: Option<&Stmt>,
        body: &[Stmt],
        line: u32,
    ) -> Result<(), String> {
        self.begin_scope();
        if let Some(s) = init {
            self.stmt(s)?;
        }
        // break/continue pop down to here (init's local stays live; the exit's `end_scope` drops it).
        let body_base = self.locals.len();
        let loop_start = self.here();
        let exit_jump = if let Some(c) = cond {
            self.expr(c)?;
            Some(self.emit_jump(Op::JumpIfFalse(0), line))
        } else {
            None // `for (init;;step)` — no condition, loop until `break`
        };
        self.loop_frames.push(LoopFrame {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
            body_base,
        });
        self.begin_scope();
        for s in body {
            self.stmt(s)?;
        }
        self.end_scope(line);
        let frame = self.loop_frames.pop().expect("c-for loop frame");

        // `continue` lands at the step (run the step, then re-test).
        let cont_target = self.here();
        for j in &frame.continue_jumps {
            self.patch_jump_to(*j, cont_target);
        }
        if let Some(s) = step {
            self.stmt(s)?;
        }
        self.emit(Op::Jump(loop_start), line);

        if let Some(e) = exit_jump {
            self.patch_jump(e);
        }
        for j in &frame.break_jumps {
            self.patch_jump(*j);
        }
        self.end_scope(line); // pops init's local
        Ok(())
    }

    /// Emit a `break` (`is_break`) or `continue`: pop every body-scope local back down to the
    /// innermost loop's `body_base`, then a placeholder `Jump` recorded in the loop frame for the
    /// loop to patch (exit for `break`, continue-target for `continue`). No new `Op` (F5). The
    /// checker rejects break/continue outside a loop, so the frame is always present.
    pub(in crate::compiler) fn compile_break_continue(
        &mut self,
        is_break: bool,
        line: u32,
    ) -> Result<(), String> {
        let body_base = self
            .loop_frames
            .last()
            .map(|f| f.body_base)
            .ok_or("`break`/`continue` outside a loop")?;
        // Run the `finally` (+ `PopHandler`) of every `try` entered *inside* this (innermost) loop,
        // before unwinding the loop-body locals (M-faults 2b). A `try` outside the loop is not exited.
        let cur_loop_depth = self.loop_frames.len();
        let n_exit = self
            .finally_stack
            .iter()
            .rev()
            .take_while(|c| c.loop_depth >= cur_loop_depth)
            .count();
        self.emit_finally_for_exit(n_exit, line)?;
        for _ in 0..(self.locals.len() - body_base) {
            self.emit(Op::Pop, line);
        }
        let j = self.emit_jump(Op::Jump(0), line);
        let frame = self.loop_frames.last_mut().expect("loop frame present");
        if is_break {
            frame.break_jumps.push(j);
        } else {
            frame.continue_jumps.push(j);
        }
        Ok(())
    }

    /// `try { body } catch (T e) { … } … [finally { … }]` — native unwinding (M-faults 2b).
    ///
    /// Shape emitted:
    /// ```text
    ///   PushHandler(catch_lp)        ; capture frame depth + stack height
    ///   <body>                       ; a Throw unwinds to catch_lp
    ///   PopHandler ; <finally> ; Jump(end)        ; normal completion
    /// catch_lp:                       ; VM landed with the thrown value at slot `v_slot`
    ///   ; ($exc local registered here so catch-body locals stack above the value)
    ///   for each clause: <type test(s)> → bind+body → <finally> → Jump(cleanup)
    ///   <finally> ; Throw             ; no clause matched → re-throw
    /// cleanup: Pop($exc)              ; caught paths discard the value, converge with the normal path
    /// end:
    /// ```
    /// A `return`/`break`/`continue` inside the body or a catch runs the same `finally` (and pops the
    /// handler) via the `finally_stack` (see `emit_finally_for_exit`).
    pub(in crate::compiler) fn compile_try(
        &mut self,
        body: &[Stmt],
        catches: &[crate::ast::CatchClause],
        finally: Option<&[Stmt]>,
        line: u32,
    ) -> Result<(), String> {
        // A `try` is a statement, so the operand stack is clean here: height == locals.len().
        let body_base = self.locals.len();
        let push_idx = self.emit_jump(Op::PushHandler(0), line); // target patched to catch_lp

        // --- try body ---
        self.finally_stack.push(TryCtx {
            finally: finally.map(<[Stmt]>::to_vec),
            has_handler: true,
            loop_depth: self.loop_frames.len(),
        });
        self.begin_scope();
        for s in body {
            self.stmt(s)?;
        }
        self.end_scope(line);
        self.finally_stack.pop();

        // --- normal completion: drop the handler, run finally, skip the catch block ---
        self.emit(Op::PopHandler, line);
        self.emit_finally_block(finally, line)?;
        let normal_jump = self.emit_jump(Op::Jump(0), line);

        // --- catch landing pad: the VM pushed the thrown value at slot `v_slot` ---
        let catch_lp = self.here();
        self.patch_jump_to(push_idx, catch_lp);
        let v_slot = body_base;
        self.height = v_slot + 1;
        // Register the thrown value as a local so a catch body's own locals stack above it (the
        // per-statement `height = locals.len()` reset then keeps every slot aligned).
        let exc = self.add_local("$exc", CTy::Other);
        debug_assert_eq!(exc, v_slot, "thrown value slot must be the next frame slot");

        let mut caught_jumps = Vec::new();
        for clause in catches {
            // Dispatch: a match on any of the clause's type names falls through to `bind`; a full
            // miss jumps to `no_match` (the next clause).
            let names = catch_clause_names(&clause.ty);
            let mut to_bind = Vec::new();
            for name in &names {
                self.height = v_slot + 1;
                self.emit(Op::GetLocal(v_slot), line);
                self.emit(Op::IsInstance(name.clone()), line);
                let next_name = self.emit_jump(Op::JumpIfFalse(0), line); // false → try next name
                to_bind.push(self.emit_jump(Op::Jump(0), line)); // true → bind
                self.patch_jump(next_name);
            }
            let no_match = self.emit_jump(Op::Jump(0), line);
            let bind = self.here();
            for j in to_bind {
                self.patch_jump_to(j, bind);
            }

            // Bind the value to the clause variable (via `match_bindings`, reading slot `v_slot`),
            // then compile the catch body with its own finally/transfer context (no handler — the
            // unwind already consumed it).
            self.height = v_slot + 1;
            self.begin_scope();
            let n_before = self.match_bindings.len();
            self.match_bindings.push(MatchBinding {
                name: clause.name.clone(),
                match_slot: v_slot,
                path: Vec::new(),
                ty: catch_binding_cty(&clause.ty),
            });
            self.finally_stack.push(TryCtx {
                finally: finally.map(<[Stmt]>::to_vec),
                has_handler: false,
                loop_depth: self.loop_frames.len(),
            });
            for s in &clause.body {
                self.stmt(s)?;
            }
            self.finally_stack.pop();
            self.match_bindings.truncate(n_before);
            self.end_scope(line);

            // Caught: run finally, drop the value, converge.
            self.height = v_slot + 1;
            self.emit_finally_block(finally, line)?;
            self.emit(Op::Pop, line); // discard $exc → height v_slot
            caught_jumps.push(self.emit_jump(Op::Jump(0), line)); // → end (past cleanup's Pop)
            self.patch_jump(no_match);
        }

        // --- no clause matched: run finally, re-throw the value (still on top at `v_slot`) ---
        self.height = v_slot + 1;
        self.emit_finally_block(finally, line)?;
        self.emit(Op::Throw, line);

        // --- converge ---
        self.locals.pop(); // unregister $exc
        let end = self.here();
        self.patch_jump_to(normal_jump, end);
        for j in caught_jumps {
            self.patch_jump_to(j, end);
        }
        self.height = body_base;
        Ok(())
    }

    /// Emit a `finally` block inline (a fresh scope), or nothing when there is no `finally`. Balanced:
    /// `self.height`/`self.locals` are unchanged on return (M-faults 2b).
    pub(in crate::compiler) fn emit_finally_block(
        &mut self,
        finally: Option<&[Stmt]>,
        line: u32,
    ) -> Result<(), String> {
        if let Some(stmts) = finally {
            self.begin_scope();
            for s in stmts {
                self.stmt(s)?;
            }
            self.end_scope(line);
        }
        Ok(())
    }

    /// Emit the `PopHandler` (when the context's handler is still installed) and `finally` block for
    /// the innermost `n_exit` `try` contexts, innermost-first — run before a `return`/`break`/
    /// `continue` transfers out of them (M-faults 2b). The contexts are temporarily removed while
    /// their finallys are emitted so a transfer *inside* a finally doesn't re-enter them, then
    /// restored (the `try`s remain lexically active for the fall-through paths).
    pub(in crate::compiler) fn emit_finally_for_exit(
        &mut self,
        n_exit: usize,
        line: u32,
    ) -> Result<(), String> {
        if n_exit == 0 {
            return Ok(());
        }
        let start = self.finally_stack.len() - n_exit;
        let removed = self.finally_stack.split_off(start); // innermost == last
        let mut result = Ok(());
        for ctx in removed.iter().rev() {
            if ctx.has_handler {
                self.emit(Op::PopHandler, line);
            }
            if let Some(stmts) = &ctx.finally {
                self.begin_scope();
                for s in stmts {
                    if let Err(e) = self.stmt(s) {
                        result = Err(e);
                        break;
                    }
                }
                if result.is_ok() {
                    self.end_scope(line);
                }
            }
            if result.is_err() {
                break;
            }
        }
        self.finally_stack.extend(removed); // the try contexts are still lexically active
        result
    }
}
