//! Collection pass — interface `extends` graph, throwable-naming, and class→interface conformance.

use super::*;

impl Checker {
    /// `extends` targets must be interfaces; detect cycles (`E-IFACE-IMPL` / `E-IFACE-CYCLE`).
    pub(in crate::checker) fn check_interface_extends(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        for item in &program.items {
            if let Item::Interface(i) = item {
                for parent in &i.extends {
                    if !self.interfaces.contains_key(parent) {
                        self.err_coded(
                            i.span,
                            format!(
                                "interface `{}` extends `{parent}`, which is not an interface",
                                i.name
                            ),
                            "E-IFACE-IMPL",
                            Some("`extends` on an interface lists other interfaces".into()),
                        );
                    }
                }
                let mut visited = std::collections::BTreeSet::new();
                if self.iface_in_cycle(&i.name, &mut visited) {
                    self.err_coded(
                        i.span,
                        format!("interface `{}` is part of an `extends` cycle", i.name),
                        "E-IFACE-CYCLE",
                        Some("interfaces may not extend themselves transitively".into()),
                    );
                }
            }
        }
    }

    /// DEC-275: a throwable type must READ as one — any class that implements `Error`
    /// (directly, via a parent class, or via interface extends — `class_implements` is fully
    /// transitive) must be named `*Error` or `*Exception`. Enforced for stdlib and user code
    /// alike ("the normal behavior", developer-ruled 2026-07-16); the motivating ambiguity was
    /// `catch (InvalidUrl e)` reading like a value type at every site.
    pub(in crate::checker) fn check_error_names(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        for item in &program.items {
            if let Item::Class(c) = item {
                let throwable = self
                    .class_implements
                    .get(&c.name)
                    .is_some_and(|is| is.iter().any(|i| i == "Error"));
                if throwable && !(c.name.ends_with("Error") || c.name.ends_with("Exception")) {
                    self.err_coded(
                        c.span,
                        format!(
                            "`{}` implements `Error` but its name does not say so — a throwable type must end in `Error` or `Exception`",
                            c.name
                        ),
                        "E-ERROR-NAME",
                        Some(format!("rename it, e.g. `{}Error`", c.name)),
                    );
                }
                // A `declare class` DESCRIBES an existing PHP class rather than defining one, so
                // declaring a signature for a method that is final over there is exactly right — it is
                // how `examples/interop/exceptions.phg` binds PHP's own `DivisionByZeroError`. Only a
                // class we EMIT can collide. (Caught by the pre-push gate, not by reasoning.)
                if throwable && !c.foreign {
                    self.check_final_parent_method_collisions(c);
                }
            }
        }
    }

    /// DEC-367 (Invariant-1 breach) — a throwable class may not define a method that PHP's `Exception`
    /// declares `final`.
    ///
    /// A phorj class implementing `Error` transpiles to one extending `Exception`. PHP marks seven of
    /// `Exception`'s methods `final`, so defining one of them emitted a program that BOTH Rust backends
    /// ran happily while the PHP leg died at runtime with
    /// `Fatal error: Cannot override final method Exception::getMessage()`. Reproduced on php-8.5.8.
    ///
    /// Rejected at CHECK time, so the failure is one diagnostic at the declaration instead of a runtime
    /// fatal on one leg only. Renaming on emission was explicitly REJECTED by the ruling: the program
    /// would keep running while silently diverging from what the author wrote, and anything catching it
    /// as a PHP `Exception` would break.
    ///
    /// The existing `E-RESERVED-NAME` guard (DEC-202) covers colliding class NAMES; this is its method
    /// counterpart, which that guard did not reach.
    fn check_final_parent_method_collisions(&mut self, c: &crate::ast::ClassDecl) {
        use crate::ast::ClassMember;
        for m in &c.members {
            let ClassMember::Method(f) = m else { continue };
            if !FINAL_EXCEPTION_METHODS.contains(&f.name.as_str()) {
                continue;
            }
            self.err_coded(
                f.span,
                format!(
                    "`{}` implements `Error`, so it transpiles to a class extending PHP's `Exception` — and `{}` is `final` there, which PHP refuses to override",
                    c.name, f.name
                ),
                "E-FINAL-PARENT-METHOD",
                Some(format!(
                    "rename it (e.g. `{}Text`), or expose the value through a field — `Error`'s message is already carried by its constructor",
                    f.name
                )),
            );
        }
    }

    /// Class conformance: every interface method (own + inherited) must be provided
    /// (`E-IFACE-IMPL` / `E-TYPE-ARG-COUNT` / `E-IFACE-UNIMPL` / `E-IFACE-SIG` / `E-IFACE-VIS`).
    pub(in crate::checker) fn check_class_conformance(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        for item in &program.items {
            if let Item::Class(c) = item {
                for (iface_idx, iface) in c.implements.iter().enumerate() {
                    if !self.interfaces.contains_key(iface) {
                        self.err_coded(
                            c.span,
                            format!(
                                "class `{}` implements `{iface}`, which is not an interface",
                                c.name
                            ),
                            "E-IFACE-IMPL",
                            Some("`implements` lists declared interfaces".into()),
                        );
                        continue;
                    }
                    // DEC-257 generic interfaces: `implements Iterator<int>` must supply exactly the
                    // interface's declared arity; the arguments (resolved with the class's own type
                    // parameters in scope, so `DatabaseStream<T> implements Iterator<T>` works) substitute
                    // into the interface's method signatures before conformance is compared.
                    let iface_tps = self.interfaces[iface].type_params.clone();
                    let arg_asts = c
                        .implements_args
                        .get(iface_idx)
                        .cloned()
                        .unwrap_or_default();
                    if arg_asts.len() != iface_tps.len() {
                        self.err_coded(
                            c.span,
                            format!(
                                "interface `{iface}` takes {} type argument{}, but `{}` implements it with {}",
                                iface_tps.len(),
                                if iface_tps.len() == 1 { "" } else { "s" },
                                c.name,
                                arg_asts.len()
                            ),
                            "E-TYPE-ARG-COUNT",
                            Some(format!(
                                "write `implements {iface}<…>` with exactly {} argument{}",
                                iface_tps.len(),
                                if iface_tps.len() == 1 { "" } else { "s" }
                            )),
                        );
                        continue;
                    }
                    let theta: HashMap<String, Ty> = if iface_tps.is_empty() {
                        HashMap::new()
                    } else {
                        self.active_type_params = c.type_params.clone();
                        let arg_tys: Vec<Ty> =
                            arg_asts.iter().map(|t| self.resolve_type(t)).collect();
                        self.active_type_params.clear();
                        // Record the class's instantiation of the generic interface for later
                        // assignability / foreach-element lookups (`Ints` → `Producer<int>`).
                        if let Some(ci) = self.classes.get_mut(&c.name) {
                            ci.iface_args.insert(iface.clone(), arg_tys.clone());
                        }
                        iface_tps.into_iter().zip(arg_tys).collect()
                    };
                    let mut required = self.iface_flat_methods(iface);
                    if !theta.is_empty() {
                        for (_, (params, ret)) in &mut required {
                            for p in params.iter_mut() {
                                *p = crate::checker::common::apply_subst(p, &theta);
                            }
                            *ret = crate::checker::common::apply_subst(ret, &theta);
                        }
                    }
                    for (mname, sig) in &required {
                        match self
                            .classes
                            .get(&c.name)
                            .and_then(|ci| ci.methods.get(mname))
                        {
                            None => {
                                self.err_coded(
                                    c.span,
                                    format!(
                                        "class `{}` does not implement method `{mname}` required by interface `{iface}`",
                                        c.name
                                    ),
                                    "E-IFACE-UNIMPL",
                                    Some(format!("add `function {mname}(…)` to `{}`", c.name)),
                                );
                            }
                            Some(have) => {
                                if !self.sig_conforms(have, sig) {
                                    self.err_coded(
                                        c.span,
                                        format!(
                                            "class `{}` method `{mname}` does not match interface `{iface}`'s signature",
                                            c.name
                                        ),
                                        "E-IFACE-SIG",
                                        Some("the parameter types and return type must match the interface".into()),
                                    );
                                }
                                // DEC-251(c) root cause: an interface method is public, so implementing
                                // it as `private`/`protected` REDUCES visibility — PHP fatals on this,
                                // and it is what let a private method slip through an intersection-typed
                                // receiver (the resolver could find the public interface member first).
                                // Rejecting it here closes the hole at its source.
                                //
                                // SCOPE: only when the class provides a SINGLE overload of `mname`.
                                // `method_vis` records just the first-declared overload's modifiers, so
                                // on an overload SET (e.g. a `private m()` beside a `public m(int)` that
                                // is the one satisfying the interface) it can't tell which overload
                                // conforms — checking the first would false-reject valid code.
                                // KNOWN GAP (tracked, Q-B DV-3 panel — pre-existing, equal for
                                // private/protected/internal): with >1 overload the reduced-visibility
                                // impl is reachable through a plain interface-TYPED receiver
                                // (`Shape s = new Box(); s.m()`) with NO enforcement — an interface is not
                                // in `self.classes`, so the methods.rs access-site check finds no
                                // `method_vis` (⇒ treated public). (The methods.rs backstop covers only
                                // the lone CLASS member of an INTERSECTION type, not a plain interface
                                // receiver.) Closing it needs per-overload conformance tracking; deferred
                                // to a dev ruling (whether a whole overload set must be public). See the
                                // visibility-model spec's PENDING.
                                let overloads = self
                                    .classes
                                    .get(&c.name)
                                    .and_then(|ci| ci.methods.get(mname))
                                    .map_or(0, Vec::len);
                                let impl_vis = self
                                    .classes
                                    .get(&c.name)
                                    .and_then(|ci| ci.method_vis.get(mname).map(|(v, _)| *v));
                                if overloads == 1
                                    && matches!(
                                        impl_vis,
                                        Some(MemberVis::Private)
                                            | Some(MemberVis::Protected)
                                            | Some(MemberVis::Internal)
                                    )
                                {
                                    // Q-B DV-3: `internal` is ALSO a reduction below the public
                                    // interface contract — else the member-`internal` boundary is
                                    // bypassable by upcasting to the interface (the concrete-call path
                                    // does enforce it, so the two would disagree).
                                    let kind = match impl_vis {
                                        Some(MemberVis::Private) => "private",
                                        Some(MemberVis::Internal) => "internal",
                                        _ => "protected",
                                    };
                                    self.err_coded(
                                        c.span,
                                        format!(
                                            "class `{}` implements interface `{iface}`'s method `{mname}` as {kind}, but an interface method is public — reducing its visibility is not allowed",
                                            c.name
                                        ),
                                        "E-IFACE-VIS",
                                        Some(format!("make `{mname}` public on `{}`", c.name)),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The methods PHP declares `final` on `Exception` (DEC-367). [Verified against php-8.5.8 by
/// reflection — `(new ReflectionClass('Exception'))->getMethods()` filtered on `isFinal()` — rather than
/// from memory.] `__construct` and `__toString` are NOT final and stay overridable, which is what lets a
/// throwable carry its own constructor and `#[ToString]`.
const FINAL_EXCEPTION_METHODS: &[&str] = &[
    "getMessage",
    "getCode",
    "getFile",
    "getLine",
    "getTrace",
    "getPrevious",
    "getTraceAsString",
];
