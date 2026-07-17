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
            if let Some(meta) = self.variants.get(name) {
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
            .get("Failure")
            .ok_or_else(|| {
                "`?` requires a Result-shaped enum (no `Failure` variant in scope)".to_string()
            })?
            .index;
        self.emit(Op::GetLocal(slot), line); // [.., r, r]
        self.emit(Op::MatchTag(err_idx), line); // [.., r, isErr]
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

    /// Compile a `function(params) => body` expression-body lambda (M3 S3 Task 4).
    ///
    /// Layout:
    ///   - Compute the lambda's free variables (sorted, deterministic — invariant #8).
    ///   - Filter out names that resolve to top-level functions (not captures).
    ///   - For each capture: emit `GetLocal(slot)` to push it onto the stack.
    ///   - Build a sub-`Function` with layout `[captures.., params..]`.
    ///     * The sub-compiler's locals start with the captures (in free-var order),
    ///       then the declared params — matching the frame layout `CallValue` sets up.
    ///   - Append the sub-`Function` to `self.extra_functions` and record its `n_captures`.
    ///   - Emit `Op::MakeClosure(fn_idx)` which pops the captures and pushes a `Value::Closure`.
    pub(in crate::compiler) fn compile_lambda(
        &mut self,
        params: &[Param],
        body: &LambdaBody,
        _ret: Option<&Type>,
        line: u32,
    ) -> Result<(), String> {
        // 1. Compute free variables of the lambda body.
        let all_free = free_vars(params, body);
        // 2. Filter to only variables that resolve to a local in the *enclosing* scope
        //    (names that are top-level functions are resolved statically at call time and
        //    don't need to be captured — `compile_call` handles them via `Op::Call`).
        let captures: Vec<(usize, String)> = all_free
            .into_iter()
            .filter_map(|name| {
                // Only capture locals; top-level functions, variants, and classes are not.
                self.resolve_local(&name)
                    .filter(|_| !self.fns.contains_key(&name))
                    .map(|slot| (slot, name))
            })
            .collect();
        // `this`-capture (Phase 1 closures slice): when the body references `this` (directly or
        // through a nested lambda) and we have a receiver in scope, capture the enclosing `this` as
        // an extra, *first* capture so it lands at the sub-frame's slot 0 — exactly where the
        // sub-compiler's `this_slot` will point. The receiver value is the live `Rc` instance handle,
        // so a field write through it is visible to the closure, matching the interpreter + PHP.
        let uses_this = self.this_slot.is_some() && crate::ast::lambda_uses_this(body);
        let n_captures = captures.len() + usize::from(uses_this);

        // 3. Build the sub-function's index in the global table.
        //    `base_fn_idx` is the start of this compilation's slice of the trailing lambda block;
        //    each lambda this compiler emits takes the next slot, hence `base + len`.
        let fn_idx = self.base_fn_idx + self.extra_functions.len();

        // 4. Build a sub-compiler for the lambda body.
        //    This lambda occupies global slot `fn_idx`; step 8 appends its nested lambdas
        //    *immediately after* it, so they start at `fn_idx + 1`. The sub-compiler therefore
        //    treats `fn_idx + 1` as the start of its own (nested) lambda slice.
        let sub_base = fn_idx + 1;
        let empty_fields: HashMap<String, CTy> = HashMap::new();
        // A lambda body cannot reference `this` or bare fields (checker enforces E-LAMBDA-THIS),
        // so we create the sub-compiler without field scope or a class context.
        let mut sub = Compiler::new(
            self.fns,
            self.arities,
            self.variants,
            self.enum_descs,
            self.classes,
            self.imports,
            self.statics_index,
            self.consts_index,
            self.class_descs,
            self.names_index,
            &empty_fields,
            self.class_field_ctys,
            self.method_rets,
            self.method_generic_ret_from_param,
            self.reified_operands,
            self.methods,
            self.method_overloads,
            sub_base,
        );

        // 5. Seed the sub-compiler's locals: [this?, captures.., params..] — matching the frame
        //    layout `Op::CallValue` builds (the receiver, if captured, is pushed first below).
        if uses_this {
            // The receiver's operand type is its class, so `this.x + 1` specializes in the lambda
            // (without `cur_class`, `ctype(This)` would fail — the documented CTy-operand trap).
            let this_cty = self.cur_class.clone().map_or(CTy::Other, CTy::Class);
            sub.add_local("$this", this_cty);
            sub.this_slot = Some(0);
            sub.cur_class = self.cur_class.clone();
        }
        for (_, cap_name) in &captures {
            // The capture's type comes from the enclosing scope's local.
            let slot = self
                .resolve_local(cap_name)
                .expect("capture must resolve in enclosing scope");
            let ty = self.locals[slot].ty.clone();
            sub.add_local(cap_name, ty);
        }
        for p in params {
            sub.add_local(&p.name, resolve_cty(&p.ty));
        }
        sub.height = sub.locals.len();

        // 6. Compile the body. Expression-body: evaluate + explicit Return.
        match body {
            LambdaBody::Expr(e) => {
                sub.expr(e)?;
                sub.emit(Op::Return, line);
            }
            LambdaBody::Block(stmts) => {
                for s in stmts {
                    sub.stmt(s)?;
                }
                sub.emit_const(Value::Unit, line);
                sub.emit(Op::Return, line);
            }
        }

        // 7. Collect any nested lambdas compiled inside the sub-compiler.
        let mut nested_extras = sub.extra_functions;

        // 8. Build the sub-function and append it to our own extra_functions.
        let lambda_fn = Function {
            name: format!("<lambda@{line}>"),
            arity: n_captures + params.len(),
            n_captures,

            unchecked: false,
            // Lambdas record no union-param stamps (v1: the JIT's Dyn seeding covers named
            // functions/methods; a lambda union param stays Unknown — fail-closed decline).
            dyn_params: Vec::new(),
            chunk: sub.chunk,
        };
        self.extra_functions.push(lambda_fn);
        self.lambda_n_captures.push(n_captures);
        // Drain nested extras: their indices follow this lambda in the table.
        self.extra_functions.append(&mut nested_extras);

        // 9. Push capture values onto the stack (enclosing scope), then emit MakeClosure.
        //    The receiver is pushed first (→ sub slot 0), then the free-var captures — matching the
        //    sub-compiler's local order above and the frame `Op::CallValue` rebuilds.
        if uses_this {
            self.emit(
                Op::GetLocal(self.this_slot.expect("uses_this implies a receiver slot")),
                line,
            );
        }
        for (slot, _) in &captures {
            self.emit(Op::GetLocal(*slot), line);
        }
        self.emit(Op::MakeClosure(fn_idx), line);
        Ok(())
    }
}
