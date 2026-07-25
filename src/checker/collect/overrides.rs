//! Collection pass — method-override checks (`open`-required + return-covariance / param-contravariance).

use super::*;

impl Checker {
    /// M-RT S6: a method that overrides an ancestor's method requires that ancestor's method to be
    /// `open` (final-by-default), else `E-OVERRIDE-FINAL`; plus DEC-251(a) return-covariance /
    /// parameter-contravariance (`E-OVERRIDE-SIG`). `method_open[(class, name)]` is true if the class
    /// declares that name with at least one `open` overload.
    pub(in crate::checker) fn check_method_overrides(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        let mut method_open: std::collections::HashMap<(String, String), bool> =
            std::collections::HashMap::new();
        // Shared method-resolution order (nearest-first BFS over *every* parent) — the same table the
        // backends dispatch through, so the override check sees the exact ancestor a call would (M-RT
        // S6b: multi-parent, not just the first-parent chain).
        let mro = crate::ast::class_mro(program);
        for item in &program.items {
            if let Item::Class(c) = item {
                for m in &c.members {
                    if let crate::ast::ClassMember::Method(f) = m {
                        // An `abstract` method is implicitly `open` (it exists to be implemented).
                        let is_open = f.modifiers.contains(&crate::ast::Modifier::Open)
                            || f.modifiers.contains(&crate::ast::Modifier::Abstract);
                        method_open
                            .entry((c.name.clone(), f.name.clone()))
                            .and_modify(|v| *v = *v || is_open)
                            .or_insert(is_open);
                    }
                }
            }
        }
        for item in &program.items {
            if let Item::Class(c) = item {
                let mut checked: std::collections::BTreeSet<&str> =
                    std::collections::BTreeSet::new();
                for m in &c.members {
                    let crate::ast::ClassMember::Method(f) = m else {
                        continue;
                    };
                    if !checked.insert(f.name.as_str()) {
                        continue; // one diagnostic per overridden name
                    }
                    // Nearest ancestor (across every parent, nearest-first) that declares this name.
                    for anc in mro.get(&c.name).into_iter().flatten() {
                        if let Some(&open) = method_open.get(&(anc.clone(), f.name.clone())) {
                            if !open {
                                self.err_coded(
                                    f.span,
                                    format!(
                                        "method `{}` overrides `{anc}`'s `{}`, which is not `open`",
                                        f.name, f.name
                                    ),
                                    "E-OVERRIDE-FINAL",
                                    Some(format!(
                                        "mark it `open function {}(…)` on `{anc}` to allow overriding",
                                        f.name
                                    )),
                                );
                            }
                            // M-DX S1 (soundness hole B): an override's return type must be a subtype
                            // of the overridden one (covariance). A wider/unrelated return used to
                            // type-check clean, then store a wrong-typed value on the Rust backends —
                            // and *fatal* in transpiled PHP (`Sub::k(): string` vs `Base::k(): int`).
                            // Scoped to the simple case: single (non-overloaded), non-generic
                            // signatures on both sides. Parameter contravariance and overloaded/
                            // generic overrides remain documented deferrals (KNOWN_ISSUES).
                            let sigs = {
                                let child = self
                                    .classes
                                    .get(&c.name)
                                    .and_then(|ci| ci.methods.get(&f.name));
                                let parent =
                                    self.classes.get(anc).and_then(|ci| ci.methods.get(&f.name));
                                match (child, parent) {
                                    (Some(cs), Some(ps))
                                        if cs.len() == 1
                                            && ps.len() == 1
                                            && cs[0].type_params.is_empty()
                                            && ps[0].type_params.is_empty() =>
                                    {
                                        Some((cs[0].clone(), ps[0].clone()))
                                    }
                                    _ => None,
                                }
                            };
                            if let Some((child_sig, parent_sig)) = sigs {
                                let (child_ret, parent_ret) = (&child_sig.ret, &parent_sig.ret);
                                if !self.ty_assignable(child_ret, parent_ret) {
                                    self.err_coded(
                                        f.span,
                                        format!(
                                            "method `{}` overrides `{anc}`'s `{}` but returns \
                                             `{child_ret}`, which is not assignable to the \
                                             overridden return type `{parent_ret}`",
                                            f.name, f.name
                                        ),
                                        "E-OVERRIDE-SIG",
                                        Some(format!(
                                            "make `{}`'s return type `{parent_ret}` or a subtype of it",
                                            f.name
                                        )),
                                    );
                                }
                                // DEC-251(a): parameter types are CONTRAVARIANT — an override may WIDEN
                                // a parameter (accept a supertype) but NARROWING it is unsound and
                                // *transpile-fatal* (PHP "Declaration must be compatible"). The sound,
                                // PHP-compatible rule (META-7: Kotlin/C# invariant, PHP contravariant):
                                // the parent's param type must be assignable TO the child's at each
                                // position. Same-arity simple case only (mirrors the return check's
                                // scope; overloaded/generic/default-arity-diff overrides stay deferred).
                                if child_sig.params.len() == parent_sig.params.len() {
                                    for (i, (cp, pp)) in child_sig
                                        .params
                                        .iter()
                                        .zip(parent_sig.params.iter())
                                        .enumerate()
                                    {
                                        if !self.ty_assignable(pp, cp) {
                                            self.err_coded(
                                                f.span,
                                                format!(
                                                    "method `{}` overrides `{anc}`'s `{}` but narrows \
                                                     parameter {} to `{cp}`, which the overridden \
                                                     parameter type `{pp}` is not assignable to \
                                                     (parameters are contravariant — a narrower \
                                                     parameter is unsound and fatal in transpiled PHP)",
                                                    f.name,
                                                    f.name,
                                                    i + 1
                                                ),
                                                "E-OVERRIDE-SIG",
                                                Some(format!(
                                                    "make parameter {}'s type `{pp}` or a supertype of it",
                                                    i + 1
                                                )),
                                            );
                                        }
                                    }
                                }
                            }
                            break; // the nearest declaration decides
                        }
                    }
                }
            }
        }
    }
}
