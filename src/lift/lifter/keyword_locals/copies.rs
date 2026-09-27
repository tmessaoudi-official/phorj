//! Row 4q, DEC-548 — a parameter its body REASSIGNS lifts to a mutable local copy.
//!
//! PHP parameters are ordinary locals; phorj's are immutable, with no `mutable` parameter form, and a
//! local may not shadow one. So a parameter the body writes — `$bp = …`, `$bp += …`, `$bp++`,
//! `$bp[…] = …`, `$bp[] = …`, a destructure or `foreach` binder, a `catch` variable — gets a fresh
//! local, `bpLocal`, assigned from it on the first line, and every use in the body (closures
//! included: a capture must see the copy) is renamed to it. The signature is untouched, so a named
//! argument `cap(bp: 12)` still binds. A write THROUGH the parameter (`$box->x = …`) is not a
//! reassignment of it and copies nothing.
//!
//! The copy's initializer carries the parameter's declared type (`Declared`, the `@var` form), so a
//! shaped or collection parameter's copy is registered exactly as the parameter was (DEC-515).
//! Kotlin parameters are `val` and Swift removed `var` parameters (SE-0003); the copy is the idiom
//! both leave.

use std::collections::{BTreeMap, BTreeSet};

use crate::lift::ast as php;

/// The copy each written parameter gets, in parameter order: `(parameter, copy)`. A name the scope
/// already uses is skipped (`vLocal` taken → `vLocal2`), deterministically.
pub(super) fn plan(
    params: &[php::PhpParam],
    written: &BTreeSet<String>,
    seen: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let mut taken = seen.clone();
    let mut out = Vec::new();
    for p in params.iter().filter(|p| written.contains(&p.name)) {
        let base = format!("{}Local", p.name);
        let mut to = base.clone();
        let mut n = 2;
        while taken.contains(&to) {
            to = format!("{base}{n}");
            n += 1;
        }
        taken.insert(to.clone());
        out.push((p.name.clone(), to));
    }
    out
}

/// The body's first statements: `$copy = $param;`, typed as the parameter was declared.
pub(super) fn prepend(
    body: &mut Vec<php::PhpStmt>,
    params: &[php::PhpParam],
    copies: &[(String, String)],
) {
    let ty: BTreeMap<&str, &php::PhpType> = params
        .iter()
        .filter_map(|p| p.ty.as_ref().map(|t| (p.name.as_str(), t)))
        .collect();
    let first = copies.iter().map(|(from, to)| {
        let read = php::PhpExpr::Var(from.clone());
        let value = match ty.get(from.as_str()) {
            Some(t) => php::PhpExpr::Declared {
                ty: (*t).clone(),
                value: Box::new(read),
            },
            None => read,
        };
        php::PhpStmt::Expr(php::PhpExpr::Assign {
            target: Box::new(php::PhpExpr::Var(to.clone())),
            value: Box::new(value),
        })
    });
    body.splice(0..0, first.collect::<Vec<_>>());
}
