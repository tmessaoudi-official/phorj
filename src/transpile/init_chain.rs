//! PHP leg of the initializer chain (DEC-575, DEC-585, DEC-586; plan
//! `docs/plans/2026-11-18-init-chain.plan.md`). PHP has no field-initializer expressions and never
//! chains constructors, so:
//! - each class with initializers of its own (its traits' then its own) emits `protected function
//!   __phorj_init_<Class>(): void`, assigning them in the class's own scope, where a `private` field is
//!   reachable (a decomposed multi-parent ancestor emits it in its `T<name>` trait);
//! - every class of a hierarchy that initializes anything emits `protected function __phorj_inits():
//!   void`, calling its chain (`ast::init_chain_classes`) in order;
//! - every constructor prelude of such a hierarchy calls `$this->__phorj_inits()`. The call is virtual,
//!   so a constructor a subclass inherits still runs the SUBCLASS's chain, after promotion and before the
//!   body: the native order (`ast::field_initializers`).

use super::*;

/// The precomputed chain tables (the member emitter has no `program`).
#[derive(Default)]
pub(super) struct InitChain {
    /// Per class: the classes of its chain that have a set of their own, in call order.
    calls: HashMap<String, Vec<String>>,
    /// Classes and traits of a hierarchy in which some chain is non-empty (joined through `extends` and
    /// through every trait a class uses): they define `__phorj_inits` and their preludes call it. Only a
    /// trait constructor needs the trait join (its prelude is shared by every user); through a trait with
    /// no constructor it only adds an inert empty `__phorj_inits()` and its call.
    needing: HashSet<String>,
    /// Classes whose constructor plan is empty (`ast::ctor_plan`): only they synthesize a zero-arg
    /// `__construct`; any other class inherits or declares one that takes arguments.
    plan_empty: HashSet<String>,
}

/// The PHP method holding `class`'s own initializers (a qualified name keeps two packages apart).
pub(super) fn init_method(class: &str) -> String {
    let safe: String = class
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    format!("__phorj_init_{safe}")
}

fn root(links: &mut HashMap<String, String>, x: &str) -> String {
    let mut cur = x.to_string();
    while let Some(next) = links.get(&cur).cloned() {
        if next == cur {
            break;
        }
        cur = next;
    }
    links.insert(x.to_string(), cur.clone());
    cur
}

fn join(links: &mut HashMap<String, String>, a: &str, b: &str) {
    let (ra, rb) = (root(links, a), root(links, b));
    if ra != rb {
        links.insert(ra, rb);
    }
}

impl Transpiler {
    /// Precompute the chain tables before any class is emitted.
    pub(super) fn seed_init_chain(&mut self, program: &Program) {
        let mut links: HashMap<String, String> = HashMap::new();
        let mut members: Vec<String> = Vec::new();
        for it in &program.items {
            match it {
                Item::Class(c) if !c.foreign => {
                    members.push(c.name.clone());
                    root(&mut links, &c.name);
                    for p in &c.extends {
                        join(&mut links, &c.name, p);
                    }
                    for u in &c.uses {
                        join(&mut links, &c.name, &u.name);
                    }
                    if crate::ast::ctor_plan(program, &c.name).is_empty() {
                        self.init_chain.plan_empty.insert(c.name.clone());
                    }
                    let calls: Vec<String> = crate::ast::init_chain_classes(program, &c.name)
                        .into_iter()
                        .filter(|k| {
                            self.composition
                                .init_sets
                                .get(k)
                                .is_some_and(|s| !s.is_empty())
                        })
                        .collect();
                    self.init_chain.calls.insert(c.name.clone(), calls);
                }
                Item::Trait(t) => {
                    members.push(t.name.clone());
                    root(&mut links, &t.name);
                }
                _ => {}
            }
        }
        let live: HashSet<String> = self
            .init_chain
            .calls
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| root(&mut links, k))
            .collect();
        for m in members {
            if live.contains(&root(&mut links, &m)) {
                self.init_chain.needing.insert(m);
            }
        }
    }

    /// Whether `name`'s constructor preludes call `$this->__phorj_inits()`.
    pub(super) fn inits_needed(&self, name: &str) -> bool {
        self.init_chain.needing.contains(name)
    }

    /// The prelude call (nothing outside an initializing hierarchy).
    pub(super) fn emit_inits_call(&mut self, name: &str) {
        if self.inits_needed(name) {
            self.line("$this->__phorj_inits();");
        }
    }

    /// `__phorj_init_<C>` (C's own set; also inside a decomposed `T<C>` trait) and, unless `as_trait`,
    /// the `__phorj_inits` dispatcher. A user trait emits neither: its set belongs to each user's.
    pub(super) fn emit_init_methods(
        &mut self,
        c: &ClassDecl,
        as_trait: bool,
    ) -> Result<(), String> {
        if self.composition.trait_names.contains(&c.name) {
            return Ok(());
        }
        let own = self.init_set(c);
        if !own.is_empty() {
            self.line(&format!(
                "protected function {}(): void {{",
                init_method(&c.name)
            ));
            self.indent += 1;
            self.push_scope();
            for (fname, init) in &own {
                let e = self.emit_expr(init)?;
                self.line(&format!("$this->{fname} = {e};"));
            }
            self.pop_scope();
            self.indent -= 1;
            self.line("}");
        }
        if !as_trait {
            self.emit_inits_dispatch(&c.name);
        }
        Ok(())
    }

    /// `protected function __phorj_inits(): void` for `class` (a decomposed concrete class calls this
    /// directly, its members having gone to its trait).
    pub(super) fn emit_inits_dispatch(&mut self, class: &str) {
        if !self.inits_needed(class) {
            return;
        }
        let calls = self
            .init_chain
            .calls
            .get(class)
            .cloned()
            .unwrap_or_default();
        if calls.is_empty() {
            self.line("protected function __phorj_inits(): void {}");
            return;
        }
        self.line("protected function __phorj_inits(): void {");
        self.indent += 1;
        for k in &calls {
            self.line(&format!("$this->{}();", init_method(k)));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// A class with an empty constructor plan in an initializing hierarchy synthesizes a zero-arg
    /// `__construct` that runs its chain. Never for a trait body or a user trait, never over a declared
    /// or trait-composed constructor, and never when the plan takes arguments (an inherited constructor
    /// already calls the dispatcher; a zero-arg one would override it: KNOWN_ISSUES INIT-CHAIN).
    pub(super) fn emit_init_only_construct(
        &mut self,
        c: &ClassDecl,
        as_trait: bool,
    ) -> Result<(), String> {
        let declares = c
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor { .. }));
        // A multi-parent class gets its `__construct` from `emit_synth_construct` (`emit_multi_class`).
        if !as_trait
            && c.extends.len() < 2
            && !declares
            && !self.composition.trait_names.contains(&c.name)
            && !self.composition.trait_ctor_users.contains(&c.name)
            && self.init_chain.plan_empty.contains(&c.name)
            && self.inits_needed(&c.name)
        {
            self.line("function __construct() {");
            self.indent += 1;
            self.line("$this->__phorj_inits();");
            self.indent -= 1;
            self.line("}");
        }
        Ok(())
    }
}
