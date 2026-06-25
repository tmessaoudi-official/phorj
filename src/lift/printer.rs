//! M-Lift L3 — a Phorge AST → `.phg` source **pretty-printer**, the inverse of what the transpiler
//! does for PHP. Scoped to the **subset the L4 lifter emits** (functions/classes/enums + the Tier-1
//! statement and expression set); any node outside that subset returns a clear `Err` rather than
//! guessing at syntax. (Growing this into a full `phg fmt` is a later, independent expansion.)
//!
//! Correctness discipline: strings are escaped (incl. `{`/`}` → `\{`/`\}`, since a bare `{` opens a
//! Phorge interpolation) and binary/unary expressions are **fully parenthesized** — both keep the
//! printed text re-parsing to the *same* AST, which the round-trip tests assert directly.

use crate::ast::{
    BinaryOp, ClassDecl, ClassMember, CtorParam, EnumDecl, Expr, FunctionDecl, Item, Modifier,
    Param, Pattern, Program, Stmt, StrPart, Type, UnaryOp,
};

/// Print a whole Phorge program to `.phg` source. `Err` if it contains a node outside the lift subset.
pub fn print_program(p: &Program) -> Result<String, String> {
    let mut pr = Printer {
        out: String::new(),
        indent: 0,
    };
    pr.program(p)?;
    Ok(pr.out)
}

struct Printer {
    out: String,
    indent: usize,
}

impl Printer {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn program(&mut self, p: &Program) -> Result<(), String> {
        let pkg = if p.package.is_empty() {
            "Main".to_string()
        } else {
            p.package.join(".")
        };
        self.line(&format!("package {pkg};"));
        for item in &p.items {
            self.out.push('\n');
            self.item(item)?;
        }
        Ok(())
    }

    fn item(&mut self, item: &Item) -> Result<(), String> {
        match item {
            Item::Import {
                path,
                alias,
                type_only,
                ..
            } => {
                let kw = if *type_only { "import type" } else { "import" };
                let path = path.join(".");
                match alias {
                    Some(a) => self.line(&format!("{kw} {path} as {a};")),
                    None => self.line(&format!("{kw} {path};")),
                }
                Ok(())
            }
            Item::Function(f) => self.function(f),
            Item::Class(c) => self.class(c),
            Item::Enum(e) => self.enum_decl(e),
            Item::Interface(_) | Item::Trait(_) | Item::TypeAlias { .. } => {
                Err("printer: interfaces/traits/type-aliases are outside the lift subset".into())
            }
        }
    }

    // ── declarations ──

    fn function(&mut self, f: &FunctionDecl) -> Result<(), String> {
        let mods = modifiers_str(&f.modifiers);
        let generics = if f.type_params.is_empty() {
            String::new()
        } else {
            format!("<{}>", f.type_params.join(", "))
        };
        let params = self.params(&f.params)?;
        let ret = match &f.ret {
            Some(t) => format!(" -> {}", ty(t)?),
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

    fn class(&mut self, c: &ClassDecl) -> Result<(), String> {
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
            "enum {}{generics} {{ {} }}",
            e.name,
            variants.join(", ")
        ));
        Ok(())
    }

    fn params(&self, params: &[Param]) -> Result<String, String> {
        let mut out = Vec::new();
        for p in params {
            out.push(format!("{} {}", ty(&p.ty)?, p.name));
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
                iter,
                body,
                ..
            } => {
                let head = format!("for ({} {name} in {})", ty(t)?, self.expr(iter)?);
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
                self.line(&format!("{};", self.expr(e)?));
                Ok(())
            }
            Stmt::Throw { .. } | Stmt::Try { .. } | Stmt::Destructure { .. } => {
                Err("printer: throw/try/destructure are outside the lift subset".into())
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
        match e {
            Expr::Int(n, _) => Ok(n.to_string()),
            Expr::Float(f, _) => Ok(format!("{f:?}")),
            Expr::Bool(b, _) => Ok(b.to_string()),
            Expr::Null(_) => Ok("null".to_string()),
            Expr::Str(parts, _) => self.str_lit(parts),
            Expr::Ident(name, _) => Ok(name.clone()),
            Expr::This(_) => Ok("this".to_string()),
            Expr::List(items, _) => {
                let xs: Result<Vec<_>, _> = items.iter().map(|x| self.expr(x)).collect();
                Ok(format!("[{}]", xs?.join(", ")))
            }
            Expr::Map(pairs, _) => {
                let mut xs = Vec::new();
                for (k, v) in pairs {
                    xs.push(format!("{} => {}", self.expr(k)?, self.expr(v)?));
                }
                Ok(format!("[{}]", xs.join(", ")))
            }
            Expr::Unary { op, expr, .. } => Ok(format!("{}({})", unary_op(*op), self.expr(expr)?)),
            Expr::Binary { op, lhs, rhs, .. } => Ok(format!(
                "({} {} {})",
                self.expr(lhs)?,
                binary_op(*op),
                self.expr(rhs)?
            )),
            Expr::InstanceOf {
                value, type_name, ..
            } => Ok(format!("({} instanceof {type_name})", self.expr(value)?)),
            Expr::Call { callee, args, .. } => {
                let a: Result<Vec<_>, _> = args.iter().map(|x| self.expr(x)).collect();
                Ok(format!("{}({})", self.expr(callee)?, a?.join(", ")))
            }
            Expr::Member {
                object, name, safe, ..
            } => {
                let dot = if *safe { "?." } else { "." };
                Ok(format!("{}{dot}{name}", self.expr(object)?))
            }
            Expr::Index { object, index, .. } => {
                Ok(format!("{}[{}]", self.expr(object)?, self.expr(index)?))
            }
            Expr::Force { inner, .. } => Ok(format!("{}!", self.expr(inner)?)),
            Expr::Propagate { inner, .. } => Ok(format!("{}?", self.expr(inner)?)),
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                let dots = if *inclusive { "..=" } else { ".." };
                Ok(format!("{}{dots}{}", self.expr(start)?, self.expr(end)?))
            }
            Expr::If {
                cond,
                then_expr,
                else_expr,
                ..
            } => Ok(format!(
                "if ({}) {{ {} }} else {{ {} }}",
                self.expr(cond)?,
                self.expr(then_expr)?,
                self.expr(else_expr)?
            )),
            Expr::New(inner, _) => Ok(format!("new {}", self.expr(inner)?)),
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let mut out = Vec::new();
                for arm in arms {
                    let guard = match &arm.guard {
                        Some(g) => format!(" when {}", self.expr(g)?),
                        None => String::new(),
                    };
                    out.push(format!(
                        "{}{guard} => {}",
                        self.pattern(&arm.pattern)?,
                        self.expr(&arm.body)?
                    ));
                }
                Ok(format!(
                    "match ({}) {{ {} }}",
                    self.expr(scrutinee)?,
                    out.join(", ")
                ))
            }
            Expr::Bytes(_, _) | Expr::Lambda { .. } | Expr::CloneWith { .. } | Expr::Html(_, _) => {
                Err("printer: bytes/lambda/clone-with/html are outside the lift subset".into())
            }
        }
    }

    fn str_lit(&self, parts: &[StrPart]) -> Result<String, String> {
        let mut s = String::from("\"");
        for part in parts {
            match part {
                StrPart::Literal(lit) => s.push_str(&escape_str(lit)),
                StrPart::Expr(e) => s.push_str(&format!("{{{}}}", self.expr(e)?)),
            }
        }
        s.push('"');
        Ok(s)
    }

    fn pattern(&self, p: &Pattern) -> Result<String, String> {
        match p {
            Pattern::Wildcard(_) => Ok("_".to_string()),
            Pattern::Binding { name, .. } => Ok(name.clone()),
            Pattern::Int(n, _) => Ok(n.to_string()),
            Pattern::Float(f, _) => Ok(format!("{f:?}")),
            Pattern::Str(s, _) => Ok(format!("\"{}\"", escape_str(s))),
            Pattern::Bool(b, _) => Ok(b.to_string()),
            Pattern::Null(_) => Ok("null".to_string()),
            Pattern::Variant { name, fields, .. } => {
                let fs: Result<Vec<_>, _> = fields.iter().map(|f| self.pattern(f)).collect();
                Ok(format!("{name}({})", fs?.join(", ")))
            }
            Pattern::Type {
                type_name, binding, ..
            } => match binding {
                Some(b) => Ok(format!("{type_name} {b}")),
                None => Ok(format!("{type_name} _")),
            },
            Pattern::Struct { .. } => {
                Err("printer: struct patterns are outside the lift subset".into())
            }
        }
    }
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
        _ => Err("printer: this type is outside the lift subset".into()),
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

/// Escape a string literal's contents for a Phorge double-quoted string. `{`/`}` become `\{`/`\}`
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
