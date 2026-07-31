//! Lift printer — items + statements.

use super::*;

/// Print a whole Phorj program to `.phg` source. `Err` if it contains a node outside the lift subset.
impl Printer {
    pub(super) fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub(super) fn program(&mut self, p: &Program) -> Result<(), String> {
        let pkg = if p.package.is_empty() {
            "Main".to_string()
        } else {
            p.package.join(".")
        };
        self.line(&format!("package {pkg};"));
        for item in &p.items {
            self.out.push('\n');
            // DEC-419: PHPDoc lifted from the source becomes a phorj doc comment. `/** … */` is the
            // same spelling on both sides, so this is a re-emission, not a translation.
            self.doc_comment(item);
            self.item(item)?;
        }
        Ok(())
    }

    pub(super) fn item(&mut self, item: &Item) -> Result<(), String> {
        match item {
            Item::Import { path, alias, .. } => {
                let path = path.join(".");
                match alias {
                    Some(a) => self.line(&format!("import {path} as {a};")),
                    None => self.line(&format!("import {path};")),
                }
                Ok(())
            }
            Item::Function(f) => self.function(f),
            Item::Class(c) => self.class(c),
            Item::Enum(e) => self.enum_decl(e),
            Item::Interface(_) | Item::Trait(_) | Item::TypeAlias { .. } | Item::Test { .. } => {
                Err(
                    "printer: interfaces/traits/type-aliases/tests are outside the lift subset"
                        .into(),
                )
            }
        }
    }

    // ── declarations ──

    pub(super) fn function(&mut self, f: &FunctionDecl) -> Result<(), String> {
        // DEC-191 / DEC-331: print item attributes WITH their arguments. The synthesized entry now
        // carries `#[Entry(kind: EntryKind.Cli)]` (kind is REQUIRED since DEC-331 — bare `#[Entry]` is rejected
        // by the checker), so the printer must render args; `self.expr` handles `NamedArg` (`kind: EntryKind.Cli`).
        for attr in &f.attrs {
            if attr.args.is_empty() {
                self.line(&format!("#[{}]", attr.name));
            } else {
                let args = attr
                    .args
                    .iter()
                    .map(|a| self.expr(a))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                self.line(&format!("#[{}({args})]", attr.name));
            }
        }
        let mods = modifiers_str(&f.modifiers);
        let generics = if f.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", f.type_params.join(", "))
        };
        let params = self.params(&f.params)?;
        let ret = match &f.ret {
            Some(t) => format!(": {}", ty(t)?),
            None => String::new(),
        };
        let is_abstract = f.modifiers.contains(&Modifier::Abstract);
        if is_abstract {
            // A bodyless abstract method signature.
            self.line(&format!(
                "{mods}function {}{generics}({params}){ret};",
                f.name
            ));
            return Ok(());
        }
        self.line(&format!(
            "{mods}function {}{generics}({params}){ret} {{",
            f.name
        ));
        self.indent += 1;
        for s in &f.body {
            self.stmt(s)?;
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn class(&mut self, c: &ClassDecl) -> Result<(), String> {
        // `abstract` implies `open`, so emit only the stronger keyword.
        let prefix = if c.is_abstract {
            "abstract "
        } else if c.open {
            "open "
        } else {
            ""
        };
        let mut header = format!("{prefix}class {}", c.name);
        if !c.extends.is_empty() {
            header.push_str(&format!(" extends {}", c.extends.join(", ")));
        }
        if !c.implements.is_empty() {
            header.push_str(&format!(" implements {}", c.implements.join(", ")));
        }
        header.push_str(" {");
        self.line(&header);
        self.indent += 1;
        for m in &c.members {
            self.member(m)?;
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn member(&mut self, m: &ClassMember) -> Result<(), String> {
        match m {
            ClassMember::Field {
                modifiers,
                ty: t,
                name,
                init,
                ..
            } => {
                let mods = modifiers_str(modifiers);
                match init {
                    Some(e) => self.line(&format!("{mods}{} {name} = {};", ty(t)?, self.expr(e)?)),
                    None => self.line(&format!("{mods}{} {name};", ty(t)?)),
                }
                Ok(())
            }
            ClassMember::Constructor { params, body, .. } => {
                let ps = self.ctor_params(params)?;
                if body.is_empty() {
                    self.line(&format!("constructor({ps}) {{}}"));
                } else {
                    self.line(&format!("constructor({ps}) {{"));
                    self.indent += 1;
                    for s in body {
                        self.stmt(s)?;
                    }
                    self.indent -= 1;
                    self.line("}");
                }
                Ok(())
            }
            ClassMember::Method(f) => self.function(f),
            ClassMember::Hook { .. } => {
                Err("printer: property hooks are outside the lift subset".into())
            }
        }
    }

    pub(super) fn enum_decl(&mut self, e: &EnumDecl) -> Result<(), String> {
        let generics = if e.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", e.type_params.join(", "))
        };
        // DEC-302 backed enum: `: int`/`: string` header + per-variant `= value`.
        let backing = match &e.backing_type {
            Some(t) => format!(": {}", ty(t)?),
            None => String::new(),
        };
        let mut variants = Vec::new();
        for v in &e.variants {
            let base = if v.fields.is_empty() {
                v.name.clone()
            } else {
                format!("{}({})", v.name, self.params(&v.fields)?)
            };
            match &v.backing_value {
                Some(val) => variants.push(format!("{base} = {}", self.expr(val)?)),
                None => variants.push(base),
            }
        }
        self.line(&format!(
            "enum {}{generics}{backing} {{ {} }}",
            e.name,
            variants.join(", ")
        ));
        Ok(())
    }

    pub(super) fn params(&self, params: &[Param]) -> Result<String, String> {
        let mut out = Vec::new();
        for p in params {
            // A default parameter (M4) prints its `= <expr>` so a format round-trip preserves it.
            let default = match &p.default {
                Some(e) => format!(" = {}", self.expr(e)?),
                None => String::new(),
            };
            out.push(format!("{} {}{default}", ty(&p.ty)?, p.name));
        }
        Ok(out.join(", "))
    }

    pub(super) fn ctor_params(&self, params: &[CtorParam]) -> Result<String, String> {
        let mut out = Vec::new();
        for p in params {
            let mods = modifiers_str(&p.modifiers);
            out.push(format!("{mods}{} {}", ty(&p.ty)?, p.name));
        }
        Ok(out.join(", "))
    }

    // ── statements ──

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
