//! PHP transpiler — synthetic construction, MI decomposition (interface+trait), interfaces,
//! and the parent-method-call scan.

use super::program_emit::ParentAliasMap;
use super::*;

impl Transpiler {
    /// M-RT S6b: emit a class that is an ancestor of some multi-parent class as the interface+trait
    /// decomposition PHP needs for multiple inheritance — `interface I<name>` (the type side, so a
    /// subtype is `instanceof` it), `trait T<name>` (the impl side, `use`d by subclasses), and a
    /// concrete `class <name> implements I<name> { use T<name>; }` so the class is still directly
    /// instantiable and single-`extends`able. An ancestor's own parents are decomposed too, so the
    /// interface `extends I<parent>` and the trait `use T<parent>` (which is how a diamond shared base
    /// auto-merges — both arms reach the same flattened trait method).
    /// M-RT S6c.2b: emit an explicit-assignment `__construct` from a class's constructor *plan*
    /// (`ast::ctor_plan`) — used where promotion cannot be (a decomposed concrete class and a
    /// multi-parent subclass, whose fields live in `use`d traits as plain properties). Params are the
    /// plan entries' params concatenated; the body sets each promoted param (`$this->p = $p;`) then runs
    /// each entry's body, in order — mirroring the interpreter's per-entry promote-then-body and the
    /// VM's `MakeInstance`-then-bodies. Emits nothing for an empty plan (a zero-arg class).
    pub(super) fn emit_synth_construct(
        &mut self,
        c: &ClassDecl,
        program: &Program,
    ) -> Result<(), String> {
        let plan = crate::ast::ctor_plan(program, &c.name);
        if plan.is_empty() {
            return Ok(());
        }
        let params: Vec<String> = plan
            .iter()
            .flat_map(|(ps, _)| ps.iter())
            .map(|p| format!("{} ${}", self.emit_type(&p.ty), p.name))
            .collect();
        self.line(&format!(
            "public function __construct({}) {{",
            params.join(", ")
        ));
        self.indent += 1;
        self.push_scope();
        for (ps, _) in &plan {
            for p in ps {
                self.declare(&p.name);
            }
        }
        for (ps, body) in &plan {
            for p in ps {
                if is_promoted(&p.modifiers) {
                    self.line(&format!("$this->{0} = ${0};", p.name));
                }
            }
            for s in body {
                self.emit_stmt(s)?;
            }
        }
        self.pop_scope();
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn emit_decomposed_class(
        &mut self,
        c: &ClassDecl,
        program: &Program,
    ) -> Result<(), String> {
        // interface I<name> [extends I<parent>, …] { method signatures }
        let iparents: Vec<String> = c.extends.iter().map(|p| format!("I{p}")).collect();
        let iext = if iparents.is_empty() {
            String::new()
        } else {
            format!(" extends {}", iparents.join(", "))
        };
        self.line(&format!("interface I{}{} {{", c.name, iext));
        self.indent += 1;
        let mut sig_emitted: HashSet<String> = HashSet::new();
        for m in &c.members {
            if let ClassMember::Method(f) = m {
                // One signature per name (a PHP interface cannot redeclare a name; overload sets in a
                // decomposed class are rare and resolved by the trait body).
                if !sig_emitted.insert(f.name.clone()) {
                    continue;
                }
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{} ${}", self.emit_type(&p.ty), p.name))
                    .collect();
                self.line(&format!(
                    "public function {}({}){};",
                    f.name,
                    params.join(", "),
                    self.ret_suffix(&f.ret)
                ));
            }
        }
        self.indent -= 1;
        self.line("}");

        // trait T<name> { [use T<parent> [{ aliases }], …;] members }
        self.line(&format!("trait T{} {{", c.name));
        self.indent += 1;
        // B2: a decomposed ancestor's own `parent.m(…)` calls (to its direct parent) need the same
        // trait-alias lowering — `parent::` is wrong inside a trait body. The alias clauses ride this
        // trait's `use T<parent>` block.
        let (parent_aliases, alias_clauses) = self.mi_parent_aliases(c, program);
        if !c.extends.is_empty() {
            let tparents: Vec<String> = c.extends.iter().map(|p| format!("T{p}")).collect();
            if alias_clauses.is_empty() {
                self.line(&format!("use {};", tparents.join(", ")));
            } else {
                self.line(&format!("use {} {{", tparents.join(", ")));
                self.indent += 1;
                for cl in &alias_clauses {
                    self.line(cl);
                }
                self.indent -= 1;
                self.line("}");
            }
        }
        let prev_aliases = self.parent_aliases.replace(parent_aliases);
        let (promoted_names, fields, is_error) = self.class_field_context(c);
        let prev = self.cur_class_fields.replace(fields);
        // `as_trait = true`: promoted ctor params become plain fields, the constructor is NOT emitted
        // here (it would be a colliding trait `__construct`).
        self.emit_class_members(c, &promoted_names, is_error, true)?;
        self.cur_class_fields = prev;
        self.indent -= 1;
        self.line("}");

        // concrete class <name> implements I<name> { use T<name>; <explicit __construct> } — directly
        // instantiable + single-`extends`able. The constructor logic the trait dropped lives here as an
        // explicit-assignment ctor (M-RT S6c.2b). `parent_aliases` stays active: a synth ctor body's
        // `parent.m(…)` resolves to the same `private` alias (declared in `use T<name>`).
        self.line(&format!("class {0} implements I{0} {{", c.name));
        self.indent += 1;
        self.line(&format!("use T{};", c.name));
        let prev = self.cur_class_fields.replace(self.class_field_context(c).1);
        self.emit_synth_construct(c, program)?;
        self.cur_class_fields = prev;
        self.indent -= 1;
        self.line("}");
        self.parent_aliases = prev_aliases;
        Ok(())
    }

    /// M-RT S6b: emit a multi-parent class (`class C extends A, B`) as a PHP class that `implements`
    /// each parent's interface and `use`s each parent's trait, with `insteadof`/`as` clauses resolving
    /// cross-parent method collisions (from the `use`/`rename`/`exclude` resolution clauses). A diamond
    /// shared base needs no clause — PHP auto-dedups a method reached identically through two traits.
    pub(super) fn emit_multi_class(
        &mut self,
        c: &ClassDecl,
        program: &Program,
    ) -> Result<(), String> {
        let iparents: Vec<String> = c.extends.iter().map(|p| format!("I{p}")).collect();
        let tparents: Vec<String> = c.extends.iter().map(|p| format!("T{p}")).collect();
        let final_kw = if c.open { "" } else { "final " };
        self.line(&format!(
            "{final_kw}class {} implements {} {{",
            c.name,
            iparents.join(", ")
        ));
        self.indent += 1;
        // B2: trait aliases for any `parent.m(…)`/`parent(A).m(…)` inside this MI class's bodies (no
        // native `parent::` here) — their `T<dp>::m as private …;` clauses join the `insteadof` block.
        let (parent_aliases, alias_clauses) = self.mi_parent_aliases(c, program);
        let mut clauses = self.build_trait_clauses(c, program);
        clauses.extend(alias_clauses);
        if clauses.is_empty() {
            self.line(&format!("use {};", tparents.join(", ")));
        } else {
            self.line(&format!("use {} {{", tparents.join(", ")));
            self.indent += 1;
            for cl in &clauses {
                self.line(cl);
            }
            self.indent -= 1;
            self.line("}");
        }
        let prev_aliases = self.parent_aliases.replace(parent_aliases);
        let (promoted_names, fields, is_error) = self.class_field_context(c);
        let prev = self.cur_class_fields.replace(fields);
        self.emit_class_members(c, &promoted_names, is_error, false)?;
        // M-RT S6c.2b: a multi-parent class with no own constructor gets a synthesized orchestrating
        // `__construct` (explicit assignments + each parent body, from `ctor_plan`); its fields live in
        // the `use`d parent traits. A class that declares its own ctor already emitted it above.
        if !c
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor { .. }))
        {
            self.emit_synth_construct(c, program)?;
        }
        self.cur_class_fields = prev;
        self.parent_aliases = prev_aliases;
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// The `insteadof`/`as` clauses for a multi-parent class's `use` block (M-RT S6b). A method name
    /// supplied by ≥2 direct parents with **distinct origins** is a real PHP trait collision needing
    /// `insteadof` (a diamond shared base — same origin through both arms — is skipped, PHP auto-merges
    /// it). The winner is the parent named by a `use P.m` clause, else the single parent left after
    /// `rename`/`exclude` remove the others; every other providing parent's trait is listed after
    /// `insteadof`. A class that overrides the method itself needs no clause (the class method wins). A
    /// `rename P.m as n` also emits `T<P>::m as n;`.
    pub(super) fn build_trait_clauses(&self, c: &ClassDecl, program: &Program) -> Vec<String> {
        use crate::ast::Resolution;
        let (origins, _conflicts) = crate::ast::class_method_origins(program);
        // method name -> [(direct parent, origin (declaring class, method))]
        let mut provides: std::collections::BTreeMap<String, Vec<(String, Origin)>> =
            std::collections::BTreeMap::new();
        for ((cls, name), origin) in &origins {
            if c.extends.contains(cls) {
                provides
                    .entry(name.clone())
                    .or_default()
                    .push((cls.clone(), origin.clone()));
            }
        }
        let own: std::collections::BTreeSet<&str> = c
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(f) => Some(f.name.as_str()),
                _ => None,
            })
            .collect();
        let mut clauses = Vec::new();
        for (m, entries) in &provides {
            let distinct: std::collections::BTreeSet<&Origin> =
                entries.iter().map(|(_, o)| o).collect();
            if distinct.len() < 2 || own.contains(m.as_str()) {
                continue; // diamond auto-merge, single source, or overridden by the class itself
            }
            let providing: std::collections::BTreeSet<String> =
                entries.iter().map(|(p, _)| p.clone()).collect();
            // The winner: `use P.m` names it; otherwise the one parent left after rename/exclude.
            let used = c.resolutions.iter().find_map(|r| match r {
                Resolution::Use { parent, method, .. } if method == m => Some(parent.clone()),
                _ => None,
            });
            let removed: std::collections::BTreeSet<String> = c
                .resolutions
                .iter()
                .filter_map(|r| match r {
                    Resolution::Rename { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    Resolution::Exclude { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    _ => None,
                })
                .collect();
            let winner = used.or_else(|| providing.iter().find(|p| !removed.contains(*p)).cloned());
            if let Some(w) = winner {
                let losers: Vec<String> = providing
                    .iter()
                    .filter(|p| **p != w)
                    .map(|p| format!("T{p}"))
                    .collect();
                if !losers.is_empty() {
                    clauses.push(format!("T{w}::{m} insteadof {};", losers.join(", ")));
                }
            }
        }
        for r in &c.resolutions {
            if let Resolution::Rename {
                parent,
                method,
                as_name,
                ..
            } = r
            {
                clauses.push(format!("T{parent}::{method} as {as_name};"));
            }
        }
        clauses
    }

    /// The `insteadof`/`as` clauses for an explicit trait-composition (`use P; use Q;`) block when two
    /// composed traits supply the same method name (Wave 1.3). The Phorj-side resolution
    /// (`use P.m`/`rename`/`exclude`) is already validated by the checker; this lowers it to PHP. The
    /// trait-composition analogue of [`build_trait_clauses`] (which handles MI-decomposed parents and
    /// uses `T<parent>` names): here the providing sources are the directly-declared methods of each
    /// `use`d trait, named directly. A method the class overrides itself, or supplied by only one trait,
    /// needs no clause. (A collision via a trait's *own* nested `use` is not detected here — only direct
    /// declarations; that narrower case is caught by the PHP oracle if it ever arises.)
    pub(super) fn build_use_trait_clauses(&self, c: &ClassDecl, program: &Program) -> Vec<String> {
        use crate::ast::{Item, Resolution};
        // Directly-declared method names of a `use`d trait.
        let trait_methods = |name: &str| -> std::collections::BTreeSet<String> {
            program
                .items
                .iter()
                .find_map(|it| match it {
                    Item::Trait(t) if t.name == name => Some(
                        t.members
                            .iter()
                            .filter_map(|m| match m {
                                ClassMember::Method(f) => Some(f.name.clone()),
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default()
        };
        // method name -> set of composed traits supplying it directly.
        let mut provides: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
            std::collections::BTreeMap::new();
        for u in &c.uses {
            for m in trait_methods(&u.name) {
                provides.entry(m).or_default().insert(u.name.clone());
            }
        }
        let own: std::collections::BTreeSet<&str> = c
            .members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Method(f) => Some(f.name.as_str()),
                _ => None,
            })
            .collect();
        let mut clauses = Vec::new();
        for (m, traits) in &provides {
            if traits.len() < 2 || own.contains(m.as_str()) {
                continue; // single source, or the class overrides it (the class method wins)
            }
            // The winner: `use P.m` names it; else the one trait left after `rename`/`exclude`.
            let used = c.resolutions.iter().find_map(|r| match r {
                Resolution::Use { parent, method, .. } if method == m => Some(parent.clone()),
                _ => None,
            });
            let removed: std::collections::BTreeSet<String> = c
                .resolutions
                .iter()
                .filter_map(|r| match r {
                    Resolution::Rename { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    Resolution::Exclude { parent, method, .. } if method == m => {
                        Some(parent.clone())
                    }
                    _ => None,
                })
                .collect();
            let winner = used.or_else(|| traits.iter().find(|p| !removed.contains(*p)).cloned());
            if let Some(w) = winner {
                let losers: Vec<String> = traits.iter().filter(|p| **p != w).cloned().collect();
                if !losers.is_empty() {
                    clauses.push(format!("{w}::{m} insteadof {};", losers.join(", ")));
                }
            }
        }
        for r in &c.resolutions {
            if let Resolution::Rename {
                parent,
                method,
                as_name,
                ..
            } = r
            {
                clauses.push(format!("{parent}::{method} as {as_name};"));
            }
        }
        clauses
    }

    /// B2 — the trait-alias lookup + `use`-block clauses for every **direct-parent** `parent.m(…)` /
    /// `parent(A).m(…)` call in `c`'s bodies, when `c` is emitted as an MI class or a decomposed trait
    /// (PHP has no native `parent::`/`A::` there — the ancestor lives in a `use`d trait). `lookup` keys
    /// each call's `(ancestor-as-written, method)` to its `private` alias; `clauses` are the deduped
    /// `T<dp>::m as private __super_<dp>_<m>;` lines. A call to a non-direct ancestor (a transitive MI
    /// jump) is intentionally absent from `lookup` — the emit arm surfaces it as a transpile error
    /// rather than emitting invalid PHP. Empty when `c` has no parent calls (the common case → the `use`
    /// block is unchanged and existing MI output stays byte-identical).
    pub(super) fn mi_parent_aliases(
        &self,
        c: &ClassDecl,
        program: &Program,
    ) -> (ParentAliasMap, std::collections::BTreeSet<String>) {
        let mut lookup = std::collections::BTreeMap::new();
        let mut clauses = std::collections::BTreeSet::new();
        let calls = collect_parent_method_calls(c);
        if calls.is_empty() {
            return (lookup, clauses);
        }
        let (origins, _) = crate::ast::class_method_origins(program);
        for (ancestor, method) in calls {
            // The direct parent whose trait carries the target method.
            let dp = match &ancestor {
                Some(a) if c.extends.iter().any(|p| p == a) => Some(a.clone()),
                Some(_) => None, // transitive ancestor — not lowerable here
                None => {
                    let providers: Vec<String> = c
                        .extends
                        .iter()
                        .filter(|p| origins.contains_key(&((**p).clone(), method.clone())))
                        .cloned()
                        .collect();
                    // Exactly one direct provider ⇒ that parent; zero/≥2 are checker errors or not
                    // direct (left out so the emit arm reports a clean transpile error).
                    if providers.len() == 1 {
                        providers.into_iter().next()
                    } else {
                        None
                    }
                }
            };
            if let Some(dp) = dp {
                let alias = format!("__super_{dp}_{method}");
                clauses.insert(format!("T{dp}::{method} as private {alias};"));
                lookup.insert((ancestor, method), alias);
            }
        }
        (lookup, clauses)
    }

    /// The `(promoted ctor-param names, instance-field set, is_error)` context a class body needs to
    /// emit its members — shared setup for `emit_class`, `emit_multi_class`, and `emit_decomposed_class`.
    pub(super) fn class_field_context(
        &self,
        c: &ClassDecl,
    ) -> (HashSet<String>, HashSet<String>, bool) {
        let mut promoted_names: HashSet<String> = HashSet::new();
        for m in &c.members {
            if let ClassMember::Constructor { params, .. } = m {
                for p in params {
                    if is_promoted(&p.modifiers) {
                        promoted_names.insert(p.name.clone());
                    }
                }
            }
        }
        let mut fields: HashSet<String> = promoted_names.clone();
        for m in &c.members {
            if let ClassMember::Field { name, .. } = m {
                fields.insert(name.clone());
            }
        }
        let is_error = c.implements.iter().any(|i| last_segment(i) == "Error");
        (promoted_names, fields, is_error)
    }

    /// Emit a PHP `interface` (M-RT S2): the name, an optional `extends A, B` clause, and one
    /// abstract method signature per declared method (`public function name(params): ret;`). PHP
    /// interface methods are implicitly public + abstract, so only the signature is emitted.
    pub(super) fn emit_interface(&mut self, i: &crate::ast::InterfaceDecl) -> Result<(), String> {
        let extends = if i.extends.is_empty() {
            String::new()
        } else {
            let parents: Vec<String> = i.extends.iter().map(|e| php_type_ref(e)).collect();
            format!(" extends {}", parents.join(", "))
        };
        let disp = php_class_name(if self.namespaced {
            last_segment(&i.name)
        } else {
            &i.name
        });
        self.line(&format!("interface {}{} {{", disp, extends));
        self.indent += 1;
        for m in &i.methods {
            let params: Vec<String> = m
                .params
                .iter()
                .map(|p| format!("{} ${}", self.emit_type(&p.ty), p.name))
                .collect();
            self.line(&format!(
                "public function {}({}){};",
                m.name,
                params.join(", "),
                self.ret_suffix(&m.ret)
            ));
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------
// B2 — `parent.m(…)` / `parent(A).m(…)` collection (for trait-aliased MI emission). A read-only walk
// over every expression position in a class's method/constructor/hook bodies, mirroring the complete
// `checker::rewrite_new` walker so no parent call is missed. Returns each call's
// `(ancestor-as-written, method)`; constructor (`parent.constructor`) calls are already inlined out by
// the front-end before transpilation, so only method calls remain.
// ---------------------------------------------------------------------------------------------------

fn collect_parent_method_calls(c: &ClassDecl) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    for m in &c.members {
        match m {
            ClassMember::Method(f) => pc_block(&f.body, &mut out),
            ClassMember::Constructor { body, .. } => pc_block(body, &mut out),
            ClassMember::Hook { get, set, .. } => {
                if let Some(g) = get {
                    pc_expr(g, &mut out);
                }
                if let Some((_, b)) = set {
                    pc_block(b, &mut out);
                }
            }
            ClassMember::Field { .. } => {} // `parent` is rejected in a field initializer (checker)
        }
    }
    out
}

fn pc_block(stmts: &[Stmt], out: &mut Vec<(Option<String>, String)>) {
    for s in stmts {
        pc_stmt(s, out);
    }
}

fn pc_stmt(s: &Stmt, out: &mut Vec<(Option<String>, String)>) {
    match s {
        Stmt::VarDecl { init, .. } => pc_expr(init, out),
        Stmt::Assign { target, value, .. } => {
            pc_expr(target, out);
            pc_expr(value, out);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                pc_expr(e, out);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            pc_expr(cond, out);
            pc_block(then_block, out);
            if let Some(b) = else_block {
                pc_block(b, out);
            }
        }
        Stmt::For { iter, body, .. } => {
            pc_expr(iter, out);
            pc_block(body, out);
        }
        Stmt::While { cond, body, .. } => {
            pc_expr(cond, out);
            pc_block(body, out);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                pc_stmt(i, out);
            }
            if let Some(co) = cond {
                pc_expr(co, out);
            }
            if let Some(st) = step {
                pc_stmt(st, out);
            }
            pc_block(body, out);
        }
        Stmt::Block(b, _) => pc_block(b, out),
        Stmt::Destructure {
            init, else_block, ..
        } => {
            pc_expr(init, out);
            if let Some(eb) = else_block {
                pc_block(eb, out);
            }
        }
        Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => pc_expr(e, out),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            pc_block(body, out);
            for CatchClause { body, .. } in catches {
                pc_block(body, out);
            }
            if let Some(fb) = finally_block {
                pc_block(fb, out);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn pc_expr(e: &Expr, out: &mut Vec<(Option<String>, String)>) {
    match e {
        Expr::ParentCall {
            ancestor,
            method,
            args,
            ..
        } => {
            out.push((ancestor.clone(), method.clone()));
            for a in args {
                pc_expr(a, out);
            }
        }
        Expr::Unary { expr, .. } => pc_expr(expr, out),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => pc_expr(inner, out),
        Expr::Binary { lhs, rhs, .. } => {
            pc_expr(lhs, out);
            pc_expr(rhs, out);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => pc_expr(value, out),
        Expr::Call { callee, args, .. } => {
            pc_expr(callee, out);
            for a in args {
                pc_expr(a, out);
            }
        }
        Expr::OverloadSelect { call, .. } => pc_expr(call, out),
        Expr::Member { object, .. } => pc_expr(object, out),
        Expr::Index { object, index, .. } => {
            pc_expr(object, out);
            pc_expr(index, out);
        }
        Expr::Str(parts, _) | Expr::Html(parts, _) => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    pc_expr(x, out);
                }
            }
        }
        Expr::List(xs, _) => {
            for x in xs {
                pc_expr(x, out);
            }
        }
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                pc_expr(k, out);
                pc_expr(v, out);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            pc_expr(scrutinee, out);
            for MatchArm { guard, body, .. } in arms {
                if let Some(g) = guard {
                    pc_expr(g, out);
                }
                pc_expr(body, out);
            }
        }
        Expr::Range { start, end, .. } => {
            pc_expr(start, out);
            pc_expr(end, out);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            pc_expr(cond, out);
            pc_expr(then_expr, out);
            pc_expr(else_expr, out);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => pc_expr(x, out),
            LambdaBody::Block(b) => pc_block(b, out),
        },
        Expr::CloneWith { object, fields, .. } => {
            pc_expr(object, out);
            for (_, v) in fields {
                pc_expr(v, out);
            }
        }
        Expr::New(inner, _) => pc_expr(inner, out),
        _ => {} // literals / Ident / This / etc. have no sub-expressions
    }
}
