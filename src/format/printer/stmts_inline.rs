//! ONE-LINE statement rendering — `inline_block` + `stmt_inline_any`. Split out of `exprs.rs` when
//! DEC-364 pushed that file past Invariant 13's cap, and a clean seam: a statement-body lambda is an
//! *expression*, which is the only reason this statement printer lived among the expression ones.
//!
//! Total over every `Stmt` variant (Invariant 3 / DEC-356) — the lambda-block path needs full
//! coverage, so a new statement form extends this in the same change.

use super::*;

impl Printer<'_> {
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
            Stmt::Using {
                ty: t,
                name,
                init,
                body,
                ..
            } => Ok(format!(
                "using ({} {name} = {}) {{ {} }}",
                ty(t)?,
                self.expr(init)?,
                self.inline_block(body)?
            )),
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
}
