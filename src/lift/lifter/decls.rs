//! PHP lifter — declarations + statements: entries, the Lifter walker.

use super::*;

pub fn lift_source(php_src: &str) -> Result<String, String> {
    let toks = lex_php(php_src)?;
    let prog = parse_php(toks)?;
    let phorj = lift(&prog)?;
    print_program(&phorj)
}

/// Lift a parsed PHP program into a Phorj program (`package Main;`).
pub fn lift(prog: &php::PhpProgram) -> Result<Program, String> {
    let mut l = Lifter {
        needs_console: false,
    };
    let mut items: Vec<Item> = Vec::new();
    let mut top_stmts: Vec<Stmt> = Vec::new();
    let mut has_main = false;

    for item in &prog.items {
        match item {
            php::PhpItem::Function(f) => {
                let mut lifted = l.lift_function(f)?;
                if f.name == "main" {
                    has_main = true;
                    // DEC-191: a PHP `main` is the entry INTENT — the lifted draft attributes it
                    // so it actually runs (entries are attribute-declared, never name-magic).
                    lifted.attrs.push(crate::ast::Attribute {
                        name: "Entry".to_string(),
                        args: Vec::new(),
                        span: SP,
                    });
                }
                items.push(Item::Function(lifted));
            }
            php::PhpItem::Class(c) => items.push(Item::Class(l.lift_class(c)?)),
            php::PhpItem::Enum(e) => items.push(Item::Enum(lift_enum(e)?)),
            php::PhpItem::Stmt(s) => {
                let mut declared = HashSet::new();
                top_stmts.extend(l.lift_stmt(s, &mut declared)?);
            }
        }
    }

    // Top-level PHP code becomes the runnable entry `function main()` (M5 model).
    if !top_stmts.is_empty() {
        if has_main {
            return Err(
                "lift: file has both a main() function and top-level code (ambiguous entry)".into(),
            );
        }
        items.push(Item::Function(FunctionDecl {
            modifiers: Vec::new(),
            // DEC-191: the synthesized entry carries #[Entry] (attribute-declared, no name magic).
            attrs: vec![crate::ast::Attribute {
                name: "Entry".to_string(),
                args: Vec::new(),
                span: SP,
            }],
            vis: crate::ast::Visibility::Public,
            name: "main".into(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params: Vec::new(),
            ret: Some(named("void")),
            throws: Vec::new(),
            body: top_stmts,
            foreign: false,
            generic_ret_from_param: None,
            span: SP,
        }));
    }

    // Prepend `import Core.Output;` if any `echo` was lifted.
    let mut final_items = Vec::new();
    if l.needs_console {
        final_items.push(Item::Import {
            path: vec!["Core".into(), "Output".into()],
            alias: None,
            span: SP,
        });
    }
    final_items.extend(items);

    Ok(Program {
        package: vec!["Main".into()],
        items: final_items,
        span: SP,
    })
}

pub(super) struct Lifter {
    /// Set when an `echo` is lifted to `Output.print`, so the import is prepended.
    needs_console: bool,
}

impl Lifter {
    // ── declarations ──

    pub(super) fn lift_function(&mut self, f: &php::PhpFunction) -> Result<FunctionDecl, String> {
        let mut declared = HashSet::new();
        let params = lift_params(&f.params)?;
        for p in &params {
            declared.insert(p.name.clone());
        }
        Ok(FunctionDecl {
            modifiers: Vec::new(),
            attrs: Vec::new(),
            vis: crate::ast::Visibility::Public,
            name: f.name.clone(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params,
            ret: lift_ret(&f.ret, Some(&f.body))?,
            throws: Vec::new(),
            body: self.lift_block(&f.body, &mut declared)?,
            foreign: false,
            generic_ret_from_param: None,
            span: SP,
        })
    }

    pub(super) fn lift_class(&mut self, c: &php::PhpClass) -> Result<ClassDecl, String> {
        let mut members = Vec::new();
        for m in &c.members {
            members.push(self.lift_member(m)?);
        }
        Ok(ClassDecl {
            vis: crate::ast::Visibility::Public,
            attrs: Vec::new(), // PHP→Phorj attribute lifting deferred (DEC-194 later slice)
            name: c.name.clone(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            extends: c.extends.clone().into_iter().collect(),
            implements_args: vec![Vec::new(); c.implements.len()],
            implements: c.implements.clone(),
            // PHP is extensible-by-default (only `final` seals it); Phorj is final-by-default, so a
            // non-final PHP class lifts to `open` to preserve extensibility. `abstract` implies open.
            open: c.is_abstract || !c.is_final,
            is_abstract: c.is_abstract,
            // PHP has no sealed classes — a lifted class is never sealed.
            sealed: false,
            resolutions: Vec::new(),
            uses: Vec::new(),
            members,
            foreign: false,
            span: SP,
        })
    }

    pub(super) fn lift_member(&mut self, m: &php::PhpMember) -> Result<ClassMember, String> {
        match m {
            php::PhpMember::Prop {
                vis,
                set_vis,
                is_static,
                is_readonly,
                ty,
                name,
                default,
            } => {
                if !is_static && default.is_some() {
                    return Err(format!(
                        "lift: instance field `{name}` has a default — needs constructor synthesis (Tier-2)"
                    ));
                }
                let ty = lift_type(
                    ty.as_ref()
                        .ok_or_else(|| format!("lift: field `{name}` has no type (Tier-2)"))?,
                )?;
                let mut modifiers = vec![vis_modifier(*vis)];
                // PHP 8.4 asymmetric visibility lifts 1:1 onto DEC-241 (`public(set)` is
                // redundant in both languages — dropped). Invalid combinations (e.g. with
                // `readonly`) are lifted faithfully and rejected by the Phorj checker's own
                // DEC-241 diagnostics, not silently repaired here.
                match set_vis {
                    Some(php::PhpVisibility::Private) => modifiers.push(Modifier::PrivateSet),
                    Some(php::PhpVisibility::Protected) => modifiers.push(Modifier::ProtectedSet),
                    Some(php::PhpVisibility::Public) | None => {}
                }
                if *is_static {
                    modifiers.push(Modifier::Static);
                }
                // PHP properties are mutable unless `readonly`; mirror that faithfully.
                if !is_readonly {
                    modifiers.push(Modifier::Mutable);
                }
                let init = if *is_static {
                    default.as_ref().map(lift_expr).transpose()?
                } else {
                    None
                };
                Ok(ClassMember::Field {
                    modifiers,
                    ty,
                    name: name.clone(),
                    init,
                    span: SP,
                })
            }
            php::PhpMember::Const { vis, name, value } => {
                let tyname = lit_type(value).ok_or_else(|| {
                    format!("lift: const `{name}` has a non-literal value (Tier-2)")
                })?;
                Ok(ClassMember::Field {
                    modifiers: vec![vis_modifier(*vis), Modifier::Const],
                    ty: named(tyname),
                    name: name.clone(),
                    init: Some(lift_expr(value)?),
                    span: SP,
                })
            }
            php::PhpMember::Method(method) => self.lift_method(method),
        }
    }

    pub(super) fn lift_method(&mut self, m: &php::PhpMethod) -> Result<ClassMember, String> {
        let mut declared = HashSet::new();
        // `__construct` → a Phorj `constructor` (with promotion), not an ordinary method.
        if m.name == "__construct" {
            let params = lift_ctor_params(&m.params)?;
            for p in &params {
                declared.insert(p.name.clone());
            }
            let body = match &m.body {
                Some(b) => self.lift_block(b, &mut declared)?,
                None => Vec::new(),
            };
            // Preserve a non-public `__construct` visibility (the factory/singleton pattern);
            // a public ctor stays modifier-free to match the bare-`constructor` printer output.
            let modifiers = if m.vis == php::PhpVisibility::Public {
                Vec::new()
            } else {
                vec![vis_modifier(m.vis)]
            };
            return Ok(ClassMember::Constructor {
                modifiers,
                params,
                // PHP has no checked exceptions — a lifted PHP constructor declares no `throws`.
                throws: Vec::new(),
                body,
                span: SP,
            });
        }
        let params = lift_params(&m.params)?;
        for p in &params {
            declared.insert(p.name.clone());
        }
        let mut modifiers = vec![vis_modifier(m.vis)];
        if m.is_static {
            modifiers.push(Modifier::Static);
        }
        if m.is_abstract {
            modifiers.push(Modifier::Abstract);
        } else if !m.is_final && m.vis != php::PhpVisibility::Private {
            // PHP methods are overridable by default; Phorj is final-by-default, so mark `open` to
            // preserve overridability (abstract is implicitly open, so only the concrete case).
            modifiers.push(Modifier::Open);
        }
        let body = match &m.body {
            Some(b) => self.lift_block(b, &mut declared)?,
            None => Vec::new(),
        };
        Ok(ClassMember::Method(FunctionDecl {
            modifiers,
            attrs: Vec::new(),
            vis: crate::ast::Visibility::Public,
            name: m.name.clone(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params,
            ret: lift_ret(&m.ret, m.body.as_deref())?,
            throws: Vec::new(),
            body,
            foreign: false,
            generic_ret_from_param: None,
            span: SP,
        }))
    }

    // ── statements ──

    pub(super) fn lift_block(
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

    pub(super) fn lift_stmt(
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
            php::PhpStmt::Block(stmts) => {
                vec![Stmt::Block(self.lift_block(stmts, declared)?, SP)]
            }
        })
    }

    /// A PHP expression statement: an assignment becomes a Phorj `var`-decl (first time) or
    /// `Stmt::Assign` (thereafter); `$i++`/`$x += e` desugar; anything else is an `Expr` statement.
    pub(super) fn lift_expr_stmt(
        &mut self,
        e: &php::PhpExpr,
        declared: &mut HashSet<String>,
    ) -> Result<Vec<Stmt>, String> {
        Ok(vec![self.lift_assign_like(e, declared)?])
    }

    pub(super) fn lift_assign_like(
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
    pub(super) fn lift_for_clause(
        &mut self,
        e: &php::PhpExpr,
        declared: &mut HashSet<String>,
    ) -> Result<Stmt, String> {
        self.lift_assign_like(e, declared)
    }
}
