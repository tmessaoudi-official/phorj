//! Statement compilation — dispatch, destructuring, narrowed branches.

use super::*;

impl Compiler<'_> {
    pub(in crate::compiler) fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        // Every statement begins with a clean operand stack (transients == 0), so the live operand
        // height equals the live-locals count. Anchoring here keeps `match`'s scrutinee slot exact
        // regardless of any height drift in preceding dead-code-after-`return`.
        self.height = self.locals.len();
        match s {
            Stmt::VarDecl { ty, name, init, .. } => {
                self.expr(init)?; // value stays on the stack as the new local's slot
                                  // `var` carries no annotation — derive the local's `CTy` from the initializer so
                                  // later arithmetic on it still specializes (AddI/AddF). `ctype` is total over
                                  // checker-valid initializers here; fall back to `Other` defensively so a `var`
                                  // never makes a program the interpreter accepts fail to compile (parity spine).
                let local_ty = match ty {
                    Type::Infer(_) => self.ctype(init).unwrap_or(CTy::Other),
                    _ => resolve_cty(ty),
                };
                self.add_local(name, local_ty);
                Ok(())
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => match target {
                // Local reassignment reuses `Op::SetLocal` — no new Op (M-mut.1). The checker
                // guarantees the target is a `mutable` in-scope local, so the slot always resolves.
                Expr::Ident(name, _) => {
                    let slot = self
                        .resolve_local(name)
                        .ok_or_else(|| format!("unresolved local in assignment: {name}"))?;
                    self.expr(value)?; // push the new value
                    self.emit(Op::SetLocal(slot), span.line); // set-and-pop into the existing slot
                    Ok(())
                }
                // Value-type element set `xs[i] = e` / `m[k] = e` (M-mut.5). The container is a
                // mutable local (checker-enforced); load it, push index + value, `SetIndex` (COW),
                // then store the resulting container back. Nested places (`a[i][j]`, `this.f[i]`)
                // are a later slice — the checker rejects a non-Ident container as `E-ASSIGN-TARGET`.
                Expr::Index { .. } => {
                    // Flatten `root[i0]…[ik]` into (root local, index exprs in source order). A single
                    // index is the flat W8 `SetIndexLocal`; a nested chain is `SetPathLocal(slot, depth)`
                    // (Spec nested-value-index). Both mutate the slot in place (COW, O(1)/level) rather
                    // than the old GetLocal+SetIndex+SetLocal clone.
                    let mut chain: Vec<&Expr> = Vec::new();
                    let mut cur: &Expr = target;
                    let name = loop {
                        match cur {
                            Expr::Index { object, index, .. } => {
                                chain.push(index);
                                cur = object;
                            }
                            Expr::Ident(n, _) => break n,
                            _ => unreachable!("checker restricts the index-assign base to a local"),
                        }
                    };
                    chain.reverse(); // [i0..ik]
                    let slot = self
                        .resolve_local(name)
                        .ok_or_else(|| format!("unresolved local in index-assignment: {name}"))?;
                    let depth = chain.len();
                    for e in chain {
                        self.expr(e)?; // push i0..ik in source order
                    }
                    self.expr(value)?; // push the value
                    if depth == 1 {
                        self.emit(Op::SetIndexLocal(slot), span.line);
                    } else {
                        self.emit(Op::SetPathLocal(slot, depth), span.line);
                    }
                    Ok(())
                }
                // Static write `ClassName.field = e` (M-mut.7): push the value, store into the
                // program-level static slot. Checked first — the head is a class name, not a local.
                Expr::Member { object, name, .. } if self.static_slot(object, name).is_some() => {
                    let idx = self.static_slot(object, name).unwrap();
                    self.expr(value)?; // [value]
                    self.emit(Op::SetStatic(idx), span.line); // pop into the static slot
                    Ok(())
                }
                // Property hook write `o.name = e` (M-mut.7b) → call the synthetic `<name>$set`
                // 1-arg method with the receiver + value; it runs the set block and returns `Unit`,
                // which we discard. Resolved before the plain field path.
                Expr::Member { object, name, .. }
                    if self.hook_set_method(object, name).is_some() =>
                {
                    let setm = self.hook_set_method(object, name).unwrap();
                    self.expr(object)?; // [instance]
                    self.expr(value)?; // [instance, value]
                    let idx = self.field_name_index(&setm)?;
                    self.emit(Op::CallMethod(idx, 1), span.line); // [unit result]
                    self.emit(Op::Pop, span.line); // discard the set's return value
                    Ok(())
                }
                // Shared-mutable instance field set `o.f = e` / `this.f = e` (M-mut.6). Evaluate the
                // object then the value (interpreter eval order), then `SetField` mutates the shared
                // `Rc<Instance>` cell in place and pops both. The field is checker-guaranteed `mutable`.
                Expr::Member { object, name, .. } => {
                    self.expr(object)?; // [instance]
                    self.expr(value)?; // [instance, value]
                    let idx = self.field_name_index(name)?;
                    self.emit(Op::SetField(idx), span.line); // mutate in place, pop both
                    Ok(())
                }
                _ => unreachable!("checker rejects other assignment targets"),
            },
            Stmt::Expr(e, span) | Stmt::Discard(e, span) => {
                self.expr(e)?;
                self.emit(Op::Pop, span.line);
                Ok(())
            }
            Stmt::Return { value, span } => {
                // Inside a synthetic constructor body, a `return` does not yield the body's value:
                // the interpreter discards it and always returns the promoted instance
                // (`construct`). So evaluate any operand for its side effects, drop it, and jump to
                // the ctor epilogue (which loads + returns the instance). The checker pins a ctor
                // body's return type to `Unit`, so `value` is `None` or a unit-typed expression.
                if self.ctor_return_jumps.is_some() {
                    if let Some(e) = value {
                        self.expr(e)?;
                        self.emit(Op::Pop, span.line);
                    }
                    let j = self.emit_jump(Op::Jump(0), span.line);
                    self.ctor_return_jumps
                        .as_mut()
                        .expect("ctor_return_jumps is Some")
                        .push(j);
                    return Ok(());
                }
                match value {
                    Some(e) => self.expr(e)?,
                    None => self.emit_const(Value::Unit, span.line),
                }
                // A `return` exits every enclosing `try` (M-faults 2b): pop their handlers and run
                // their `finally` blocks first. Because a `finally` may evaluate (and the handler ops
                // must not perturb the value), spill the return value to a temp local so finally runs
                // on a clean operand stack, then reload it. No-op when no `try` is active.
                if self.finally_stack.is_empty() {
                    self.emit(Op::Return, span.line);
                } else {
                    let tmp = self.add_local("$ret", CTy::Other);
                    self.height = self.locals.len();
                    let n = self.finally_stack.len();
                    self.emit_finally_for_exit(n, span.line)?;
                    self.emit(Op::GetLocal(tmp), span.line);
                    self.emit(Op::Return, span.line);
                    self.locals.pop(); // unregister `$ret` (dead code follows a `return`)
                }
                Ok(())
            }
            Stmt::Block(stmts, span) => {
                self.begin_scope();
                for st in stmts {
                    self.stmt(st)?;
                }
                self.end_scope(span.line);
                Ok(())
            }
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => self.compile_if(
                cond,
                bind.as_deref(),
                then_block,
                else_block.as_deref(),
                span.line,
            ),
            Stmt::For {
                ty,
                name,
                val,
                iter,
                body,
                span,
            } => {
                let kv = val
                    .as_ref()
                    .map(|(vty, vname)| (vname.as_str(), resolve_cty(vty)));
                self.compile_for(name, resolve_cty(ty), kv, iter, body, span.line)
            }
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => self.compile_while(cond, body, *post_cond, span.line),
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                span,
            } => self.compile_cfor(
                init.as_deref(),
                cond.as_ref(),
                step.as_deref(),
                body,
                span.line,
            ),
            Stmt::Break(span) => self.compile_break_continue(true, span.line),
            Stmt::Continue(span) => self.compile_break_continue(false, span.line),
            // `throw e;` — evaluate the exception and emit `Op::Throw` (M-faults 2b).
            Stmt::Throw { value, span } => {
                self.expr(value)?;
                self.emit(Op::Throw, span.line);
                Ok(())
            }
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => self.compile_try(body, catches, finally_block.as_deref(), span.line),
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => self.compile_destructure(pat, init, else_block.as_deref(), span.line),
        }
    }

    /// Let-destructuring (Phase 1 slice 5) — NO new `Op`. Spill the init to a hidden `$destructure`
    /// local, then: a **struct** pattern reads each field (`GetField`) into a fresh binder
    /// (irrefutable, no branch); a **list** pattern reserves the binder slots, length-checks
    /// (`Len`/`Eq`/`JumpIfFalse`), assigns each element on the success path (the bounds-checked
    /// `Index`), and runs the diverging `else` on the fail path — structurally the same lowering as an
    /// `if`. Reserving the binder slots up front keeps the locals layout identical on both branches,
    /// so the continuation (where the binders are live) needs no save/restore.
    pub(in crate::compiler) fn compile_destructure(
        &mut self,
        pat: &crate::ast::DestructurePat,
        init: &Expr,
        else_block: Option<&[Stmt]>,
        line: u32,
    ) -> Result<(), String> {
        use crate::ast::DestructurePat;
        self.expr(init)?; // [.., initval] — spilled to the hidden temp below
        let init_cty = self.ctype(init).unwrap_or(CTy::Other);
        let d = self.add_local("$destructure", init_cty.clone());
        match pat {
            DestructurePat::Struct {
                type_name, fields, ..
            } => {
                for f in fields {
                    self.emit(Op::GetLocal(d), line); // [.., instance]
                    let idx = self.field_name_index(&f.field)?;
                    self.emit(Op::GetField(idx), line); // [.., fieldval]
                                                        // The binder's CTy is the class-field type so a destructured int is a first-class
                                                        // arithmetic operand on the VM (`Point { x } ⇒ x + 1`), the operand trap.
                    let cty = self
                        .class_field_ctys
                        .get(type_name)
                        .and_then(|m| m.get(&f.field))
                        .cloned()
                        .unwrap_or(CTy::Other);
                    self.add_local(&f.binding, cty); // fieldval becomes the binder
                }
                Ok(())
            }
            DestructurePat::List { binders, .. } => {
                let elem_cty = match &init_cty {
                    CTy::List(e) => (**e).clone(),
                    _ => CTy::Other,
                };
                let arity = binders.len();
                // Reserve every binder slot (null placeholder) BEFORE the branch so the locals layout
                // is identical on the success and `else` paths.
                for (name, _) in binders {
                    self.emit_const(Value::Null, line);
                    self.add_local(name, elem_cty.clone());
                }
                // len(init) == arity ?
                self.emit(Op::GetLocal(d), line);
                self.emit(Op::Len, line);
                self.emit_const(Value::Int(arity as i64), line);
                self.emit(Op::Eq, line);
                let else_jump = self.emit_jump(Op::JumpIfFalse(0), line); // jump to `else` if length differs
                                                                          // Success: assign each element into its reserved slot (the `Index` is in-range here).
                for (i, (name, _)) in binders.iter().enumerate() {
                    let slot = self
                        .resolve_local(name)
                        .ok_or_else(|| format!("unresolved destructure binder: {name}"))?;
                    self.emit(Op::GetLocal(d), line);
                    self.emit_const(Value::Int(i as i64), line);
                    self.emit(Op::Index, line);
                    self.emit(Op::SetLocal(slot), line);
                }
                let end_jump = self.emit_jump(Op::Jump(0), line);
                self.patch_jump(else_jump);
                // The `else` block (checker-guaranteed to diverge) runs with the binders reserved as
                // null — harmless, the checker forbids referencing them here.
                if let Some(eb) = else_block {
                    self.begin_scope();
                    for s in eb {
                        self.stmt(s)?;
                    }
                    self.end_scope(line);
                }
                self.patch_jump(end_jump);
                Ok(())
            }
            // DEC-288: tuple destructuring is irrefutable (checker-guaranteed arity), so no length-
            // check / `else`. Reserve each binder slot with its per-position `CTy` (from the explicit
            // annotation, else `Other` for the inferred form), then read the erased list's element into
            // it. The typed slot is the CTy-operand carrier (Invariant 7): `int a` specializes `a + 1`.
            DestructurePat::Tuple { binders, .. } => {
                for (ty_opt, name, _) in binders {
                    // `materialize_tuple_binds` filled every inferred binder's type from the checker's
                    // per-position resolution (Invariant 7), so `ty_opt` is `Some` for both forms; a
                    // stray `None` (materialize didn't run) safely falls back to `Other`.
                    let cty = ty_opt.as_ref().map_or(CTy::Other, resolve_cty);
                    self.emit_const(Value::Null, line);
                    self.add_local(name, cty);
                }
                for (i, (_, name, _)) in binders.iter().enumerate() {
                    let slot = self
                        .resolve_local(name)
                        .ok_or_else(|| format!("unresolved tuple binder: {name}"))?;
                    self.emit(Op::GetLocal(d), line);
                    self.emit_const(Value::Int(i as i64), line);
                    self.emit(Op::Index, line);
                    self.emit(Op::SetLocal(slot), line);
                }
                Ok(())
            }
        }
    }

    /// The positive primitive narrowings a condition installs in its THEN-block, as
    /// `(local-slot, CTy)`. Mirrors the checker's then-branch `is`/`instanceof`-primitive narrowing
    /// (DEC-184): the VM must give the narrowed local its concrete operand `CTy` so
    /// `if (x is int) { x + 1 }` specializes the arithmetic (the CTy-operand trap, Invariant 7). Only
    /// the named (then) direction is replicated — a class name resolves members via the field table
    /// (no local-CTy narrowing needed), and the union complement stays `CTy::Other` (the checker
    /// narrows no union complement either; general fix W2-12).
    pub(in crate::compiler) fn then_prim_narrowings(&self, cond: &Expr) -> Vec<(usize, CTy)> {
        let mut out = Vec::new();
        match cond {
            Expr::InstanceOf {
                value, type_name, ..
            } => {
                if let Expr::Ident(name, _) = &**value {
                    let cty = cty_of_type_name(type_name);
                    if matches!(cty, CTy::Int | CTy::Float | CTy::Str | CTy::Decimal) {
                        if let Some(slot) = self.resolve_local(name) {
                            out.push((slot, cty));
                        }
                    }
                }
            }
            // `a && b` narrows the conjunction on its true side — the same recursion the checker uses.
            Expr::Binary {
                op: crate::ast::BinaryOp::And,
                lhs,
                rhs,
                ..
            } => {
                out.extend(self.then_prim_narrowings(lhs));
                out.extend(self.then_prim_narrowings(rhs));
            }
            _ => {}
        }
        out
    }

    pub(in crate::compiler) fn compile_if(
        &mut self,
        cond: &Expr,
        bind: Option<&str>,
        then_block: &[Stmt],
        else_block: Option<&[Stmt]>,
        line: u32,
    ) -> Result<(), String> {
        if let Some(name) = bind {
            return self.compile_if_let(name, cond, then_block, else_block, line);
        }
        self.expr(cond)?;
        let else_jump = self.emit_jump(Op::JumpIfFalse(0), line); // pops cond
                                                                  // Slice 3 (DEC-184): narrow primitive locals to their tested operand `CTy` for the THEN-block
                                                                  // only, so arithmetic on the narrowed value specializes on the VM. Save the prior `CTy` and
                                                                  // restore it after the block — the else/fall-through must never see the narrowed type (the
                                                                  // checker narrows no union complement either, keeping the two in lockstep).
        let narrowings = self.then_prim_narrowings(cond);
        let saved: Vec<(usize, CTy)> = narrowings
            .iter()
            .map(|(slot, _)| (*slot, self.locals[*slot].ty.clone()))
            .collect();
        for (slot, cty) in &narrowings {
            self.locals[*slot].ty = cty.clone();
        }
        self.begin_scope();
        for s in then_block {
            self.stmt(s)?;
        }
        self.end_scope(line);
        for (slot, cty) in saved {
            self.locals[slot].ty = cty;
        }
        let end_jump = self.emit_jump(Op::Jump(0), line);
        self.patch_jump(else_jump);
        if let Some(eb) = else_block {
            self.begin_scope();
            for s in eb {
                self.stmt(s)?;
            }
            self.end_scope(line);
        }
        self.patch_jump(end_jump);
        Ok(())
    }

    /// `if (var name = cond)` (M3 S2.4). The scrutinee value lands in a scoped local that *is* the
    /// binding `name` (its `CTy` is the optional's inner type so `name + 1` still specializes); a
    /// non-consuming null-test (`GetLocal; Const null; Ne`) selects the branch. No new `Op` — the
    /// scrutinee slot persists across both arms and is popped by the enclosing `end_scope`. The
    /// checker forbids referencing `name` in the else block, so leaving it registered is harmless.
    pub(in crate::compiler) fn compile_if_let(
        &mut self,
        name: &str,
        cond: &Expr,
        then_block: &[Stmt],
        else_block: Option<&[Stmt]>,
        line: u32,
    ) -> Result<(), String> {
        self.begin_scope();
        self.expr(cond)?; // [opt] — this slot becomes the binding `name`
        let inner_cty = self.ctype(cond).unwrap_or(CTy::Other);
        let slot = self.add_local(name, inner_cty);
        self.emit(Op::GetLocal(slot), line); // [opt, opt]
        self.emit_const(Value::Null, line); // [opt, opt, null]
        self.emit(Op::Ne, line); // [opt, opt != null]
        let else_jump = self.emit_jump(Op::JumpIfFalse(0), line); // pops bool → [opt]; jump if null
        self.begin_scope();
        for s in then_block {
            self.stmt(s)?;
        }
        self.end_scope(line);
        let end_jump = self.emit_jump(Op::Jump(0), line);
        self.patch_jump(else_jump);
        if let Some(eb) = else_block {
            self.begin_scope();
            for s in eb {
                self.stmt(s)?;
            }
            self.end_scope(line);
        }
        self.patch_jump(end_jump);
        self.end_scope(line); // pops the scrutinee slot (`name`) — both arms converge with [opt] live
        Ok(())
    }
}
