//! PHP lifter — statement and expression-statement lifting.

use super::*;

impl Lifter {
    // ── statements ──

    pub(in crate::lift::lifter) fn lift_block(
        &mut self,
        stmts: &[php::PhpStmt],
        declared: &mut HashSet<String>,
    ) -> Result<Vec<Stmt>, String> {
        let mut out = Vec::new();
        for s in stmts {
            out.extend(self.lift_stmt(s, declared)?);
        }
        Ok(out)
    }

    pub(in crate::lift::lifter) fn lift_stmt(
        &mut self,
        s: &php::PhpStmt,
        declared: &mut HashSet<String>,
    ) -> Result<Vec<Stmt>, String> {
        Ok(match s {
            php::PhpStmt::Return(e) => {
                vec![Stmt::Return {
                    value: e.as_ref().map(lift_expr).transpose()?,
                    span: SP,
                }]
            }
            php::PhpStmt::Expr(e) => self.lift_expr_stmt(e, declared)?,
            php::PhpStmt::Echo(args) => {
                self.needs_console = true;
                let mut out = Vec::new();
                for a in args {
                    out.push(Stmt::Expr(console_print(lift_expr(a)?), SP));
                }
                out
            }
            php::PhpStmt::If {
                cond,
                then,
                elifs,
                els,
            } => {
                let mut else_block = match els {
                    Some(b) => Some(self.lift_block(b, declared)?),
                    None => None,
                };
                for (c, body) in elifs.iter().rev() {
                    else_block = Some(vec![Stmt::If {
                        cond: lift_expr(c)?,
                        bind: None,
                        then_block: self.lift_block(body, declared)?,
                        else_block,
                        span: SP,
                    }]);
                }
                vec![Stmt::If {
                    cond: lift_expr(cond)?,
                    bind: None,
                    then_block: self.lift_block(then, declared)?,
                    else_block,
                    span: SP,
                }]
            }
            php::PhpStmt::While { cond, body } => vec![Stmt::While {
                cond: lift_expr(cond)?,
                body: self.lift_block(body, declared)?,
                post_cond: false,
                span: SP,
            }],
            php::PhpStmt::For {
                init,
                cond,
                step,
                body,
            } => {
                let init = match init {
                    Some(e) => Some(Box::new(self.lift_for_clause(e, declared)?)),
                    None => None,
                };
                let step = match step {
                    Some(e) => Some(Box::new(self.lift_for_clause(e, declared)?)),
                    None => None,
                };
                vec![Stmt::CFor {
                    init,
                    cond: cond.as_ref().map(lift_expr).transpose()?,
                    step,
                    body: self.lift_block(body, declared)?,
                    span: SP,
                }]
            }
            php::PhpStmt::Foreach {
                array,
                key,
                value,
                body,
            } => {
                // A-6 gave Phorj's for-in element-type inference, so a keyless PHP `foreach
                // ($xs as $v)` lifts to the idiomatic `foreach (xs as v)` (printed from a
                // `Type::Infer` for-in). DEC-248 gave Phorj the two-binding key form, so
                // `foreach ($m as $k => $v)` now lifts Tier-1 too (`foreach (m as k => v)`) —
                // the old Tier-2 rejection is retired.
                vec![Stmt::For {
                    ty: Type::Infer(SP),
                    name: key.clone().unwrap_or_else(|| value.clone()),
                    val: key.as_ref().map(|_| (Type::Infer(SP), value.clone())),
                    iter: lift_expr(array)?,
                    body: self.lift_block(body, declared)?,
                    span: SP,
                }]
            }
            php::PhpStmt::Break => vec![Stmt::Break(SP)],
            php::PhpStmt::Continue => vec![Stmt::Continue(SP)],
            php::PhpStmt::Throw(e) => vec![Stmt::Throw {
                value: lift_expr(e)?,
                span: SP,
            }],
            // LIFT-TRY: `try`/`catch`/`finally` maps 1:1 — phorj has the same three parts, so nothing is
            // approximated. A catch's binding scope is its own body, matching PHP.
            php::PhpStmt::Try {
                body,
                catches,
                finally_block,
            } => {
                let mut lifted_catches: Vec<crate::ast::CatchClause> = Vec::new();
                for c in catches {
                    // A union catch keeps every member: `catch (A | B $e)` is `catch (A | B e)`, not a
                    // silent narrowing to the first type.
                    let ty = lift_catch_type(&c.types);
                    // PHP 8 allows `catch (T)` with no variable. phorj's `CatchClause` always binds a
                    // name, so an unused one is synthesised — `_` is not a legal ident here, and
                    // inventing a name is visible in the draft, which a lift is meant to be.
                    let name = c.var.clone().unwrap_or_else(|| "ignored".to_string());
                    let mut inner = declared.clone();
                    inner.insert(name.clone());
                    lifted_catches.push(crate::ast::CatchClause {
                        ty,
                        name,
                        body: self.lift_block(&c.body, &mut inner)?,
                        span: SP,
                    });
                }
                let fin = match finally_block {
                    Some(f) => Some(self.lift_block(f, &mut declared.clone())?),
                    None => None,
                };
                vec![Stmt::Try {
                    body: self.lift_block(body, &mut declared.clone())?,
                    catches: lifted_catches,
                    finally_block: fin,
                    span: SP,
                }]
            }
            php::PhpStmt::Block(stmts) => {
                vec![Stmt::Block(self.lift_block(stmts, declared)?, SP)]
            }
        })
    }

    /// A PHP expression statement: an assignment becomes a Phorj `var`-decl (first time) or
    /// `Stmt::Assign` (thereafter); `$i++`/`$x += e` desugar; anything else is an `Expr` statement.
    pub(in crate::lift::lifter) fn lift_expr_stmt(
        &mut self,
        e: &php::PhpExpr,
        declared: &mut HashSet<String>,
    ) -> Result<Vec<Stmt>, String> {
        Ok(vec![self.lift_assign_like(e, declared)?])
    }

    pub(in crate::lift::lifter) fn lift_assign_like(
        &mut self,
        e: &php::PhpExpr,
        declared: &mut HashSet<String>,
    ) -> Result<Stmt, String> {
        match e {
            php::PhpExpr::Assign { target, value } => {
                if let php::PhpExpr::Var(name) = target.as_ref() {
                    if !declared.contains(name) {
                        declared.insert(name.clone());
                        return Ok(Stmt::VarDecl {
                            ty: Type::Infer(SP),
                            name: name.clone(),
                            init: lift_expr(value)?,
                            mutable: true, // PHP locals are freely reassignable
                            span: SP,
                        });
                    }
                }
                Ok(Stmt::Assign {
                    target: lift_expr(target)?,
                    value: lift_expr(value)?,
                    span: SP,
                })
            }
            php::PhpExpr::CompoundAssign { target, op, value } => {
                // `x op= e` → `x = x op e`.
                let t = lift_expr(target)?;
                Ok(Stmt::Assign {
                    target: lift_expr(target)?,
                    value: Expr::Binary {
                        op: lift_binop(*op)?,
                        lhs: Box::new(t),
                        rhs: Box::new(lift_expr(value)?),
                        span: SP,
                    },
                    span: SP,
                })
            }
            php::PhpExpr::IncDec { target, inc, .. } => {
                // `x++`/`x--` → `x = x +/- 1`.
                let t = lift_expr(target)?;
                Ok(Stmt::Assign {
                    target: lift_expr(target)?,
                    value: Expr::Binary {
                        op: if *inc { BinaryOp::Add } else { BinaryOp::Sub },
                        lhs: Box::new(t),
                        rhs: Box::new(Expr::Int(1, SP)),
                        span: SP,
                    },
                    span: SP,
                })
            }
            other => Ok(Stmt::Expr(lift_expr(other)?, SP)),
        }
    }

    /// Lift a single PHP expression used as a C-`for` init/step clause into one Phorj statement.
    pub(in crate::lift::lifter) fn lift_for_clause(
        &mut self,
        e: &php::PhpExpr,
        declared: &mut HashSet<String>,
    ) -> Result<Stmt, String> {
        self.lift_assign_like(e, declared)
    }
}

/// The phorj `Type` for a PHP catch clause's type list.
///
/// A PHP builtin exception maps to phorj's standard taxonomy (DEC-421 —
/// `\RuntimeException` → `RuntimeError`), which is what makes a lifted error path actually TYPE-CHECK
/// rather than merely parse. An unmapped class keeps its own name with the root `\` stripped, and the
/// caller marks it with a `// CANNOT LIFT:` note — a framework exception is left visibly for the human
/// instead of being coerced into the nearest phorj type.
///
/// A union of two or more becomes a phorj union, so `catch (A | B $e)` does not silently narrow.
fn lift_catch_type(types: &[String]) -> Type {
    let named = |t: &String| Type::Named {
        name: super::super::exceptions::phorj_error_name(t),
        args: Vec::new(),
        span: SP,
    };
    match types {
        [one] => named(one),
        many => Type::Union(many.iter().map(named).collect(), SP),
    }
}
