//! Program pass — type bodies: member checking, definite assignment, function signatures.

use super::walk::field_init_forbidden_ref;
use super::*;

impl Checker {
    /// Check the method/constructor/hook bodies of a class or trait (M-RT S8 shares this between the
    /// two). `this` resolves to `type_name`'s already-collected [`ClassInfo`]; `type_params` are in
    /// scope across every body (empty for a trait this slice).
    pub(in crate::checker) fn check_type_body(
        &mut self,
        type_name: &str,
        type_params: &[String],
        type_param_bounds: &[(String, String)],
        members: &[crate::ast::ClassMember],
    ) {
        use crate::ast::{ClassMember, Modifier};
        let prev = self.cur_class.replace(type_name.to_string());
        let prev_tp = std::mem::replace(&mut self.cur_class_type_params, type_params.to_vec());
        let prev_tpb = std::mem::replace(
            &mut self.cur_class_type_param_bounds,
            type_param_bounds.to_vec(),
        );
        // Feature B — expression field initializers. An initializer is evaluated per-instance at
        // construction in declaration order, after the promoted ctor params are bound. So promoted
        // params are always available to an initializer; an explicit field is available only to LATER
        // fields' initializers (it is set in order). `available` tracks what is initialized by the time
        // each field's initializer runs; reading anything else (a self/later field, or a not-yet-set
        // plain field) is `E-FIELD-INIT-FORWARD-REF`.
        let mut available: std::collections::HashSet<String> = members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Constructor { params, .. } => Some(params),
                _ => None,
            })
            .flatten()
            .filter(|p| {
                p.modifiers.iter().any(|md| {
                    matches!(
                        md,
                        Modifier::Public | Modifier::Private | Modifier::Protected
                    )
                })
            })
            .map(|p| p.name.clone())
            .collect();
        let instance_fields: std::collections::HashSet<String> = members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Field {
                    modifiers, name, ..
                } if !modifiers.contains(&Modifier::Static)
                    && !modifiers.contains(&Modifier::Const) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
            .collect();
        for m in members {
            match m {
                ClassMember::Method(f) => {
                    // Batch E: a static method body must not touch instance state (`this` / bare
                    // fields) — `in_static_method` forbids it while `cur_class` stays set for
                    // static-member access and factory construction.
                    let was_static = self.in_static_method;
                    self.in_static_method = f.modifiers.contains(&Modifier::Static);
                    self.check_function(f);
                    self.in_static_method = was_static;
                }
                ClassMember::Constructor { params, body, .. } => {
                    let prev_ret = std::mem::replace(&mut self.cur_ret, Ty::Void);
                    // type params in scope for any `T` annotation in the body
                    self.active_type_params = type_params.to_vec();
                    self.active_type_param_bounds = self.cur_class_type_param_bounds.clone();
                    self.push_scope();
                    // constructor params are in scope inside its body
                    let ctor = self
                        .classes
                        .get(type_name)
                        .map(|info| info.ctor.clone())
                        .unwrap_or_default();
                    for (p, t) in params.iter().zip(ctor) {
                        self.declare(&p.name, t, p.span);
                    }
                    let was_ctor = std::mem::replace(&mut self.in_constructor, true);
                    self.check_body(body);
                    self.in_constructor = was_ctor;
                    self.pop_scope();
                    self.active_type_params.clear();
                    self.active_type_param_bounds.clear();
                    self.cur_ret = prev_ret;
                }
                // A property hook (M-mut.7b) — type-check the `get` expression against the hook's
                // declared type and the `set` block with the assigned value `v` bound to that type,
                // both with `this` + the field scope live.
                ClassMember::Hook {
                    ty, name, get, set, ..
                } => {
                    self.active_type_params = type_params.to_vec();
                    self.active_type_param_bounds = self.cur_class_type_param_bounds.clone();
                    let hook_ty = self.resolve_type(ty);
                    if let Some(e) = get {
                        self.push_scope();
                        let ety = self.check_expr(e);
                        if !self.ty_assignable(&ety, &hook_ty) {
                            self.err_coded(
                                Self::expr_span(e),
                                format!("`get` of `{name}` yields `{ety}`, expected `{hook_ty}`"),
                                "E-HOOK-TYPE",
                                None,
                            );
                        }
                        self.pop_scope();
                    }
                    if let Some((p, body)) = set {
                        let prev_ret = std::mem::replace(&mut self.cur_ret, Ty::Void);
                        self.push_scope();
                        let pty = self.resolve_type(&p.ty);
                        if !(self.ty_assignable(&pty, &hook_ty)
                            && self.ty_assignable(&hook_ty, &pty))
                        {
                            self.err_coded(
                                p.span,
                                format!(
                                    "`set` parameter of `{name}` is `{pty}`, expected the hook type `{hook_ty}`"
                                ),
                                "E-HOOK-TYPE",
                                Some(format!("declare it `set({hook_ty} {})`", p.name)),
                            );
                        }
                        // Bind `v` at the hook's type so the body checks consistently even when the
                        // declared parameter type mismatched.
                        self.declare(&p.name, hook_ty.clone(), p.span);
                        self.check_body(body);
                        self.pop_scope();
                        self.cur_ret = prev_ret;
                    }
                    self.active_type_params.clear();
                    self.active_type_param_bounds.clear();
                }
                // Feature B: type-check a plain instance field's initializer with `this` + the field
                // scope live, and reject a forward reference (reading a not-yet-initialized field).
                ClassMember::Field {
                    modifiers,
                    ty,
                    name,
                    init: Some(e),
                    span,
                } if !modifiers.contains(&Modifier::Static)
                    && !modifiers.contains(&Modifier::Const) =>
                {
                    let forbidden: std::collections::HashSet<String> = instance_fields
                        .iter()
                        .filter(|f| !available.contains(*f))
                        .cloned()
                        .collect();
                    if let Some(bad) = field_init_forbidden_ref(e, &forbidden) {
                        self.err_coded(
                            *span,
                            format!(
                                "field initializer of `{name}` reads `{bad}`, which is not initialized yet"
                            ),
                            "E-FIELD-INIT-FORWARD-REF",
                            Some(format!(
                                "an initializer may read `this` and earlier-initialized fields only — declare `{bad}` before `{name}`, or set `{name}` in the constructor"
                            )),
                        );
                    }
                    self.active_type_params = type_params.to_vec();
                    self.active_type_param_bounds = self.cur_class_type_param_bounds.clone();
                    let fty = self.resolve_type(ty);
                    // A field-default lambda may not capture `this` (partially-built instance).
                    self.in_field_init = true;
                    let ity = self.check_expr(e);
                    self.in_field_init = false;
                    if !self.ty_assignable(&ity, &fty) {
                        self.err_coded(
                            Self::expr_span(e),
                            format!("field `{name}: {fty}` initialized with `{ity}`"),
                            "E-FIELD-INIT-TYPE",
                            None,
                        );
                    }
                    self.active_type_params.clear();
                    self.active_type_param_bounds.clear();
                    available.insert(name.clone());
                }
                ClassMember::Field { .. } => {}
            }
        }
        self.check_definite_assignment(type_name, members);
        self.cur_class_type_params = prev_tp;
        self.cur_class_type_param_bounds = prev_tpb;
        self.cur_class = prev;
    }

    /// Definite-assignment pass (Soundness Batch D, finding #4): every **non-optional** instance field
    /// that has no initializer and is not a promoted ctor param must be assigned on every completing
    /// path of the constructor — else the field is constructed unset and reading it faults at runtime
    /// (`no field x`), an unbacked `T`. An **optional** field is exempt: it defaults to `null`
    /// (`inject_optional_field_defaults` injects the default before any backend). A trait is skipped —
    /// its fields are the responsibility of the composing class. `E-FIELD-UNINITIALIZED`.
    pub(in crate::checker) fn check_definite_assignment(
        &mut self,
        type_name: &str,
        members: &[crate::ast::ClassMember],
    ) {
        use crate::ast::{ClassMember, Modifier};
        if self.traits.contains(type_name) {
            return;
        }
        // Promoted ctor params (visibility-modified) auto-assign their field; collect the names + the
        // constructor body (empty when the class has no constructor).
        let mut promoted: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut ctor_body: &[crate::ast::Stmt] = &[];
        for m in members {
            if let ClassMember::Constructor { params, body, .. } = m {
                ctor_body = body;
                for p in params {
                    if p.modifiers.iter().any(|md| {
                        matches!(
                            md,
                            Modifier::Public | Modifier::Private | Modifier::Protected
                        )
                    }) {
                        promoted.insert(p.name.as_str());
                    }
                }
            }
        }
        for m in members {
            let ClassMember::Field {
                modifiers,
                name,
                init: None,
                span,
                ..
            } = m
            else {
                continue;
            };
            // Only plain instance fields (a `static`/`const` field has its own init rules); an
            // optional field defaults to null; a promoted field is assigned by promotion.
            if modifiers.contains(&Modifier::Static)
                || modifiers.contains(&Modifier::Const)
                || promoted.contains(name.as_str())
            {
                continue;
            }
            let is_optional = matches!(
                self.classes.get(type_name).and_then(|i| i.fields.get(name)),
                Some(Ty::Optional(_))
            );
            if is_optional {
                continue;
            }
            if !self.block_assigns_field(ctor_body, name) {
                self.err_coded(
                    *span,
                    format!("field `{name}` is never initialized — it must be set on every path of the constructor, or given an initializer"),
                    "E-FIELD-UNINITIALIZED",
                    Some("assign `this.{name} = …` unconditionally in the constructor, give the field an initializer (`int {name} = 0;`), make it a promoted ctor param (`constructor(public int {name})`), or make it optional (`int? {name};`, defaults to null)".replace("{name}", name)),
                );
            }
        }
    }

    /// Batch-1 B: validate the entry point `main`'s signature. `main` accepts **zero or one**
    /// parameters — the one allowed param is `List<string>` (the program argv) — and returns `void`
    /// (no exit code → 0) or `int` (the process exit code). Any other shape is `E-MAIN-SIGNATURE`:
    /// the interpreter/VM call `main` with at most the argv list and read back at most an int, so a
    /// different shape would be silently mis-called. `ret` is the already-resolved return type.
    pub(in crate::checker) fn check_main_signature(
        &mut self,
        f: &crate::ast::FunctionDecl,
        ret: &Ty,
    ) {
        let params_ok = match f.params.as_slice() {
            [] => true,
            [p] => matches!(self.resolve_type(&p.ty), Ty::List(elem) if *elem == Ty::String),
            _ => false,
        };
        let ret_ok = matches!(ret, Ty::Void | Ty::Int);
        if !params_ok || !ret_ok {
            self.err_coded(
                f.span,
                "`main` must be `main(): void`, `main(): int`, or take a single `List<string>` argv \
                 parameter — found an incompatible signature"
                    .to_string(),
                "E-MAIN-SIGNATURE",
                Some(
                    "the entry point is `main([List<string> args]): int|void` — the optional \
                     parameter is the program arguments, the `int` return is the exit code"
                        .into(),
                ),
            );
        }
    }

    /// Check one free function or method body. Seeds a fresh scope with params. A generic function's
    /// type parameters are made active for the whole body so `T`-typed params/locals resolve to
    /// `Ty::Param` (M-RT S7). Functions never nest, so a flat set + clear is sufficient.
    pub(in crate::checker) fn check_function(&mut self, f: &crate::ast::FunctionDecl) {
        self.check_attributes(f);
        // S0b: every function and method declares its return type — no exemptions where a return
        // slot exists (constructors and property hooks are separate `ClassMember` variants, so they
        // never reach here; expression-body lambdas infer and are not `FunctionDecl`s). Even `main`
        // must be annotated. Falling off the end of a value-carrying function was the soundness leak
        // the totality cluster closed; mandating the annotation makes every signature self-describing.
        if f.ret.is_none() {
            self.err_coded(
                f.span,
                format!("`{}` must declare a return type", f.name),
                "E-MISSING-RETURN-TYPE",
                Some(
                    "every function and method declares its return type — add `-> void` for a side-effecting function (or `-> empty` to return the holdable empty value)"
                        .into(),
                ),
            );
        }
        // A method of a generic class sees both the class's type parameters and its own (M-RT
        // generics-all); `cur_class_type_params` is empty for free functions and non-generic classes.
        let mut active = self.cur_class_type_params.clone();
        active.extend(f.type_params.iter().cloned());
        self.active_type_params = active;
        // DEC-211: the same union for bounds — a method sees the class's + its own type-param bounds,
        // so a bounded `Ty::Param` resolves member access against its bound interface (`bound_of`).
        let mut active_bounds = self.cur_class_type_param_bounds.clone();
        active_bounds.extend(f.type_param_bounds.iter().cloned());
        self.active_type_param_bounds = active_bounds;
        let ret = match &f.ret {
            Some(t) => self.resolve_type(t),
            None => Ty::Void,
        };
        // Resolve + validate the declared throws set (type params still in scope), then make it the
        // active discharge context for the body (M-faults 2b).
        let throws = Self::flatten_throws(f.throws.iter().map(|t| self.resolve_type(t)).collect());
        self.validate_throws_decl(f, &throws);
        // Batch-1 B/D: the entry point `main` has a constrained signature — 0 or 1 params (the one
        // allowed param is `List<string>`, the argv), returning `void` or `int` (the exit code). An
        // entry is a top-level function OR a `static` method named `main` (Batch-1 D) — an *instance*
        // method named `main` is an ordinary method, not an entry, so it is not constrained.
        let is_entry_main = f.name == "main" && (self.cur_class.is_none() || self.in_static_method);
        if is_entry_main {
            self.check_main_signature(f, &ret);
        }
        let prev_ret = std::mem::replace(&mut self.cur_ret, ret.clone());
        let prev_throws = std::mem::replace(&mut self.cur_throws, throws);
        let prev_main = std::mem::replace(&mut self.cur_is_main, is_entry_main);
        self.push_scope();
        for p in &f.params {
            let pty = self.resolve_type(&p.ty);
            self.declare(&p.name, pty, p.span);
        }
        self.check_body(&f.body);
        self.pop_scope();
        self.cur_ret = prev_ret;
        self.cur_throws = prev_throws;
        self.cur_is_main = prev_main;
        self.active_type_params.clear();
        // Totality: a non-`unit` function must return (or diverge) on every path (M-RT totality
        // cluster). Run after the body walk so all signatures are visible to the divergence analysis.
        // An `abstract` method (M-RT S6b) is a bodyless signature — exempt, like an interface method. A
        // `foreign` (`declare`) function (M8.5) is likewise bodyless — its body lives in PHP.
        if !f.modifiers.contains(&crate::ast::Modifier::Abstract) && !f.foreign {
            self.check_return_totality(&ret, &f.body, f.span);
        }
    }
}
