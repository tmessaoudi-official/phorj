//! PHP -> Phorj MAPPING tables: types, binary operators, visibility, and the small constructors the
//! lifter emits (`Console.print`, a static-member access, a bare named type).
//!
//! Split out of `exprs.rs` (Invariant 13 — that file crossed the hard cap when LIFT-TRY landed). These
//! are lookup-shaped: one PHP construct in, one Phorj node out, no traversal — which is exactly the
//! cohesion boundary, since `exprs.rs` is the recursive walk.

use super::*;

/// Does any path in `body` `return` a *value* (`return expr;`)? Recurses through nested control flow
/// and blocks. A bare `return;` (and `echo`/`break`/`continue`/expr statements) do not count.
pub(super) fn body_has_value_return(body: &[php::PhpStmt]) -> bool {
    use php::PhpStmt::{Block, Echo, Expr, For, Foreach, If, Return, While};
    body.iter().any(|s| match s {
        Return(opt) => opt.is_some(),
        If {
            then, elifs, els, ..
        } => {
            body_has_value_return(then)
                || elifs.iter().any(|(_, b)| body_has_value_return(b))
                || els.as_deref().is_some_and(body_has_value_return)
        }
        While { body, .. } | For { body, .. } | Foreach { body, .. } | Block(body) => {
            body_has_value_return(body)
        }
        // A `try` returns a value if ANY of its arms does — the body, any catch, or the finally.
        php::PhpStmt::Try {
            body,
            catches,
            finally_block,
        } => {
            body_has_value_return(body)
                || catches.iter().any(|c| body_has_value_return(&c.body))
                || finally_block.as_deref().is_some_and(body_has_value_return)
        }
        // A `throw` DIVERGES rather than returning a value — it is not a value-return, and treating it
        // as one would make a function that only throws look like it returns something.
        php::PhpStmt::Throw(_) => false,
        Expr(_) | Echo(_) | php::PhpStmt::Break | php::PhpStmt::Continue => false,
    })
}

pub(super) fn lift_type(t: &php::PhpType) -> Result<Type, String> {
    match t {
        php::PhpType::Named(name) => match name.as_str() {
            "int" | "float" | "string" | "bool" | "void" => Ok(named(name)),
            "array" => Err("lift: an `array` type needs List/Map/Set inference (Tier-2) — annotate it \
                            (`@param list<T> $x`, `@return array<K, V>`, `@var T[]`) and the lifter \
                            reads the docblock"
                .into()),
            "mixed" | "iterable" | "object" | "callable" | "self" | "static" | "parent" => {
                Err(format!("lift: the `{name}` type is Tier-2/Tier-3"))
            }
            // A bare `Closure` carries no signature; phorj's function types do.
            "Closure" => Err(
                "lift: a `Closure` type needs a signature — write the phorj function \
                             type by hand (Tier-2)"
                    .into(),
            ),
            // A class/enum/interface name.
            _ => Ok(named(name)),
        },
        php::PhpType::Nullable(inner) => Ok(Type::Optional {
            inner: Box::new(lift_type(inner)?),
            span: SP,
        }),
        // Lane R-4: a docblock generic the parser substituted for a bare `array`.
        php::PhpType::Generic { name, args } => {
            let args: Vec<Type> = args.iter().map(lift_type).collect::<Result<_, _>>()?;
            let head = match (name.as_str(), args.len()) {
                ("list", 1) | ("array", 1) => "List",
                ("array", 2) => "Map",
                _ => return Err(format!("lift: the docblock generic `{name}` is Tier-2")),
            };
            Ok(Type::Named {
                name: head.into(),
                args,
                span: SP,
            })
        }
    }
}

pub(super) fn lift_binop(op: php::PhpBinOp) -> Result<BinaryOp, String> {
    use php::PhpBinOp as P;
    Ok(match op {
        P::Add => BinaryOp::Add,
        P::Sub => BinaryOp::Sub,
        P::Mul => BinaryOp::Mul,
        P::Div => BinaryOp::Div,
        P::Rem => BinaryOp::Rem,
        // PHP string concatenation `.` is Phorj's type-directed `+`.
        P::Concat => BinaryOp::Add,
        // Phorj is statically typed, so loose and strict equality coincide.
        P::Eq | P::Identical => BinaryOp::Eq,
        P::NotEq | P::NotIdentical => BinaryOp::NotEq,
        P::Lt => BinaryOp::Lt,
        P::Le => BinaryOp::Le,
        P::Gt => BinaryOp::Gt,
        P::Ge => BinaryOp::Ge,
        P::And => BinaryOp::And,
        P::Or => BinaryOp::Or,
        P::Coalesce => BinaryOp::Coalesce,
        // C-47: bitwise / shift map 1:1 to Phorj's existing operators (PHP-identical int semantics).
        P::BitAnd => BinaryOp::BitAnd,
        P::BitOr => BinaryOp::BitOr,
        P::BitXor => BinaryOp::BitXor,
        P::Shl => BinaryOp::Shl,
        P::Shr => BinaryOp::Shr,
    })
}

/// A static-member access `Class.name` (covers `Class::CONST`, `Class::$prop`, and the callee of
/// `Class::method(...)`).
pub(super) fn static_member(class: &str, name: &str) -> Expr {
    Expr::Member {
        object: Box::new(Expr::Ident(class.to_string(), SP)),
        name: name.to_string(),
        safe: false,
        // PHP `::` static access (DEC-207) — round-trips back to `::`.
        sep: crate::ast::MemberSep::ColonColon,
        span: SP,
    }
}

/// `Output.print(arg)` — the lift target of a PHP `echo`.
/// `new List<T>()` / `new Map<K, V>()` for a typed empty literal (Lane R-6) — `ty` is what
/// `lift_type` made of a docblock generic or a declared return type.
pub(super) fn new_coll(ty: &Type) -> Result<Expr, String> {
    let Type::Named { name, args, .. } = ty else {
        return Err("lift: an empty collection literal needs a List/Map type".into());
    };
    let kind = match name.as_str() {
        "List" => crate::ast::CollKind::List,
        "Map" => crate::ast::CollKind::Map,
        other => {
            return Err(format!(
                "lift: `{other}` is not a collection type for an empty literal"
            ))
        }
    };
    Ok(Expr::NewColl {
        kind,
        args: args.clone(),
        span: SP,
    })
}

/// `List.append(target, value)` for `$xs[] = v` (Lane R-3); records `Core.List` for the DEC-312
/// import pass so the draft imports what it calls.
pub(super) fn list_append(target: Expr, value: Expr) -> Expr {
    crate::lift::lifter::record_native_module("Core.List");
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident("List".into(), SP)),
            name: "append".into(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args: vec![target, value],
        type_args: Vec::new(),
        span: SP,
    }
}

pub(super) fn console_print(arg: Expr) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident("Output".into(), SP)),
            name: "print".into(),
            safe: false,
            // Synthesized echo target, not lifted from a PHP `::` (DEC-207).
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args: vec![arg],
        type_args: Vec::new(),
        span: SP,
    }
}

pub(super) fn vis_modifier(v: php::PhpVisibility) -> Modifier {
    match v {
        php::PhpVisibility::Public => Modifier::Public,
        php::PhpVisibility::Private => Modifier::Private,
        php::PhpVisibility::Protected => Modifier::Protected,
    }
}

/// The Phorj type-name for a literal expression (used to type a lifted class `const`).
pub(super) fn lit_type(e: &php::PhpExpr) -> Option<&'static str> {
    match e {
        php::PhpExpr::Int(_) => Some("int"),
        php::PhpExpr::Float(_) => Some("float"),
        php::PhpExpr::Str(_) => Some("string"),
        php::PhpExpr::Bool(_) => Some("bool"),
        _ => None,
    }
}

pub(super) fn named(name: &str) -> Type {
    Type::Named {
        name: name.to_string(),
        args: Vec::new(),
        span: SP,
    }
}

/// Strip PHP's root-namespace marker from a class name: `\RuntimeException` → `RuntimeException`.
///
/// THE one place that rule lives. `new` and `catch` both need it, and having each strip its own was the
/// bug this replaced: the catch clause stripped, the `new` did not, so a lifted
/// `throw new \RuntimeException(…)` emitted a `\` that is not valid phorj at all — an unparseable draft
/// beside a correctly-lifted catch in the same function.
///
/// Inner separators are LEFT ALONE (`Acme\MyError` keeps its shape): a namespaced user class is not the
/// same thing as a root-qualified builtin, and flattening it would invent a name.
pub(super) fn strip_root_ns(name: &str) -> &str {
    name.strip_prefix('\\').unwrap_or(name)
}
