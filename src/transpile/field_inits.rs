//! PHP transpiler — where each class's expression field initializers land in PHP (plan row 4c item 4).
//! The SET is `ast::field_inits`, shared with both native backends; this file decides only which
//! `__construct` carries it, because PHP has no field-initializer expressions and no auto-chaining.

use super::*;

/// The precomputed tables (the member emitter has no `program`).
#[derive(Default)]
pub(super) struct Composition {
    /// `(class, static field) → owning class` (row 4c item 2): `C.f` emits the one slot `Owner::$f`
    /// (for a multi-parent child, `M::$f` would be M's own copy of a decomposed parent's trait static).
    pub(super) static_owners: std::collections::BTreeMap<(String, String), String>,
    /// Each class's initializer set (its traits' then its own) and each trait's own, for the
    /// `__construct` prelude a class or a trait constructor emits (`ast::field_inits`).
    pub(super) init_sets: HashMap<String, Vec<(String, Expr)>>,
    /// The user traits: a trait never synthesizes an init-only `__construct` (its user carries them).
    pub(super) trait_names: HashSet<String>,
    /// Classes with no constructor of their own that compose a trait constructor AND have other
    /// initializers: their `__construct` is emitted from the plan (`emit_synth_construct`).
    pub(super) trait_ctor_route: HashSet<String>,
    /// Every class with no constructor of its own that takes a trait constructor: it never
    /// synthesizes a zero-arg `__construct`, which would override that constructor (routed or not).
    pub(super) trait_ctor_users: HashSet<String>,
}

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
                        self.composition.trait_ctor_users.insert(c.name.clone());
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
                            self.composition.trait_ctor_route.insert(c.name.clone());
                        }
                    }
                    self.composition.init_sets.insert(c.name.clone(), set);
                }
                Item::Trait(t) => {
                    self.composition.trait_names.insert(t.name.clone());
                    self.composition
                        .init_sets
                        .insert(t.name.clone(), crate::ast::own_initializers(&t.members));
                }
                _ => {}
            }
        }
    }

    /// Feature B's zero-arg `__construct` for a class with initializers and no constructor (moved from
    /// `classes.rs`, Invariant 13).
    pub(super) fn emit_init_only_construct(
        &mut self,
        c: &ClassDecl,
        as_trait: bool,
    ) -> Result<(), String> {
        // Feature B: a class with expression field initializers but NO constructor needs a synthesized
        // zero-arg `__construct` to run them (PHP property defaults can't be arbitrary expressions). Not
        // for a decomposed trait body (`as_trait`) — its construction is emitted via `emit_synth_construct`.
        // Plan row 4c item 4: never on a user trait — its initializers belong to the using class's set,
        // so two traits cannot collide on `__construct` and a user's own constructor cannot drop them;
        // never on a class that takes a trait constructor (`trait_ctor_users`; the routed ones emit theirs
        // from the plan).
        if !as_trait
            && !self.composition.trait_names.contains(&c.name)
            && !self.composition.trait_ctor_users.contains(&c.name)
            && !c
                .members
                .iter()
                .any(|m| matches!(m, ClassMember::Constructor { .. }))
        {
            let field_inits = self.init_set(c);
            if !field_inits.is_empty() {
                self.line("function __construct() {");
                self.indent += 1;
                self.push_scope();
                for (fname, init) in &field_inits {
                    let e = self.emit_expr(init)?;
                    self.line(&format!("$this->{fname} = {e};"));
                }
                self.pop_scope();
                self.indent -= 1;
                self.line("}");
            }
        }
        Ok(())
    }

    /// The initializer set a class's (or a user trait's real) constructor prelude runs.
    pub(super) fn init_set(&self, c: &ClassDecl) -> Vec<(String, Expr)> {
        self.composition
            .init_sets
            .get(&c.name)
            .cloned()
            .unwrap_or_else(|| crate::ast::own_field_initializers(c))
    }
}

/// True when the static `cls.field` (declared on the class or a trait it uses) is not publicly
/// writable, so `__phorj_init_statics` must set it from the class's scope (row 4c close panel, safety
/// F6: `C::$c = …` from a global function is a PHP error for a `protected`/`private` static).
pub(super) fn static_is_restricted(program: &Program, cls: &str, field: &str) -> bool {
    let restricted = |members: &[ClassMember]| {
        members.iter().any(|m| {
            matches!(m, ClassMember::Field { name, modifiers, .. }
                if name == field
                    && modifiers.iter().any(|md| matches!(
                        md,
                        Modifier::Private | Modifier::Protected | Modifier::PrivateSet | Modifier::ProtectedSet
                    )))
        })
    };
    program.items.iter().any(|it| match it {
        Item::Class(c) if c.name == cls => {
            restricted(&c.members)
                || c.uses.iter().any(|u| {
                    program.items.iter().any(
                |t| matches!(t, Item::Trait(td) if td.name == u.name && restricted(&td.members)),
            )
                })
        }
        _ => false,
    })
}
