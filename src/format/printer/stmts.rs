//! Printer — statements: blocks, if/else shaping, try, destructuring, inline forms.

use super::*;

/// Whether a destructure pat is the EXPLICIT-type tuple form `(T a, U b)` — every binder typed. It
/// prints WITHOUT a `var` prefix (`(int a, string b) = …`), unlike every other destructure form
/// (`var [a,b]`, `var Type{…}`, inferred `var (a,b)`), which keep `var` (DEC-288).
pub(crate) fn explicit_tuple_pat(p: &DestructurePat) -> bool {
    matches!(p, DestructurePat::Tuple { binders, .. } if binders.iter().all(|(t, _, _)| t.is_some()))
}

impl Printer<'_> {
    pub(super) fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        // Flush any comments that appear before this statement in source (own-line placement). v1
        // reattaches to the nearest statement boundary — a trailing same-line comment becomes a
        // leading comment of the next node (documented limitation; comments are never lost).
        self.flush_comments_before(stmt_start(s));
        match s {
            Stmt::VarDecl {
                ty: t,
                name,
                init,
                mutable,
                ..
            } => {
                let m = if *mutable { "mutable " } else { "" };
                let s = self.render_expr(&format!("{m}{} {name} = ", ty(t)?), init, ";")?;
                self.line(&s);
                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                let prefix = format!("{} = ", self.expr(target)?);
                let s = self.render_expr(&prefix, value, ";")?;
                self.line(&s);
                Ok(())
            }
            Stmt::Return { value, .. } => {
                match value {
                    Some(e) => {
                        let s = self.render_expr("return ", e, ";")?;
                        self.line(&s);
                    }
                    None => self.line("return;"),
                }
                Ok(())
            }
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                ..
            } => self.if_stmt(cond, bind.as_deref(), then_block, else_block.as_deref()),
            Stmt::While {
                cond,
                body,
                post_cond,
                ..
            } => {
                if *post_cond {
                    self.line("do {");
                    self.indent += 1;
                    for st in body {
                        self.stmt(st)?;
                    }
                    self.indent -= 1;
                    self.line(&format!("}} while ({});", self.expr(cond)?));
                } else {
                    self.block_stmt(&format!("while ({})", self.expr(cond)?), body)?;
                }
                Ok(())
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
                let s = match step {
                    Some(s) => self.stmt_inline(s)?,
                    None => String::new(),
                };
                self.block_stmt(&format!("for ({i}; {c}; {s})"), body)
            }
            Stmt::For {
                ty: t,
                name,
                val,
                iter,
                body,
                ..
            } => {
                // DEC-288: re-collapse a parser-lowered tuple loop — `for (var __fortup_N in iter) {
                // var (a, b) = __fortup_N; <rest> }` — back to its `for ((a, b) in iter) { <rest> }`
                // surface, so the sugar round-trips through the formatter (and the synthetic binder
                // never leaks). Meaning-preserving: the collapsed form re-lowers to the same AST.
                if val.is_none() && matches!(t, Type::Infer(_)) && name.starts_with("__fortup_") {
                    if let Some((
                        Stmt::Destructure {
                            pat: DestructurePat::Tuple { binders, .. },
                            init: Expr::Ident(dn, _),
                            ..
                        },
                        rest,
                    )) = body.split_first()
                    {
                        if dn == name {
                            // Preserve each binder's surface — `int n` (explicit) or `n` (inferred) —
                            // so both `for ((int n, …))` and `for ((n, …))` round-trip.
                            let parts: Result<Vec<String>, String> = binders
                                .iter()
                                .map(|(ty_opt, n, _)| match ty_opt {
                                    Some(t) => Ok(format!("{} {n}", ty(t)?)),
                                    None => Ok(n.clone()),
                                })
                                .collect();
                            let head =
                                format!("for (({}) in {})", parts?.join(", "), self.expr(iter)?);
                            return self.block_stmt(&head, rest);
                        }
                    }
                }
                // An inferred-element for-in prints as the idiomatic `foreach (iter as name)`
                // (A-6); an explicit element type keeps the typed `for (T name in iter)` form; a
                // fully-typed two-binding Map form prints `for (K k, V v in iter)` (B1).
                // DEC-280: a two-binding form with ANY inferred binding has no `for` spelling —
                // it prints the foreach form, each binding bare-or-typed as declared.
                let head = if let Some((vt, vname)) = val {
                    if matches!(t, Type::Infer(_)) || matches!(vt, Type::Infer(_)) {
                        let k = if matches!(t, Type::Infer(_)) {
                            name.clone()
                        } else {
                            format!("{} {name}", ty(t)?)
                        };
                        let v = if matches!(vt, Type::Infer(_)) {
                            vname.clone()
                        } else {
                            format!("{} {vname}", ty(vt)?)
                        };
                        format!("foreach ({} as {k} => {v})", self.expr(iter)?)
                    } else {
                        format!(
                            "for ({} {name}, {} {vname} in {})",
                            ty(t)?,
                            ty(vt)?,
                            self.expr(iter)?
                        )
                    }
                } else if matches!(t, Type::Infer(_)) {
                    format!("foreach ({} as {name})", self.expr(iter)?)
                } else {
                    format!("for ({} {name} in {})", ty(t)?, self.expr(iter)?)
                };
                self.block_stmt(&head, body)
            }
            Stmt::Break(_) => {
                self.line("break;");
                Ok(())
            }
            Stmt::Continue(_) => {
                self.line("continue;");
                Ok(())
            }
            Stmt::Block(stmts, _) => self.block_stmt("", stmts),
            Stmt::Expr(e, _) => {
                let s = self.render_expr("", e, ";")?;
                self.line(&s);
                Ok(())
            }
            Stmt::Discard(e, _) => {
                let s = self.render_expr("discard ", e, ";")?;
                self.line(&s);
                Ok(())
            }
            Stmt::Throw { value, .. } => {
                let s = self.render_expr("throw ", value, ";")?;
                self.line(&s);
                Ok(())
            }
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => self.try_stmt(body, catches, finally_block.as_deref()),
            Stmt::Destructure {
                pat,
                init,
                else_block,
                ..
            } => {
                // The explicit-type tuple form `(int a, string b) = …` has NO `var`; every other
                // destructure (`var [a,b]`, `var Type{…}`, inferred `var (a,b)`) keeps it (DEC-288).
                let kw = if explicit_tuple_pat(pat) { "" } else { "var " };
                let head = format!("{kw}{} = {}", self.destructure_pat(pat)?, self.expr(init)?);
                match else_block {
                    None => {
                        self.line(&format!("{head};"));
                        Ok(())
                    }
                    Some(eb) => {
                        self.line(&format!("{head} else {{"));
                        self.indent += 1;
                        for s in eb {
                            self.stmt(s)?;
                        }
                        self.indent -= 1;
                        self.line("}");
                        Ok(())
                    }
                }
            }
        }
    }

    pub(super) fn try_stmt(
        &mut self,
        body: &[Stmt],
        catches: &[CatchClause],
        finally_block: Option<&[Stmt]>,
    ) -> Result<(), String> {
        self.line("try {");
        self.indent += 1;
        for s in body {
            self.stmt(s)?;
        }
        self.indent -= 1;
        for c in catches {
            self.line(&format!("}} catch ({} {}) {{", ty(&c.ty)?, c.name));
            self.indent += 1;
            for s in &c.body {
                self.stmt(s)?;
            }
            self.indent -= 1;
        }
        match finally_block {
            Some(fb) => {
                self.line("} finally {");
                self.indent += 1;
                for s in fb {
                    self.stmt(s)?;
                }
                self.indent -= 1;
                self.line("}");
            }
            None => self.line("}"),
        }
        Ok(())
    }

    pub(super) fn destructure_pat(&self, p: &DestructurePat) -> Result<String, String> {
        match p {
            DestructurePat::Struct {
                type_name, fields, ..
            } => {
                let fs: Vec<String> = fields
                    .iter()
                    .map(|f: &DestructureField| {
                        if f.field == f.binding {
                            f.field.clone()
                        } else {
                            format!("{}: {}", f.field, f.binding)
                        }
                    })
                    .collect();
                Ok(format!("{type_name} {{ {} }}", fs.join(", ")))
            }
            DestructurePat::List { binders, .. } => {
                let bs: Vec<String> = binders.iter().map(|(n, _)| n.clone()).collect();
                Ok(format!("[{}]", bs.join(", ")))
            }
            // DEC-288: `(a, b)` — inferred binders (the `var` prefix is added by the caller). The
            // explicit `(T a, …)` form (no `var`) is a separate surface, not yet parsed.
            DestructurePat::Tuple { binders, .. } => {
                let bs: Result<Vec<String>, String> = binders
                    .iter()
                    .map(|(ty_opt, n, _)| match ty_opt {
                        Some(t) => Ok(format!("{} {n}", ty(t)?)),
                        None => Ok(n.clone()),
                    })
                    .collect();
                Ok(format!("({})", bs?.join(", ")))
            }
        }
    }

    /// `<head> { <body> }` — a header plus an indented statement block.
    pub(super) fn block_stmt(&mut self, head: &str, body: &[Stmt]) -> Result<(), String> {
        if head.is_empty() {
            self.line("{");
        } else {
            self.line(&format!("{head} {{"));
        }
        self.indent += 1;
        for s in body {
            self.stmt(s)?;
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn if_stmt(
        &mut self,
        cond: &Expr,
        bind: Option<&str>,
        then_block: &[Stmt],
        else_block: Option<&[Stmt]>,
    ) -> Result<(), String> {
        let cond_s = match bind {
            Some(name) => format!("var {name} = {}", self.expr(cond)?),
            None => self.expr(cond)?,
        };
        self.line(&format!("if ({cond_s}) {{"));
        self.indent += 1;
        for s in then_block {
            self.stmt(s)?;
        }
        self.indent -= 1;
        match else_block {
            None => self.line("}"),
            // `else if` chain: an else-block holding exactly one `If` renders as `} else if (...) {`.
            Some(
                [Stmt::If {
                    cond,
                    bind,
                    then_block,
                    else_block,
                    ..
                }],
            ) => {
                let cond_s = match bind {
                    Some(name) => format!("var {name} = {}", self.expr(cond)?),
                    None => self.expr(cond)?,
                };
                self.line(&format!("}} else if ({cond_s}) {{"));
                self.indent += 1;
                for s in then_block {
                    self.stmt(s)?;
                }
                self.indent -= 1;
                // Recurse for any further chained else.
                return self.close_else(else_block.as_deref());
            }
            Some(body) => {
                self.line("} else {");
                self.indent += 1;
                for s in body {
                    self.stmt(s)?;
                }
                self.indent -= 1;
                self.line("}");
            }
        }
        Ok(())
    }

    /// Close out an `else`/`else if` tail (used by the `else if` chain in [`Self::if_stmt`]).
    pub(super) fn close_else(&mut self, else_block: Option<&[Stmt]>) -> Result<(), String> {
        match else_block {
            None => self.line("}"),
            Some(
                [Stmt::If {
                    cond,
                    bind,
                    then_block,
                    else_block,
                    ..
                }],
            ) => {
                let cond_s = match bind {
                    Some(name) => format!("var {name} = {}", self.expr(cond)?),
                    None => self.expr(cond)?,
                };
                self.line(&format!("}} else if ({cond_s}) {{"));
                self.indent += 1;
                for s in then_block {
                    self.stmt(s)?;
                }
                self.indent -= 1;
                return self.close_else(else_block.as_deref());
            }
            Some(body) => {
                self.line("} else {");
                self.indent += 1;
                for s in body {
                    self.stmt(s)?;
                }
                self.indent -= 1;
                self.line("}");
            }
        }
        Ok(())
    }

    /// A statement rendered inline with no indent or trailing `;` — for the clauses of a C-style `for`.
    pub(super) fn stmt_inline(&self, s: &Stmt) -> Result<String, String> {
        match s {
            Stmt::VarDecl {
                ty: t,
                name,
                init,
                mutable,
                ..
            } => {
                let m = if *mutable { "mutable " } else { "" };
                Ok(format!("{m}{} {name} = {}", ty(t)?, self.expr(init)?))
            }
            Stmt::Assign { target, value, .. } => {
                Ok(format!("{} = {}", self.expr(target)?, self.expr(value)?))
            }
            Stmt::Expr(e, _) => self.expr(e),
            _ => Err("printer: only var-decl/assign/expr are valid in a for-clause".into()),
        }
    }

    // ── expressions ──
}
