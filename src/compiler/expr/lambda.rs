//! Expression compilation — lambda literals (M-Decomp split from `calls.rs`, Invariant 13).

use super::*;

impl Compiler<'_> {
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
        // 2. Filter to only variables that resolve in the *enclosing* scope: a `match`-arm binding
        //    first (it shadows a local, as in `eval_ident` and `Expr::Ident`'s compile), then a local.
        //    A binding is not a local but a path from the `$match` slot, so it is captured by value,
        //    re-extracted at closure creation (LAMBDA-CAPTURE-MATCH-BINDING). Names that are
        //    top-level functions are resolved statically at call time and are not captured —
        //    `compile_call` handles them via `Op::Call`.
        let captures: Vec<(Capture, String)> = all_free
            .into_iter()
            .filter_map(|name| {
                if let Some((m_slot, path)) = self.resolve_binding(&name) {
                    // `resolve_binding` found it in `match_bindings`, which `binding_cty` reads too.
                    let ty = self
                        .binding_cty(&name)
                        .expect("a resolved match binding has a type");
                    return Some((Capture::Binding(m_slot, path, ty), name));
                }
                // Only capture locals; top-level functions, variants, and classes are not.
                self.resolve_local(&name)
                    .filter(|_| !self.fns.contains_key(&name))
                    .map(|slot| (Capture::Local(slot), name))
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
        // Row 4b: class exactness, so the lambda body dispatches `with` and hooks as its encloser does.
        sub.class_parents_all = self.class_parents_all;
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
        for (cap, cap_name) in &captures {
            // The capture's type comes from the enclosing scope: the local's, or the binding's.
            let ty = match cap {
                Capture::Local(slot) => self.locals[*slot].ty.clone(),
                Capture::Binding(_, _, ty) => ty.clone(),
            };
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
            str_params: Vec::new(),
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
        for (cap, _) in &captures {
            match cap {
                Capture::Local(slot) => self.emit(Op::GetLocal(*slot), line),
                Capture::Binding(m_slot, path, _) => self.emit_load_path(*m_slot, path, line),
            }
        }
        self.emit(Op::MakeClosure(fn_idx), line);
        Ok(())
    }
}

/// Where a lambda capture's value comes from in the enclosing scope.
enum Capture {
    /// A local or parameter slot.
    Local(usize),
    /// A `match`-arm binding: the `$match` slot, the payload path to re-extract, and its type.
    Binding(usize, Vec<PathSeg>, CTy),
}
