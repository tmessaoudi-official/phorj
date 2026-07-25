//! Collection pass — class `extends` graph, `rename` resolutions, and multiple-inheritance conflicts.

use super::*;

impl Checker {
    /// Class `extends` targets must be `open` classes; detect cycles (M-RT S6).
    pub(in crate::checker) fn check_class_extends(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        let class_open: std::collections::HashMap<&str, bool> = program
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Class(c) => Some((c.name.as_str(), c.open)),
                _ => None,
            })
            .collect();
        for item in &program.items {
            if let Item::Class(c) = item {
                if self
                    .class_supertypes
                    .get(&c.name)
                    .is_some_and(|s| s.contains(&c.name))
                {
                    self.err_coded(
                        c.span,
                        format!("class `{}` is part of an `extends` cycle", c.name),
                        "E-MI-CYCLE",
                        Some("a class may not extend itself transitively".into()),
                    );
                    continue; // skip per-parent checks for a cyclic class (avoids noise)
                }
                for parent in &c.extends {
                    if !self.classes.contains_key(parent) {
                        self.err_coded(
                            c.span,
                            format!(
                                "class `{}` extends `{parent}`, which is not a class",
                                c.name
                            ),
                            "E-EXTEND-UNKNOWN",
                            Some(
                                "`extends` lists parent classes; use `implements` for interfaces"
                                    .into(),
                            ),
                        );
                    } else if !class_open.get(parent.as_str()).copied().unwrap_or(false) {
                        self.err_coded(
                            c.span,
                            format!(
                                "class `{}` cannot extend `{parent}`, which is not `open`",
                                c.name
                            ),
                            "E-EXTEND-FINAL",
                            Some(format!(
                                "mark the parent `open class {parent}` to allow extension"
                            )),
                        );
                    }
                }
            }
        }
    }

    /// M-RT S6b: a `rename P.m as n` clause exposes parent `P`'s method `m` on the child under the
    /// new name `n`, so a `child.n()` call type-checks (the backends dispatch it via the shared
    /// origin table). `use`/`exclude` keep method names unchanged, so they need no signature edit.
    pub(in crate::checker) fn apply_rename_resolutions(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        for item in &program.items {
            if let Item::Class(c) = item {
                for r in &c.resolutions {
                    if let crate::ast::Resolution::Rename {
                        parent,
                        method,
                        as_name,
                        ..
                    } = r
                    {
                        if let Some(sigs) = self
                            .classes
                            .get(parent)
                            .and_then(|p| p.methods.get(method))
                            .cloned()
                        {
                            if let Some(child) = self.classes.get_mut(&c.name) {
                                child.methods.entry(as_name.clone()).or_insert(sigs);
                            }
                        }
                    }
                }
            }
        }
    }

    /// M-RT S6b: an unresolved cross-parent method collision is `E-MI-CONFLICT`. The shared origin
    /// resolver returns every name a class inherits from ≥2 distinct parents without a `use`/
    /// `rename`/`exclude` clause (or own override) to disambiguate. A clean program produces an
    /// empty list; the backends then dispatch through the same resolved table.
    pub(in crate::checker) fn check_mi_method_conflicts(&mut self, program: &crate::ast::Program) {
        let (_, conflicts) = crate::ast::class_method_origins(program);
        for (class, name, span) in conflicts {
            self.err_coded(
                span,
                format!(
                    "method `{name}` is inherited from more than one parent of class `{class}`"
                ),
                "E-MI-CONFLICT",
                Some(format!(
                    "resolve it: `use P.{name}` to pick a parent, `rename P.{name} as <new>` to keep \
                     both, `exclude P.{name}` to drop one, or override `function {name}(…)` in `{class}`"
                )),
            );
        }
    }

    /// M-RT S6c.1: a same-named instance field inherited from ≥2 distinct parents is
    /// `E-MI-FIELD-CONFLICT`. PHP has no `insteadof` for properties, so unlike a method collision
    /// it can be resolved *only* by the child redeclaring the field (or renaming it in a parent).
    /// A diamond-shared field (both arms reach the same declaring class) auto-merges, like methods.
    pub(in crate::checker) fn check_mi_field_conflicts(&mut self, program: &crate::ast::Program) {
        for (class, name, span) in crate::ast::class_field_conflicts(program) {
            self.err_coded(
                span,
                format!("field `{name}` is inherited from more than one parent of class `{class}`"),
                "E-MI-FIELD-CONFLICT",
                Some(format!(
                    "PHP has no `insteadof` for properties — redeclare `{name}` in `{class}` (or \
                     rename it in a parent) to resolve the collision"
                )),
            );
        }
    }
}
