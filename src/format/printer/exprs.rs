//! Printer — expressions: the Wadler doc-IR emission, chains, string literals, patterns.

use super::*;

impl Printer<'_> {
    pub(super) fn expr(&self, e: &Expr) -> Result<String, String> {
        // The flat rendering of an expression's layout Doc reproduces the legacy single-line form. All
        // non-wrapping contexts (interpolation holes, decl headers, control-flow conditions, inlined
        // lambda bodies) route through here, so their output is byte-for-byte unchanged.
        Ok(doc::render_flat(&self.expr_doc(e)?))
    }

    /// Build the layout [`Doc`] for an expression — the width-canonical core (DEC-187). Flat rendering
    /// reproduces the legacy single-line form byte-for-byte; the break points (call/`new`/`match`
    /// args, collection and map literals, `match` arms) fire only when a statement value is rendered
    /// through [`Self::render_expr`] and the enclosing [`doc::Group`] would overflow the width budget.
    /// Contains NO hard break, so forced-flat rendering inside interpolation is always well defined.
    pub(super) fn expr_doc(&self, e: &Expr) -> Result<Doc, String> {
        // A postfix "call chain" (≥2 member accesses on one spine) breaks before each `.`/`?.` as a
        // single group when the line overflows — the flagship width-canonical case. Non-chains (and
        // chains with <2 dots) fall through to the per-node arms below, unchanged.
        if let Some(d) = self.chain_doc(e)? {
            return Ok(d);
        }
        match e {
            Expr::Int(n, _) => Ok(doc::text(n.to_string())),
            Expr::Float(f, _) => Ok(doc::text(format!("{f:?}"))),
            // A `decimal` literal prints back as its rendered value + the `d` suffix (M-NUM S1) — the
            // round-trip-faithful surface form (`19.99d`).
            Expr::Decimal {
                unscaled, scale, ..
            } => Ok(doc::text(format!(
                "{}d",
                crate::value::fmt_decimal(*unscaled, *scale)
            ))),
            Expr::Bool(b, _) => Ok(doc::text(b.to_string())),
            Expr::Null(_) => Ok(doc::text("null")),
            Expr::Str(parts, _) => Ok(doc::text(self.str_lit(parts)?)),
            Expr::Ident(name, _) => Ok(doc::text(name.clone())),
            Expr::This(_) => Ok(doc::text("this")),
            Expr::List(items, _) => {
                let xs: Result<Vec<_>, _> = items.iter().map(|x| self.expr_doc(x)).collect();
                Ok(bracketed("[", xs?, "]"))
            }
            // A tuple literal `(a, b)` — parens, not brackets (DEC-288). Formatted on the raw AST,
            // before the desugar-to-List erasure, so the surface syntax round-trips.
            Expr::Tuple(items, _) => {
                let xs: Result<Vec<_>, _> = items.iter().map(|x| self.expr_doc(x)).collect();
                Ok(bracketed("(", xs?, ")"))
            }
            // `new List<T>()` / `new Map<K,V>()` (DEC-214) — always inline (no elements to wrap).
            Expr::NewColl { kind, args, .. } => {
                let rendered: Result<Vec<_>, _> = args.iter().map(ty).collect();
                Ok(doc::text(format!(
                    "new {}<{}>()",
                    kind.name(),
                    rendered?.join(", ")
                )))
            }
            Expr::Map(pairs, _) => {
                let mut xs = Vec::new();
                for (k, v) in pairs {
                    xs.push(doc::concat(vec![
                        self.expr_doc(k)?,
                        doc::text(" => "),
                        self.expr_doc(v)?,
                    ]));
                }
                Ok(bracketed("[", xs, "]"))
            }
            Expr::Unary { op, expr, .. } => {
                // Unary binds tighter than every binary op; a binary/range operand needs parens, and
                // so does a nested unary (to avoid `--`/`~~` re-lexing as a multi-char token).
                let needs = prec_of(expr) < PREC_UNARY || matches!(**expr, Expr::Unary { .. });
                let inner = self.expr_doc(expr)?;
                let inner = if needs { parens(inner) } else { inner };
                Ok(doc::concat(vec![doc::text(unary_op(*op)), inner]))
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let p = bin_prec(*op);
                let right_assoc = matches!(op, BinaryOp::Pow);
                let l = self.operand_doc(lhs, p, false, right_assoc)?;
                let r = self.operand_doc(rhs, p, true, right_assoc)?;
                Ok(doc::concat(vec![
                    l,
                    doc::text(format!(" {} ", binary_op(*op))),
                    r,
                ]))
            }
            // `lhs |> rhs` (DEC-239) — round-tripped faithfully; left-associative at the pipe's
            // precedence, exactly like a `Binary` with `BinaryOp::Pipe`.
            Expr::Pipe { lhs, rhs, .. } => {
                let p = bin_prec(BinaryOp::Pipe);
                let l = self.operand_doc(lhs, p, false, false)?;
                let r = self.operand_doc(rhs, p, true, false)?;
                Ok(doc::concat(vec![l, doc::text(" |> ".to_string()), r]))
            }
            // A bare `%` placeholder argument (DEC-239) — a whole-argument slot of a pipe's RHS call.
            Expr::PipePlaceholder(_) => Ok(doc::text("%".to_string())),
            Expr::InstanceOf {
                value, type_name, ..
            } => {
                // `instanceof` is a left-precedence-8 test; its value operand needs parens below 8.
                let v = self.operand_doc(value, 8, false, false)?;
                Ok(doc::concat(vec![
                    v,
                    doc::text(format!(" instanceof {type_name}")),
                ]))
            }
            Expr::Cast {
                value, type_name, ..
            } => {
                // `as` is a left-precedence-8 cast (same level as `instanceof`); its value operand
                // needs parens below 8.
                let v = self.operand_doc(value, 8, false, false)?;
                Ok(doc::concat(vec![v, doc::text(format!(" as {type_name}"))]))
            }
            // `<Type>f(args)` — a return-overload selector prefix (Slice C1). Prints the selector type
            // in angle brackets immediately before its call (NOT a cast — `as` is the cast).
            Expr::OverloadSelect { ty: t, call, .. } => Ok(doc::concat(vec![
                doc::text(format!("<{}>", ty(t)?)),
                self.expr_doc(call)?,
            ])),
            // `parent.m(args)` / `parent(A).m(args)` — super/parent dispatch (M-RT super/parent).
            Expr::ParentCall {
                ancestor,
                method,
                args,
                ..
            } => {
                let head = match ancestor {
                    Some(a) => format!("parent({a})"),
                    None => "parent".to_string(),
                };
                Ok(doc::concat(vec![
                    doc::text(format!("{head}.{method}")),
                    self.args_doc(args)?,
                ]))
            }
            Expr::Call {
                callee,
                args,
                type_args,
                ..
            } => Ok(doc::concat(vec![
                self.postfix_doc(callee)?,
                self.turbofish_doc(type_args)?,
                self.args_doc(args)?,
            ])),
            Expr::Member {
                object,
                name,
                safe,
                sep,
                ..
            } => {
                // DEC-207: render the written separator — `::` for class-level access, else `?.`/`.`.
                let dot = match sep {
                    crate::ast::MemberSep::ColonColon => "::",
                    _ if *safe => "?.",
                    _ => ".",
                };
                Ok(doc::concat(vec![
                    self.postfix_doc(object)?,
                    doc::text(format!("{dot}{name}")),
                ]))
            }
            Expr::Index { object, index, .. } => Ok(doc::concat(vec![
                self.postfix_doc(object)?,
                doc::text("["),
                self.expr_doc(index)?,
                doc::text("]"),
            ])),
            Expr::Force { inner, .. } => {
                Ok(doc::concat(vec![self.postfix_doc(inner)?, doc::text("!")]))
            }
            Expr::Propagate { inner, .. } => {
                Ok(doc::concat(vec![self.postfix_doc(inner)?, doc::text("?")]))
            }
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                // Range is the loosest expression (operands are full binaries); only a nested range
                // operand needs parens.
                let dots = if *inclusive { "..=" } else { ".." };
                let wrap = |pr: &Self, e: &Expr| -> Result<Doc, String> {
                    let d = pr.expr_doc(e)?;
                    // Only a nested range (the single loosest form) needs parens here.
                    Ok(if prec_of(e) == PREC_RANGE {
                        parens(d)
                    } else {
                        d
                    })
                };
                Ok(doc::concat(vec![
                    wrap(self, start)?,
                    doc::text(dots),
                    wrap(self, end)?,
                ]))
            }
            Expr::If {
                cond,
                then_expr,
                else_expr,
                ..
            } => Ok(doc::text(format!(
                "if ({}) {{ {} }} else {{ {} }}",
                self.expr(cond)?,
                self.expr(then_expr)?,
                self.expr(else_expr)?
            ))),
            Expr::New(inner, _) => Ok(doc::concat(vec![doc::text("new "), self.expr_doc(inner)?])),
            // `spawn <call>` (M6 W4) — contextual keyword, printed as a prefix on the call.
            Expr::Spawn { call, .. } => {
                Ok(doc::concat(vec![doc::text("spawn "), self.expr_doc(call)?]))
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let mut arm_docs = Vec::new();
                for arm in arms {
                    let guard = match &arm.guard {
                        Some(g) => format!(" when {}", self.expr(g)?),
                        None => String::new(),
                    };
                    arm_docs.push(doc::concat(vec![
                        doc::text(format!("{}{guard} => ", self.arm_pattern(&arm.pattern)?)),
                        self.expr_doc(&arm.body)?,
                    ]));
                }
                // `match (s) { a => b, c => d }` flat; one arm per line when it overflows. The
                // scrutinee stays on the head line (rendered flat via `self.expr`).
                Ok(doc::concat(vec![
                    doc::text(format!("match ({}) {{", self.expr(scrutinee)?)),
                    doc::group(doc::concat(vec![
                        doc::nest(
                            4,
                            doc::concat(vec![
                                doc::line(),
                                doc::join(arm_docs, doc::concat(vec![doc::text(","), doc::line()])),
                            ]),
                        ),
                        doc::line(),
                        doc::text("}"),
                    ])),
                ]))
            }
            Expr::Bytes(bytes, _) => Ok(doc::text(format!("b\"{}\"", escape_bytes(bytes)))),
            Expr::Lambda {
                params,
                ret,
                throws,
                body,
                ..
            } => {
                // DEC-239: a contextually-typed pipe lambda — the single param carries `Type::Infer`
                // (only the pipe parser produces one) — round-trips in its short surface form
                // `v => expr`; the enclosing pipe's precedence wrap restores the `( … )`.
                if let [p] = params.as_slice() {
                    if matches!(p.ty, crate::ast::Type::Infer(_)) {
                        if let LambdaBody::Expr(e) = body {
                            return Ok(doc::concat(vec![
                                doc::text(format!("{} => ", p.name)),
                                self.expr_doc(e)?,
                            ]));
                        }
                    }
                }
                let ps = self.params(params)?;
                // DEC-222: render the lambda's `throws` clause (after the return annotation, before the
                // body) so a throwing lambda round-trips through `phg format`. Empty ⇒ nothing.
                let th = if throws.is_empty() {
                    String::new()
                } else {
                    let es: Result<Vec<_>, _> = throws.iter().map(ty).collect();
                    format!(" throws {}", es?.join(", "))
                };
                match body {
                    LambdaBody::Expr(e) => {
                        // Expression body: `function(params)[: Ret] [throws E] => expr` (the `: Ret`
                        // annotation is optional on an expression lambda; print it when present).
                        let r = match ret {
                            Some(t) => format!(": {}", ty(t)?),
                            None => String::new(),
                        };
                        Ok(doc::concat(vec![
                            doc::text(format!("function({ps}){r}{th} => ")),
                            self.expr_doc(e)?,
                        ]))
                    }
                    LambdaBody::Block(stmts) => {
                        // Statement body: `function(params): Ret [throws E] { … }` (return type required).
                        let r = match ret {
                            Some(t) => format!(": {}", ty(t)?),
                            None => String::new(),
                        };
                        // A lambda is an expression, so its block body is rendered on one line (v1 has
                        // no reflow). `inline_block` handles any statement, including control flow.
                        Ok(doc::text(format!(
                            "function({ps}){r}{th} {{ {} }}",
                            self.inline_block(stmts)?
                        )))
                    }
                }
            }
            Expr::CloneWith { object, fields, .. } => {
                // `obj with { field = value, … }` — the functional-update syntax uses `=`, not `:`.
                let mut fs = Vec::new();
                for (name, e) in fields {
                    fs.push(doc::concat(vec![
                        doc::text(format!("{name} = ")),
                        self.expr_doc(e)?,
                    ]));
                }
                Ok(doc::concat(vec![
                    self.postfix_doc(object)?,
                    doc::text(" with { "),
                    doc::join(fs, doc::text(", ")),
                    doc::text(" }"),
                ]))
            }
            // DI composition root — rendered faithfully so `phg format` (which parses without running
            // `desugar_di`) round-trips it. The parser only ever produces this node for the explicit
            // turbofish surface (`inject<T>()` / `DependencyInjection.inject<T>()`); the annotation forms are ordinary
            // calls handled by the `Call` path. `qualified` restores the `DependencyInjection.` prefix.
            Expr::Inject {
                ty: t, qualified, ..
            } => {
                let head = if *qualified {
                    "DependencyInjection.inject"
                } else {
                    "inject"
                };
                Ok(doc::text(match t {
                    Some(inner) => format!("{head}<{}>()", ty(inner)?),
                    None => format!("{head}()"),
                }))
            }
            // `html"…"` literal — same segment model as a string, different delimiter.
            Expr::Html(parts, _) => {
                let mut s = String::from("html\"");
                for part in parts {
                    match part {
                        StrPart::Literal(lit) => s.push_str(&escape_str(lit)),
                        StrPart::Expr(e) => {
                            s.push_str(&format!("{{{}}}", escape_interp(&self.expr(e)?)));
                        }
                    }
                }
                s.push('"');
                Ok(doc::text(s))
            }
            // A generalized tagged template `tag"…"` — identical rendering to html, only the tag
            // prefix differs (DEC-212 scaffold).
            Expr::TaggedTemplate { tag, parts, .. } => {
                let mut s = format!("{tag}\"");
                for part in parts {
                    match part {
                        StrPart::Literal(lit) => s.push_str(&escape_str(lit)),
                        StrPart::Expr(e) => {
                            s.push_str(&format!("{{{}}}", escape_interp(&self.expr(e)?)));
                        }
                    }
                }
                s.push('"');
                Ok(doc::text(s))
            }
        }
    }

    /// Render a statement list on one line (for a statement-body lambda — a lambda is an expression,
    /// so v1 prints its block inline; no reflow). Each statement via [`Self::stmt_inline_any`].
    pub(super) fn inline_block(&self, stmts: &[Stmt]) -> Result<String, String> {
        let xs: Result<Vec<_>, _> = stmts.iter().map(|s| self.stmt_inline_any(s)).collect();
        Ok(xs?.join(" "))
    }

    /// Render ANY statement to a single line (trailing `;` where one belongs, nested blocks as
    /// `{ … }`). Total over every `Stmt` variant — the lambda-block path needs full coverage, unlike
    /// the for-clause [`Self::stmt_inline`] (which the parser restricts to var-decl/assign/expr).
    pub(super) fn stmt_inline_any(&self, s: &Stmt) -> Result<String, String> {
        match s {
            Stmt::VarDecl {
                ty: t,
                name,
                init,
                mutable,
                ..
            } => {
                let m = if *mutable { "mutable " } else { "" };
                Ok(format!("{m}{} {name} = {};", ty(t)?, self.expr(init)?))
            }
            Stmt::Assign { target, value, .. } => {
                Ok(format!("{} = {};", self.expr(target)?, self.expr(value)?))
            }
            Stmt::Return { value, .. } => match value {
                Some(e) => Ok(format!("return {};", self.expr(e)?)),
                None => Ok("return;".to_string()),
            },
            Stmt::Expr(e, _) => Ok(format!("{};", self.expr(e)?)),
            Stmt::Discard(e, _) => Ok(format!("discard {};", self.expr(e)?)),
            Stmt::Break(_) => Ok("break;".to_string()),
            Stmt::Continue(_) => Ok("continue;".to_string()),
            Stmt::Throw { value, .. } => Ok(format!("throw {};", self.expr(value)?)),
            Stmt::Block(b, _) => Ok(format!("{{ {} }}", self.inline_block(b)?)),
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                ..
            } => {
                let c = match bind {
                    Some(name) => format!("var {name} = {}", self.expr(cond)?),
                    None => self.expr(cond)?,
                };
                let mut out = format!("if ({c}) {{ {} }}", self.inline_block(then_block)?);
                if let Some(eb) = else_block {
                    out.push_str(&format!(" else {{ {} }}", self.inline_block(eb)?));
                }
                Ok(out)
            }
            Stmt::While {
                cond,
                body,
                post_cond,
                ..
            } => {
                if *post_cond {
                    Ok(format!(
                        "do {{ {} }} while ({});",
                        self.inline_block(body)?,
                        self.expr(cond)?
                    ))
                } else {
                    Ok(format!(
                        "while ({}) {{ {} }}",
                        self.expr(cond)?,
                        self.inline_block(body)?
                    ))
                }
            }
            Stmt::For {
                ty: t,
                name,
                iter,
                body,
                ..
            } => {
                let head = if matches!(t, Type::Infer(_)) {
                    format!("foreach ({} as {name})", self.expr(iter)?)
                } else {
                    format!("for ({} {name} in {})", ty(t)?, self.expr(iter)?)
                };
                Ok(format!("{head} {{ {} }}", self.inline_block(body)?))
            }
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                ..
            } => {
                let i = match init {
                    Some(s) => self.stmt_inline(s)?,
                    None => String::new(),
                };
                let c = match cond {
                    Some(e) => self.expr(e)?,
                    None => String::new(),
                };
                let st = match step {
                    Some(s) => self.stmt_inline(s)?,
                    None => String::new(),
                };
                Ok(format!(
                    "for ({i}; {c}; {st}) {{ {} }}",
                    self.inline_block(body)?
                ))
            }
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                let mut out = format!("try {{ {} }}", self.inline_block(body)?);
                for cat in catches {
                    out.push_str(&format!(
                        " catch ({} {}) {{ {} }}",
                        ty(&cat.ty)?,
                        cat.name,
                        self.inline_block(&cat.body)?
                    ));
                }
                if let Some(fb) = finally_block {
                    out.push_str(&format!(" finally {{ {} }}", self.inline_block(fb)?));
                }
                Ok(out)
            }
            Stmt::Destructure {
                pat,
                init,
                else_block,
                ..
            } => {
                let kw = if crate::format::printer::stmts::explicit_tuple_pat(pat) {
                    ""
                } else {
                    "var "
                };
                let head = format!("{kw}{} = {}", self.destructure_pat(pat)?, self.expr(init)?);
                match else_block {
                    None => Ok(format!("{head};")),
                    Some(eb) => Ok(format!("{head} else {{ {} }}", self.inline_block(eb)?)),
                }
            }
        }
    }

    pub(super) fn str_lit(&self, parts: &[StrPart]) -> Result<String, String> {
        let mut s = String::from("\"");
        for part in parts {
            match part {
                StrPart::Literal(lit) => s.push_str(&escape_str(lit)),
                // An interpolation hole's expression is re-lexed by the parser, so any `"`/`}`/`\` in
                // its printed form must be escaped or it would close the string / the hole early
                // (e.g. `{scores["alice"]}` ⇒ `{scores[\"alice\"]}`).
                StrPart::Expr(e) => s.push_str(&format!("{{{}}}", escape_interp(&self.expr(e)?))),
            }
        }
        s.push('"');
        Ok(s)
    }

    /// Render a statement's value expression width-canonically: `<prefix><expr><suffix>` laid out so
    /// the expression wraps (call/`new` args, collection/map literals, `match` arms, member chains)
    /// when the whole line would exceed [`doc::MAX_WIDTH`]. `prefix`/`suffix` are the fixed statement
    /// scaffolding (`return `…`;`) — the prefix stays on line 1 and its width counts against the
    /// budget; broken continuation lines hang past the statement indent. The returned string's FIRST
    /// line carries no leading indent (the caller's [`Self::line`] prepends it); continuation lines
    /// carry their absolute indentation. Idempotent by construction: derived purely from the AST.
    pub(super) fn render_expr(
        &self,
        prefix: &str,
        e: &Expr,
        suffix: &str,
    ) -> Result<String, String> {
        let base = self.indent * 4;
        let start_col = base + prefix.chars().count();
        let d = doc::concat(vec![self.expr_doc(e)?, doc::text(suffix)]);
        Ok(format!(
            "{prefix}{}",
            doc::render(&d, doc::MAX_WIDTH, base, start_col, false)
        ))
    }

    /// Layout doc for a comma-separated, parenthesized argument list (`(a, b, c)`) — the shared break
    /// group behind calls, `parent.m(…)`, and `new`. Empty → `()`; otherwise flat as `(a, b)` and one
    /// argument per line (indented) when the group overflows.
    pub(super) fn args_doc(&self, args: &[Expr]) -> Result<Doc, String> {
        let xs: Result<Vec<_>, _> = args.iter().map(|a| self.expr_doc(a)).collect();
        Ok(bracketed("(", xs?, ")"))
    }

    /// DEC-208 slice A: render a call's turbofish type arguments (`<T, U>`) immediately before its
    /// `(args)`. Empty doc in the common inferred form (`type_args` empty), so a non-turbofish call is
    /// byte-identical to before.
    pub(super) fn turbofish_doc(&self, type_args: &[Type]) -> Result<Doc, String> {
        if type_args.is_empty() {
            return Ok(doc::text(String::new()));
        }
        let ts: Result<Vec<_>, _> = type_args.iter().map(ty).collect();
        Ok(doc::text(format!("<{}>", ts?.join(", "))))
    }

    /// If `e` is a postfix "call chain" spine with ≥2 member accesses (`a.b(…).c(…)`), lay it out as a
    /// single break group: the head (plus any leading `()`/`[]`/`!`/`?` before the first dot) stays on
    /// line 1, then each `.`/`?.` link breaks onto its own line (indented four columns) when the chain
    /// overflows. Flat form is byte-identical to the per-node concat, so idempotence and meaning are
    /// preserved. Returns `None` for anything that is not a ≥2-dot chain (handled by the per-node arms).
    pub(super) fn chain_doc(&self, e: &Expr) -> Result<Option<Doc>, String> {
        enum Seg<'a> {
            Dot(&'a str, bool, crate::ast::MemberSep),
            // DEC-208 slice A: a call segment carries its turbofish type arguments (empty in the
            // common form) so `.method<T>(args)` round-trips through the chain layout.
            Args(&'a [Expr], &'a [Type]),
            Index(&'a Expr),
            Force,
            Propagate,
        }
        let mut segs: Vec<Seg> = Vec::new();
        let mut cur = e;
        loop {
            match cur {
                Expr::Member {
                    object,
                    name,
                    safe,
                    sep,
                    ..
                } => {
                    segs.push(Seg::Dot(name, *safe, *sep));
                    cur = object;
                }
                Expr::Call {
                    callee,
                    args,
                    type_args,
                    ..
                } => {
                    segs.push(Seg::Args(args, type_args));
                    cur = callee;
                }
                Expr::Index { object, index, .. } => {
                    segs.push(Seg::Index(index));
                    cur = object;
                }
                Expr::Force { inner, .. } => {
                    segs.push(Seg::Force);
                    cur = inner;
                }
                Expr::Propagate { inner, .. } => {
                    segs.push(Seg::Propagate);
                    cur = inner;
                }
                _ => break,
            }
        }
        if segs.iter().filter(|s| matches!(s, Seg::Dot(..))).count() < 2 {
            return Ok(None);
        }
        segs.reverse();
        let seg_doc = |pr: &Self, s: &Seg| -> Result<Doc, String> {
            Ok(match s {
                Seg::Dot(name, safe, sep) => {
                    // DEC-207: render the written separator (`::` for class-level, else `?.`/`.`).
                    let d = match sep {
                        crate::ast::MemberSep::ColonColon => "::",
                        _ if *safe => "?.",
                        _ => ".",
                    };
                    doc::text(format!("{d}{name}"))
                }
                Seg::Args(args, type_args) => {
                    doc::concat(vec![pr.turbofish_doc(type_args)?, pr.args_doc(args)?])
                }
                Seg::Index(ix) => {
                    doc::concat(vec![doc::text("["), pr.expr_doc(ix)?, doc::text("]")])
                }
                Seg::Force => doc::text("!"),
                Seg::Propagate => doc::text("?"),
            })
        };
        // Head + any leading postfixes before the first `.` stay on the first line.
        let mut head_line = vec![self.postfix_doc(cur)?];
        let mut i = 0;
        while i < segs.len() && !matches!(segs[i], Seg::Dot(..)) {
            head_line.push(seg_doc(self, &segs[i])?);
            i += 1;
        }
        // Remaining links: a soft break before every `.`/`?.`; trailing `()`/`[]`/`!`/`?` ride its line.
        let mut chain = Vec::new();
        while i < segs.len() {
            if matches!(segs[i], Seg::Dot(..)) {
                chain.push(doc::softline());
            }
            chain.push(seg_doc(self, &segs[i])?);
            i += 1;
        }
        Ok(Some(doc::concat(vec![
            doc::concat(head_line),
            doc::group(doc::nest(4, doc::concat(chain))),
        ])))
    }

    /// Layout doc for a binary operand, parenthesizing only when precedence/associativity requires it.
    /// `parent` is the operator's binding power; `is_right`/`right_assoc` pick the associativity
    /// side. Left-assoc: the right operand needs parens at equal precedence; right-assoc (`**`):
    /// the left operand does.
    pub(super) fn operand_doc(
        &self,
        e: &Expr,
        parent: u8,
        is_right: bool,
        right_assoc: bool,
    ) -> Result<Doc, String> {
        let cp = prec_of(e);
        let needs = if is_right == right_assoc {
            cp < parent
        } else {
            cp <= parent
        };
        let d = self.expr_doc(e)?;
        Ok(if needs { parens(d) } else { d })
    }

    /// Layout doc for the receiver of a postfix operator (`.`/`[]`/call/`!`/`?`), which binds tighter
    /// than every prefix/binary form — so a non-atomic receiver (a binary, unary, or range) needs
    /// parens.
    pub(super) fn postfix_doc(&self, e: &Expr) -> Result<Doc, String> {
        let d = self.expr_doc(e)?;
        Ok(if prec_of(e) < PREC_ATOM { parens(d) } else { d })
    }

    /// Render a **top-level match-arm** pattern. A catch-all Wildcard prints as `default` (DEC-209 —
    /// a top-level arm Wildcard can only originate from `default`, since a standalone `_` arm is a
    /// parse error); every other pattern, and any *nested* Wildcard (`Some(_)`), renders via `pattern`.
    fn arm_pattern(&self, p: &Pattern) -> Result<String, String> {
        match p {
            Pattern::Wildcard(_) => Ok("default".to_string()),
            _ => self.pattern(p),
        }
    }

    pub(super) fn pattern(&self, p: &Pattern) -> Result<String, String> {
        match p {
            Pattern::Wildcard(_) => Ok("_".to_string()),
            Pattern::Binding { name, .. } => Ok(name.clone()),
            Pattern::Int(n, _) => Ok(n.to_string()),
            Pattern::Float(f, _) => Ok(format!("{f:?}")),
            Pattern::Decimal {
                unscaled, scale, ..
            } => Ok(format!("{}d", crate::value::fmt_decimal(*unscaled, *scale))),
            Pattern::Str(s, _) => Ok(format!("\"{}\"", escape_str(s))),
            Pattern::Bool(b, _) => Ok(b.to_string()),
            Pattern::Null(_) => Ok("null".to_string()),
            Pattern::Variant {
                name,
                fields,
                enum_qualifier,
                ..
            } => {
                let fs: Result<Vec<_>, _> = fields.iter().map(|f| self.pattern(f)).collect();
                // Preserve a qualified pattern `Enum.Variant(..)` (variant-qualification A2/B) — an
                // injected enum's variant is match-legal ONLY qualified, so dropping the qualifier
                // would change behavior (E-INJECTED-VARIANT-BARE) and break fmt's meaning-preservation.
                let head = match enum_qualifier {
                    Some(q) => format!("{q}.{name}"),
                    None => name.clone(),
                };
                Ok(format!("{head}({})", fs?.join(", ")))
            }
            Pattern::Type {
                type_name, binding, ..
            } => match binding {
                Some(b) => Ok(format!("{type_name} {b}")),
                None => Ok(format!("{type_name} _")),
            },
            Pattern::Struct {
                type_name, fields, ..
            } => {
                let fs: Result<Vec<_>, _> = fields
                    .iter()
                    .map(|f: &FieldPat| {
                        Ok::<_, String>(format!("{}: {}", f.field, self.pattern(&f.pat)?))
                    })
                    .collect();
                Ok(format!("{type_name} {{ {} }}", fs?.join(", ")))
            }
        }
    }
}
