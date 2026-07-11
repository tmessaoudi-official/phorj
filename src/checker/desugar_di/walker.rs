//! DI v1 — the `Di` walker: graph resolution (SHARED/TRANSIENT/providers) and the TOTAL
//! expression/statement rewrite (new expr-AST forms MUST add an arm here — see MEMORY).

use super::*;

impl Di<'_> {
    /// Resolve one *requested* type name to its concrete injectable class, or record a diagnostic.
    pub(super) fn resolve_concrete(&mut self, name: &str, span: Span) -> Option<String> {
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

    /// Resolve a requested type name to its construction node: `(key, how-to-construct, dep param types)`.
    /// Precedence (advisor-ruled): a `#[Provides]` factory for the type WINS over `new`/single-impl; then
    /// an `#[Injectable]` class; then a single-implementation interface (whose impl may itself be
    /// provider-built). Records the matching diagnostic and returns `None` on ambiguity/missing.
    pub(super) fn resolve_node(&mut self, name: &str, span: Span) -> Option<Node> {
        // A provider for the requested type itself takes precedence over `new` and interface auto-bind.
        if let Some(node) = self.provider_node(name, span) {
            return node;
        }
        // Otherwise resolve to a concrete injectable class (interface → its single impl).
        let concrete = self.resolve_concrete(name, span)?;
        // The concrete impl may itself be provider-built (a `#[Provides]` on the chosen impl).
        if let Some(node) = self.provider_node(&concrete, span) {
            return node;
        }
        let deps = self.reg.deps.get(&concrete).cloned().unwrap_or_default();
        Some((concrete.clone(), Construct::New(concrete), deps))
    }

    /// If `name` has a `#[Provides]` factory, the node that builds it via that factory — or `Some(None)`
    /// (via the outer `?`) after recording `E-DI-AMBIGUOUS` when more than one provider returns `name`.
    /// Returns `None` (the outer Option) when `name` has no provider at all.
    pub(super) fn provider_node(&mut self, name: &str, span: Span) -> Option<Option<Node>> {
        if self.reg.ambiguous_providers.contains(name) {
            self.diags.push(
                err(
                    format!("ambiguous injection of `{name}`: more than one `#[Provides]` factory returns it"),
                    span,
                )
                .with_code("E-DI-AMBIGUOUS")
                .with_hint("declare exactly one `#[Provides]` factory per provided type".to_string()),
            );
            return Some(None);
        }
        self.reg.providers.get(name).map(|(owner, method, params)| {
            Some((
                name.to_string(),
                Construct::Provides(owner.clone(), method.clone()),
                params.clone(),
            ))
        })
    }

    /// Resolve the full dependency graph for `name` into a [`Built`] tree. `in_progress` (the DFS path)
    /// detects cycles; `shared_cache` memoizes SHARED subtrees so a diamond does not rebuild (and does not
    /// blow up) — a TRANSIENT subtree is never cached, so it is rebuilt fresh at each use. Records a
    /// diagnostic and returns `None` on any failure.
    pub(super) fn resolve_graph(
        &mut self,
        name: &str,
        span: Span,
        in_progress: &mut Vec<String>,
        shared_cache: &mut BTreeMap<String, Built>,
    ) -> Option<Built> {
        let (key, construct, dep_params) = self.resolve_node(name, span)?;
        let transient = self.reg.transient.contains(&key);
        if !transient {
            if let Some(b) = shared_cache.get(&key) {
                return Some(b.clone()); // a shared subtree — built once, reused (diamond)
            }
        }
        if in_progress.contains(&key) {
            let mut chain = in_progress.clone();
            chain.push(key.clone());
            self.diags.push(
                err(
                    format!("dependency cycle in injection: {}", chain.join(" → ")),
                    span,
                )
                .with_code("E-DI-CYCLE")
                .with_hint(
                    "break the cycle — the construction graph must be acyclic (field-injection cycle-breaking is not in v1)"
                        .to_string(),
                ),
            );
            return None;
        }
        in_progress.push(key.clone());
        let mut deps = Vec::new();
        for (dep_name, dep_span) in dep_params {
            let Some(dep) = dep_name else {
                self.diags.push(
                    err(
                        format!("`{key}` has a dependency whose type is not injectable"),
                        dep_span,
                    )
                    .with_code("E-DI-MISSING")
                    .with_hint(
                        "every dependency must be an injectable class, a single-impl interface, or a `#[Provides]`-provided type (raw config-value provision is a later slice)"
                            .to_string(),
                    ),
                );
                in_progress.pop();
                return None;
            };
            let Some(built) = self.resolve_graph(&dep, dep_span, in_progress, shared_cache) else {
                in_progress.pop();
                return None;
            };
            deps.push(built);
        }
        in_progress.pop();
        let built = Built {
            key: key.clone(),
            construct,
            transient,
            deps,
        };
        if !transient {
            shared_cache.insert(key, built.clone());
        }
        Some(built)
    }

    /// Resolve (memoized) the requested type `t`; returns the factory name to call, or `None` on error.
    pub(super) fn factory_for(&mut self, t: &str, span: Span) -> Option<String> {
        if let Some(entry) = self.resolved.get(t) {
            return entry.as_ref().map(|_| factory_name(t));
        }
        let mut in_progress = Vec::new();
        let mut shared_cache = BTreeMap::new();
        let built = self.resolve_graph(t, span, &mut in_progress, &mut shared_cache);
        match built {
            Some(built) => {
                self.resolved.insert(t.to_string(), Some(built));
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

    pub(super) fn ritem(&mut self, it: Item) -> Item {
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
    pub(super) fn rfn(&mut self, f: &mut FunctionDecl) {
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
    pub(super) fn rmember(&mut self, m: &mut ClassMember) {
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

    pub(super) fn rblock(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        stmts.into_iter().map(|s| self.rstmt(s)).collect()
    }

    /// Gate a composition root against the import discipline (§7), then resolve it. `ty` is the explicit
    /// turbofish target (`Some`) or `None` for the annotation form; `expected` is the annotation source
    /// (a typed declaration / return type — already `Infer`-stripped by the caller); `qualified` selects
    /// which import gates it.
    pub(super) fn rinject(
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
    pub(super) fn inject_to_call(&mut self, name: &str, span: Span) -> Expr {
        match self.factory_for(name, span) {
            Some(fname) => Expr::Call {
                callee: Box::new(Expr::Ident(fname, span)),
                args: Vec::new(),
                span,
            },
            None => self.di_error_placeholder(span),
        }
    }

    pub(super) fn di_error_placeholder(&self, span: Span) -> Expr {
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
    pub(super) fn annotation_inject(&self, callee: &Expr) -> Option<bool> {
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
    pub(super) fn rexpr_expected(&mut self, e: Expr, expected: Option<&Type>) -> Expr {
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

    pub(super) fn rexpr(&mut self, e: Expr) -> Expr {
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

    pub(super) fn rparts(&mut self, parts: Vec<StrPart>) -> Vec<StrPart> {
        parts
            .into_iter()
            .map(|p| match p {
                StrPart::Expr(e) => StrPart::Expr(Box::new(self.rexpr(*e))),
                lit => lit,
            })
            .collect()
    }

    pub(super) fn rstmt(&mut self, s: Stmt) -> Stmt {
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
