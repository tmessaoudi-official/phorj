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
//! `#[Provides]` FACTORIES (slice 4a) construct a type via a `static` method (`Owner.method(deps)`)
//! instead of `new` — precedence over `new`/single-impl auto-bind, the params autowired (a provider
//! module is any class of such statics). `#[Transient]` (slice 4b) opts a class OUT of the default-shared
//! lifetime: it is built fresh at each injection point. The resolved graph is a [`Built`] tree and the
//! factory is emitted by LET-FLOATING it — shared nodes hoisted to `var`s once, transient nodes inlined
//! fresh — with construction-kind (new/provides) and sharing (shared/transient) fully orthogonal.
//!
//! INVARIANT — keep the rewriter TOTAL. `ritem`/`rfn`/`rmember`/`rexpr`/`rstmt` must recurse EVERY
//! expression-bearing AST position, so no `Expr::Inject` can survive to a backend `unreachable!`. When a
//! new expression-bearing AST node is added, add its arm here. There is no runtime backstop (matching
//! `desugar_router` / `rewrite_ufcs`); totality is maintained by this rule.
//!
//! Scope (disclosed): constructor + field injection, `#[Provides]` factories, `#[Transient]` (class-level)
//! lifetime; dependency types must be concrete class / interface names (an alias or generic-parameter
//! dependency type is a clean `E-DI-MISSING`, since this runs before alias/generic expansion).
//! Annotation-driven `inject()` draws its target only from a typed `var` declaration, a `return`, or a
//! lambda return type (a call-argument or param-default position is NOT an annotation source — write
//! `inject<T>()` there).

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
    /// `provided type → (owner class, static method, method params)` — a `#[Provides]` factory (slice 4a).
    /// A provider takes PRECEDENCE over `new` for its return type; its params are the deps to autowire.
    providers: BTreeMap<String, ProviderInfo>,
    /// Provided types with MORE than one `#[Provides]` factory — an ambiguous provider (E-DI-AMBIGUOUS).
    ambiguous_providers: BTreeSet<String>,
    /// Classes carrying `#[Transient]` (slice 4b) — built FRESH at each injection point (opt out of the
    /// default-shared lifetime).
    transient: BTreeSet<String>,
}

/// How the resolver constructs one node of the graph.
#[derive(Clone)]
enum Construct {
    /// `new <class>(args…)` — ordinary constructor injection.
    New(String),
    /// `<owner>.<method>(args…)` — a `#[Provides]` static factory method (slice 4a).
    Provides(String, String),
}

/// The resolved construction graph for one requested type, as a TREE (slice 4b): each node records how to
/// build its key and its resolved dependency sub-nodes. A SHARED node is emitted once (hoisted to a
/// `var`) and referenced by every dependent; a TRANSIENT node is inlined fresh at each use. Because
/// sharing (hoist-vs-inline) and construction kind (new-vs-provides) are orthogonal, the let-float emit
/// in [`synth_factory`] handles all four combinations from this one shape. A shared subtree is built once
/// and reused during resolution (so a diamond does not blow up); a transient subtree is rebuilt each time.
#[derive(Clone)]
struct Built {
    key: String,
    construct: Construct,
    transient: bool,
    deps: Vec<Built>,
}

/// A dependency parameter list: each `(Some(type_name) | None, span)` — `None` for a non-injectable
/// (primitive/optional/generic) type the graph can't wire.
type DepParams = Vec<(Option<String>, Span)>;

/// A `#[Provides]` factory: `(owner class, static method, method params)`.
type ProviderInfo = (String, String, DepParams);

/// A resolved construction node: `(key, how-to-construct, dep param types)`.
type Node = (String, Construct, DepParams);

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
    for (t, built) in &di.resolved {
        if let Some(built) = built {
            items.push(synth_factory(t, built));
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
    let (providers, ambiguous_providers) = collect_providers(program);
    let transient = program
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Class(c) if c.attrs.iter().any(crate::ast::Attribute::is_di_transient) => {
                Some(c.name.clone())
            }
            _ => None,
        })
        .collect();
    Registry {
        injectable,
        all_classes,
        deps,
        impls,
        providers,
        ambiguous_providers,
        transient,
    }
}

/// Scan EVERY class (not only `#[Injectable]` ones — a provider module is typically a plain class of
/// static factory methods) for `#[Provides]` static methods, mapping each method's return type → its
/// `(owner, method, params)`. A return type with more than one provider is recorded ambiguous. Providers
/// with no return type are skipped here (the checker already reported `E-PROVIDES-TARGET`).
fn collect_providers(program: &Program) -> (BTreeMap<String, ProviderInfo>, BTreeSet<String>) {
    let mut providers: BTreeMap<String, ProviderInfo> = BTreeMap::new();
    let mut ambiguous = BTreeSet::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            for m in &c.members {
                let ClassMember::Method(f) = m else { continue };
                if !f.attrs.iter().any(crate::ast::Attribute::is_di_provides) {
                    continue;
                }
                let Some(ret) = f.ret.as_ref().and_then(type_head_name) else {
                    continue; // E-PROVIDES-TARGET already reported for a return-less provider
                };
                let params: DepParams = f
                    .params
                    .iter()
                    .map(|p| (type_head_name(&p.ty), type_span(&p.ty)))
                    .collect();
                if providers
                    .insert(ret.clone(), (c.name.clone(), f.name.clone(), params))
                    .is_some()
                {
                    ambiguous.insert(ret);
                }
            }
        }
    }
    (providers, ambiguous)
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
                            // A DI-synthesized constructor only stores injected deps — it throws nothing.
                            throws: Vec::new(),
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
    resolved: BTreeMap<String, Option<Built>>,
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

mod synth;
mod walker;

use self::synth::*;
