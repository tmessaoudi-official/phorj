//! Collection pass — enum and class declarations.

use super::*;

impl Checker {
    pub(in crate::checker) fn collect_enum(&mut self, e: &crate::ast::EnumDecl) {
        if is_builtin_type_name(&e.name) {
            self.err(
                e.span,
                format!("cannot redefine built-in type `{}`", e.name),
            );
            return;
        }
        if !self.prebound.contains(&e.name)
            && (self.enums.contains_key(&e.name) || self.classes.contains_key(&e.name))
        {
            self.err_coded(
                e.span,
                format!("type `{}` is already defined", e.name),
                "E-DUP-TYPE",
                Some("rename one declaration — a class/enum/interface/trait/type name must be unique".into()),
            );
            return;
        }
        // Register the name + type parameters first so variant field types can reference the enum
        // itself (including a self-referential `Tree<T>` payload) with correct arity (M-RT generic
        // enums).
        self.validate_type_params(&e.type_params, e.span);
        self.enums.insert(
            e.name.clone(),
            EnumInfo {
                variants: HashMap::new(),
                type_params: e.type_params.clone(),
                injected: e.injected,
            },
        );
        // The enum's type parameters are in scope while resolving every variant field type, so a bare
        // `T` resolves to `Ty::Param("T")` (M-RT generic enums); cleared after, like a generic class.
        self.active_type_params = e.type_params.clone();
        let mut variants = HashMap::new();
        for v in &e.variants {
            let fields = v.fields.iter().map(|p| self.resolve_type(&p.ty)).collect();
            // M-DX S1 (soundness hole C): a repeated variant name used to silently overwrite the
            // first in this `HashMap` — a duplicate `enum E { A, A }` type-checked clean. Reject it.
            if variants.insert(v.name.clone(), fields).is_some() {
                self.err_coded(
                    v.span,
                    format!("duplicate enum variant `{}`", v.name),
                    "E-DUP-VARIANT",
                    Some("each variant of an enum must have a distinct name".into()),
                );
            }
        }
        self.active_type_params.clear();
        self.enums.get_mut(&e.name).unwrap().variants = variants;
    }

    pub(in crate::checker) fn collect_class(&mut self, c: &crate::ast::ClassDecl) {
        use crate::ast::ClassMember;
        if is_builtin_type_name(&c.name) {
            self.err(
                c.span,
                format!("cannot redefine built-in type `{}`", c.name),
            );
            return;
        }
        if !self.prebound.contains(&c.name)
            && (self.classes.contains_key(&c.name) || self.enums.contains_key(&c.name))
        {
            self.err_coded(
                c.span,
                format!("type `{}` is already defined", c.name),
                "E-DUP-TYPE",
                Some("rename one declaration — a class/enum/interface/trait/type name must be unique".into()),
            );
            return;
        }
        // W5-3: record a `sealed` class so a `match` over it is exhaustive over its whole-program
        // permitted subtypes (checked in `check_match`; compile-time-only).
        if c.sealed {
            self.sealed_types.insert(c.name.clone());
        }
        // Register the name + type parameters first so members can reference the class type itself
        // (including a self-referential `Box<T> next` field) with correct arity (M-RT generics-all).
        self.validate_type_params(&c.type_params, c.span);
        self.classes.insert(
            c.name.clone(),
            ClassInfo {
                fields: HashMap::new(),
                mutable_fields: std::collections::HashSet::new(),
                statics: HashMap::new(),
                consts: HashMap::new(),
                static_mut: std::collections::HashSet::new(),
                methods: HashMap::new(),
                hooks: HashMap::new(),
                ctor: Vec::new(),
                has_ctor: false,
                is_user_attribute: c.attrs.iter().any(|a| a.is_attribute_marker()),
                ctor_vis: MemberVis::Public,
                ctor_owner: c.name.clone(),
                type_params: c.type_params.clone(),
                is_abstract: c.is_abstract,
                field_vis: HashMap::new(),
                static_vis: HashMap::new(),
                method_vis: HashMap::new(),
                static_methods: std::collections::HashSet::new(),
            },
        );
        use crate::ast::Modifier;
        // Batch G (finding #7): reject an explicit instance field declared twice (previously the last
        // silently won). An explicit field that *also* names a promoted ctor param is intentionally
        // allowed — the explicit declaration is authoritative (`explicit_field_decl_wins_over_promotion`);
        // a duplicate *promoted* param is caught by `E-DUP-PARAM` on the constructor.
        {
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            // M-DX S1 (soundness hole D): statics and consts each have their own namespace and used
            // to skip this loop entirely (`continue`), so a duplicate `static`/`const` name silently
            // overwrote the first in the `statics`/`consts` `HashMap`. Track each namespace so a
            // repeat is rejected, mirroring the instance-field `E-DUP-FIELD` check.
            let mut seen_static: std::collections::HashSet<&str> = std::collections::HashSet::new();
            let mut seen_const: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for m in &c.members {
                if let ClassMember::Field {
                    modifiers,
                    name,
                    span,
                    ..
                } = m
                {
                    if modifiers.contains(&Modifier::Static) {
                        if !seen_static.insert(name.as_str()) {
                            self.err_coded(
                                *span,
                                format!("duplicate static field `{name}`"),
                                "E-DUP-STATIC",
                                Some("each static field must have a distinct name".into()),
                            );
                        }
                        continue;
                    }
                    if modifiers.contains(&Modifier::Const) {
                        if !seen_const.insert(name.as_str()) {
                            self.err_coded(
                                *span,
                                format!("duplicate `const {name}`"),
                                "E-DUP-CONST",
                                Some("each class constant must have a distinct name".into()),
                            );
                        }
                        continue;
                    }
                    if !seen.insert(name.as_str()) {
                        self.err_coded(
                            *span,
                            format!("duplicate field `{name}`"),
                            "E-DUP-FIELD",
                            Some("each field must have a distinct name".into()),
                        );
                    }
                }
            }
        }
        let mut fields = HashMap::new();
        // Member visibility (Wave 1.1): instance-field and method name → (vis, owner==this class).
        // Inherited entries (with their original owner) are merged in by `merge_inherited`.
        let mut field_vis: HashMap<String, (MemberVis, String)> = HashMap::new();
        let mut method_vis: HashMap<String, (MemberVis, String)> = HashMap::new();
        let mut mutable_fields = std::collections::HashSet::new();
        let mut statics: HashMap<String, Ty> = HashMap::new();
        let mut static_vis: HashMap<String, (MemberVis, String)> = HashMap::new();
        let mut consts: HashMap<String, ConstEntry> = HashMap::new();
        let mut static_mut = std::collections::HashSet::new();
        let mut methods: HashMap<String, Vec<FnSig>> = HashMap::new();
        let mut static_methods: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut hooks: HashMap<String, HookInfo> = HashMap::new();
        let mut ctor = Vec::new();
        let mut ctor_vis = MemberVis::Public;
        // The class's type parameters are in scope while resolving every member signature (fields,
        // constructor, methods), so a bare `T` resolves to `Ty::Param("T")` (M-RT generics-all). A
        // generic method adds its own parameters on top.
        let class_tp = &c.type_params;
        // Promoted ctor params (carrying a visibility modifier) also become fields,
        // matching the evaluator's runtime promotion (EV-4). Deferred to after the
        // member loop via `or_insert` so an explicit `Field` decl of the same name
        // stays authoritative regardless of member order.
        let mut promoted: Vec<(String, Ty, MemberVis)> = Vec::new();
        for m in &c.members {
            match m {
                ClassMember::Field {
                    ty,
                    name,
                    modifiers,
                    init,
                    span,
                } => {
                    self.active_type_params = class_tp.clone();
                    let fty = self.resolve_type(ty);
                    self.active_type_params.clear();
                    if modifiers.contains(&Modifier::Const) {
                        // A `const` class constant (Feature A): compile-time, immutable, class-level,
                        // accessed only `ClassName.NAME`. It needs a literal-const initializer and must
                        // not be `mutable`. Disjoint from instance fields and statics.
                        if modifiers.contains(&Modifier::Mutable) {
                            self.err_coded(
                                *span,
                                format!("`const {name}` cannot be `mutable` — a constant is immutable"),
                                "E-CONST-MUTABLE",
                                Some("drop `mutable`, or use a `static mutable` field for class-level state".into()),
                            );
                        }
                        match init {
                            None => {
                                self.err_coded(
                                    *span,
                                    format!("`const {name}` needs an initializer"),
                                    "E-CONST-NO-INIT",
                                    Some("e.g. `const int MAX = 100;`".into()),
                                );
                            }
                            Some(e) => {
                                if crate::value::const_literal(e).is_none() {
                                    self.err_coded(
                                        Self::expr_span(e),
                                        format!(
                                            "`const {name}` initializer must be a literal constant"
                                        ),
                                        "E-CONST-NOT-LITERAL",
                                        Some("use an int/float/bool/string/null literal".into()),
                                    );
                                } else {
                                    let ity = self.check_expr(e);
                                    if !self.ty_assignable(&ity, &fty) {
                                        self.err_coded(
                                            Self::expr_span(e),
                                            format!(
                                                "`const {name}: {fty}` initialized with `{ity}`"
                                            ),
                                            "E-CONST-INIT-TYPE",
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                        consts.insert(
                            name.clone(),
                            ConstEntry {
                                ty: fty,
                                vis: MemberVis::of(modifiers),
                                owner: c.name.clone(),
                            },
                        );
                    } else if modifiers.contains(&Modifier::Static) {
                        // A `static` field is class-level state (M-mut.7): it needs an initializer (no
                        // constructor sets it) and is NOT an instance field. Feature B-static lifts the
                        // old literal-only restriction — the initializer may be ANY expression, evaluated
                        // once at program start in declaration order. Its TYPE is checked later
                        // (`check_static_inits`, pass 2) where every function + static is collected, so
                        // an initializer may call a function or read another (earlier) static.
                        if init.is_none() {
                            self.err_coded(
                                *span,
                                format!("static field `{name}` needs an initializer"),
                                "E-STATIC-NO-INIT",
                                Some("e.g. `static mutable int total = 0;`".into()),
                            );
                        }
                        statics.insert(name.clone(), fty);
                        // W0-2: record vis + declaring owner alongside the type, so a
                        // `private`/`protected` static read/write from outside is rejected (mirrors
                        // `field_vis`; owner preserved through inheritance for owner/subclass checks).
                        static_vis.insert(name.clone(), (MemberVis::of(modifiers), c.name.clone()));
                        if modifiers.contains(&Modifier::Mutable) {
                            static_mut.insert(name.clone());
                        }
                    } else {
                        // A plain instance field. An optional expression initializer (Feature B) is
                        // evaluated per-instance at construction (declaration order, after promotion);
                        // its type + forward-reference are checked in `check_type_body`, where `this`
                        // and the field scope are live. Just record the field here.
                        fields.insert(name.clone(), fty);
                        field_vis.insert(name.clone(), (MemberVis::of(modifiers), c.name.clone()));
                        if modifiers.contains(&Modifier::Mutable) {
                            mutable_fields.insert(name.clone());
                        }
                    }
                }
                ClassMember::Constructor {
                    modifiers,
                    params,
                    span,
                    ..
                } => {
                    // The constructor's own visibility (Batch A). A non-visibility modifier
                    // (`abstract`/`static`/`const`/`open`/`mutable`) on a constructor is meaningless —
                    // reject it rather than silently dropping it (closes the §5 dropped-modifier gaps).
                    ctor_vis = MemberVis::of(modifiers);
                    if modifiers.iter().any(|m| {
                        !matches!(
                            m,
                            Modifier::Public | Modifier::Private | Modifier::Protected
                        )
                    }) {
                        self.err_coded(
                            *span,
                            "a constructor takes only a visibility modifier (`private`/`protected`/`public`)".to_string(),
                            "E-CTOR-MODIFIER",
                            Some("remove `abstract`/`static`/`const`/`open`/`mutable` from the constructor".into()),
                        );
                    }
                    self.reject_dup_param_names(params.iter().map(|p| (p.name.as_str(), p.span)));
                    // Resolve each param type once; reuse for both the ctor signature
                    // and field promotion to avoid duplicate "unknown type" errors.
                    self.active_type_params = class_tp.clone();
                    ctor = params
                        .iter()
                        .map(|p| {
                            let ty = self.resolve_type(&p.ty);
                            if p.modifiers.iter().any(|m| {
                                matches!(
                                    m,
                                    Modifier::Public | Modifier::Private | Modifier::Protected
                                )
                            }) {
                                promoted.push((
                                    p.name.clone(),
                                    ty.clone(),
                                    MemberVis::of(&p.modifiers),
                                ));
                                // A `public mutable int x` promoted param yields a mutable field.
                                if p.modifiers.contains(&Modifier::Mutable) {
                                    mutable_fields.insert(p.name.clone());
                                }
                            }
                            ty
                        })
                        .collect();
                    self.active_type_params.clear();
                }
                ClassMember::Method(f) => {
                    // A method reuses the free-fn machinery (M-RT generics-all): with the class's
                    // type parameters AND the method's own in scope, a bare `T`/`U` resolves to
                    // `Ty::Param`; class params are substituted with the instance's type arguments at
                    // the call site, method params unified from the call's arguments. A method param
                    // that shadows a class param is rejected so composition stays unambiguous. Erased
                    // before any backend by `erase_generics`.
                    self.reject_dup_param_names(f.params.iter().map(|p| (p.name.as_str(), p.span)));
                    self.validate_type_params(&f.type_params, f.span);
                    for tp in &f.type_params {
                        if class_tp.iter().any(|c| c == tp) {
                            self.err_coded(
                                f.span,
                                format!(
                                    "method type parameter `{tp}` shadows the class type parameter `{tp}`"
                                ),
                                "E-GENERIC-PARAM",
                                Some("rename the method's type parameter".into()),
                            );
                        }
                    }
                    let mut active = class_tp.clone();
                    active.extend(f.type_params.iter().cloned());
                    self.active_type_params = active;
                    let p = f.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                    let ret = match &f.ret {
                        Some(t) => self.resolve_type(t),
                        None => Ty::Void,
                    };
                    let throws = Self::flatten_throws(
                        f.throws.iter().map(|t| self.resolve_type(t)).collect(),
                    );
                    self.active_type_params.clear();
                    // M4 default parameters are free-function-only in v1; a default on a method param
                    // is rejected (the fill pass resolves free/native calls, not method dispatch).
                    self.reject_member_defaults(&f.params, "method");
                    // M-RT overloading: a same-named method joins an overload set (same rules as free
                    // functions — same return, no identical signatures, no generic member).
                    let sig = FnSig {
                        params: p,
                        defaults: vec![None; f.params.len()],
                        ret,
                        type_params: f.type_params.clone(),
                        throws,
                        is_static: f.modifiers.contains(&Modifier::Static),
                    };
                    let existing = methods.get(&f.name).cloned().unwrap_or_default();
                    // M-RT S2.2: INSTANCE methods may return-overload (identical params, distinct
                    // returns), resolved by a `<Type>` selector and mangled per return before any
                    // backend — exactly like free functions. The same soundness guards apply (a set is
                    // EITHER a parameter-overload set OR a pure return-overload set, never mixed;
                    // identical params AND return is still a duplicate). `static` methods are excluded
                    // (`allow_return_overload = !is_static`): a static call is `ClassName.m(args)`,
                    // dispatched by `check_static_method_call` which has no `<Type>` selector path — a
                    // return-overloaded static would mangle its definition with no matching call-site
                    // rewrite. So statics keep the classic shared-return rule (`E-OVERLOAD-RETURN`).
                    self.validate_new_overload(
                        &existing,
                        &sig,
                        &f.name,
                        f.span,
                        "method",
                        !sig.is_static,
                    );
                    // Record the declaration site so `finalize_method_overloads` can emit a per-decl
                    // mangled rename (reuses `overload_def_renames`; method/free-fn spans are disjoint).
                    self.method_fn_decls.push((
                        c.name.clone(),
                        f.name.clone(),
                        f.span,
                        sig.params.clone(),
                        sig.ret.clone(),
                    ));
                    methods.entry(f.name.clone()).or_default().push(sig);
                    // First-declared overload's visibility represents the method name (Wave 1.1).
                    method_vis
                        .entry(f.name.clone())
                        .or_insert((MemberVis::of(&f.modifiers), c.name.clone()));
                    // slice B0: a `static` method is callable via the class name (`ClassName.m(args)`).
                    if f.modifiers.contains(&Modifier::Static) {
                        static_methods.insert(f.name.clone());
                    }
                }
                // A property hook (M-mut.7b): record its declared type and which accessors it
                // provides. The body is type-checked in phase 2 (`check_program`), with `this` and
                // the field scope live. Class type params are in scope for the hook's type.
                ClassMember::Hook {
                    ty, name, get, set, ..
                } => {
                    self.active_type_params = class_tp.clone();
                    let hty = self.resolve_type(ty);
                    self.active_type_params.clear();
                    if hooks.contains_key(name) {
                        self.err_coded(
                            c.span,
                            format!("property hook `{name}` is declared more than once"),
                            "E-HOOK-DUP",
                            None,
                        );
                    }
                    hooks.insert(
                        name.clone(),
                        HookInfo {
                            ty: hty,
                            has_get: get.is_some(),
                            has_set: set.is_some(),
                        },
                    );
                }
            }
        }
        // Explicit field decls win: only insert a promoted field if not already declared.
        for (name, ty, pvis) in promoted {
            fields.entry(name.clone()).or_insert(ty);
            field_vis.entry(name).or_insert((pvis, c.name.clone()));
        }
        // A property hook is virtual: its name must not also name a stored field, a static, or a
        // method (the read/write path resolves a hook before the field, so a collision would shadow
        // the storage silently). Order-independent — checked after every member is collected.
        for hname in hooks.keys() {
            if fields.contains_key(hname)
                || statics.contains_key(hname)
                || methods.contains_key(hname)
            {
                self.err_coded(
                    c.span,
                    format!("property hook `{hname}` collides with a field, static, or method of the same name"),
                    "E-HOOK-DUP",
                    Some("a hook is virtual — give it a distinct name from any stored member".into()),
                );
            }
        }
        let info = self.classes.get_mut(&c.name).unwrap();
        info.fields = fields;
        info.field_vis = field_vis;
        info.static_vis = static_vis;
        info.method_vis = method_vis;
        info.static_methods = static_methods;
        info.mutable_fields = mutable_fields;
        info.statics = statics;
        info.consts = consts;
        info.static_mut = static_mut;
        info.methods = methods;
        info.hooks = hooks;
        info.has_ctor = c
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor { .. }));
        info.ctor = ctor;
        info.ctor_vis = ctor_vis;
        // `ctor_owner` was initialized to the class's own name; an own ctor keeps it. An inherited
        // ctor's owner/visibility are merged in `merge_inherited` for a class with no own ctor.
    }
}
