//! Printer — leaf renderers: types, operators, precedence, escaping, comment anchors.

use super::*;

/// Wrap `d` in literal parentheses (precedence/associativity disambiguation — never a break point).
pub(super) fn parens(d: Doc) -> Doc {
    doc::concat(vec![doc::text("("), d, doc::text(")")])
}

/// A comma-separated, delimiter-bracketed break group (`[a, b]`, `(a, b)`). Empty renders as
/// `<open><close>`. Flat: `<open>a, b<close>` (byte-identical to the legacy `join(", ")` form).
/// Broken: each item on its own line, indented four columns, the closer dedented to the group's base.
pub(super) fn bracketed(open: &str, items: Vec<Doc>, close: &str) -> Doc {
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
        // DEC-253: a union inner is parenthesized — `(A | B)?` — so the printed form re-parses to
        // the same type (`?` binds to its immediate member in the grammar).
        Type::Optional { inner, .. } if matches!(**inner, Type::Union(..)) => {
            Ok(format!("({})?", ty(inner)?))
        }
        Type::Optional { inner, .. } => Ok(format!("{}?", ty(inner)?)),
        Type::Infer(_) => Ok("var".to_string()),
        Type::Union(members, _) => {
            // DEC-253: the `A | B | null` spelling canonicalizes to `(A | B)?` on format (a lone
            // non-null remainder is just `A?`) — the two spellings are the same type.
            let (nulls, rest): (Vec<&Type>, Vec<&Type>) = members
                .iter()
                .partition(|m| matches!(m, Type::Named { name, args, .. } if name == "null" && args.is_empty()));
            if !nulls.is_empty() && !rest.is_empty() {
                let m: Result<Vec<_>, _> = rest.iter().copied().map(ty).collect();
                let m = m?;
                return if m.len() == 1 {
                    Ok(format!("{}?", m[0]))
                } else {
                    Ok(format!("({})?", m.join(" | ")))
                };
            }
            let m: Result<Vec<_>, _> = members.iter().map(ty).collect();
            Ok(m?.join(" | "))
        }
        Type::Intersection(members, _) => {
            let m: Result<Vec<_>, _> = members.iter().map(ty).collect();
            Ok(m?.join(" & "))
        }
        Type::Function {
            params,
            ret,
            throws,
            ..
        } => {
            let ps: Result<Vec<_>, _> = params.iter().map(ty).collect();
            let base = format!("({}) => {}", ps?.join(", "), ty(ret)?);
            // DEC-222: render the `throws` clause so a function type round-trips through `phg format`.
            if throws.is_empty() {
                Ok(base)
            } else {
                let es: Result<Vec<_>, _> = throws.iter().map(ty).collect();
                Ok(format!("{base} throws {}", es?.join(", ")))
            }
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
pub(super) fn vis_str(v: crate::ast::Visibility) -> &'static str {
    match v {
        crate::ast::Visibility::Public => "",
        crate::ast::Visibility::Internal => "internal ",
        crate::ast::Visibility::Private => "private ",
    }
}

pub(super) fn modifiers_str(mods: &[Modifier]) -> String {
    // A stable canonical order, each followed by a space.
    const ORDER: &[(Modifier, &str)] = &[
        (Modifier::Public, "public"),
        (Modifier::Private, "private"),
        (Modifier::Protected, "protected"),
        // DEC-241 asymmetric visibility — printed after the read visibility (PHP 8.4 order).
        (Modifier::PrivateSet, "private(set)"),
        (Modifier::ProtectedSet, "protected(set)"),
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
pub(super) const PREC_ATOM: u8 = 100;
/// Prefix unary (`-`/`!`/`~`): tighter than every binary op, looser than postfix.
pub(super) const PREC_UNARY: u8 = 80;
/// Ranges (`a..b`) bind looser than every binary operator (operands are full binaries).
pub(super) const PREC_RANGE: u8 = 0;

/// Binding power of a binary operator — mirrors the Phorj parser's `infix_op` table exactly
/// (`src/parser/exprs.rs`); higher binds tighter. The shared source of truth for re-parse fidelity.
pub(super) fn bin_prec(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Coalesce => 2,
        BinaryOp::Or => 3,
        BinaryOp::And => 4,
        BinaryOp::BitOr => 5,
        BinaryOp::BitXor => 6,
        BinaryOp::BitAnd => 7,
        BinaryOp::Eq | BinaryOp::NotEq => 8,
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => 9,
        // DEC-239 precedence fix: `|>` sits in PHP 8.5's slot — tighter than comparison, looser
        // than shifts/arithmetic (verified against php-8.5.8).
        BinaryOp::Pipe => 10,
        BinaryOp::Shl | BinaryOp::Shr => 11,
        BinaryOp::Add | BinaryOp::Sub => 12,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 13,
        BinaryOp::Pow => 14,
    }
}

/// The precedence of an expression's top node, for deciding whether it needs parens as a child.
pub(super) fn prec_of(e: &Expr) -> u8 {
    match e {
        Expr::Binary { op, .. } => bin_prec(*op),
        // `lhs |> rhs` (DEC-239) — parenthesized as a child exactly like a Pipe binary would be.
        Expr::Pipe { .. } => bin_prec(BinaryOp::Pipe),
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

/// Byte offset where an item's source begins — for flushing own-line comments before it.
pub(super) fn item_start(item: &Item) -> usize {
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
pub(super) fn stmt_start(s: &Stmt) -> usize {
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
pub(super) fn escape_interp(s: &str) -> String {
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
pub(super) fn str_quote(s: &str) -> String {
    format!("\"{}\"", escape_str(s))
}

/// Render a multi-inheritance resolution clause (M-RT S6b) inside a class body.
pub(super) fn resolution_str(r: &Resolution) -> String {
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
pub(super) fn escape_bytes(bytes: &[u8]) -> String {
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
