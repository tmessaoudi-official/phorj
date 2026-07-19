//! Printer — top-level items: program walk, interfaces/traits/functions/classes/enums,
//! signatures, params.

use super::*;

/// Render a class's `implements` list with per-name type arguments (DEC-257 generic
/// interfaces) — `Producer<int>, Named`. `args` is parallel to `names`; an absent/empty
/// entry renders the bare name (the common non-generic case).
fn implements_body(names: &[String], args: &[Vec<crate::ast::Type>]) -> Result<String, String> {
    let mut parts = Vec::with_capacity(names.len());
    for (i, n) in names.iter().enumerate() {
        match args.get(i).filter(|a| !a.is_empty()) {
            None => parts.push(n.clone()),
            Some(a) => {
                let rendered = a.iter().map(ty).collect::<Result<Vec<_>, _>>()?;
                parts.push(format!("{n}<{}>", rendered.join(", ")));
            }
        }
    }
    Ok(parts.join(", "))
}

/// Render a generic parameter list body `T, U: Bound, …` (DEC-207/DEC-211) — each param with its
/// optional `: Interface` bound. Shared by the function/class/enum headers so bounds round-trip.
fn type_params_body(params: &[String], bounds: &[(String, String)]) -> String {
    params
        .iter()
        .map(|p| match bounds.iter().find(|(n, _)| n == p) {
            Some((_, b)) => format!("{p}: {b}"),
            None => p.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl Printer<'_> {
    pub(super) fn program(&mut self, p: &Program) -> Result<(), String> {
        // A comment above the `package` line (a file header) is emitted first, before the package.
        let pkg_start = p.span.start;
        self.flush_comments_before(pkg_start);
        // Preserve an ABSENT package: a `.d.phg` foreign-declaration file has no package and MUST NOT
        // gain one (`E-DECL-PACKAGE`). Only emit the line when the source actually declared a package —
        // never synthesize "Main".
        let mut emitted = false;
        if !p.package.is_empty() {
            self.line(&format!("package {};", p.package.join(".")));
            emitted = true;
        }
        let mut prev: Option<&Item> = None;
        for item in &p.items {
            // Consecutive `import`s are grouped tightly (no blank line between them); every other
            // item pair — and an import adjacent to a non-import — gets a blank-line separator.
            let grouped_import = matches!(item, Item::Import { .. })
                && matches!(prev, Some(Item::Import { .. }))
                && !self.has_comment_before(item_start(item));
            if !emitted {
                // First output when there was no package line — no leading blank.
                emitted = true;
            } else if !grouped_import {
                self.out.push('\n');
            }
            // Own-line comments that precede this item (after the blank separator).
            self.flush_comments_before(item_start(item));
            self.item(item)?;
            prev = Some(item);
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
            Item::Trait(t) => self.trait_decl(t),
            Item::TypeAlias { name, ty: t, .. } => {
                self.line(&format!("type {name} = {};", ty(t)?));
                Ok(())
            }
            Item::Test { name, body, .. } => {
                self.block_stmt(&format!("test {}", str_quote(name)), body)
            }
        }
    }

    pub(super) fn interface(&mut self, i: &crate::ast::InterfaceDecl) -> Result<(), String> {
        let sealed = if i.sealed { "sealed " } else { "" };
        // DEC-257 generic interfaces: `<T, U>` (bounds are parser-rejected on interfaces).
        let generics = if i.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", i.type_params.join(", "))
        };
        let mut header = format!("{}{sealed}interface {}{generics}", vis_str(i.vis), i.name);
        if !i.extends.is_empty() {
            header.push_str(&format!(" extends {}", i.extends.join(", ")));
        }
        self.line(&format!("{header} {{"));
        self.indent += 1;
        for m in &i.methods {
            // An interface method is a bodyless signature terminated by `;`.
            self.line(&format!("{};", self.fn_signature(m)?));
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn trait_decl(&mut self, t: &crate::ast::TraitDecl) -> Result<(), String> {
        self.line(&format!("trait {} {{", t.name));
        self.indent += 1;
        for m in &t.members {
            self.member(m)?;
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    // ── declarations ──

    /// The signature text of a function/method up to (not including) the body or `;`:
    /// `[mods]function name[<T>](params)[: Ret][ throws E]`. Shared by free functions, methods,
    /// abstract signatures, and interface method signatures — so every one prints `throws`.
    pub(super) fn fn_signature(&self, f: &FunctionDecl) -> Result<String, String> {
        let mods = modifiers_str(&f.modifiers);
        let generics = if f.type_params.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                type_params_body(&f.type_params, &f.type_param_bounds)
            )
        };
        let params = self.params(&f.params)?;
        let ret = match &f.ret {
            Some(t) => format!(": {}", ty(t)?),
            None => String::new(),
        };
        let throws = if f.throws.is_empty() {
            String::new()
        } else {
            let ts: Result<Vec<_>, _> = f.throws.iter().map(ty).collect();
            format!(" throws {}", ts?.join(" | "))
        };
        Ok(format!(
            "{}{mods}function {}{generics}({params}){ret}{throws}",
            vis_str(f.vis),
            f.name
        ))
    }

    /// Render a `throws` clause (`" throws A | B"`, empty when the list is empty), shared by the two
    /// constructor arms (DEC-221) — the same ` throws {…}` form [`Self::fn_signature`] uses, joined with
    /// `|`, so a formatted throwing ctor round-trips idempotently through the parser.
    pub(super) fn throws_clause(throws: &[crate::ast::Type]) -> Result<String, String> {
        if throws.is_empty() {
            return Ok(String::new());
        }
        let ts: Result<Vec<_>, _> = throws.iter().map(ty).collect();
        Ok(format!(" throws {}", ts?.join(" | ")))
    }

    /// Item attributes (`#[Route("GET", "/p")]`, `#[UncheckedOverflow]`, `#[Attribute]`, a user `#[Tag(…)]`)
    /// print one per line above the declaration they annotate. Single source for functions AND classes so
    /// the two never drift (a class attribute dropped here would silently corrupt `fmt` idempotence).
    pub(super) fn item_attrs(&mut self, attrs: &[crate::ast::Attribute]) -> Result<(), String> {
        for attr in attrs {
            if attr.args.is_empty() {
                self.line(&format!("#[{}]", attr.name));
            } else {
                let args: Result<Vec<_>, _> = attr.args.iter().map(|a| self.expr(a)).collect();
                self.line(&format!("#[{}({})]", attr.name, args?.join(", ")));
            }
        }
        Ok(())
    }

    pub(super) fn function(&mut self, f: &FunctionDecl) -> Result<(), String> {
        self.item_attrs(&f.attrs)?;
        let sig = self.fn_signature(f)?;
        if f.foreign {
            // A foreign `declare function …;` (M8.5) — a bodyless signature, prefixed with `declare`.
            self.line(&format!("declare {sig};"));
            return Ok(());
        }
        if f.modifiers.contains(&Modifier::Abstract) {
            // A bodyless abstract method signature.
            self.line(&format!("{sig};"));
            return Ok(());
        }
        self.line(&format!("{sig} {{"));
        self.indent += 1;
        for s in &f.body {
            self.stmt(s)?;
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    pub(super) fn class(&mut self, c: &ClassDecl) -> Result<(), String> {
        // M8.5: a foreign `declare class` prints as bodyless member signatures (attrs rejected on those).
        if c.foreign {
            return self.declare_class(c);
        }
        // DEC-194: class-level attributes (`#[Attribute]`, a user `#[Tag(…)]`) print above the header.
        self.item_attrs(&c.attrs)?;
        // `abstract` and `sealed` both imply `open`, so emit `open` only when it is the sole
        // extensibility marker. `sealed` composes with `abstract` (`sealed abstract class`).
        let mut prefix = String::new();
        if c.sealed {
            prefix.push_str("sealed ");
        }
        if c.is_abstract {
            prefix.push_str("abstract ");
        } else if c.open && !c.sealed {
            prefix.push_str("open ");
        }
        let prefix = prefix.as_str();
        let generics = if c.type_params.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                type_params_body(&c.type_params, &c.type_param_bounds)
            )
        };
        let mut header = format!("{}{prefix}class {}{generics}", vis_str(c.vis), c.name);
        if !c.extends.is_empty() {
            header.push_str(&format!(" extends {}", c.extends.join(", ")));
        }
        if !c.implements.is_empty() {
            header.push_str(&format!(
                " implements {}",
                implements_body(&c.implements, &c.implements_args)?
            ));
        }
        header.push_str(" {");
        self.line(&header);
        self.indent += 1;
        // Trait composition (`use T;`) and multi-inheritance resolution clauses precede the members.
        for u in &c.uses {
            self.line(&format!("use {};", u.name));
        }
        for r in &c.resolutions {
            self.line(&resolution_str(r));
        }
        for m in &c.members {
            self.member(m)?;
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// Print a foreign `declare class` (M8.5 S2/S3a): bodyless member signatures terminated by `;`,
    /// with the optional `extends`/`implements` header (S3a — `implements Error` makes it catchable).
    pub(super) fn declare_class(&mut self, c: &ClassDecl) -> Result<(), String> {
        let mut header = format!("declare class {}", c.name);
        if !c.extends.is_empty() {
            header.push_str(&format!(" extends {}", c.extends.join(", ")));
        }
        if !c.implements.is_empty() {
            header.push_str(&format!(
                " implements {}",
                implements_body(&c.implements, &c.implements_args)?
            ));
        }
        header.push_str(" {");
        self.line(&header);
        self.indent += 1;
        for m in &c.members {
            match m {
                ClassMember::Constructor { params, throws, .. } => {
                    let ps = self.ctor_params(params)?;
                    let th = Self::throws_clause(throws)?;
                    self.line(&format!("constructor({ps}){th};"));
                }
                ClassMember::Method(f) => {
                    let sig = self.fn_signature(f)?;
                    self.line(&format!("{sig};"));
                }
                ClassMember::Field {
                    modifiers,
                    ty: t,
                    name,
                    ..
                } => {
                    let mods = modifiers_str(modifiers);
                    self.line(&format!("{mods}{} {name};", ty(t)?));
                }
                // Hooks never appear in a foreign class (only ctor/method/field signatures parse).
                ClassMember::Hook { .. } => {}
            }
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
                    Some(e) => {
                        let s = self.render_expr(&format!("{mods}{} {name} = ", ty(t)?), e, ";")?;
                        self.line(&s);
                    }
                    None => self.line(&format!("{mods}{} {name};", ty(t)?)),
                }
                Ok(())
            }
            ClassMember::Constructor {
                params,
                throws,
                body,
                ..
            } => {
                let ps = self.ctor_params(params)?;
                let th = Self::throws_clause(throws)?;
                if body.is_empty() {
                    self.line(&format!("constructor({ps}){th} {{}}"));
                } else {
                    self.line(&format!("constructor({ps}){th} {{"));
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
            ClassMember::Hook {
                ty: t,
                name,
                get,
                set,
                ..
            } => {
                self.line(&format!("{} {name} {{", ty(t)?));
                self.indent += 1;
                if let Some(g) = get {
                    let s = self.render_expr("get => ", g, ";")?;
                    self.line(&s);
                }
                if let Some((param, body)) = set {
                    self.line(&format!("set({} {}) {{", ty(&param.ty)?, param.name));
                    self.indent += 1;
                    for s in body {
                        self.stmt(s)?;
                    }
                    self.indent -= 1;
                    self.line("}");
                }
                self.indent -= 1;
                self.line("}");
                Ok(())
            }
        }
    }

    pub(super) fn enum_decl(&mut self, e: &EnumDecl) -> Result<(), String> {
        let generics = if e.type_params.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                type_params_body(&e.type_params, &e.type_param_bounds)
            )
        };
        // DEC-302 backed enum: `: int`/`: string` after the generics, and per-variant `= value`.
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
            "{}enum {}{generics}{backing} {{ {} }}",
            vis_str(e.vis),
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
            // DEC-298: a variadic param prints `...` between the element type and the name, so a
            // format round-trip preserves `int ...nums` (else the formatter silently drops it).
            let dots = if p.variadic { "..." } else { "" };
            out.push(format!("{} {dots}{}{default}", ty(&p.ty)?, p.name));
        }
        Ok(out.join(", "))
    }

    pub(super) fn ctor_params(&self, params: &[CtorParam]) -> Result<String, String> {
        let mut out = Vec::new();
        for p in params {
            let mods = modifiers_str(&p.modifiers);
            // A defaulted ctor param (DEC-236) prints its `= <expr>` so a format round-trip
            // preserves it (the same M4 rule as `params`).
            let default = match &p.default {
                Some(e) => format!(" = {}", self.expr(e)?),
                None => String::new(),
            };
            out.push(format!("{mods}{} {}{default}", ty(&p.ty)?, p.name));
        }
        Ok(out.join(", "))
    }

    // ── statements ──
}
