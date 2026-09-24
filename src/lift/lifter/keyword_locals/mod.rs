//! DEC-531 (scout row 5k): PHP locals and parameters named with a phorj reserved word.
//!
//! PHP lets `$type`, `$new`, `$match` or `$class` name a variable; phorj reserves those words. A
//! local or a plain parameter is private to its function, so it is renamed with a `Value` suffix
//! (`$type` → `typeValue`) BEFORE lifting, across the whole function body including closures — a closure
//! capturing `$type` must see the same new name. Members keep their names: the parser accepts a
//! reserved word in member position, so a promoted `$type` stays the field `type`.
//!
//! Two refusals, both by name, never a guess:
//! * `typeValue` already taken in the same function. Choosing another spelling instead would mean
//!   a named argument `type:` (renamed `typeValue:`) could bind the OTHER parameter, silently.
//!
//! Why `Value` and not `_`: `E-NAME-CASE` rejects any `_` in a value name, so `type_` never checked
//! (DEC-531 as amended, 2026-09-24).
//! * a promoted reserved-word parameter READ in its constructor body — the body would need a local
//!   phorj cannot name; `$this->type` reads the same value.
//!
//! Named arguments follow their parameter: on `new X(…)` they name a constructor parameter, which
//! for a reserved word can only be a promoted one (a plain one is refused above), so they keep the
//! name; on any other call they get the `Value` suffix. A mismatch fails `phg check` loudly, never silently.
//!
//! The walks are total over the PHP AST (Invariant 3's rule applied here: no `_` arm), so a new
//! variant cannot be skipped without a compile error.

use std::collections::{BTreeMap, BTreeSet};

use crate::lift::ast as php;
use crate::tokenizer::is_reserved_word;

mod walk;
use walk::{walk_params, walk_stmts};

/// What a visited name is: a variable (renamed through the scope's map) or a named argument of a
/// non-`new` call (renamed by the word alone — its parameter's scope is not this one).
#[derive(Clone, Copy, PartialEq)]
enum Site {
    Var,
    NamedArg,
}

type Sites<'a> = dyn FnMut(&mut String, Site) + 'a;

/// The suffix a reserved-word local gets (`$type` → `typeValue`).
const SUFFIX: &str = "Value";

/// A copy of `prog` with every reserved-word local and parameter renamed.
pub(in crate::lift) fn rename(prog: &php::PhpProgram) -> Result<php::PhpProgram, String> {
    let mut prog = prog.clone();
    let mut top: Vec<&mut php::PhpStmt> = Vec::new();
    for item in &mut prog.items {
        match item {
            php::PhpItem::Function(f) => {
                scope(&f.name, &mut f.params, &mut f.body, false)?;
            }
            php::PhpItem::Class(c) => {
                let class = c.name.clone();
                for m in &mut c.members {
                    match m {
                        php::PhpMember::Method(m) => method(&class, m)?,
                        php::PhpMember::Prop { .. } | php::PhpMember::Const { .. } => {}
                    }
                }
            }
            php::PhpItem::Enum(e) => {
                let class = e.name.clone();
                for m in &mut e.methods {
                    method(&class, m)?;
                }
            }
            php::PhpItem::Interface(i) => {
                let class = i.name.clone();
                for m in &mut i.methods {
                    method(&class, m)?;
                }
            }
            php::PhpItem::Stmt(s) => top.push(s),
        }
    }
    // The file's top-level statements share ONE synthesized `main`, so they share one scope.
    let mut body: Vec<php::PhpStmt> = top.iter().map(|s| (**s).clone()).collect();
    scope("main", &mut [], &mut body, false)?;
    for (slot, new) in top.into_iter().zip(body) {
        *slot = new;
    }
    Ok(prog)
}

fn method(class: &str, m: &mut php::PhpMethod) -> Result<(), String> {
    let name = format!("{class}::{}", m.name);
    let mut empty = Vec::new();
    let body = m.body.as_mut().unwrap_or(&mut empty);
    scope(&name, &mut m.params, body, m.name == "__construct")
}

/// Rename the reserved-word variables of ONE function scope (params + body, closures included).
fn scope(
    fname: &str,
    params: &mut [php::PhpParam],
    body: &mut [php::PhpStmt],
    ctor: bool,
) -> Result<(), String> {
    // Promoted constructor parameters are FIELDS: they keep their name.
    let promoted: BTreeSet<String> = params
        .iter()
        .filter(|p| ctor && p.promotion.is_some())
        .map(|p| p.name.clone())
        .collect();
    let mut seen = BTreeSet::new();
    for p in params.iter_mut().filter(|p| !promoted.contains(&p.name)) {
        seen.insert(p.name.clone());
    }
    let mut collect = |n: &mut String, site: Site| {
        if site == Site::Var {
            seen.insert(n.clone());
        }
    };
    walk_params(params, &mut collect);
    walk_stmts(body, &mut collect);
    if let Some(n) = promoted
        .iter()
        .find(|n| is_reserved_word(n) && seen.contains(*n))
    {
        return Err(format!(
            "lift: promoted parameter `${n}` of `{fname}` is a phorj reserved word, so it lifts as the \
             field `{n}` — but the constructor body reads `${n}`, a local phorj cannot name; read \
             `$this->{n}` there instead (DEC-531)"
        ));
    }
    let mut map = BTreeMap::new();
    for n in seen.iter().filter(|n| is_reserved_word(n) && *n != "this") {
        let to = format!("{n}{SUFFIX}");
        if seen.contains(&to) {
            return Err(format!(
                "lift: `${n}` in `{fname}` is a phorj reserved word and its renaming `${to}` is \
                 already taken — rename one of them in the PHP source (DEC-531)"
            ));
        }
        map.insert(n.clone(), to);
    }
    let mut apply = |n: &mut String, site: Site| match site {
        Site::Var => {
            if let Some(to) = map.get(n.as_str()) {
                *n = to.clone();
            }
        }
        Site::NamedArg => {
            if is_reserved_word(n) {
                n.push_str(SUFFIX);
            }
        }
    };
    for p in params.iter_mut().filter(|p| !promoted.contains(&p.name)) {
        apply(&mut p.name, Site::Var);
    }
    walk_params(params, &mut apply);
    walk_stmts(body, &mut apply);
    Ok(())
}
