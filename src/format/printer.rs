//! `phg format` — a comment-preserving, **full-surface** Phorj AST → `.phg` source printer. Unlike the
//! Tier-1-subset lift printer (`src/lift/printer.rs`), this one covers the *entire* language so it can
//! format any parseable program. Its matches are exhaustive — the Rust compiler proves completeness,
//! so it can never silently mis-handle a node — and the only `Err` arms are for AST shapes that a
//! *parsed* program can never contain (e.g. `Type::Erased`, which is produced only by a post-check
//! pass `phg format` never runs).
//!
//! Correctness discipline (the formatter's one hard rule — meaning preservation): strings are escaped
//! (incl. `{`/`}` → `\{`/`\}`, since a bare `{` opens an interpolation); binary/unary expressions are
//! parenthesized **only where precedence/associativity requires it** mirroring the parser's
//! binding-power table; and every meaning-carrying field (class generics / `use` traits / resolution
//! clauses, function `throws`, …) is printed. The invariants `parse(fmt(x)) ≡ parse(x)` and
//! `fmt(fmt(x)) == fmt(x)` are asserted by the round-trip tests.
//!
//! Comments (which the token stream discards) are carried in via the tokenizer's `lex_with_comments`
//! side-channel (F1) and interleaved by source span (F2b).

use super::doc::{self, Doc};
use crate::ast::{
    BinaryOp, CatchClause, ClassDecl, ClassMember, CtorParam, DestructureField, DestructurePat,
    EnumDecl, Expr, FieldPat, FunctionDecl, Item, LambdaBody, Modifier, Param, Pattern, Program,
    Resolution, Stmt, StrPart, Type, UnaryOp,
};
use crate::token::Comment;

/// Format a whole program (already parsed) to canonical `.phg` source, interleaving `comments`
/// (from [`crate::tokenizer::lex_with_comments`]) by source position. `Err` only for an AST a parsed
/// program cannot contain (see the module docs).
pub fn format_program(p: &Program, comments: &[Comment]) -> Result<String, String> {
    let mut pr = Printer {
        out: String::new(),
        indent: 0,
        comments,
        next_comment: 0,
    };
    pr.program(p)?;
    pr.flush_remaining_comments();
    Ok(pr.out)
}

struct Printer<'a> {
    out: String,
    indent: usize,
    /// Captured comments in source order (F1 side-channel); `next_comment` is the cursor.
    comments: &'a [Comment],
    next_comment: usize,
}

impl Printer<'_> {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// F2b: flush every own-line comment whose source position precedes byte offset `before`, each on
    /// its own indented line, ahead of the node about to be printed. (A trailing comment on the same
    /// line as preceding code is handled separately.) Comment text is emitted verbatim (no reflow).
    /// Whether an own-line comment is still pending before source offset `before` (used to keep a
    /// commented import from being tightly grouped with the previous import).
    fn has_comment_before(&self, before: usize) -> bool {
        self.next_comment < self.comments.len()
            && self.comments[self.next_comment].span.start < before
    }

    fn flush_comments_before(&mut self, before: usize) {
        while self.next_comment < self.comments.len()
            && self.comments[self.next_comment].span.start < before
        {
            let c = self.comments[self.next_comment].clone();
            self.next_comment += 1;
            for cl in c.text.lines() {
                self.line(cl);
            }
        }
    }

    /// Emit any comments that appear after the last printed node (trailing block/footer comments).
    fn flush_remaining_comments(&mut self) {
        let n = self.comments.len();
        while self.next_comment < n {
            let c = self.comments[self.next_comment].clone();
            self.next_comment += 1;
            for cl in c.text.lines() {
                self.line(cl);
            }
        }
    }

    fn program(&mut self, p: &Program) -> Result<(), String> {
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

    fn item(&mut self, item: &Item) -> Result<(), String> {
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

    fn interface(&mut self, i: &crate::ast::InterfaceDecl) -> Result<(), String> {
        let sealed = if i.sealed { "sealed " } else { "" };
        let mut header = format!("{}{sealed}interface {}", vis_str(i.vis), i.name);
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

    fn trait_decl(&mut self, t: &crate::ast::TraitDecl) -> Result<(), String> {
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
    fn fn_signature(&self, f: &FunctionDecl) -> Result<String, String> {
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

    /// Item attributes (`#[Route("GET", "/p")]`, `#[UncheckedOverflow]`, `#[Attribute]`, a user `#[Tag(…)]`)
    /// print one per line above the declaration they annotate. Single source for functions AND classes so
    /// the two never drift (a class attribute dropped here would silently corrupt `fmt` idempotence).
    fn item_attrs(&mut self, attrs: &[crate::ast::Attribute]) -> Result<(), String> {
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

    fn function(&mut self, f: &FunctionDecl) -> Result<(), String> {
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

    fn class(&mut self, c: &ClassDecl) -> Result<(), String> {
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
            format!("<{}>", c.type_params.join(", "))
        };
        let mut header = format!("{}{prefix}class {}{generics}", vis_str(c.vis), c.name);
        if !c.extends.is_empty() {
            header.push_str(&format!(" extends {}", c.extends.join(", ")));
        }
        if !c.implements.is_empty() {
            header.push_str(&format!(" implements {}", c.implements.join(", ")));
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
    fn declare_class(&mut self, c: &ClassDecl) -> Result<(), String> {
        let mut header = format!("declare class {}", c.name);
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
            match m {
                ClassMember::Constructor { params, .. } => {
                    let ps = self.ctor_params(params)?;
                    self.line(&format!("constructor({ps});"));
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

    fn member(&mut self, m: &ClassMember) -> Result<(), String> {
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

    fn enum_decl(&mut self, e: &EnumDecl) -> Result<(), String> {
        let generics = if e.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", e.type_params.join(", "))
        };
        let mut variants = Vec::new();
        for v in &e.variants {
            if v.fields.is_empty() {
                variants.push(v.name.clone());
            } else {
                variants.push(format!("{}({})", v.name, self.params(&v.fields)?));
            }
        }
        self.line(&format!(
            "{}enum {}{generics} {{ {} }}",
            vis_str(e.vis),
            e.name,
            variants.join(", ")
        ));
        Ok(())
    }

    fn params(&self, params: &[Param]) -> Result<String, String> {
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

    fn ctor_params(&self, params: &[CtorParam]) -> Result<String, String> {
        let mut out = Vec::new();
        for p in params {
            let mods = modifiers_str(&p.modifiers);
            out.push(format!("{mods}{} {}", ty(&p.ty)?, p.name));
        }
        Ok(out.join(", "))
    }

    // ── statements ──

    fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
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
                // An inferred-element for-in prints as the idiomatic `foreach (iter as name)`
                // (A-6); an explicit element type keeps the typed `for (T name in iter)` form; a
                // two-binding Map form prints `for (K k, V v in iter)` (B1).
                let head = if let Some((vt, vname)) = val {
                    format!(
                        "for ({} {name}, {} {vname} in {})",
                        ty(t)?,
                        ty(vt)?,
                        self.expr(iter)?
                    )
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
                let head = format!("var {} = {}", self.destructure_pat(pat)?, self.expr(init)?);
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

    fn try_stmt(
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

    fn destructure_pat(&self, p: &DestructurePat) -> Result<String, String> {
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
        }
    }

    /// `<head> { <body> }` — a header plus an indented statement block.
    fn block_stmt(&mut self, head: &str, body: &[Stmt]) -> Result<(), String> {
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

    fn if_stmt(
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
    fn close_else(&mut self, else_block: Option<&[Stmt]>) -> Result<(), String> {
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
    fn stmt_inline(&self, s: &Stmt) -> Result<String, String> {
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

    fn expr(&self, e: &Expr) -> Result<String, String> {
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
    fn expr_doc(&self, e: &Expr) -> Result<Doc, String> {
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
            Expr::Call { callee, args, .. } => Ok(doc::concat(vec![
                self.postfix_doc(callee)?,
                self.args_doc(args)?,
            ])),
            Expr::Member {
                object, name, safe, ..
            } => {
                let dot = if *safe { "?." } else { "." };
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
                        doc::text(format!("{}{guard} => ", self.pattern(&arm.pattern)?)),
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
                params, ret, body, ..
            } => {
                let ps = self.params(params)?;
                match body {
                    LambdaBody::Expr(e) => {
                        // Expression body: `function(params)[: Ret] => expr` (the `: Ret` annotation is
                        // optional on an expression lambda; print it when present).
                        let r = match ret {
                            Some(t) => format!(": {}", ty(t)?),
                            None => String::new(),
                        };
                        Ok(doc::concat(vec![
                            doc::text(format!("function({ps}){r} => ")),
                            self.expr_doc(e)?,
                        ]))
                    }
                    LambdaBody::Block(stmts) => {
                        // Statement body: `function(params): Ret { … }` (the return type is required).
                        let r = match ret {
                            Some(t) => format!(": {}", ty(t)?),
                            None => String::new(),
                        };
                        // A lambda is an expression, so its block body is rendered on one line (v1 has
                        // no reflow). `inline_block` handles any statement, including control flow.
                        Ok(doc::text(format!(
                            "function({ps}){r} {{ {} }}",
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
            // `desugar_di`) round-trips it: `inject<T>()` or bare `inject()`.
            Expr::Inject { ty: t, .. } => Ok(doc::text(match t {
                Some(inner) => format!("inject<{}>()", ty(inner)?),
                None => "inject()".to_string(),
            })),
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
        }
    }

    /// Render a statement list on one line (for a statement-body lambda — a lambda is an expression,
    /// so v1 prints its block inline; no reflow). Each statement via [`Self::stmt_inline_any`].
    fn inline_block(&self, stmts: &[Stmt]) -> Result<String, String> {
        let xs: Result<Vec<_>, _> = stmts.iter().map(|s| self.stmt_inline_any(s)).collect();
        Ok(xs?.join(" "))
    }

    /// Render ANY statement to a single line (trailing `;` where one belongs, nested blocks as
    /// `{ … }`). Total over every `Stmt` variant — the lambda-block path needs full coverage, unlike
    /// the for-clause [`Self::stmt_inline`] (which the parser restricts to var-decl/assign/expr).
    fn stmt_inline_any(&self, s: &Stmt) -> Result<String, String> {
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
                let head = format!("var {} = {}", self.destructure_pat(pat)?, self.expr(init)?);
                match else_block {
                    None => Ok(format!("{head};")),
                    Some(eb) => Ok(format!("{head} else {{ {} }}", self.inline_block(eb)?)),
                }
            }
        }
    }

    fn str_lit(&self, parts: &[StrPart]) -> Result<String, String> {
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
    fn render_expr(&self, prefix: &str, e: &Expr, suffix: &str) -> Result<String, String> {
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
    fn args_doc(&self, args: &[Expr]) -> Result<Doc, String> {
        let xs: Result<Vec<_>, _> = args.iter().map(|a| self.expr_doc(a)).collect();
        Ok(bracketed("(", xs?, ")"))
    }

    /// If `e` is a postfix "call chain" spine with ≥2 member accesses (`a.b(…).c(…)`), lay it out as a
    /// single break group: the head (plus any leading `()`/`[]`/`!`/`?` before the first dot) stays on
    /// line 1, then each `.`/`?.` link breaks onto its own line (indented four columns) when the chain
    /// overflows. Flat form is byte-identical to the per-node concat, so idempotence and meaning are
    /// preserved. Returns `None` for anything that is not a ≥2-dot chain (handled by the per-node arms).
    fn chain_doc(&self, e: &Expr) -> Result<Option<Doc>, String> {
        enum Seg<'a> {
            Dot(&'a str, bool),
            Args(&'a [Expr]),
            Index(&'a Expr),
            Force,
            Propagate,
        }
        let mut segs: Vec<Seg> = Vec::new();
        let mut cur = e;
        loop {
            match cur {
                Expr::Member {
                    object, name, safe, ..
                } => {
                    segs.push(Seg::Dot(name, *safe));
                    cur = object;
                }
                Expr::Call { callee, args, .. } => {
                    segs.push(Seg::Args(args));
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
                Seg::Dot(name, safe) => {
                    doc::text(format!("{}{name}", if *safe { "?." } else { "." }))
                }
                Seg::Args(args) => pr.args_doc(args)?,
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
    fn operand_doc(
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
    fn postfix_doc(&self, e: &Expr) -> Result<Doc, String> {
        let d = self.expr_doc(e)?;
        Ok(if prec_of(e) < PREC_ATOM { parens(d) } else { d })
    }

    fn pattern(&self, p: &Pattern) -> Result<String, String> {
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

/// Wrap `d` in literal parentheses (precedence/associativity disambiguation — never a break point).
fn parens(d: Doc) -> Doc {
    doc::concat(vec![doc::text("("), d, doc::text(")")])
}

/// A comma-separated, delimiter-bracketed break group (`[a, b]`, `(a, b)`). Empty renders as
/// `<open><close>`. Flat: `<open>a, b<close>` (byte-identical to the legacy `join(", ")` form).
/// Broken: each item on its own line, indented four columns, the closer dedented to the group's base.
fn bracketed(open: &str, items: Vec<Doc>, close: &str) -> Doc {
    if items.is_empty() {
        return doc::text(format!("{open}{close}"));
    }
    doc::group(doc::concat(vec![
        doc::text(open),
        doc::nest(
            4,
            doc::concat(vec![
                doc::softline(),
                doc::join(items, doc::concat(vec![doc::text(","), doc::line()])),
            ]),
        ),
        doc::softline(),
        doc::text(close),
    ]))
}

/// Render a `Type`. `Type::Infer` prints as `var` (the local-inference keyword). `Err` for nodes the
/// lift subset never produces (function types, fixed lists, unions/intersections, erased).
fn ty(t: &Type) -> Result<String, String> {
    match t {
        Type::Named { name, args, .. } => {
            if args.is_empty() {
                Ok(name.clone())
            } else {
                let a: Result<Vec<_>, _> = args.iter().map(ty).collect();
                Ok(format!("{name}<{}>", a?.join(", ")))
            }
        }
        Type::Optional { inner, .. } => Ok(format!("{}?", ty(inner)?)),
        Type::Infer(_) => Ok("var".to_string()),
        Type::Union(members, _) => {
            let m: Result<Vec<_>, _> = members.iter().map(ty).collect();
            Ok(m?.join(" | "))
        }
        Type::Intersection(members, _) => {
            let m: Result<Vec<_>, _> = members.iter().map(ty).collect();
            Ok(m?.join(" & "))
        }
        Type::Function { params, ret, .. } => {
            let ps: Result<Vec<_>, _> = params.iter().map(ty).collect();
            Ok(format!("({}) => {}", ps?.join(", "), ty(ret)?))
        }
        Type::FixedList { elem, len, .. } => Ok(format!("[{}; {len}]", ty(elem)?)),
        // `Type::Erased` is produced only by the post-check `erase_generics` pass, which `phg format`
        // (parse → print, no checking) never runs — so a parsed program cannot contain it.
        Type::Erased(_) => Err("printer: Type::Erased cannot occur in a parsed program".into()),
    }
}

/// Declaration-level visibility keyword for a top-level item (free function / class / enum /
/// interface), trailing space included. `Public` is the default and is omitted (canonical form);
/// `internal`/`private` are emitted so the loader's visibility semantics survive a format round-trip.
/// (Method/field member visibility lives in `modifiers`, emitted by [`modifiers_str`] — not here.)
fn vis_str(v: crate::ast::Visibility) -> &'static str {
    match v {
        crate::ast::Visibility::Public => "",
        crate::ast::Visibility::Internal => "internal ",
        crate::ast::Visibility::Private => "private ",
    }
}

fn modifiers_str(mods: &[Modifier]) -> String {
    // A stable canonical order, each followed by a space.
    const ORDER: &[(Modifier, &str)] = &[
        (Modifier::Public, "public"),
        (Modifier::Private, "private"),
        (Modifier::Protected, "protected"),
        (Modifier::Open, "open"),
        (Modifier::Abstract, "abstract"),
        (Modifier::Mutable, "mutable"),
        (Modifier::Static, "static"),
        (Modifier::Const, "const"),
    ];
    let mut s = String::new();
    for (m, kw) in ORDER {
        if mods.contains(m) {
            s.push_str(kw);
            s.push(' ');
        }
    }
    s
}

/// Atoms and every postfix form (member/index/call/force/propagate) — and keyword-led primaries
/// (`if`/`match`/`new`) — never need parentheses as a child. Above any operator.
const PREC_ATOM: u8 = 100;
/// Prefix unary (`-`/`!`/`~`): tighter than every binary op, looser than postfix.
const PREC_UNARY: u8 = 80;
/// Ranges (`a..b`) bind looser than every binary operator (operands are full binaries).
const PREC_RANGE: u8 = 0;

/// Binding power of a binary operator — mirrors the Phorj parser's `infix_op` table exactly
/// (`src/parser/exprs.rs`); higher binds tighter. The shared source of truth for re-parse fidelity.
fn bin_prec(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Pipe => 1,
        BinaryOp::Coalesce => 2,
        BinaryOp::Or => 3,
        BinaryOp::And => 4,
        BinaryOp::BitOr => 5,
        BinaryOp::BitXor => 6,
        BinaryOp::BitAnd => 7,
        BinaryOp::Eq | BinaryOp::NotEq => 8,
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => 9,
        BinaryOp::Shl | BinaryOp::Shr => 10,
        BinaryOp::Add | BinaryOp::Sub => 11,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 12,
        BinaryOp::Pow => 13,
    }
}

/// The precedence of an expression's top node, for deciding whether it needs parens as a child.
fn prec_of(e: &Expr) -> u8 {
    match e {
        Expr::Binary { op, .. } => bin_prec(*op),
        Expr::InstanceOf { .. } | Expr::Cast { .. } => 8,
        Expr::Unary { .. } => PREC_UNARY,
        Expr::Range { .. } => PREC_RANGE,
        // A lambda (`function(x) => …`) and the value-position `if`/`match` are "loose" primaries: their
        // bodies/arms extend rightward, so as a postfix receiver (`(lambda)(args)`, the pipe-desugared
        // call) or a binary operand they MUST be parenthesized. Treat them at the loosest precedence so
        // `operand`/`postfix_operand` wrap them.
        Expr::Lambda { .. } | Expr::If { .. } | Expr::Match { .. } => PREC_RANGE,
        _ => PREC_ATOM,
    }
}

fn binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Pow => "**",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::Le => "<=",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::Pipe => "|>",
        BinaryOp::Coalesce => "??",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
    }
}

fn unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
    }
}

/// Byte offset where an item's source begins — for flushing own-line comments before it.
fn item_start(item: &Item) -> usize {
    match item {
        Item::Import { span, .. } | Item::TypeAlias { span, .. } | Item::Test { span, .. } => {
            span.start
        }
        Item::Function(f) => f.span.start,
        Item::Enum(e) => e.span.start,
        Item::Class(c) => c.span.start,
        Item::Interface(i) => i.span.start,
        Item::Trait(t) => t.span.start,
    }
}

/// Byte offset where a statement's source begins — for flushing own-line comments before it.
fn stmt_start(s: &Stmt) -> usize {
    match s {
        Stmt::VarDecl { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::If { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::CFor { span, .. }
        | Stmt::Throw { span, .. }
        | Stmt::Try { span, .. }
        | Stmt::Destructure { span, .. } => span.start,
        Stmt::Break(sp)
        | Stmt::Continue(sp)
        | Stmt::Block(_, sp)
        | Stmt::Expr(_, sp)
        | Stmt::Discard(_, sp) => sp.start,
    }
}

/// Escape the printed text of an interpolation hole's expression so the tokenizer re-captures it intact:
/// `\` → `\\`, `"` → `\"` (else it closes the surrounding string), `}` → `\}` (else it closes the
/// hole early). A `{` needs no escape — inside an open interpolation it does not start a nested hole.
fn escape_interp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '}' => out.push_str("\\}"),
            _ => out.push(c),
        }
    }
    out
}

/// A plain double-quoted string literal (for a `test` name) — escaped, no interpolation holes.
fn str_quote(s: &str) -> String {
    format!("\"{}\"", escape_str(s))
}

/// Render a multi-inheritance resolution clause (M-RT S6b) inside a class body.
fn resolution_str(r: &Resolution) -> String {
    match r {
        Resolution::Use { parent, method, .. } => format!("use {parent}.{method};"),
        Resolution::Exclude { parent, method, .. } => format!("exclude {parent}.{method};"),
        Resolution::Rename {
            parent,
            method,
            as_name,
            ..
        } => format!("rename {parent}.{method} as {as_name};"),
    }
}

/// Escape raw bytes for a `b"…"` byte-string literal: printable ASCII verbatim (with `\`/`"`
/// escaped), everything else as `\xHH`.
fn escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out
}

/// Escape a string literal's contents for a Phorj double-quoted string. `{`/`}` become `\{`/`\}`
/// because a bare `{` opens an interpolation.
fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            _ => out.push(c),
        }
    }
    out
}
