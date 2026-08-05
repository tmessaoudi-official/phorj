//! Lift printer — STATEMENTS.
//!
//! Split out of `printer/items.rs` by cohesion (Invariant 13): that file had reached the 500-line hard
//! cap exactly, so LIFT-ATTR's class-attribute printing had nowhere to go. The declaration printers and
//! the statement printers are two independent units over the same `Printer`, and the parser next door
//! is already laid out this way (`parser/{items,stmts,exprs}.rs`).

use super::*;

impl Printer {
    pub(super) fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match s {
            Stmt::VarDecl {
                ty: t,
                name,
                init,
                mutable,
                ..
            } => {
                let m = if *mutable { "mutable " } else { "" };
                self.line(&format!("{m}{} {name} = {};", ty(t)?, self.expr(init)?));
                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                self.line(&format!("{} = {};", self.expr(target)?, self.expr(value)?));
                Ok(())
            }
            Stmt::Return { value, .. } => {
                match value {
                    Some(e) => self.line(&format!("return {};", self.expr(e)?)),
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
                // An inferred-element for-in prints as the idiomatic `foreach (iter as name)`
                // (A-6); an explicit element type keeps the typed `for (T name in iter)` form.
                // The DEC-248 two-binding map form (`val` present — `name` is the KEY) prints
                // `foreach (iter as k => v)`, with a type prefix per binding when not inferred.
                let head = if let Some((vt, vname)) = val {
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
                    // DEC-280 lift marker (developer-ruled): every lifted inferred key/value loop
                    // carries a local, greppable review pointer AFTER the opening brace — the code
                    // is legal Phorj, the marker is a draft-review aid, not a correctness warning.
                    // Emitted by opening the block manually (`block_stmt` can't carry a trailing
                    // comment) with the identical brace/indent discipline.
                    let head = format!("foreach ({} as {k} => {v})", self.expr(iter)?);
                    if matches!(t, Type::Infer(_)) && matches!(vt, Type::Infer(_)) {
                        self.line(&format!(
                            "{head} {{ // lift: key/value types inferred — spell them out for an explicit header"
                        ));
                        self.indent += 1;
                        for s in body {
                            self.stmt(s)?;
                        }
                        self.indent -= 1;
                        self.line("}");
                        return Ok(());
                    }
                    return self.block_stmt(&head, body);
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
            Stmt::Expr(e, _) | Stmt::Discard(e, _) => {
                self.line(&format!("{};", self.expr(e)?));
                Ok(())
            }
            // LIFT-TRY (2026-07-31): `try`/`catch`/`finally` is now IN the subset.
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                self.block_stmt("try", body)?;
                for c in catches {
                    // `catch (T e)` — phorj's spelling puts the binder after the type, no `$`.
                    let head = format!("catch ({} {})", ty(&c.ty)?, c.name);
                    self.block_stmt(&head, &c.body)?;
                }
                if let Some(f) = finally_block {
                    self.block_stmt("finally", f)?;
                }
                Ok(())
            }
            Stmt::Throw { value, .. } => {
                self.line(&format!("throw {};", self.expr(value)?));
                Ok(())
            }
            // `using` remains outside the subset:
            // raising a PHP `try { … } finally { $h->close(); }` back to `using` is a
            // SHAPE-RECOGNITION decision, not a printing one — the lifter would have to decide that a
            // particular try/finally *is* a scope guard, and today it faithfully lifts it as the
            // try/finally the source actually wrote. Recorded in KNOWN_ISSUES rather than guessed at.
            Stmt::Using { .. } | Stmt::Destructure { .. } => {
                Err("printer: using/destructure are outside the lift subset".into())
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
}
