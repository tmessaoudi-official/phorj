//! PHP transpiler — where each class's expression field initializers land in PHP (plan row 4c item 4).
//! The SET is `ast::field_inits`, shared with both native backends; this file decides only which
//! `__construct` carries it, because PHP has no field-initializer expressions and no auto-chaining.

use super::*;

impl Transpiler {
    /// Precompute the initializer sets and the trait-constructor route before any class is emitted
    /// (the member emitter has no `program`).
    pub(super) fn seed_init_sets(&mut self, program: &Program) {
        for it in &program.items {
            match it {
                Item::Class(c) => {
                    let set = crate::ast::own_field_initializers_with_traits(program, c);
                    let has_ctor = c
                        .members
                        .iter()
                        .any(|m| matches!(m, ClassMember::Constructor { .. }));
                    // A class with no constructor of its own takes a trait constructor, whose PHP
                    // `__construct` runs only that trait's initializers; when the class's set holds
                    // more, the class emits its own `__construct` from the plan instead.
                    if !has_ctor && crate::ast::uses_trait_ctor(program, c) {
                        self.trait_ctor_users.insert(c.name.clone());
                        let ctor_trait_inits: usize = c
                            .uses
                            .iter()
                            .filter_map(|u| {
                                program.items.iter().find_map(|t| match t {
                                    Item::Trait(td)
                                        if td.name == u.name
                                            && td.members.iter().any(|m| {
                                                matches!(m, ClassMember::Constructor { .. })
                                            }) =>
                                    {
                                        Some(td)
                                    }
                                    _ => None,
                                })
                            })
                            .map(|td| crate::ast::own_initializers(&td.members).len())
                            .sum();
                        if set.len() > ctor_trait_inits {
                            self.trait_ctor_route.insert(c.name.clone());
                        }
                    }
                    self.init_sets.insert(c.name.clone(), set);
                }
                Item::Trait(t) => {
                    self.trait_names.insert(t.name.clone());
                    self.init_sets
                        .insert(t.name.clone(), crate::ast::own_initializers(&t.members));
                }
                _ => {}
            }
        }
    }

    /// The initializer set a class's (or a user trait's real) constructor prelude runs.
    pub(super) fn init_set(&self, c: &ClassDecl) -> Vec<(String, Expr)> {
        self.init_sets
            .get(&c.name)
            .cloned()
            .unwrap_or_else(|| crate::ast::own_field_initializers(c))
    }
}
