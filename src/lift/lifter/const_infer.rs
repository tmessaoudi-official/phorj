//! PHP lifter — the type of a `const array` with no usable declared type (DEC-533, scout row 5m).
//! PHP's `const array X = [...]` says nothing about its elements, so the literal is read instead:
//! every level must be one type, to any depth (Q2), and one scalar type plus `null` is `T?` (Q3). A
//! reference to another constant is refused (Q-0924-1, unruled), as is anything else — each by
//! naming the shape, never by suggesting a docblock the literal would still have to satisfy.

use super::*;

/// The phorj type of `value`, the initializer of the PHP constant `name`.
pub(super) fn infer_const_type(name: &str, value: &php::PhpExpr) -> Result<Type, String> {
    match infer(value) {
        Ok(Shape::Ty(t)) => Ok(t),
        Ok(Shape::Null) => Err(format!(
            "lift: const `{name}` is `null` — its type cannot be inferred"
        )),
        Err(why) => Err(format!("lift: const `{name}` {why}")),
    }
}

enum Shape {
    Null,
    Ty(Type),
}

fn infer(e: &php::PhpExpr) -> Result<Shape, String> {
    if matches!(e, php::PhpExpr::Null) {
        return Ok(Shape::Null);
    }
    if let Some(t) = lit_type(e) {
        return Ok(Shape::Ty(named(t)));
    }
    match e {
        php::PhpExpr::Array(elems) if elems.is_empty() => Err(
            "is an empty array — a phorj constant is never empty (`new List<T>()` is not a constant)"
                .into(),
        ),
        php::PhpExpr::Array(elems) => {
            let keyed = elems.iter().filter(|x| x.key.is_some()).count();
            if keyed != 0 && keyed != elems.len() {
                return Err("mixes keyed and positional elements".into());
            }
            let value = join(elems.iter().map(|x| &x.value))?;
            if keyed == 0 {
                return Ok(Shape::Ty(generic("List", vec![value])));
            }
            let key = join(elems.iter().filter_map(|x| x.key.as_ref()))?;
            if !matches!(&key, Type::Named { name, .. } if name == "int" || name == "string") {
                return Err("has a key that is not an `int` or a `string`".into());
            }
            Ok(Shape::Ty(generic("Map", vec![key, value])))
        }
        php::PhpExpr::ClassConst { .. } => Err(
            "references another constant — not yet a constant in phorj (Q-0924-1, unruled)".into(),
        ),
        _ => Err("has a value that is not a literal".into()),
    }
}

/// One type for every element: the shared non-null type, made `T?` when a `null` is present.
fn join<'a>(elems: impl Iterator<Item = &'a php::PhpExpr>) -> Result<Type, String> {
    let mut ty: Option<Type> = None;
    let mut nullable = false;
    for e in elems {
        match infer(e)? {
            Shape::Null => nullable = true,
            Shape::Ty(t) => match &ty {
                None => ty = Some(t),
                Some(u) if *u == t => {}
                Some(_) => return Err("mixes element types — every level must be one type".into()),
            },
        }
    }
    let t = ty.ok_or("holds only `null` — its element type cannot be inferred")?;
    Ok(if nullable {
        Type::Optional {
            inner: Box::new(t),
            span: SP,
        }
    } else {
        t
    })
}

fn generic(name: &str, args: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_string(),
        args,
        span: SP,
    }
}
