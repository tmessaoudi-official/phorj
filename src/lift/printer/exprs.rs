//! Lift printer — expressions + leaf renderers.

use super::*;

impl Printer {
    /// A statement rendered inline with no indent or trailing `;` — for the clauses of a C-style `for`.
    pub(super) fn stmt_inline(&self, s: &Stmt) -> Result<String, String> {
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

    pub(super) fn expr(&self, e: &Expr) -> Result<String, String> {
        match e {
            Expr::Int(n, _) => Ok(n.to_string()),
            Expr::Float(f, _) => Ok(format!("{f:?}")),
            // A `decimal` literal prints back as its rendered value + the `d` suffix (M-NUM S1) — the
            // round-trip-faithful surface form (`19.99d`).
            Expr::Decimal {
                unscaled, scale, ..
            } => Ok(format!("{}d", crate::value::fmt_decimal(*unscaled, *scale))),
            Expr::Bool(b, _) => Ok(b.to_string()),
            Expr::Null(_) => Ok("null".to_string()),
            Expr::Str(parts, _) => self.str_lit(parts),
            Expr::Ident(name, _) => Ok(name.clone()),
            Expr::This(_) => Ok("this".to_string()),
            Expr::List(items, _) => {
                let xs: Result<Vec<_>, _> = items.iter().map(|x| self.expr(x)).collect();
                Ok(format!("[{}]", xs?.join(", ")))
            }
            // `new List<T>()` / `new Map<K,V>()` (DEC-214). The lifter never synthesizes this today, but
            // print it faithfully for completeness.
            Expr::NewColl { kind, args, .. } => {
                let a: Result<Vec<_>, _> = args.iter().map(ty).collect();
                Ok(format!("new {}<{}>()", kind.name(), a?.join(", ")))
            }
            Expr::Map(pairs, _) => {
                let mut xs = Vec::new();
                for (k, v) in pairs {
                    xs.push(format!("{} => {}", self.expr(k)?, self.expr(v)?));
                }
                Ok(format!("[{}]", xs.join(", ")))
            }
            Expr::Unary { op, expr, .. } => {
                // Unary binds tighter than every binary op; a binary/range operand needs parens, and
                // so does a nested unary (to avoid `--`/`~~` re-lexing as a multi-char token).
                let needs = prec_of(expr) < PREC_UNARY || matches!(**expr, Expr::Unary { .. });
                let inner = self.expr(expr)?;
                let inner = if needs { format!("({inner})") } else { inner };
                Ok(format!("{}{inner}", unary_op(*op)))
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let p = bin_prec(*op);
                let right_assoc = matches!(op, BinaryOp::Pow);
                let l = self.operand(lhs, p, false, right_assoc)?;
                let r = self.operand(rhs, p, true, right_assoc)?;
                Ok(format!("{l} {} {r}", binary_op(*op)))
            }
            Expr::InstanceOf {
                value, type_name, ..
            } => {
                // `instanceof` is a left-precedence-8 test; its value operand needs parens below 8.
                let v = self.operand(value, 8, false, false)?;
                Ok(format!("{v} instanceof {type_name}"))
            }
            Expr::Cast {
                value, type_name, ..
            } => {
                // `as` is a left-precedence-8 cast (same level as `instanceof`); its value operand
                // needs parens below 8.
                let v = self.operand(value, 8, false, false)?;
                Ok(format!("{v} as {type_name}"))
            }
            // The PHP→Phorj lifter never produces a return-overload selector, but the printer's match
            // is exhaustive; render it faithfully should one ever be hand-constructed (Slice C1).
            Expr::OverloadSelect { ty: t, call, .. } => {
                Ok(format!("<{}>{}", ty(t)?, self.expr(call)?))
            }
            // Likewise a `parent`/super call (M-RT super/parent) — the lifter never emits one, but the
            // exhaustive printer renders it.
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
                let xs: Result<Vec<_>, _> = args.iter().map(|a| self.expr(a)).collect();
                Ok(format!("{head}.{method}({})", xs?.join(", ")))
            }
            Expr::Call { callee, args, .. } => {
                let a: Result<Vec<_>, _> = args.iter().map(|x| self.expr(x)).collect();
                Ok(format!(
                    "{}({})",
                    self.postfix_operand(callee)?,
                    a?.join(", ")
                ))
            }
            Expr::Member {
                object,
                name,
                safe,
                sep,
                ..
            } => {
                // DEC-207: render the written separator — `::` for PHP-`::`-lifted class access,
                // else `?.`/`.`. Makes the PHP->Phorj draft round-trip faithful.
                let dot = match sep {
                    crate::ast::MemberSep::ColonColon => "::",
                    _ if *safe => "?.",
                    _ => ".",
                };
                Ok(format!("{}{dot}{name}", self.postfix_operand(object)?))
            }
            Expr::Index { object, index, .. } => Ok(format!(
                "{}[{}]",
                self.postfix_operand(object)?,
                self.expr(index)?
            )),
            Expr::Force { inner, .. } => Ok(format!("{}!", self.postfix_operand(inner)?)),
            Expr::Propagate { inner, .. } => Ok(format!("{}?", self.postfix_operand(inner)?)),
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                // Range is the loosest expression (operands are full binaries); only a nested range
                // operand needs parens.
                let dots = if *inclusive { "..=" } else { ".." };
                let wrap = |pr: &Self, e: &Expr| -> Result<String, String> {
                    let s = pr.expr(e)?;
                    // Only a nested range (the single loosest form) needs parens here.
                    Ok(if prec_of(e) == PREC_RANGE {
                        format!("({s})")
                    } else {
                        s
                    })
                };
                Ok(format!("{}{dots}{}", wrap(self, start)?, wrap(self, end)?))
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
            // The PHP→Phorj lifter never produces `spawn` (PHP has no green threads); printed
            // defensively for totality.
            Expr::Spawn { call, .. } => Ok(format!("spawn {}", self.expr(call)?)),
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let mut out = Vec::new();
                for arm in arms {
                    let guard = match &arm.guard {
                        Some(g) => format!(" when {}", self.expr(g)?),
                        None => String::new(),
                    };
                    // DEC-209: a top-level catch-all arm renders as `default` (a standalone `_` arm
                    // is now a parse error); nested wildcards still render as `_` via `pattern`.
                    let head = match &arm.pattern {
                        Pattern::Wildcard(_) => "default".to_string(),
                        p => self.pattern(p)?,
                    };
                    out.push(format!("{head}{guard} => {}", self.expr(&arm.body)?));
                }
                Ok(format!(
                    "match ({}) {{ {} }}",
                    self.expr(scrutinee)?,
                    out.join(", ")
                ))
            }
            Expr::Bytes(_, _)
            | Expr::Lambda { .. }
            | Expr::CloneWith { .. }
            | Expr::Inject { .. }
            | Expr::TaggedTemplate { .. }
            | Expr::Html(_, _) => Err(
                "printer: bytes/lambda/clone-with/inject/html/tagged-template are outside the lift subset"
                    .into(),
            ),
        }
    }

    pub(super) fn str_lit(&self, parts: &[StrPart]) -> Result<String, String> {
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

    /// Print a binary operand, parenthesizing only when precedence/associativity requires it.
    /// `parent` is the operator's binding power; `is_right`/`right_assoc` pick the associativity
    /// side. Left-assoc: the right operand needs parens at equal precedence; right-assoc (`**`):
    /// the left operand does.
    pub(super) fn operand(
        &self,
        e: &Expr,
        parent: u8,
        is_right: bool,
        right_assoc: bool,
    ) -> Result<String, String> {
        let cp = prec_of(e);
        let needs = if is_right == right_assoc {
            cp < parent
        } else {
            cp <= parent
        };
        let s = self.expr(e)?;
        Ok(if needs { format!("({s})") } else { s })
    }

    /// Print the receiver of a postfix operator (`.`/`[]`/call/`!`/`?`), which binds tighter than
    /// every prefix/binary form — so a non-atomic receiver (a binary, unary, or range) needs parens.
    pub(super) fn postfix_operand(&self, e: &Expr) -> Result<String, String> {
        let s = self.expr(e)?;
        Ok(if prec_of(e) < PREC_ATOM {
            format!("({s})")
        } else {
            s
        })
    }

    pub(super) fn pattern(&self, p: &Pattern) -> Result<String, String> {
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
pub(super) fn ty(t: &Type) -> Result<String, String> {
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

pub(super) fn modifiers_str(mods: &[Modifier]) -> String {
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
pub(super) fn bin_prec(op: BinaryOp) -> u8 {
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
pub(super) fn prec_of(e: &Expr) -> u8 {
    match e {
        Expr::Binary { op, .. } => bin_prec(*op),
        Expr::InstanceOf { .. } | Expr::Cast { .. } => 8,
        Expr::Unary { .. } => PREC_UNARY,
        Expr::Range { .. } => PREC_RANGE,
        _ => PREC_ATOM,
    }
}

pub(super) fn binary_op(op: BinaryOp) -> &'static str {
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

pub(super) fn unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
    }
}

/// Escape a string literal's contents for a Phorj double-quoted string. `{`/`}` become `\{`/`\}`
/// because a bare `{` opens an interpolation.
pub(super) fn escape_str(s: &str) -> String {
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
