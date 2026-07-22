//! Expression compilation — calls, safe access (`?.`/`!`/`?`), intrinsics, clone-with,
//! lambdas.

use super::*;

impl Compiler<'_> {
    pub(in crate::compiler) fn compile_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        line: u32,
    ) -> Result<(), String> {
        if let Expr::Ident(name, _) = callee {
            // Fault intrinsics (M-faults 2a) lower to `Op::Fault` — no user-function dispatch.
            if self.compile_intrinsic(name, args, line)? {
                return Ok(());
            }
            if let Some(meta) = self.fns.get(name) {
                let dispatch = meta.overload;
                let index = meta.index;
                for a in args {
                    self.expr(a)?;
                }
                // An overloaded name dispatches on the runtime argument types (M-RT); a single
                // overload is a direct call as before.
                match dispatch {
                    Some(set_id) => self.emit(Op::CallOverload(set_id, args.len()), line),
                    None => self.emit(Op::Call(index), line),
                }
                return Ok(());
            }
            // A local variable with a function type (lambda or named-fn ref): push the closure
            // first (by its local slot), then the args, then dispatch with `CallValue`.
            if let Some(slot) = self.resolve_local(name) {
                if matches!(self.locals[slot].ty, CTy::Fn { .. }) {
                    self.emit(Op::GetLocal(slot), line); // push the closure value
                    for a in args {
                        self.expr(a)?;
                    }
                    self.emit(Op::CallValue(args.len()), line);
                    return Ok(());
                }
            }
            // A match-arm binding with a function type (lambda passed as an argument).
            if let Some((slot, path)) = self.resolve_binding(name) {
                if matches!(
                    self.match_bindings
                        .iter()
                        .rev()
                        .find(|b| b.name == *name)
                        .map(|b| &b.ty),
                    Some(CTy::Fn { .. })
                ) {
                    // Re-extract the closure from its binding path.
                    self.emit(Op::GetLocal(slot), line);
                    self.emit_path(&path, line);
                    for a in args {
                        self.expr(a)?;
                    }
                    self.emit(Op::CallValue(args.len()), line);
                    return Ok(());
                }
            }
            // An enum variant constructor: `Variant(args)` (or a bare `Variant`, args empty).
            // The checker has already verified arity, so push the payload and tag it (P4-3).
            // Bare-owner fallback only — `qualify_variants` rewrites every construction to the
            // qualified `Member` form handled below (DEC-329.3).
            if let Some(meta) = self.variants.get(None, name) {
                let idx = meta.index;
                for a in args {
                    self.expr(a)?;
                }
                self.emit(Op::MakeEnum(idx), line);
                return Ok(());
            }
            // A class constructor: `ClassName(args)` calls the synthetic `<Class>::new`, which
            // promotes its params into fields and returns the instance (decision P4-4).
            if let Some(&ctor_idx) = self.classes.get(name) {
                for a in args {
                    self.expr(a)?;
                }
                self.emit(Op::Call(ctor_idx), line);
                return Ok(());
            }
            // Unreachable for checker-validated programs; mirrors `interpreter::eval_call`'s wording.
            return Err(format!("`{name}` is not a function, variant, or class"));
        }
        // Method call `object.name(args)`: evaluate the receiver, then the args, and dispatch by
        // name at runtime off the receiver's class (decision P4-6).
        if let Expr::Member {
            object, name, safe, ..
        } = callee
        {
            // Namespaced native call: `console.println(x)` — a member call whose head is an imported
            // module qualifier rather than a value (M3 Wave 1). Locals-first: only an identifier that
            // is *not* a bound local/match-binding can be a qualifier, and the checker has already
            // enforced that it was imported and the native exists. Resolution is import-map-first
            // with a Native-excluded leaf fallback (`native::index_of_qualified`): the DEC-277
            // preludes import their raw natives under an alias, and a prelude class (`Uri`,
            // `Database`) must never leaf-capture its same-leaf `Core.Native.*` module. Lowers to
            // `Op::CallNative`, which pushes the native's result — no separate `Const(Unit)` (the
            // old `Print` path's pair).
            if !*safe {
                if let Expr::Ident(q, _) = &**object {
                    if self.resolve_local(q).is_none() && self.resolve_binding(q).is_none() {
                        if let Some(idx) = crate::native::index_of_qualified(self.imports, q, name)
                        {
                            for a in args {
                                self.expr(a)?;
                            }
                            self.emit(Op::CallNative(idx, args.len()), line);
                            return Ok(());
                        }
                    }
                }
            }
            // Built-in concurrency static `Channel.create()` (M6 W4): `Channel` is a reserved type
            // name, not a value/class, so intercept it before the class-static and instance paths
            // (both of which would treat `Channel` as a binding). Args are empty (checker-enforced).
            if !*safe {
                if let Expr::Ident(h, _) = &**object {
                    if h == "Channel"
                        && name == "create"
                        && self.resolve_local(h).is_none()
                        && self.resolve_binding(h).is_none()
                    {
                        self.emit(Op::ChannelNew, line);
                        return Ok(());
                    }
                }
            }
            // Built-in concurrency instance methods (M6 W4): `ch.send(v)` / `ch.receive()` / `t.join()`,
            // dispatched off the receiver's compile-time type (`Channel`/`Task` are reserved class
            // names, never user classes). Lowers to the dedicated op — there is no method-table entry
            // to find, so this MUST precede the generic `CallMethod` path.
            if !*safe {
                if let Ok(CTy::Class(cls)) = self.ctype(object) {
                    if cls == "Channel" || cls == "Task" {
                        self.expr(object)?;
                        for a in args {
                            self.expr(a)?;
                        }
                        match (cls.as_str(), name.as_str()) {
                            ("Channel", "send") => self.emit(Op::ChannelSend, line),
                            ("Channel", "receive") => self.emit(Op::ChannelRecv, line),
                            ("Task", "join") => self.emit(Op::Join, line),
                            _ => {
                                return Err(format!("`{cls}` has no built-in method `{name}`"));
                            }
                        }
                        return Ok(());
                    }
                }
            }
            // DEC-302 enum static methods (compile-time known enum). `cases()` inlines to one
            // `MakeEnum` per variant (payload-less, declaration order) + `MakeList`. `from(x)` /
            // `tryFrom(x)` push the arg then `Op::EnumFrom` over the enum's contiguous descriptor
            // range. Resolved before the class-static path (an enum is not a class). Not `new`-wrapped,
            // so the `Member` callee is intact (variant construction is bare after `unwrap_new`).
            if !*safe {
                if let Expr::Ident(en, _) = &**object {
                    if self.resolve_local(en).is_none()
                        && self.resolve_binding(en).is_none()
                        && matches!(name.as_str(), "cases" | "from" | "tryFrom")
                    {
                        if let Some((start, count)) = self.enum_desc_range(en) {
                            if name == "cases" {
                                for i in start..start + count {
                                    self.emit(Op::MakeEnum(i), line);
                                }
                                self.emit(Op::MakeList(count), line);
                            } else {
                                for a in args {
                                    self.expr(a)?;
                                }
                                self.emit(Op::EnumFrom(start, count, name == "tryFrom"), line);
                            }
                            return Ok(());
                        }
                    }
                }
            }
            // DEC-329.3: qualified enum-variant construction `Enum.Variant(args)` — the canonical
            // form `qualify_variants` produces for EVERY construction, so the (enum, variant) key
            // picks the RIGHT descriptor when two enums share a variant name (the bare map above
            // is last-declaration-wins). After the enum statics (`cases`/`from`/`tryFrom` are
            // checker-reserved, never variant names), before the class-static path.
            if !*safe {
                if let Expr::Ident(en, _) = &**object {
                    if self.resolve_local(en).is_none() && self.resolve_binding(en).is_none() {
                        if let Some(meta) = self.variants.get(Some(en), name) {
                            let idx = meta.index;
                            for a in args {
                                self.expr(a)?;
                            }
                            self.emit(Op::MakeEnum(idx), line);
                            return Ok(());
                        }
                    }
                }
            }
            // Static method call `ClassName.method(args)` (slice B0): the class is known at compile
            // time. Push a dummy receiver (slot 0 of the compiled method is `$this`, which a static
            // method never reads) then the args. A *non-overloaded* static lowers to a direct
            // `Op::Call` to the `(class, method)` function index; an *overloaded* static (Statics-B)
            // lowers to `Op::CallOverload(set_id, argc)` — the same runtime selector instance overloads
            // use, with the dummy receiver sitting below the `argc` args (so selection sees only the
            // args, and the selected body's arity pops the dummy + args). Resolved after the native
            // path (an explicit import wins a name collision), before instance dispatch.
            if !*safe {
                if let Expr::Ident(cls, _) = &**object {
                    if self.resolve_local(cls).is_none()
                        && self.resolve_binding(cls).is_none()
                        && self.classes.contains_key(cls)
                    {
                        let key = (cls.clone(), name.clone());
                        if let Some(&set_id) = self.method_overloads.get(&key) {
                            self.emit_const(Value::Unit, line); // dummy receiver in slot 0
                            for a in args {
                                self.expr(a)?;
                            }
                            self.emit(Op::CallStaticOverload(set_id, args.len()), line);
                            return Ok(());
                        }
                        if let Some(&fn_idx) = self.methods.get(&key) {
                            self.emit_const(Value::Unit, line); // dummy receiver in slot 0
                            for a in args {
                                self.expr(a)?;
                            }
                            self.emit(Op::Call(fn_idx), line);
                            return Ok(());
                        }
                    }
                }
            }
            // `o?.m(args)`: a null receiver short-circuits — the args are NOT evaluated and the
            // method is NOT dispatched (the null-skip lowering jumps over the whole `access`).
            if *safe {
                return self.compile_safe_access(object, line, |c| {
                    for a in args {
                        c.expr(a)?;
                    }
                    let idx = c.field_name_index(name)?;
                    c.emit(Op::CallMethod(idx, args.len()), line);
                    Ok(())
                });
            }
            self.expr(object)?;
            for a in args {
                self.expr(a)?;
            }
            let idx = self.field_name_index(name)?;
            self.emit(Op::CallMethod(idx, args.len()), line);
            return Ok(());
        }
        // Inline lambda call: `(function(int x) => x+1)(3)` or (after pipe lowering) `3 |> function(int v) =>
        // v+10`. Compile the lambda expression to push a closure, then push args, then dispatch.
        if let Expr::Lambda {
            params,
            body,
            ret,
            span,
            ..
        } = callee
        {
            self.compile_lambda(params, body, ret.as_ref(), span.line)?;
            for a in args {
                self.expr(a)?;
            }
            self.emit(Op::CallValue(args.len()), line);
            return Ok(());
        }
        // A general expression that evaluates to a function value — `adder()(x)` (call a returned
        // closure), `fns[i](x)`, `(c ? f : g)(x)`. The checker has verified the callee is
        // function-typed, so compile it (pushes the closure), then the args, then dispatch via
        // `CallValue` — the same path a lambda local takes. Mirrors `interpreter::eval_call`.
        self.expr(callee)?;
        for a in args {
            self.expr(a)?;
        }
        self.emit(Op::CallValue(args.len()), line);
        Ok(())
    }

    /// Lower a `?.` access (field read or method call): evaluate `object`; if it is `null`, the
    /// whole access short-circuits to `null`; otherwise run `access`, which transforms the receiver
    /// on top of the stack into the member result. No new `Op` (decision S2-OPS): a scratch local
    /// peeks the receiver for the null test (the `$coalesce` trick from `??`), then a
    /// `JumpIfFalse`/`Jump` pair selects the path. Both paths leave exactly one value at the
    /// receiver's slot, so the static height is the receiver's height throughout.
    pub(in crate::compiler) fn compile_safe_access(
        &mut self,
        object: &Expr,
        line: u32,
        access: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        self.expr(object)?; // [.., recv]
                            // `recv`'s frame-relative slot (top of stack), NOT `locals.len()`: live transients (earlier
                            // interpolation segments, an enclosing `??`'s lhs, …) may sit below it. Mirrors
                            // `compile_match`'s `m_slot = self.height - 1`; addressed numerically, no `Local` entry.
        let slot = self.height - 1;
        self.emit(Op::GetLocal(slot), line); // [.., recv, recv]
        self.emit_const(Value::Null, line); // [.., recv, recv, null]
        self.emit(Op::Eq, line); // [.., recv, bool]
        let do_access = self.emit_jump(Op::JumpIfFalse(0), line); // [.., recv]; recv != null → access
        let to_end = self.emit_jump(Op::Jump(0), line); // recv == null → keep recv (= null), skip access
        self.patch_jump(do_access);
        let h = self.height;
        access(self)?; // [.., recv] -> [.., member]
        self.patch_jump(to_end);
        self.height = h; // both paths converge here with one value at the receiver's slot
        Ok(())
    }

    /// `inner!` checked force-unwrap (M3 S2.5). Evaluate the inner; a non-consuming null-test keeps
    /// the value when present, else raises `Op::Fault(ForceUnwrapNull)` — byte-identical to the
    /// interpreter's `"force-unwrap of null"` fault. No new `Op` (the fault op is the generalized
    /// `MatchFail`). `o! + 1` still specializes because `ctype(Force)` resolves the result's type.
    pub(in crate::compiler) fn compile_force(
        &mut self,
        inner: &Expr,
        line: u32,
    ) -> Result<(), String> {
        self.expr(inner)?; // [opt] — stays as the result when non-null
                           // `opt`'s frame-relative slot (top of stack), NOT `locals.len()`: transients may sit below
                           // it (e.g. `"{a!} {b!}"`). Mirrors `compile_match`. `ctype(Force)` handles operand typing of
                           // the *result*, so the scratch needs no `CTy`. Addressed numerically, no `Local` entry.
        let slot = self.height - 1;
        self.emit(Op::GetLocal(slot), line); // [opt, opt]
        self.emit_const(Value::Null, line); // [opt, opt, null]
        self.emit(Op::Eq, line); // [opt, opt == null]
        let ok = self.emit_jump(Op::JumpIfFalse(0), line); // [opt]; non-null → keep, skip the fault
        self.emit(Op::Fault(FaultMsg::ForceUnwrapNull), line); // null → clean fault (terminal)
        self.patch_jump(ok);
        Ok(())
    }

    /// `expr?` — Result-error propagation (M-faults 2a). Evaluate the operand; if it is `Failure(_)`,
    /// `Op::Return` the whole `Failure` value (`do_return` truncates to the frame base, so this mid-expression
    /// early-return is clean even nested); otherwise unwrap the `Success` payload. No new `Op` — reuses
    /// `MatchTag`/`GetEnumField`/`Return`. The checker restricts `?` to a let-initializer, so the result
    /// (the `Success` payload) is what the binding receives.
    pub(in crate::compiler) fn compile_propagate(
        &mut self,
        inner: &Expr,
        line: u32,
    ) -> Result<(), String> {
        self.expr(inner)?; // [.., r]
        let slot = self.height - 1; // r's frame-relative slot (transients may sit below it)
        let err_idx = self
            .variants
            .get(None, "Failure")
            .ok_or_else(|| {
                "`?` requires a Result-shaped enum (no `Failure` variant in scope)".to_string()
            })?
            .index;
        self.emit(Op::GetLocal(slot), line); // [.., r, r]
                                             // DEC-329.3: `?` is DUCK-TYPED (the checker accepts any Result-shaped enum), so the test
                                             // is by variant NAME (`MatchTagName`) — never the (ty, variant) `MatchTag`, which would
                                             // reject a user Result-shaped enum's `Failure` against the bare-owner descriptor picked
                                             // above. Byte-identical to the interpreter's name-only `Expr::Propagate` arm.
        self.emit(Op::MatchTagName(err_idx), line); // [.., r, isErr]
        let not_err = self.emit_jump(Op::JumpIfFalse(0), line); // pops isErr -> [.., r]
        self.emit(Op::Return, line); // Err: return r (do_return truncates the frame stack)
        self.patch_jump(not_err); // Ok path: [.., r]
        self.height = slot + 1; // reassert post-branch height (the terminal Return desynced the tracker)
        self.emit(Op::GetEnumField(0), line); // [.., ok_payload]
        Ok(())
    }

    /// Lower a fault intrinsic (`panic`/`todo`/`unreachable`/`assert`) to `Op::Fault` (M-faults 2a).
    /// Returns `true` if `name` was an intrinsic (and was compiled), `false` otherwise. Messages are
    /// compile-time literals (the checker enforces this), so they bake straight into the `FaultMsg` —
    /// no new `Op`. `panic`/`todo`/`unreachable` are `never`-typed: the trailing `self.height += 1`
    /// keeps the expression's "produces one value" contract for the (dead) code after the terminal
    /// `Op::Fault`. `assert` produces `unit` on the true path and faults on the false path.
    pub(in crate::compiler) fn compile_intrinsic(
        &mut self,
        name: &str,
        args: &[Expr],
        line: u32,
    ) -> Result<bool, String> {
        let base = self.height;
        match name {
            "panic" => {
                let msg = str_literal(args.first());
                self.emit(Op::Fault(FaultMsg::Panic(msg)), line);
                self.height = base + 1;
            }
            "todo" => {
                self.emit(Op::Fault(FaultMsg::Todo), line);
                self.height = base + 1;
            }
            "unreachable" => {
                self.emit(Op::Fault(FaultMsg::Unreachable), line);
                self.height = base + 1;
            }
            "assert" => {
                let msg = args
                    .get(1)
                    .map_or_else(String::new, |m| str_literal(Some(m)));
                self.expr(&args[0])?; // [.., cond]
                let to_fault = self.emit_jump(Op::JumpIfFalse(0), line); // false → fault (pops cond)
                self.emit_const(Value::Unit, line); // true: [.., unit]
                let to_end = self.emit_jump(Op::Jump(0), line);
                self.patch_jump(to_fault);
                self.emit(Op::Fault(FaultMsg::Assert(msg)), line);
                self.patch_jump(to_end);
                self.height = base + 1; // unit result
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// `obj with { f = e, … }` (M-mut.4a). Reconstruct a fresh instance: evaluate `obj` into a
    /// scratch slot, then push each descriptor field in order — the override expr if named, else a
    /// `GetField` re-read of the source — and `MakeInstance` (which runs **no** constructor body,
    /// Fork 2 = B). Collapse the new instance over the scratch slot so the expression leaves one
    /// value. No new `Op`. The checker proved `obj` is a concrete class and the names are its fields.
    pub(in crate::compiler) fn compile_clone_with(
        &mut self,
        object: &Expr,
        fields: &[(String, Expr)],
        line: u32,
    ) -> Result<(), String> {
        let class = match self.ctype(object)? {
            CTy::Class(name) => name,
            _ => return Err("`with` requires a class instance".into()),
        };
        let desc_idx = self
            .class_descs
            .iter()
            .position(|d| d.class.as_ref() == class)
            .ok_or_else(|| format!("unknown class `{class}` in `with`"))?;
        let field_names = self.class_descs[desc_idx].fields.clone();
        self.expr(object)?; // [.., src]
        let src_slot = self.height - 1; // numeric scratch (transients may sit below), like `compile_match`
        for fname in &field_names {
            if let Some((_, e)) = fields.iter().find(|(n, _)| n == fname) {
                self.expr(e)?; // [.., override]
            } else {
                self.emit(Op::GetLocal(src_slot), line); // [.., src]
                let idx = self.field_name_index(fname)?;
                self.emit(Op::GetField(idx), line); // [.., src.field]
            }
        }
        self.emit(Op::MakeInstance(desc_idx), line); // pops the fields → [.., src, newInstance]
        self.emit(Op::SetLocal(src_slot), line); // collapse newInstance over the scratch → [.., newInstance]
        Ok(())
    }
}
