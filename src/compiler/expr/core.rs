//! Expression compilation — dispatch, parent calls, string interpolation.

use super::*;

impl Compiler<'_> {
    /// Resolve a `parent`/super call to the baked target function index (M-RT super/parent). Uses the
    /// `methods` table (which already encodes dispatch origins) keyed by the named ancestor (qualified
    /// form) or, for the immediate form, the lexical class's direct parents — the unique resolved index
    /// (a diamond's shared base collapses to one; ≥2 distinct is checker-rejected as ambiguous, so a
    /// valid program always yields exactly one). Matches the interpreter's `resolve_parent_method`.
    pub(in crate::compiler) fn resolve_parent_target(
        &self,
        ancestor: Option<&str>,
        method: &str,
    ) -> Result<usize, String> {
        let cur = self
            .cur_class
            .as_deref()
            .ok_or("`parent` used outside a method")?;
        let lookup = |class: &str| {
            self.methods
                .get(&(class.to_string(), method.to_string()))
                .copied()
        };
        let func = match ancestor {
            Some(a) => lookup(a),
            None => {
                let parents = self.parent_parents.and_then(|p| p.get(cur));
                let mut found: Vec<usize> = Vec::new();
                for p in parents.map(Vec::as_slice).unwrap_or(&[]) {
                    if let Some(idx) = lookup(p) {
                        if !found.contains(&idx) {
                            found.push(idx);
                        }
                    }
                }
                found.first().copied()
            }
        };
        func.ok_or_else(|| {
            format!(
                "internal: unresolved parent method `{method}` (checker should have caught this)"
            )
        })
    }

    /// The operand `CTy` of a `parent`/super call's result — the resolved target method's return type
    /// from `method_rets` (M-RT super/parent). `CTy::Other` when not resolvable (a non-operand context;
    /// the checker has already validated the call). Lets `parent.m(…) + 1` specialize on the VM.
    pub(in crate::compiler) fn parent_ret_cty(&self, ancestor: Option<&str>, method: &str) -> CTy {
        let Some(cur) = self.cur_class.as_deref() else {
            return CTy::Other;
        };
        let lookup = |class: &str| {
            self.method_rets
                .get(&(class.to_string(), method.to_string()))
                .cloned()
        };
        let cty = match ancestor {
            Some(a) => lookup(a),
            None => self
                .parent_parents
                .and_then(|p| p.get(cur))
                .and_then(|parents| parents.iter().find_map(|p| lookup(p))),
        };
        cty.unwrap_or(CTy::Other)
    }

    pub(in crate::compiler) fn expr(&mut self, e: &Expr) -> Result<(), String> {
        match e {
            Expr::Int(n, sp) => self.emit_const(Value::Int(*n), sp.line),
            Expr::Float(x, sp) => self.emit_const(Value::Float(*x), sp.line),
            // A `decimal` literal rides the constant pool like float/bytes — no new Op (M-NUM S1).
            Expr::Decimal {
                unscaled,
                scale,
                span,
            } => self.emit_const(
                Value::Decimal {
                    unscaled: *unscaled,
                    scale: *scale,
                },
                span.line,
            ),
            Expr::Bool(b, sp) => self.emit_const(Value::Bool(*b), sp.line),
            Expr::Str(parts, sp) => self.compile_str(parts, sp.line)?,
            Expr::Bytes(b, sp) => {
                self.emit_const(Value::Bytes(std::rc::Rc::new(b.clone())), sp.line)
            }
            Expr::Ident(name, sp) => {
                // Resolution order mirrors the interpreter's `eval_ident`: a `match`-arm binding
                // (re-extracted from `$match` along its payload path; P4-7) shadows a local/param,
                // which shadows a bare field of `this` (a method/ctor body, lowered to
                // `this.field`). An unresolved name is a compiler bug (the checker ran first).
                if let Some((slot, path)) = self.resolve_binding(name) {
                    self.emit(Op::GetLocal(slot), sp.line);
                    self.emit_path(&path, sp.line);
                } else if let Some(slot) = self.resolve_local(name) {
                    self.emit(Op::GetLocal(slot), sp.line);
                } else if let (Some(this), true) =
                    (self.this_slot, self.field_tags.contains_key(name))
                {
                    let idx = self.field_name_index(name)?;
                    self.emit(Op::GetLocal(this), sp.line);
                    self.emit(Op::GetField(idx), sp.line);
                } else if let Some(idx) = self.fns.get(name).map(|m| m.index) {
                    // Bare named-function reference in value position → a zero-capture closure.
                    // Read the index from the immutable `self.fns` borrow into a local before
                    // calling `self.emit` (which needs `&mut self`).
                    self.emit(Op::MakeClosure(idx), sp.line);
                } else {
                    return Err(format!("undefined variable `{name}`"));
                }
            }
            Expr::List(items, sp) => {
                for it in items {
                    self.expr(it)?;
                }
                self.emit(Op::MakeList(items.len()), sp.line);
            }
            // `new List<T>()` / `new Map<K,V>()` (DEC-214) — emit an empty collection (no elements
            // pushed); reuses the existing MakeList/MakeMap ops, so no new `Op`.
            Expr::NewColl { kind, span, .. } => match kind {
                crate::ast::CollKind::List => self.emit(Op::MakeList(0), span.line),
                crate::ast::CollKind::Map => self.emit(Op::MakeMap(0), span.line),
            },
            Expr::Map(pairs, sp) => {
                // Push each key then its value (source order); `Op::MakeMap(n)` pops the 2n values and
                // builds the insertion-ordered map via the shared `build_map` kernel (M-RT S3).
                for (k, v) in pairs {
                    self.expr(k)?;
                    self.expr(v)?;
                }
                self.emit(Op::MakeMap(pairs.len()), sp.line);
            }
            Expr::Unary { op, expr, span } => {
                self.expr(expr)?;
                match op {
                    UnaryOp::Neg => self.emit(Op::Neg, span.line),
                    UnaryOp::Not => self.emit(Op::Not, span.line),
                    UnaryOp::BitNot => self.emit(Op::BitNot, span.line),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => self.compile_binary(*op, lhs, rhs, span.line)?,
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => {
                // Push the value, then a single `IsInstance` op carrying the class name inline pops
                // it and pushes a `Bool` (M-RT S1). The class name lives in the op (like `Fault`), so
                // no name-pool entry is needed and the runtime predicate matches the interpreter.
                self.expr(value)?;
                self.emit(Op::IsInstance(type_name.clone()), span.line);
            }
            Expr::Cast {
                value,
                type_name,
                span,
            } => {
                // M4 as-matrix: a primitive `as` CONVERSION was rewritten to a native call before this
                // backend; the only primitive `Cast` reaching here is the **identity** (`T as T`) —
                // compile the value unchanged.
                if matches!(
                    type_name.as_str(),
                    "int" | "float" | "string" | "bool" | "decimal"
                ) {
                    self.expr(value)?;
                    return Ok(());
                }
                // Checked downcast (M4 casting axis 2): keep `value` if it `IsInstance` of `type_name`,
                // else replace it with `null` — result type `type_name?`. `value` is evaluated ONCE:
                // stash it in a scratch slot (the `??`/`$match` trick — its frame-relative top, NOT
                // `add_local`, since live transients may sit below), duplicate it to feed `IsInstance`,
                // then branch. Reuses `Op::IsInstance` — no new `Op` (decision S2-OPS).
                self.expr(value)?; // [v] — v lands in the scratch slot
                let slot = self.height - 1;
                self.emit(Op::GetLocal(slot), span.line); // [v, v]
                self.emit(Op::IsInstance(type_name.clone()), span.line); // [v, bool]
                let to_null = self.emit_jump(Op::JumpIfFalse(0), span.line); // [v]; jump if !instanceof
                let h_merge = self.height;
                // true path: `v` is already the result; jump past the null branch.
                let end_j = self.emit_jump(Op::Jump(0), span.line);
                self.patch_jump(to_null); // null path arrives with [v]
                self.height = h_merge;
                self.emit_const(Value::Null, span.line); // [v, null]
                self.emit(Op::SetLocal(slot), span.line); // [null] — overwrite the slot with null
                self.patch_jump(end_j); // both paths leave one value at `slot`
                self.height = h_merge;
            }
            Expr::Call { callee, args, span } => self.compile_call(callee, args, span.line)?,
            // `spawn <call>` (M6 W4): synchronous-degenerate — compile the call (it runs inline,
            // leaving its result on top), then `Op::Spawn` registers a finished task. Compiling the
            // call inline (rather than wrapping it in a thunk lambda) keeps the fault stack trace
            // identical to the interpreter's — a synthetic thunk frame would show as `<lambda@N>` only
            // on the VM (closures are real frames there, invisible in the tree-walker), breaking the
            // run≡runvm trace. The cooperative cutover will introduce deferral with trace consistency.
            // `spawn <call>` (M6 W4 / S4.3). A single-overload **free-function** call lowers to
            // args-push + `Op::SpawnCall(func_idx, argc)` — the call body is NOT run before the op, so
            // the cooperative driver can defer it as a task rooted at the function's own frame (no
            // synthetic lambda → fault traces match the interpreter). Any other operand (method,
            // overloaded, closure, variant) keeps the eager `<call>; Op::Spawn` form; the cooperative
            // driver rejects those on both backends (a free-function-only restriction, matching the
            // interpreter), so `run≡runvm`.
            Expr::Spawn { call, span } => {
                let deferred = if let Expr::Call { callee, args, .. } = &**call {
                    if let Expr::Ident(name, _) = &**callee {
                        self.fns
                            .get(name)
                            .filter(|m| m.overload.is_none())
                            .map(|m| (m.index, args))
                    } else {
                        None
                    }
                } else {
                    None
                };
                match deferred {
                    Some((index, args)) => {
                        for a in args {
                            self.expr(a)?;
                        }
                        self.emit(Op::SpawnCall(index, args.len()), span.line);
                    }
                    None => {
                        self.expr(call)?;
                        self.emit(Op::Spawn, span.line);
                    }
                }
            }
            Expr::Null(sp) => self.emit_const(Value::Null, sp.line),
            Expr::This(sp) => match self.this_slot {
                // `this` is the receiver local: slot 0 in a method, the instance slot in a ctor.
                Some(slot) => self.emit(Op::GetLocal(slot), sp.line),
                // Checker-unreachable (`this` outside a method/ctor); mirrors the interpreter.
                None => return Err("`this` used outside a method".into()),
            },
            Expr::Member {
                object,
                name,
                safe,
                sep: _,
                span,
            } => {
                // Field read: evaluate the object, then look its field up at runtime by name
                // (decision P4-5). Runtime lookup keeps the compiler untyped; the fault on a miss
                // is byte-identical to the interpreter's. `?.` (safe) short-circuits a null receiver.
                let line = span.line;
                // A `const` class constant (Feature A): inline the literal `Value` via `Op::Const` —
                // no runtime store. Checked before the static path (a const is never a static slot).
                if let Some(v) = self.const_value(object, name) {
                    self.emit_const(v, line);
                } else if let Some(idx) = self.static_slot(object, name) {
                    // Static read `ClassName.field` (M-mut.7): the head is a class name (not a local),
                    // so this is program-level state, not an instance field.
                    self.emit(Op::GetStatic(idx), line);
                } else if let Some(getm) = self.hook_get_method(object, name) {
                    // Property hook read `o.name` (M-mut.7b) → call the synthetic `<name>$get`
                    // 0-arg method, which leaves the computed value on the stack. `?.` short-circuits
                    // a null receiver before dispatch (the interpreter does the same).
                    if *safe {
                        self.compile_safe_access(object, line, |c| {
                            let idx = c.field_name_index(&getm)?;
                            c.emit(Op::CallMethod(idx, 0), line);
                            Ok(())
                        })?;
                    } else {
                        self.expr(object)?;
                        let idx = self.field_name_index(&getm)?;
                        self.emit(Op::CallMethod(idx, 0), line);
                    }
                } else if *safe {
                    self.compile_safe_access(object, line, |c| {
                        let idx = c.field_name_index(name)?;
                        c.emit(Op::GetField(idx), line);
                        Ok(())
                    })?;
                } else {
                    self.expr(object)?;
                    let idx = self.field_name_index(name)?;
                    self.emit(Op::GetField(idx), line);
                }
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                // Push the list, then the index; `Op::Index` pops index-then-list and pushes the
                // bounds-checked element clone (the same op `compile_for` already uses).
                self.expr(object)?;
                self.expr(index)?;
                self.emit(Op::Index, span.line);
            }
            Expr::Force { inner, span } => self.compile_force(inner, span.line)?,
            Expr::Propagate { inner, span } => self.compile_propagate(inner, span.line)?,
            Expr::CloneWith {
                object,
                fields,
                span,
            } => self.compile_clone_with(object, fields, span.line)?,
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => self.compile_match(scrutinee, arms, span.line)?,
            Expr::Range {
                start,
                end,
                inclusive,
                span,
            } => {
                // Push start, then end; `MakeRange` pops end-then-start and materializes the list.
                self.expr(start)?;
                self.expr(end)?;
                self.emit(Op::MakeRange(*inclusive), span.line);
            }
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => {
                // Lower like `&&`/`||`: branch on the cond, each arm leaves exactly one value, and
                // the merge height is reset so both arms agree on the single result slot.
                self.expr(cond)?;
                let else_j = self.emit_jump(Op::JumpIfFalse(0), span.line); // pops cond
                let h_merge = self.height; // both arms converge to one value above this
                self.expr(then_expr)?;
                let end_j = self.emit_jump(Op::Jump(0), span.line);
                self.patch_jump(else_j);
                self.height = h_merge; // else path starts at the merge height
                self.expr(else_expr)?;
                self.patch_jump(end_j);
            }
            Expr::Lambda {
                params,
                body,
                ret,
                span,
                ..
            } => self.compile_lambda(params, body, ret.as_ref(), span.line)?,
            // `html"…"` literals are erased to `html.concat([…])` kernel calls by
            // `checker::resolve_html` before compilation; the compiler never sees one.
            Expr::Html(..) => unreachable!("html literal not resolved before compilation"),
            Expr::TaggedTemplate { .. } => {
                unreachable!("non-html tagged template rejected (E-UNKNOWN-TAG) before compilation")
            }
            Expr::Inject { .. } => unreachable!("inject() not expanded before compilation"),
            Expr::OverloadSelect { .. } => {
                unreachable!("overload selector resolved + rewritten before compilation (Slice C1)")
            }
            // `parent.m(args)` / `parent(A).m(args)` — super/parent dispatch (M-RT super/parent).
            Expr::ParentCall {
                ancestor,
                method,
                args,
                span,
            } => {
                let func = self.resolve_parent_target(ancestor.as_deref(), method)?;
                // Push the current receiver (`this`) as the call's slot 0, then the args.
                let this_slot = self
                    .this_slot
                    .ok_or_else(|| "`parent` used outside a method".to_string())?;
                self.emit(Op::GetLocal(this_slot), span.line);
                for a in args {
                    self.expr(a)?;
                }
                self.emit(Op::CallParent(func, args.len()), span.line);
            }
            Expr::New(..) => {
                unreachable!("Expr::New is unwrapped before compilation (checker::unwrap_new)")
            }
        }
        Ok(())
    }

    pub(in crate::compiler) fn compile_str(
        &mut self,
        parts: &[StrPart],
        line: u32,
    ) -> Result<(), String> {
        // A single literal segment (or empty) is just a string constant.
        if let [StrPart::Literal(s)] = parts {
            self.emit_const(Value::Str(crate::phstr::PhStr::literal(s)), line);
            return Ok(());
        }
        if parts.is_empty() {
            self.emit_const(Value::Str(crate::phstr::PhStr::empty()), line);
            return Ok(());
        }
        for part in parts {
            match part {
                StrPart::Literal(s) => {
                    self.emit_const(Value::Str(crate::phstr::PhStr::literal(s)), line)
                }
                StrPart::Expr(e) => self.expr(e)?,
            }
        }
        self.emit(Op::Concat(parts.len()), line);
        Ok(())
    }
}
