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
//! (`E-DI-MISSING`/`E-DI-AMBIGUOUS`), and a dependency cycle (`E-DI-CYCLE`). Bare `inject()` (no type
//! argument) is `E-INJECT-NO-TYPE` in v1 (the annotation-driven form is a later slice).
//!
//! INVARIANT — keep the rewriter TOTAL. `ritem`/`rfn`/`rmember`/`rexpr`/`rstmt` must recurse EVERY
//! expression-bearing AST position, so no `Expr::Inject` can survive to a backend `unreachable!`. When a
//! new expression-bearing AST node is added (the next slices — field injection, `#[Provides]` — touch
//! exactly this surface), add its arm here. There is no runtime backstop (matching `desugar_router` /
//! `rewrite_ufcs`); totality is maintained by this rule.
//!
//! Slice 1 scope (disclosed): constructor injection only (field injection is a later slice); dependency
//! types must be concrete class / interface names (an alias or generic-parameter dependency type is a
//! clean `E-DI-MISSING`, since this runs before alias/generic expansion); `#[Transient]` / `#[Provides]`
//! are later slices (v1 is default-shared, plain-`new` construction).

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
    let reg = build_registry(&program);
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

fn build_registry(program: &Program) -> Registry {
    let mut injectable = BTreeSet::new();
    let mut all_classes = BTreeSet::new();
    let mut deps: BTreeMap<String, Vec<(Option<String>, Span)>> = BTreeMap::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            all_classes.insert(c.name.clone());
            if c.attrs.iter().any(crate::ast::Attribute::is_di_builtin) {
                injectable.insert(c.name.clone());
            }
        }
    }
    // Constructor dependencies for every injectable (via ctor_plan → inherited promoted params too).
    for cls in &injectable {
        let plan = ctor_plan(program, cls);
        let params: Vec<(Option<String>, Span)> = plan
            .iter()
            .flat_map(|(ps, _)| ps.iter())
            .map(|p| (type_head_name(&p.ty), type_span(&p.ty)))
            .collect();
        deps.insert(cls.clone(), params);
    }
    // interface → injectable implementors (sorted).
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
    Registry {
        injectable,
        all_classes,
        deps,
        impls,
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
        let body = std::mem::take(&mut f.body);
        f.body = self.rblock(body);
        for p in &mut f.params {
            if let Some(d) = p.default.take() {
                p.default = Some(Box::new(self.rexpr(*d)));
            }
        }
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

    fn rexpr(&mut self, e: Expr) -> Expr {
        match e {
            Expr::Inject { ty, span } => match ty {
                Some(Type::Named { name, .. }) => match self.factory_for(&name, span) {
                    Some(fname) => Expr::Call {
                        callee: Box::new(Expr::Ident(fname, span)),
                        args: Vec::new(),
                        span,
                    },
                    // error already recorded; emit a placeholder call (the Err return discards it).
                    None => Expr::Call {
                        callee: Box::new(Expr::Ident("__phorj_di_error".to_string(), span)),
                        args: Vec::new(),
                        span,
                    },
                },
                Some(_other) => {
                    self.diags.push(
                        err(
                            "`inject<T>()` requires a concrete injectable class or interface"
                                .to_string(),
                            span,
                        )
                        .with_code("E-DI-MISSING")
                        .with_hint("use `inject<SomeInjectableClass>()`".to_string()),
                    );
                    Expr::Null(span)
                }
                None => {
                    self.diags.push(
                        err(
                            "`inject()` needs an explicit target type in v1 — write `inject<T>()`".to_string(),
                            span,
                        )
                        .with_code("E-INJECT-NO-TYPE")
                        .with_hint(
                            "the annotation-driven bare `inject()` form is a later slice; name the type: `inject<App>()`"
                                .to_string(),
                        ),
                    );
                    Expr::Null(span)
                }
            },
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
            } => Expr::Lambda {
                params,
                ret,
                body: match body {
                    LambdaBody::Expr(e) => LambdaBody::Expr(Box::new(self.rexpr(*e))),
                    LambdaBody::Block(stmts) => LambdaBody::Block(self.rblock(stmts)),
                },
                span,
            },
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
            } => Stmt::VarDecl {
                ty,
                name,
                init: self.rexpr(init),
                mutable,
                span,
            },
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
                value: value.map(|e| self.rexpr(e)),
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
