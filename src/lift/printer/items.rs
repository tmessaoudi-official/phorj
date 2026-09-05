//! Lift printer — the program and its DECLARATIONS (items, classes, members, enums, attributes).
//! Statements live in `stmts.rs`.

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
            Item::Interface(i) => self.interface(i),
            Item::Trait(_) | Item::TypeAlias { .. } | Item::Test { .. } => {
                Err("printer: traits/type-aliases/tests are outside the lift subset".into())
            }
        }
    }

    // ── declarations ──

    /// Print item attributes, one `#[…]` per line, WITH their arguments.
    ///
    /// Arguments matter: the synthesized entry carries `#[Entry(kind: EntryKind.Cli)]` (DEC-331 — a bare
    /// `#[Entry]` is rejected by the checker) and a lifted PHP attribute carries whatever the source
    /// wrote (LIFT-ATTR). `self.expr` renders `NamedArg` as `name: value`, so both spellings round-trip.
    /// Shared by functions and classes — printing them in only one of the two is how class attributes
    /// stayed invisible until LIFT-ATTR needed them.
    pub(super) fn attrs(&mut self, attrs: &[Attribute]) -> Result<(), String> {
        for attr in attrs {
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
        Ok(())
    }

    pub(super) fn function(&mut self, f: &FunctionDecl) -> Result<(), String> {
        self.attrs(&f.attrs)?;
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
        self.attrs(&c.attrs)?;
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
            // A promoted default (DEC-236) prints its `= <expr>` — dropping it would be a silent loss.
            let default = match &p.default {
                Some(e) => format!(" = {}", self.expr(e)?),
                None => String::new(),
            };
            out.push(format!("{mods}{} {}{default}", ty(&p.ty)?, p.name));
        }
        Ok(out.join(", "))
    }
}
