//! Lane R-6 — the builder idiom `$out = []; …; return $out;` gets its type from the function's
//! declared return (`@return list<T>` / `array<K, V>` after R-4, or a phorj-typed return): a local
//! declared with an EMPTY list literal, returned at the top level of the same body, becomes
//! `mutable var out = new List<T>()` typed as the return. Nothing is inferred from the elements —
//! the type comes from a declaration the program wrote. Any other empty literal is left as is (the
//! checker then asks for its type, exactly as before).

use super::*;

pub(super) fn seed_returned_empty_literals(body: &mut [Stmt], ret: &Option<Type>) {
    let Some(ret) = ret else {
        return;
    };
    // A nullable return (`?array` + `@return list<T>`) seeds from its inner type: the local is a
    // plain `List<T>`, and `return $out;` widens to `List<T>?` at the return, as it would in PHP.
    let ret = match ret {
        Type::Optional { inner, .. } => inner.as_ref(),
        other => other,
    };
    if !matches!(ret, Type::Named { name, .. } if name == "List" || name == "Map") {
        return;
    }
    let returned: Vec<String> = body
        .iter()
        .filter_map(|s| match s {
            Stmt::Return {
                value: Some(Expr::Ident(n, _)),
                ..
            } => Some(n.clone()),
            _ => None,
        })
        .collect();
    for s in body.iter_mut() {
        if let Stmt::VarDecl { name, init, .. } = s {
            if returned.contains(name) && matches!(init, Expr::List(items, _) if items.is_empty()) {
                // The declaration stays `var`: `new Map<K, V>()` already carries the type, and the
                // `@var` path prints the same shape.
                if let Ok(coll) = super::super::mappings::new_coll(ret) {
                    *init = coll;
                }
            }
        }
    }
}
