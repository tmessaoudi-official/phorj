//! DI v1 — compile-time dependency-injection composition-root desugar
//! (`docs/plans/di-attributes.plan.md` §1+§6).
//!
//! A PRE-CHECK desugar (mirrors [`crate::checker::desugar_auto_router`]): it expands every
//! `inject<T>()` composition root into plain construction BEFORE the type-checker, so the generated
//! `new …` graph is checked like hand-written code and every backend sees the same explicit
//! construction — the expand-before-backends discipline (Inv-5), so byte-identity is trivial and there
//! is no runtime DI machinery. `#[Injectable]` attributes stay on the classes for the checker's
//! validation pass, then are inert for the backends.
//!
//! For each distinct requested type `T` used in an `inject<T>()`, the injectable dependency graph is
//! resolved by TYPE (constructor parameters; a single-implementor interface auto-binds to its impl) and
//! emitted as a synthesized nullary factory `function phorjInject<T>(): T { … }` whose body constructs
//! each type in the graph EXACTLY ONCE (topological, deps first) and shares it — the ruled default
//! SHARED lifetime, realized without any silent semantic downgrade (§14): a diamond `C(A(Db), B(Db))`
//! builds one `Db`. Each `inject<T>()` site is rewritten to a call `phorjInject<T>()` (a camelCase
//! synthetic name — free functions are `E-NAME-CASE`-checked, so the `__phorj_` transpile-helper
//! convention cannot be used).
//!
//! Compile-time errors (all deterministic, sorted iteration — Inv-10): a non-injectable dependency or an
//! unknown/primitive dependency type (`E-DI-MISSING`), an interface with zero or many injectable impls
//! (`E-DI-MISSING`/`E-DI-AMBIGUOUS`), and a dependency cycle (`E-DI-CYCLE`). A bare `inject()` in a
//! position with no expected type (a `var` binding, a discard, a call argument) is `E-INJECT-NO-TYPE`.
//!
//! IMPORT DISCIPLINE (§7, 2026-07-10). `inject` is a `Core.DI` member, NOT a keyword — nothing in the
//! wind ([[nothing-in-the-wind-import-discipline]]). Two surfaces feed ONE resolver:
//! - bare `inject<T>()` / `inject()` — requires the member-import `import Core.DI.inject;`;
//! - qualified `DI.inject<T>()` / `DI.inject()` — requires `import Core.DI;` (or any `Core.DI.*`).
//!
//! The parser emits `Expr::Inject` only for the explicit turbofish forms; the no-turbofish forms arrive
//! as ordinary `Call`s (`inject()` → `Call{Ident("inject")}`, `DI.inject()` → `Call{Member{DI,inject}}`)
//! and are converted here ONLY when the matching import is present — so an un-imported `inject()` stays a
//! plain call to a user function named `inject` (the identifier is freed). An explicit `inject<T>()`
//! whose import is absent is `E-DI-NO-IMPORT` (a turbofish call cannot be anything but the composition
//! root). The DI ATTRIBUTES (`#[Injectable]`, qualified `#[DI.Injectable]`) get the same discipline via
//! `enforce_injected_discipline` (`module_of("Injectable") == "DI"`) + `Attribute::is_di_builtin`.
//!
//! FIELD INJECTION (slice 3). Before the registry is built, [`fold_injected_fields`] folds each
//! injectable's injectable-typed, no-initializer INSTANCE field into its constructor as a promoted
//! parameter (the "synthesized-ctor" model, §1) — so a field dependency is resolved, shared, and
//! cycle-checked by the SAME graph machinery as a constructor dependency, and transpiles to an ordinary
//! promoted-constructor property. A field WITH an initializer is left alone; a non-injectable-typed
//! field is untouched.
//!
//! INVARIANT — keep the rewriter TOTAL. `ritem`/`rfn`/`rmember`/`rexpr`/`rstmt` must recurse EVERY
//! expression-bearing AST position, so no `Expr::Inject` can survive to a backend `unreachable!`. When a
//! new expression-bearing AST node is added (a later slice like `#[Provides]` touches exactly this
//! surface), add its arm here. There is no runtime backstop (matching `desugar_router` /
//! `rewrite_ufcs`); totality is maintained by this rule.
//!
//! Scope (disclosed): constructor + field injection; dependency types must be concrete class / interface
//! names (an alias or generic-parameter dependency type is a clean `E-DI-MISSING`, since this runs before
//! alias/generic expansion); `#[Transient]` / `#[Provides]` are later slices (v1 is default-shared,
//! plain-`new` construction). Annotation-driven `inject()` draws its target only from a typed `var`
//! declaration, a `return`, or a lambda return type (a call-argument or param-default position is NOT an
//! annotation source — write `inject<T>()` there).

use crate::ast::{
    ctor_plan, CatchClause, ClassMember, Expr, FunctionDecl, Item, LambdaBody, MatchArm, Program,
    Stmt, StrPart, Type,
};
use crate::diagnostic::{Diagnostic, Stage};
use crate::token::Span;
use std::collections::{BTreeMap, BTreeSet};

/// The structural injectable registry, built once from the raw program (pre-check).
struct Registry {
    /// Classes carrying `#[Injectable]`.
    injectable: BTreeSet<String>,
    /// Every declared class name (to tell "not injectable" from "unknown type").
    all_classes: BTreeSet<String>,
    /// `class → its constructor dependencies` (via `ctor_plan`, so inherited promoted params count).
    /// Each dep is `(Some(type_name))` for a concrete `Type::Named`, or `None` for any other type shape
    /// (primitive / optional / generic — not injectable in v1), with the param's span for diagnostics.
    deps: BTreeMap<String, Vec<(Option<String>, Span)>>,
    /// `interface → the injectable classes that implement it` (sorted; the single-impl auto-bind + the
    /// ambiguity check read this).
    impls: BTreeMap<String, Vec<String>>,
}

/// The resolved construction order for one requested type: `(concrete_class, [dep_concrete_classes])`
/// entries in topological order (dependencies before dependents), each class appearing exactly once.
type Plan = Vec<(String, Vec<String>)>;

pub fn desugar_di(program: Program) -> Result<Program, Vec<Diagnostic>> {
    // Field injection (slice 3): fold injectable-typed no-initializer fields into promoted ctor params
    // BEFORE the registry is built, so `ctor_plan` sees them and the graph resolver wires + shares them
    // exactly like ctor dependencies (§1 synthesized-ctor model).
    let injectable = collect_injectable(&program);
    let impls = collect_impls(&program, &injectable);
    let program = fold_injected_fields(program, &injectable, &impls);
    let reg = build_registry(&program);
    let bare_inject_imported = imports_path(&program, &["Core", "DI", "inject"]);
    let di_qualifier_imported = imports_di_module(&program);
    let Program {
        package,
        items,
        span,
    } = program;
    let mut di = Di {
        reg: &reg,
        diags: Vec::new(),
        // requested-type name → Some(plan) once resolved, or None if it errored (memoized so a repeated
        // `inject<T>()` resolves once and reports once).
        resolved: BTreeMap::new(),
        bare_inject_imported,
        di_qualifier_imported,
        current_ret: None,
    };
    let mut items: Vec<Item> = items.into_iter().map(|it| di.ritem(it)).collect();
    if !di.diags.is_empty() {
        return Err(di.diags);
    }
    // Append a synthesized factory for every successfully-resolved requested type (sorted → Inv-10).
    for (t, plan) in &di.resolved {
        if let Some(plan) = plan {
            items.push(synth_factory(t, plan));
        }
    }
    Ok(Program {
        package,
        items,
        span,
    })
}

/// The set of `#[Injectable]` classes (bare or qualified attribute — `is_di_builtin`). Cheap; used both
/// to classify field types in the fold pass and to seed the registry.
fn collect_injectable(program: &Program) -> BTreeSet<String> {
    program
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Class(c) if c.attrs.iter().any(crate::ast::Attribute::is_di_builtin) => {
                Some(c.name.clone())
            }
            _ => None,
        })
        .collect()
}

/// `interface → the injectable classes that implement it` (sorted, deduped).
fn collect_impls(
    program: &Program,
    injectable: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    let implements = crate::ast::class_implements(program);
    let mut impls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (class, ifaces) in &implements {
        if !injectable.contains(class) {
            continue;
        }
        for iface in ifaces {
            impls.entry(iface.clone()).or_default().push(class.clone());
        }
    }
    for v in impls.values_mut() {
        v.sort();
        v.dedup();
    }
    impls
}

fn build_registry(program: &Program) -> Registry {
    let injectable = collect_injectable(program);
    let mut all_classes = BTreeSet::new();
    let mut deps: BTreeMap<String, Vec<(Option<String>, Span)>> = BTreeMap::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            all_classes.insert(c.name.clone());
        }
    }
    // Constructor dependencies for every injectable (via ctor_plan → inherited promoted params too).
    // After `fold_injected_fields`, injectable-typed no-initializer fields are already promoted ctor
    // params here, so field injection is resolved by the SAME graph machinery as ctor injection.
    for cls in &injectable {
        let plan = ctor_plan(program, cls);
        let params: Vec<(Option<String>, Span)> = plan
            .iter()
            .flat_map(|(ps, _)| ps.iter())
            .map(|p| (type_head_name(&p.ty), type_span(&p.ty)))
            .collect();
        deps.insert(cls.clone(), params);
    }
    let impls = collect_impls(program, &injectable);
    Registry {
        injectable,
        all_classes,
        deps,
        impls,
    }
}

/// Field injection (slice 3): fold each injectable's injectable-typed, no-initializer INSTANCE field into
/// its constructor as an appended **promoted** parameter (synthesizing an empty-body constructor if the
/// class has none). This is the ruled "synthesized-ctor model" (§1): once the field is a promoted ctor
/// param, the graph resolver wires it, shares it in a diamond, and detects field cycles EXACTLY like a
/// ctor dependency — and it transpiles to an ordinary PHP promoted-constructor property (byte-identical).
/// A field WITH an initializer is user-provided and left alone; a non-injectable-typed field is an
/// ordinary field the constructor body sets. Determinism (Inv-10): injected fields are appended in
/// sorted name order. Runs before [`build_registry`], so `ctor_plan` already sees the promoted params.
fn fold_injected_fields(
    program: Program,
    injectable: &BTreeSet<String>,
    impls: &BTreeMap<String, Vec<String>>,
) -> Program {
    use crate::ast::{CtorParam, Modifier};
    let is_injectable_typed = |ty: &Type| {
        type_head_name(ty).is_some_and(|n| injectable.contains(&n) || impls.contains_key(&n))
    };
    let items = program
        .items
        .into_iter()
        .map(|it| match it {
            Item::Class(mut c) if injectable.contains(&c.name) => {
                let mut injected: Vec<CtorParam> = Vec::new();
                let mut kept: Vec<ClassMember> = Vec::new();
                for m in c.members.drain(..) {
                    if let ClassMember::Field {
                        modifiers,
                        ty,
                        name,
                        init: None,
                        span,
                    } = &m
                    {
                        let is_static = modifiers.iter().any(|md| matches!(md, Modifier::Static));
                        if !is_static && is_injectable_typed(ty) {
                            // Ensure the promoted param carries a visibility (promotion requires one);
                            // a field without an explicit visibility defaults to private.
                            let mut mods = modifiers.clone();
                            if !mods.iter().any(|md| {
                                matches!(
                                    md,
                                    Modifier::Public | Modifier::Private | Modifier::Protected
                                )
                            }) {
                                mods.insert(0, Modifier::Private);
                            }
                            injected.push(CtorParam {
                                modifiers: mods,
                                ty: ty.clone(),
                                name: name.clone(),
                                span: *span,
                            });
                            continue;
                        }
                    }
                    kept.push(m);
                }
                if !injected.is_empty() {
                    injected.sort_by(|a, b| a.name.cmp(&b.name));
                    match kept.iter_mut().find_map(|m| match m {
                        ClassMember::Constructor { params, .. } => Some(params),
                        _ => None,
                    }) {
                        Some(params) => params.extend(injected),
                        None => kept.push(ClassMember::Constructor {
                            modifiers: Vec::new(),
                            params: injected,
                            body: Vec::new(),
                            span: c.span,
                        }),
                    }
                }
                c.members = kept;
                Item::Class(c)
            }
            other => other,
        })
        .collect();
    Program {
        package: program.package.clone(),
        items,
        span: program.span,
    }
}

/// The head type name of a concrete `Type::Named`, else `None` (primitive/optional/generic/etc.).
fn type_head_name(t: &Type) -> Option<String> {
    match t {
        Type::Named { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn type_span(t: &Type) -> Span {
    match t {
        Type::Named { span, .. } | Type::Optional { span, .. } => *span,
        _ => Span {
            start: 0,
            len: 0,
            line: 1,
            col: 1,
        },
    }
}

struct Di<'a> {
    reg: &'a Registry,
    diags: Vec<Diagnostic>,
    resolved: BTreeMap<String, Option<Plan>>,
    /// `import Core.DI.inject;` is present → bare `inject…` is allowed (else a bare turbofish is
    /// `E-DI-NO-IMPORT` and a bare annotation `inject()` stays an ordinary call to a user function).
    bare_inject_imported: bool,
    /// `import Core.DI;` (or any `Core.DI.*`) is present → qualified `DI.inject…` is allowed.
    di_qualifier_imported: bool,
    /// The enclosing function/method/lambda return type — the annotation source for `return inject();`
    /// (slice 2). Saved/restored across `rfn` and every lambda so an inner scope never inherits an outer
    /// return type.
    current_ret: Option<Type>,
}

/// True iff the program has an import whose path is exactly `want`.
fn imports_path(program: &Program, want: &[&str]) -> bool {
    program.items.iter().any(|it| {
        matches!(it, Item::Import { path, .. }
            if path.len() == want.len() && path.iter().zip(want).all(|(a, b)| a == b))
    })
}

/// True iff `Core.DI` is imported in any form — the module (`import Core.DI;`) or any member
/// (`import Core.DI.inject;`, `import Core.DI.Injectable;`). Any of these binds the `DI` qualifier.
fn imports_di_module(program: &Program) -> bool {
    program.items.iter().any(|it| {
        matches!(it, Item::Import { path, .. }
            if path.len() >= 2 && path[0] == "Core" && path[1] == "DI")
    })
}

impl Di<'_> {
    /// Resolve one *requested* type name to its concrete injectable class, or record a diagnostic.
    fn resolve_concrete(&mut self, name: &str, span: Span) -> Option<String> {
        if self.reg.injectable.contains(name) {
            return Some(name.to_string());
        }
        if let Some(list) = self.reg.impls.get(name) {
            match list.len() {
                1 => return Some(list[0].clone()),
                0 => {}
                _ => {
                    self.diags.push(
                        err(
                            format!(
                                "ambiguous injection of interface `{name}`: {} injectable implementations ({})",
                                list.len(),
                                list.join(", ")
                            ),
                            span,
                        )
                        .with_code("E-DI-AMBIGUOUS")
                        .with_hint(
                            "provide exactly one `#[Injectable]` implementation (multi-impl qualifiers are a later slice)"
                                .to_string(),
                        ),
                    );
                    return None;
                }
            }
        }
        let hint = if self.reg.all_classes.contains(name) {
            format!("mark `{name}` `#[Injectable]` so the graph can construct it")
        } else {
            format!("`{name}` is not an injectable class or a single-implementation interface")
        };
        self.diags.push(
            err(
                format!("cannot inject `{name}`: no `#[Injectable]` provider"),
                span,
            )
            .with_code("E-DI-MISSING")
            .with_hint(hint),
        );
        None
    }

    /// Resolve the full dependency graph for a concrete class into a topological plan. `in_progress`
    /// detects cycles. Returns `None` (and records a diagnostic) on any failure.
    fn resolve_graph(
        &mut self,
        class: &str,
        span: Span,
        in_progress: &mut Vec<String>,
        order: &mut Plan,
    ) -> Option<()> {
        if order.iter().any(|(c, _)| c == class) {
            return Some(()); // already constructed once (shared)
        }
        if in_progress.iter().any(|c| c == class) {
            let mut chain = in_progress.clone();
            chain.push(class.to_string());
            self.diags.push(
                err(
                    format!("dependency cycle in injection: {}", chain.join(" → ")),
                    span,
                )
                .with_code("E-DI-CYCLE")
                .with_hint(
                    "break the cycle — a constructor graph must be acyclic (field-injection cycle-breaking is not in v1)"
                        .to_string(),
                ),
            );
            return None;
        }
        in_progress.push(class.to_string());
        let deps = self.reg.deps.get(class).cloned().unwrap_or_default();
        let mut dep_classes = Vec::new();
        for (dep_name, dep_span) in deps {
            let Some(name) = dep_name else {
                self.diags.push(
                    err(
                        format!("constructor of `{class}` has a dependency whose type is not an injectable class"),
                        dep_span,
                    )
                    .with_code("E-DI-MISSING")
                    .with_hint(
                        "every constructor parameter of an injectable must be an injectable class or a single-impl interface (config-value provision is a later slice)"
                            .to_string(),
                    ),
                );
                in_progress.pop();
                return None;
            };
            let Some(concrete) = self.resolve_concrete(&name, dep_span) else {
                in_progress.pop();
                return None;
            };
            self.resolve_graph(&concrete, dep_span, in_progress, order)?;
            dep_classes.push(concrete);
        }
        in_progress.pop();
        order.push((class.to_string(), dep_classes));
        Some(())
    }

    /// Resolve (memoized) the requested type `t`; returns the factory name to call, or `None` on error.
    fn factory_for(&mut self, t: &str, span: Span) -> Option<String> {
        if let Some(entry) = self.resolved.get(t) {
            return entry.as_ref().map(|_| factory_name(t));
        }
        let plan = self.resolve_concrete(t, span).and_then(|concrete| {
            let mut order = Plan::new();
            let mut in_progress = Vec::new();
            self.resolve_graph(&concrete, span, &mut in_progress, &mut order)
                .map(|()| (concrete, order))
        });
        match plan {
            Some((_concrete, order)) => {
                self.resolved.insert(t.to_string(), Some(order));
                Some(factory_name(t))
            }
            None => {
                self.resolved.insert(t.to_string(), None);
                None
            }
        }
    }

    // --- the AST rewriter (mirrors desugar_router's proven complete walk; the behavioural difference is
    //     the `Expr::Inject` arm, plus `ParentCall`/`OverloadSelect` are walked so a nested `inject` is
    //     never left for a backend). ---

    fn ritem(&mut self, it: Item) -> Item {
        match it {
            Item::Function(mut f) => {
                self.rfn(&mut f);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                for m in &mut c.members {
                    self.rmember(m);
                }
                Item::Class(c)
            }
            other => other,
        }
    }

    /// Walk a function/method: its body AND its parameter default expressions (a default like
    /// `f(Db d = inject<Db>())` is a real `inject` site — total coverage means no `Expr::Inject` can
    /// survive to a backend `unreachable!`).
    fn rfn(&mut self, f: &mut FunctionDecl) {
        let prev_ret = std::mem::replace(&mut self.current_ret, f.ret.clone());
        let body = std::mem::take(&mut f.body);
        f.body = self.rblock(body);
        for p in &mut f.params {
            if let Some(d) = p.default.take() {
                p.default = Some(Box::new(self.rexpr(*d)));
            }
        }
        self.current_ret = prev_ret;
    }

    /// Walk every expression-bearing position of a class member — method bodies+defaults, field
    /// initializers, constructor bodies, and property-hook get/set bodies. (`CtorParam` carries no
    /// default in the AST, so constructors have only a body to walk.)
    fn rmember(&mut self, m: &mut ClassMember) {
        match m {
            ClassMember::Method(f) => self.rfn(f),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init.take() {
                    *init = Some(self.rexpr(e));
                }
            }
            ClassMember::Constructor { body, .. } => {
                let b = std::mem::take(body);
                *body = self.rblock(b);
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(e) = get.take() {
                    *get = Some(self.rexpr(e));
                }
                if let Some((_param, stmts)) = set {
                    let b = std::mem::take(stmts);
                    *stmts = self.rblock(b);
                }
            }
        }
    }

    fn rblock(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        stmts.into_iter().map(|s| self.rstmt(s)).collect()
    }

    /// Gate a composition root against the import discipline (§7), then resolve it. `ty` is the explicit
    /// turbofish target (`Some`) or `None` for the annotation form; `expected` is the annotation source
    /// (a typed declaration / return type — already `Infer`-stripped by the caller); `qualified` selects
    /// which import gates it.
    fn rinject(
        &mut self,
        ty: Option<Type>,
        qualified: bool,
        expected: Option<&Type>,
        span: Span,
    ) -> Expr {
        // Import gate. Annotation forms reach here only when already imported (see `annotation_inject`),
        // so this bites an explicit turbofish `inject<T>()`/`DI.inject<T>()` whose import is absent.
        let imported = if qualified {
            self.di_qualifier_imported
        } else {
            self.bare_inject_imported
        };
        if !imported {
            let (surface, fix) = if qualified {
                ("DI.inject", "import Core.DI;")
            } else {
                ("inject", "import Core.DI.inject;")
            };
            self.diags.push(
                err(format!("`{surface}` is used without importing `Core.DI`"), span)
                    .with_code("E-DI-NO-IMPORT")
                    .with_hint(format!(
                        "add `{fix}` — the DI composition root must be imported, never used in the wind"
                    )),
            );
            return self.di_error_placeholder(span);
        }
        // Explicit `ty` wins; otherwise draw the target from the position's expected type.
        let name = match ty.as_ref().or(expected) {
            Some(Type::Named { name, .. }) => name.clone(),
            Some(_other) => {
                self.diags.push(
                    err(
                        "`inject<T>()` requires a concrete injectable class or interface"
                            .to_string(),
                        span,
                    )
                    .with_code("E-DI-MISSING")
                    .with_hint(
                        "name a concrete injectable class or a single-implementation interface"
                            .to_string(),
                    ),
                );
                return self.di_error_placeholder(span);
            }
            None => {
                self.diags.push(
                    err(
                        "`inject()` could not infer a target type from its position".to_string(),
                        span,
                    )
                    .with_code("E-INJECT-NO-TYPE")
                    .with_hint(
                        "use it as the initializer of a typed declaration (`App app = inject();`), a typed `return`, or name the type explicitly: `inject<App>()`"
                            .to_string(),
                    ),
                );
                return self.di_error_placeholder(span);
            }
        };
        self.inject_to_call(&name, span)
    }

    /// Resolve a requested type name to its factory call, or a placeholder if resolution errored (the
    /// diagnostic is recorded inside `factory_for`, and the `Err` return discards the placeholder).
    fn inject_to_call(&mut self, name: &str, span: Span) -> Expr {
        match self.factory_for(name, span) {
            Some(fname) => Expr::Call {
                callee: Box::new(Expr::Ident(fname, span)),
                args: Vec::new(),
                span,
            },
            None => self.di_error_placeholder(span),
        }
    }

    fn di_error_placeholder(&self, span: Span) -> Expr {
        Expr::Call {
            callee: Box::new(Expr::Ident("__phorj_di_error".to_string(), span)),
            args: Vec::new(),
            span,
        }
    }

    /// Is this nullary call's `callee` an annotation-form composition root whose import is present?
    /// Returns `Some(qualified)` if so; `None` if it is an ordinary user call (a bare `inject()` with no
    /// member-import, or `DI.inject()` on a user object when `Core.DI` is not imported — the freed-
    /// identifier guarantee).
    fn annotation_inject(&self, callee: &Expr) -> Option<bool> {
        match callee {
            Expr::Ident(name, _) if name == "inject" && self.bare_inject_imported => Some(false),
            Expr::Member {
                object,
                name,
                safe: false,
                ..
            } if name == "inject"
                && self.di_qualifier_imported
                && matches!(object.as_ref(), Expr::Ident(q, _) if q == "DI") =>
            {
                Some(true)
            }
            _ => None,
        }
    }

    /// Rewrite an expression sitting in an *annotation position* (typed `var` init, `return` value,
    /// lambda expr-body): a composition root there draws its target from `expected`. Anything else falls
    /// through to the context-free walk. `Type::Infer` (`var`) is not an annotation → stripped to `None`.
    fn rexpr_expected(&mut self, e: Expr, expected: Option<&Type>) -> Expr {
        let expected = expected.filter(|t| !matches!(t, Type::Infer(_)));
        match e {
            Expr::Inject {
                ty,
                qualified,
                span,
            } => self.rinject(ty, qualified, expected, span),
            Expr::Call { callee, args, span } if args.is_empty() => {
                match self.annotation_inject(&callee) {
                    Some(qualified) => self.rinject(None, qualified, expected, span),
                    None => Expr::Call {
                        callee: Box::new(self.rexpr(*callee)),
                        args: Vec::new(),
                        span,
                    },
                }
            }
            other => self.rexpr(other),
        }
    }

    fn rexpr(&mut self, e: Expr) -> Expr {
        match e {
            // Explicit turbofish `inject<T>()` / `DI.inject<T>()` (parser-produced). Gate the import,
            // then resolve. In a non-annotation position, `ty: None` cannot arise from the parser; it
            // only reaches here via `annotation_inject` re-dispatch, so a `None` here means an annotation
            // `inject()` used where no expected type is available → `E-INJECT-NO-TYPE`.
            Expr::Inject {
                ty,
                qualified,
                span,
            } => self.rinject(ty, qualified, None, span),
            // Recognize an annotation-form composition root written as an ordinary call — but only when
            // the matching import is present; otherwise it is a genuine user call and recurses normally.
            Expr::Call { callee, args, span } if args.is_empty() => {
                match self.annotation_inject(&callee) {
                    Some(qualified) => self.rinject(None, qualified, None, span),
                    None => Expr::Call {
                        callee: Box::new(self.rexpr(*callee)),
                        args: Vec::new(),
                        span,
                    },
                }
            }
            Expr::Call { callee, args, span } => Expr::Call {
                callee: Box::new(self.rexpr(*callee)),
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
            },
            Expr::ParentCall {
                ancestor,
                method,
                args,
                span,
            } => Expr::ParentCall {
                ancestor,
                method,
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
            },
            Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
                ty,
                call: Box::new(self.rexpr(*call)),
                span,
            },
            Expr::Str(parts, span) => Expr::Str(self.rparts(parts), span),
            Expr::List(items, span) => {
                Expr::List(items.into_iter().map(|e| self.rexpr(e)).collect(), span)
            }
            Expr::Map(pairs, span) => Expr::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| (self.rexpr(k), self.rexpr(v)))
                    .collect(),
                span,
            ),
            Expr::Unary { op, expr, span } => Expr::Unary {
                op,
                expr: Box::new(self.rexpr(*expr)),
                span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op,
                lhs: Box::new(self.rexpr(*lhs)),
                rhs: Box::new(self.rexpr(*rhs)),
                span,
            },
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => Expr::InstanceOf {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Cast {
                value,
                type_name,
                span,
            } => Expr::Cast {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Member {
                object,
                name,
                safe,
                span,
            } => Expr::Member {
                object: Box::new(self.rexpr(*object)),
                name,
                safe,
                span,
            },
            Expr::Index {
                object,
                index,
                span,
            } => Expr::Index {
                object: Box::new(self.rexpr(*object)),
                index: Box::new(self.rexpr(*index)),
                span,
            },
            Expr::Force { inner, span } => Expr::Force {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Propagate { inner, span } => Expr::Propagate {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Expr::Match {
                scrutinee: Box::new(self.rexpr(*scrutinee)),
                arms: arms
                    .into_iter()
                    .map(|a| MatchArm {
                        pattern: a.pattern,
                        guard: a.guard.map(|g| self.rexpr(g)),
                        body: self.rexpr(a.body),
                        span: a.span,
                    })
                    .collect(),
                span,
            },
            Expr::Range {
                start,
                end,
                inclusive,
                span,
            } => Expr::Range {
                start: Box::new(self.rexpr(*start)),
                end: Box::new(self.rexpr(*end)),
                inclusive,
                span,
            },
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => Expr::If {
                cond: Box::new(self.rexpr(*cond)),
                then_expr: Box::new(self.rexpr(*then_expr)),
                else_expr: Box::new(self.rexpr(*else_expr)),
                span,
            },
            Expr::Lambda {
                params,
                ret,
                body,
                span,
            } => {
                // A lambda is its own return-type scope: save/restore `current_ret` so a `return
                // inject()` inside never inherits the enclosing function's return type, and its expr-body
                // is itself a return position (draws from the lambda's declared `ret`).
                let prev_ret = std::mem::replace(&mut self.current_ret, ret.clone());
                let new_body = match body {
                    LambdaBody::Expr(e) => {
                        let expected = self.current_ret.clone();
                        LambdaBody::Expr(Box::new(self.rexpr_expected(*e, expected.as_ref())))
                    }
                    LambdaBody::Block(stmts) => LambdaBody::Block(self.rblock(stmts)),
                };
                self.current_ret = prev_ret;
                Expr::Lambda {
                    params,
                    ret,
                    body: new_body,
                    span,
                }
            }
            Expr::CloneWith {
                object,
                fields,
                span,
            } => Expr::CloneWith {
                object: Box::new(self.rexpr(*object)),
                fields: fields
                    .into_iter()
                    .map(|(n, e)| (n, self.rexpr(e)))
                    .collect(),
                span,
            },
            Expr::New(inner, span) => Expr::New(Box::new(self.rexpr(*inner)), span),
            Expr::Spawn { call, span } => Expr::Spawn {
                call: Box::new(self.rexpr(*call)),
                span,
            },
            Expr::Html(parts, span) => Expr::Html(self.rparts(parts), span),
            // true leaves (Int/Float/Decimal/Bool/Null/Bytes/Ident/This) carry no nested expression.
            leaf => leaf,
        }
    }

    fn rparts(&mut self, parts: Vec<StrPart>) -> Vec<StrPart> {
        parts
            .into_iter()
            .map(|p| match p {
                StrPart::Expr(e) => StrPart::Expr(Box::new(self.rexpr(*e))),
                lit => lit,
            })
            .collect()
    }

    fn rstmt(&mut self, s: Stmt) -> Stmt {
        match s {
            Stmt::VarDecl {
                ty,
                name,
                init,
                mutable,
                span,
            } => {
                // A typed declaration is an annotation position: `App app = inject();` draws its target
                // from `ty` (slice 2). `var app = …` (`ty` is `Type::Infer`) is not an annotation and is
                // stripped inside `rexpr_expected`.
                let init = self.rexpr_expected(init, Some(&ty));
                Stmt::VarDecl {
                    ty,
                    name,
                    init,
                    mutable,
                    span,
                }
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => Stmt::Assign {
                target: self.rexpr(target),
                value: self.rexpr(value),
                span,
            },
            Stmt::Return { value, span } => Stmt::Return {
                // A `return` draws its annotation from the enclosing function/method/lambda return type.
                value: value.map(|e| {
                    let expected = self.current_ret.clone();
                    self.rexpr_expected(e, expected.as_ref())
                }),
                span,
            },
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => Stmt::If {
                cond: self.rexpr(cond),
                bind,
                then_block: self.rblock(then_block),
                else_block: else_block.map(|b| self.rblock(b)),
                span,
            },
            Stmt::For {
                ty,
                name,
                val,
                iter,
                body,
                span,
            } => Stmt::For {
                ty,
                name,
                val,
                iter: self.rexpr(iter),
                body: self.rblock(body),
                span,
            },
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => Stmt::While {
                cond: self.rexpr(cond),
                body: self.rblock(body),
                post_cond,
                span,
            },
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                span,
            } => Stmt::CFor {
                init: init.map(|s| Box::new(self.rstmt(*s))),
                cond: cond.map(|e| self.rexpr(e)),
                step: step.map(|s| Box::new(self.rstmt(*s))),
                body: self.rblock(body),
                span,
            },
            Stmt::Block(stmts, span) => Stmt::Block(self.rblock(stmts), span),
            Stmt::Expr(e, span) => Stmt::Expr(self.rexpr(e), span),
            Stmt::Discard(e, span) => Stmt::Discard(self.rexpr(e), span),
            Stmt::Throw { value, span } => Stmt::Throw {
                value: self.rexpr(value),
                span,
            },
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => Stmt::Try {
                body: self.rblock(body),
                catches: catches
                    .into_iter()
                    .map(|c| CatchClause {
                        ty: c.ty,
                        name: c.name,
                        body: self.rblock(c.body),
                        span: c.span,
                    })
                    .collect(),
                finally_block: finally_block.map(|b| self.rblock(b)),
                span,
            },
            leaf => leaf, // Break / Continue carry no expression
        }
    }
}

// A camelCase synthetic factory name (free functions are `E-NAME-CASE`-checked, so the `__phorj_`
// transpile-helper convention can't be used here). Type names are PascalCase, so `phorjInject<Type>` is
// camelCase. Collision with a hand-written `phorjInject…` function is astronomically unlikely and
// disclosed (KNOWN_ISSUES); a dedicated reserved-prefix check is a later refinement.
fn factory_name(t: &str) -> String {
    format!("phorjInject{t}")
}

fn di_span() -> Span {
    Span {
        start: 0,
        len: 0,
        line: 1,
        col: 1,
    }
}

fn err(message: String, span: Span) -> Diagnostic {
    Diagnostic::new(Stage::Type, message, span.line, span.col)
}

/// A local-variable name for a class instance inside a factory body (unique per class → sharing).
/// Class names are PascalCase, so `di<Class>` is camelCase (locals are convention-checked too).
fn inst_var(class: &str) -> String {
    format!("di{class}")
}

/// Build `function __phorj_di_<T>(): T { var __di_C = new C(...); …; return __di_<root>; }`.
/// The last entry in `plan` is the root (topological order emits deps first, root last).
fn synth_factory(requested: &str, plan: &Plan) -> Item {
    let sp = di_span();
    let mut body: Vec<Stmt> = Vec::new();
    for (class, dep_classes) in plan {
        let args: Vec<Expr> = dep_classes
            .iter()
            .map(|d| Expr::Ident(inst_var(d), sp))
            .collect();
        let construct = Expr::New(
            Box::new(Expr::Call {
                callee: Box::new(Expr::Ident(class.clone(), sp)),
                args,
                span: sp,
            }),
            sp,
        );
        body.push(Stmt::VarDecl {
            ty: named_type(class, sp),
            name: inst_var(class),
            init: construct,
            mutable: false,
            span: sp,
        });
    }
    let root = plan
        .last()
        .map(|(c, _)| c.clone())
        .unwrap_or_else(|| requested.to_string());
    body.push(Stmt::Return {
        value: Some(Expr::Ident(inst_var(&root), sp)),
        span: sp,
    });
    Item::Function(crate::ast::FunctionDecl {
        modifiers: Vec::new(),
        attrs: Vec::new(),
        vis: crate::ast::Visibility::Public,
        name: factory_name(requested),
        type_params: Vec::new(),
        params: Vec::new(),
        // Return the REQUESTED type (`inject<Logger>()` returns `Logger`, built as its single impl —
        // assignable), so the call site types exactly as the user wrote.
        ret: Some(named_type(requested, sp)),
        throws: Vec::new(),
        body,
        foreign: false,
        generic_ret_from_param: None,
        span: sp,
    })
}

fn named_type(name: &str, span: Span) -> Type {
    Type::Named {
        name: name.to_string(),
        args: Vec::new(),
        span,
    }
}
