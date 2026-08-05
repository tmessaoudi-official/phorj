//! PHP lifter — declaration lifting (functions, classes, members, methods).

use super::*;

impl Lifter {
    // ── declarations ──

    pub(in crate::lift::lifter) fn lift_function(
        &mut self,
        f: &php::PhpFunction,
    ) -> Result<FunctionDecl, String> {
        let mut declared = HashSet::new();
        let params = lift_params(&f.params)?;
        for p in &params {
            declared.insert(p.name.clone());
        }
        // DEC-397: PHP has FUNCTION scope, phorj has BLOCK scope, so a variable first assigned inside a
        // block would be DECLARED inside it and then be unknown outside. Plan the hoists BEFORE lifting
        // the body and seed `declared` with each hoisted name — seeding is what makes every in-block
        // assignment lift as a plain assignment for free, which is also what keeps the output clear of
        // `E-SHADOW-LOCAL` (a second declaration). See `hoist` for why this is restricted to blocks
        // that always execute.
        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let plan = super::hoist::plan(&f.body, &param_names);
        let mut prelude: Vec<Stmt> = Vec::new();
        for (name, lit) in &plan.hoists {
            declared.insert(name.clone());
            prelude.push(Stmt::VarDecl {
                ty: Type::Infer(SP),
                name: name.clone(),
                init: lift_expr(lit)?,
                mutable: true,
                span: SP,
            });
        }
        let mut body = prelude;
        body.extend(self.lift_block(&f.body, &mut declared)?);
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
            body,
            foreign: false,
            generic_ret_from_param: None,
            span: SP,
        })
    }

    pub(in crate::lift::lifter) fn lift_class(
        &mut self,
        c: &php::PhpClass,
    ) -> Result<ClassDecl, String> {
        let mut members = Vec::new();
        for m in &c.members {
            members.push(self.lift_member(m)?);
        }
        Ok(ClassDecl {
            vis: crate::ast::Visibility::Public,
            // Attributes are attached by `lift` (LIFT-ATTR), which holds the `namespace`/`use` context
            // their names resolve against.
            attrs: Vec::new(),
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

    pub(in crate::lift::lifter) fn lift_member(
        &mut self,
        m: &php::PhpMember,
    ) -> Result<ClassMember, String> {
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

    pub(in crate::lift::lifter) fn lift_method(
        &mut self,
        m: &php::PhpMethod,
    ) -> Result<ClassMember, String> {
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
        // DEC-331 D9: `__toString`→`#[ToString]` (etc.) lifts via `lift_magic_method`; `None` ⇒ ordinary.
        if let Some(res) = self.lift_magic_method(m) {
            return res;
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
}
