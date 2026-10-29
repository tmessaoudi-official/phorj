//! Collection pass — interface flattening and the nominal subtype / assignability oracle. Split from
//! `inherit.rs` (Invariant 13), which keeps the member merging.

use super::*;

impl Checker {
    /// True if `name`'s `extends` chain reaches `name` again (a cycle). Visited-guarded.
    pub(in crate::checker) fn iface_in_cycle(
        &self,
        name: &str,
        stack: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        fn walk(
            this: &Checker,
            cur: &str,
            target: &str,
            seen: &mut std::collections::BTreeSet<String>,
        ) -> bool {
            let Some(info) = this.interfaces.get(cur) else {
                return false;
            };
            for parent in &info.extends {
                if parent == target {
                    return true;
                }
                if seen.insert(parent.clone()) && walk(this, parent, target, seen) {
                    return true;
                }
            }
            false
        }
        walk(self, name, name, stack)
    }

    /// An interface's flattened method set: its own methods plus every (transitive) parent's,
    /// the child's signature winning on a name clash. Cycle-guarded.
    pub(in crate::checker) fn iface_flat_methods(
        &self,
        name: &str,
    ) -> Vec<(String, (Vec<Ty>, Ty))> {
        let mut acc: HashMap<String, (Vec<Ty>, Ty)> = HashMap::new();
        let mut seen = std::collections::BTreeSet::new();
        self.iface_collect_methods(name, &mut acc, &mut seen);
        let mut out: Vec<_> = acc.into_iter().collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub(in crate::checker) fn iface_collect_methods(
        &self,
        name: &str,
        acc: &mut HashMap<String, (Vec<Ty>, Ty)>,
        seen: &mut std::collections::BTreeSet<String>,
    ) {
        if !seen.insert(name.to_string()) {
            return;
        }
        let Some(info) = self.interfaces.get(name) else {
            return;
        };
        // Parents first, so a child interface's own signature overrides on a clash.
        for parent in &info.extends {
            self.iface_collect_methods(parent, acc, seen);
        }
        for (m, sig) in &info.methods {
            acc.insert(m.clone(), (sig.params.clone(), sig.ret.clone()));
        }
    }

    /// A class method conforms to an interface signature when arities match and each parameter type
    /// and the return type are equal (exact — no variance this slice, matching `assignable`'s
    /// function rule).
    /// `have` is the class's overload set for the method name (M-RT overloading): conformance holds
    /// when *any* overload matches the interface signature exactly.
    pub(in crate::checker) fn sig_conforms(&self, have: &[FnSig], want: &(Vec<Ty>, Ty)) -> bool {
        have.iter().any(|h| Self::one_sig_conforms(h, want))
    }

    /// Whether ONE overload matches the interface signature exactly — extracted from
    /// [`Self::sig_conforms`] so DEC-379's conformance check can ask *which* overload matches, not just
    /// whether any does. Single-sourced deliberately: two copies of this predicate could drift, and the
    /// visibility rule would then enforce against a different overload than the one conformance blessed.
    pub(in crate::checker) fn one_sig_conforms(h: &FnSig, want: &(Vec<Ty>, Ty)) -> bool {
        h.params.len() == want.0.len()
            && h.params.iter().zip(&want.0).all(|(a, b)| a == b)
            && h.ret == want.1
    }

    /// Nominal subtyping for assignability and `instanceof`: `a` is a subtype of `b` when they are
    /// equal, when class `a` implements interface `b` (transitively, via [`Self::class_implements`]),
    /// when class `a` extends class `b` (transitively, via [`Self::class_supertypes`] — M-RT S6), or
    /// when interface `a` extends interface `b` (transitively).
    pub(in crate::checker) fn is_subtype(&self, a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }
        if self
            .class_implements
            .get(a)
            .is_some_and(|ifaces| ifaces.iter().any(|i| i == b))
        {
            return true;
        }
        // class `a` extends class `b` transitively (M-RT S6)?
        if self
            .class_supertypes
            .get(a)
            .is_some_and(|sup| sup.iter().any(|s| s == b))
        {
            return true;
        }
        // interface `a` extends `b` transitively?
        if self.interfaces.contains_key(a) {
            let mut seen = std::collections::BTreeSet::new();
            return self.iface_in_cycle_to(a, b, &mut seen);
        }
        false
    }

    pub(in crate::checker) fn iface_in_cycle_to(
        &self,
        cur: &str,
        target: &str,
        seen: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        let Some(info) = self.interfaces.get(cur) else {
            return false;
        };
        for parent in &info.extends {
            if parent == target {
                return true;
            }
            if seen.insert(parent.clone()) && self.iface_in_cycle_to(parent, target, seen) {
                return true;
            }
        }
        false
    }

    /// Context-aware assignability: [`Ty::assignable`] plus this checker's nominal subtyping.
    pub(in crate::checker) fn ty_assignable(&self, from: &Ty, to: &Ty) -> bool {
        // DEC-211: a BOUNDED type parameter is assignable to its bound interface (and anything the
        // bound is assignable to) — so `T` where `T: Comparable` satisfies a `Comparable` parameter.
        // The same-param `T <: T` case falls through to the structural path below.
        if let Ty::Param(p) = from {
            if let Some((_, iface)) = self.active_type_param_bounds.iter().find(|(n, _)| n == p) {
                if self.ty_assignable(&Ty::Named(iface.clone(), Vec::new()), to) {
                    return true;
                }
            }
        }
        // DEC-257 generic interfaces: a class instance flowing into a PARAMETERIZED interface type
        // (`Ints` → `Producer<int>`) must match the arguments it implements the interface with —
        // the name-only `subtype` closure below can't see them, and would wrongly accept
        // `Ints` → `Producer<string>`. Invariant per argument (`Ty::Error` is the wildcard, same
        // rule as same-head generics); the class's own type parameters in the recorded arguments
        // are substituted from the instance (`Boxed<int>` → `Producer<int>`). A class whose
        // generic-implements is only INHERITED from a parent has no recorded entry and falls
        // through to the name-based path (documented deferral — direct implementors only).
        if let (Ty::Named(fc, fargs), Ty::Named(tn, targs)) = (from, to) {
            if !targs.is_empty() && fc != tn && self.interfaces.contains_key(tn) {
                if let Some(ci) = self.classes.get(fc) {
                    if let Some(decl_args) = ci.iface_args.get(tn) {
                        let theta: HashMap<String, Ty> = ci
                            .type_params
                            .iter()
                            .cloned()
                            .zip(fargs.iter().cloned())
                            .collect();
                        return decl_args.len() == targs.len()
                            && decl_args.iter().zip(targs).all(|(da, ta)| {
                                let da = crate::checker::common::apply_subst(da, &theta);
                                da == *ta || da == Ty::Error || *ta == Ty::Error
                            });
                    }
                }
            }
        }
        Ty::assignable_with(from, to, &|a, b| self.is_subtype(a, b))
    }
}
