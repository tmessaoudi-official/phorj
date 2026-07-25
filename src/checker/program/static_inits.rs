//! Program pass — static-field initializer type-checking (Feature B-static).

use super::*;

impl Checker {
    /// Feature B-static: type-check each class's static-field initializers (now arbitrary expressions,
    /// not just literals), evaluated once at program start. Checked with **no `this`** (statics are
    /// class-level — referencing `this` errors) and after full collection, so an initializer may call a
    /// function or read another static. A type mismatch is `E-STATIC-INIT-TYPE`.
    pub(in crate::checker) fn check_static_inits(&mut self, program: &crate::ast::Program) {
        use crate::ast::{ClassMember, Item, Modifier};
        let prev = self.cur_class.take();
        let prev_pkg = std::mem::take(&mut self.cur_package);
        // A static initializer runs in its owning class's scope (so it may call that class's
        // `private`/`protected` constructor — the singleton pattern), but there is no instance, so
        // `this` is forbidden via `in_static_init` (Batch A).
        self.in_static_init = true;
        for item in &program.items {
            let Item::Class(c) = item else { continue };
            self.cur_class = Some(c.name.clone());
            // Q-B DV-3: a static initializer runs in its class's package (for `internal` gating).
            self.cur_package = Self::pkg_of_mangled(&c.name).to_string();
            for m in &c.members {
                if let ClassMember::Field {
                    modifiers,
                    ty,
                    name,
                    init: Some(e),
                    ..
                } = m
                {
                    if modifiers.contains(&Modifier::Static)
                        && !modifiers.contains(&Modifier::Const)
                    {
                        let fty = self.resolve_type(ty);
                        let ity = self.check_expr(e);
                        if !self.ty_assignable(&ity, &fty) {
                            self.err_coded(
                                Self::expr_span(e),
                                format!("static field `{name}: {fty}` initialized with `{ity}`"),
                                "E-STATIC-INIT-TYPE",
                                None,
                            );
                        }
                    }
                }
            }
        }
        self.in_static_init = false;
        self.cur_class = prev;
        self.cur_package = prev_pkg;
    }
}
