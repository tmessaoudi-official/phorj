//! Collection pass — traits and interfaces: declaration collection + interface-graph driver.
//!
//! The interface-graph validation sub-phases live in sibling modules: class `extends`/rename/MI
//! conflicts in `class_graph`, override variance in `overrides`, abstract/trait checks in
//! `abstract_traits`, and interface `extends` + class conformance in `conformance`.

use super::*;

impl Checker {
    /// M-RT S8: collect a trait by reusing the class machinery. A synthetic `ClassDecl` carries the
    /// trait's members into a [`ClassInfo`] (keyed by the trait name) so the trait body type-checks and
    /// the trait's members can be merged into each using class. Marked `is_abstract` so an abstract
    /// *requirement* method doesn't trip the concrete-class unimpl check on the trait itself; recorded
    /// in [`Self::traits`] so the name is rejected wherever a *type* is expected (a trait is reuse, not
    /// a type), and so construction (`Loud()`) is caught by the abstract-instantiate guard.
    pub(in crate::checker) fn collect_trait(&mut self, t: &crate::ast::TraitDecl) {
        let synthetic = crate::ast::ClassDecl {
            vis: crate::ast::Visibility::Public,
            attrs: Vec::new(), // synthetic trait→class carries no attributes
            name: t.name.clone(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            extends: Vec::new(),
            implements: Vec::new(),
            implements_args: Vec::new(),
            open: false,
            is_abstract: true,
            sealed: false,
            resolutions: Vec::new(),
            uses: Vec::new(),
            members: t.members.clone(),
            foreign: false,
            span: t.span,
        };
        self.collect_class(&synthetic);
        self.traits.insert(t.name.clone());
    }

    pub(in crate::checker) fn collect_interface(&mut self, i: &crate::ast::InterfaceDecl) {
        if is_builtin_type_name(&i.name) {
            self.err(
                i.span,
                format!("cannot redefine built-in type `{}`", i.name),
            );
            return;
        }
        if !self.prebound.contains(&i.name)
            && (self.classes.contains_key(&i.name)
                || self.enums.contains_key(&i.name)
                || self.interfaces.contains_key(&i.name))
        {
            self.err_coded(
                i.span,
                format!("type `{}` is already defined", i.name),
                "E-DUP-TYPE",
                Some("rename one declaration — a class/enum/interface/trait/type name must be unique".into()),
            );
            return;
        }
        // W5-3: record a `sealed` interface so a `match` over it is exhaustive over its whole-program
        // permitted implementors (checked in `check_match`; compile-time-only).
        if i.sealed {
            self.sealed_types.insert(i.name.clone());
        }
        // Register the name first so a method signature may reference the interface itself.
        self.interfaces.insert(
            i.name.clone(),
            InterfaceInfo {
                methods: HashMap::new(),
                extends: i.extends.clone(),
                type_params: i.type_params.clone(),
            },
        );
        // DEC-257 generic interfaces: while resolving the signatures below, the interface's own
        // type parameters are in scope — `T` in `function next(): T;` resolves to `Ty::Param`.
        self.active_type_params = i.type_params.clone();
        let mut methods = HashMap::new();
        for m in &i.methods {
            if methods.contains_key(&m.name) {
                self.err(
                    m.span,
                    format!("duplicate method `{}` in interface `{}`", m.name, i.name),
                );
                continue;
            }
            let params = m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
            // S0b: an interface method signature must declare its return type too (it never flows
            // through `check_function`, so enforce it here at collection).
            if m.ret.is_none() {
                self.err_coded(
                    m.span,
                    format!("interface method `{}` must declare a return type", m.name),
                    "E-MISSING-RETURN-TYPE",
                    Some(
                        "every function and method declares its return type — add `-> void` for a side-effecting method".into(),
                    ),
                );
            }
            let ret = match &m.ret {
                Some(t) => self.resolve_type(t),
                None => Ty::Void,
            };
            methods.insert(
                m.name.clone(),
                FnSig {
                    params,
                    // Default parameters are free-function-only in v1 (methods are a deferral); an
                    // interface method carries none.
                    defaults: vec![None; m.params.len()],
                    ret,
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    // Interface-method throws are not enforced through dynamic dispatch this slice
                    // (a documented deferral); keep the set empty so no call site mis-discharges.
                    throws: Vec::new(),
                    is_static: false,
                    variadic: false, // DEC-298: interface methods don't support variadics in v1
                    param_names: m.params.iter().map(|p| p.name.clone()).collect(), // DEC-297
                    // Interface method SIGNATURES carry no attributes (the parser builds them without), so a
                    // deprecation can only live on the implementing method. DEC-417.
                    deprecated: None,
                },
            );
        }
        self.interfaces.get_mut(&i.name).unwrap().methods = methods;
        self.active_type_params.clear();
    }

    /// Validate the interface graph and class conformance, then build [`Self::class_implements`].
    ///
    /// A thin driver over the sub-phase checks (in the sibling modules named above). The ordering is
    /// load-bearing — it fixes diagnostic emission order — and matches the pre-split single method:
    /// class `extends` → member inheritance → `rename` → override variance → MI method/field
    /// conflicts → abstract/trait checks → interface `extends` → throwable naming → class conformance.
    pub(in crate::checker) fn check_interface_graph(&mut self, program: &crate::ast::Program) {
        // Always safe to compute (the shared fn is cycle-guarded); diagnostics below catch malformed
        // graphs, and the backends only run after a clean check, so a cyclic table never reaches them.
        self.class_implements = crate::ast::class_implements(program);
        self.class_supertypes = crate::ast::class_supertypes(program);

        // Class `extends` targets must be `open` classes; detect cycles (M-RT S6).
        self.check_class_extends(program);

        // Inherit each class's ancestors' members into its `ClassInfo` (child wins on a clash),
        // before interface-conformance below — so an inherited method can satisfy an interface.
        self.inherit_class_members(program);

        // M-RT S6b: `rename P.m as n` exposes a parent method under a new name (must run before the
        // override check, which reads the resulting method table).
        self.apply_rename_resolutions(program);

        // M-RT S6: override must target an `open` ancestor + return-covariance / param-contravariance.
        self.check_method_overrides(program);

        // M-RT S6b/S6c.1: unresolved cross-parent method / field collisions.
        self.check_mi_method_conflicts(program);
        self.check_mi_field_conflicts(program);

        // M-RT S6b/S8: abstract bookkeeping, `use`-trait validation, trait-ctor footguns, and the
        // concrete-class abstract-unimpl check.
        self.check_abstract_and_traits(program);

        // Interface `extends` must target interfaces; detect cycles.
        self.check_interface_extends(program);

        // DEC-275: a throwable type must be named `*Error`/`*Exception`.
        self.check_error_names(program);

        // Class conformance: every interface method (own + inherited) must be provided.
        self.check_class_conformance(program);
    }
}
