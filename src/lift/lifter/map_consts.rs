//! DEC-534 (scout row 5q) — when PHP `+` is a MAP union. PHP spells array union and addition with
//! one operator, and the lifter has no types, so it rewrites `$a + $b` to `Map.union(a, b)` only
//! where it KNOWS both operands are maps: a keyed array literal, or a class constant of this file
//! whose lifted type is a `Map` (the same `const_type` rule the declaration uses, so the two cannot
//! disagree). A PHP `array` parameter or property can be a Map or a List, so it is not assumed —
//! DEC-166, the lifter never guesses — and a constant of ANOTHER file is not seen either.
//!
//! Thread-local for the reason `ENUM_NAMES` is (`enums.rs`): the lifter is stateless free functions
//! on one thread. Reset at the top of every file, so no file inherits another's constants.

use super::*;

thread_local! {
    /// `(class, constant)` for every Map-typed class constant of the file being lifted.
    static MAP_CONSTS: std::cell::RefCell<std::collections::BTreeSet<(String, String)>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

/// Begin one file: record its Map-typed class constants. A constant whose type does not lift is
/// simply absent here — its own declaration reports the refusal.
pub(in crate::lift) fn begin_file(prog: &php::PhpProgram) {
    let mut found = std::collections::BTreeSet::new();
    for item in &prog.items {
        let php::PhpItem::Class(c) = item else {
            continue;
        };
        for m in &c.members {
            if let php::PhpMember::Const {
                ty, name, value, ..
            } = m
            {
                if matches!(const_type(name, ty, value), Ok(Type::Named { name: n, .. }) if n == "Map")
                {
                    found.insert((c.name.clone(), name.clone()));
                }
            }
        }
    }
    MAP_CONSTS.with(|m| *m.borrow_mut() = found);
}

/// Is `e` known to lift to a phorj `Map`? A non-empty array literal whose every element is keyed, or
/// a registered constant (`self::`/`static::` arrive already resolved to the class name).
pub(super) fn is_known_map(e: &php::PhpExpr) -> bool {
    match e {
        php::PhpExpr::Array(elems) => !elems.is_empty() && elems.iter().all(|x| x.key.is_some()),
        php::PhpExpr::ClassConst { class, name } => {
            let leaf = class.rsplit('\\').next().unwrap_or(class);
            MAP_CONSTS.with(|m| m.borrow().contains(&(leaf.to_string(), name.clone())))
        }
        _ => false,
    }
}

/// `Map.union(left, right)` — the module form the DEC-534 ruling states, recording `Core.Map` for
/// the import pass. Called only after both operands lifted, so a failed lift leaves no import.
pub(super) fn map_union(left: Expr, right: Expr) -> Expr {
    record_native_module("Core.Map");
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident("Map".to_string(), SP)),
            name: "union".to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args: vec![left, right],
        type_args: Vec::new(),
        span: SP,
    }
}
