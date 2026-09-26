//! PHP parser — ASSIGNMENT TARGETS: what may appear on the left of `=`, and the destructuring form
//! that is not an lvalue at all but a binder list (DEC-510). Split out of `exprs.rs` under
//! Invariant 13 (lane L1c, 2026-09-07).

use super::*;

/// A valid assignment / increment target: a variable, an index, an instance/static property.
/// The binder names of a POSITIONAL destructuring target, or `None` when `e` is not one.
///
/// `Err` is reserved for a target that IS a destructure but not a positional one: a keyed
/// `['k' => $v]` reads a map by key rather than a tuple by position, and a nested or defaulted
/// element has no flat phorj form. Refusing those beats treating them as positional, which would
/// silently bind the wrong values (DEC-166 — the lifter does not guess).
pub(super) fn destructure_binders(e: &PhpExpr) -> Result<Option<Vec<String>>, String> {
    let elems: &[PhpArrayElem] = match e {
        PhpExpr::Array(elems) => elems,
        // `list($a, $b)` parses as a call to a function named `list`.
        PhpExpr::Call { callee, args } => match callee.as_ref() {
            PhpExpr::Name(n) if n == "list" => {
                let mut names = Vec::new();
                for a in args {
                    match a {
                        PhpExpr::Var(v) => names.push(v.clone()),
                        _ => {
                            return Err(
                                "lift: a `list(…)` destructure binds plain variables only — \
                                        a nested or keyed element has no flat phorj form"
                                    .into(),
                            )
                        }
                    }
                }
                return Ok(Some(names));
            }
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    if elems.is_empty() {
        return Ok(None);
    }
    let mut names = Vec::new();
    for el in elems {
        if el.key.is_some() {
            return Err(
                "lift: a KEYED destructure (`['k' => $v] = …`) reads a map by key, not a \
                        tuple by position — it has no phorj form and is not treated as positional"
                    .into(),
            );
        }
        match &el.value {
            PhpExpr::Var(v) => names.push(v.clone()),
            // An array literal on the LEFT of `=` whose elements are not plain variables is not a
            // destructure at all (it is an ordinary literal being assigned to, which PHP rejects);
            // fall through to the existing lvalue error rather than inventing a message.
            _ => return Ok(None),
        }
    }
    Ok(Some(names))
}

pub(super) fn is_lvalue(e: &PhpExpr) -> bool {
    matches!(
        e,
        PhpExpr::Var(_)
            | PhpExpr::AppendSlot(_)
            | PhpExpr::Index { .. }
            | PhpExpr::Member { .. }
            | PhpExpr::StaticProp { .. }
    )
}

impl super::PParser {
    /// A `foreach` value that is a positional destructure: `[` or `list(` (row 5v).
    pub(super) fn at_foreach_pattern(&self) -> bool {
        self.at(&PTok::LBracket)
            || matches!((self.peek(), self.peek_at(1)), (PTok::Ident(n), PTok::LParen) if n == "list")
    }

    /// `[$a, $b]` / `list($a, $b)` after `as` — DEC-510's positional destructure, two or more plain
    /// variables. The pattern parses as an expression and goes through [`destructure_binders`], so
    /// a keyed or nested element is refused with the same words as an assignment destructure. A
    /// SKIPPED slot (`[, $x]`) is caught first: phorj has no wildcard binder (`_` is not a legal
    /// name), and the expression parser would only say "found Comma".
    pub(super) fn parse_foreach_pattern(&mut self) -> Result<PhpForeachValue, String> {
        let mut i = if self.at(&PTok::LBracket) { 1 } else { 2 };
        let (mut depth, mut after_sep) = (1usize, true);
        loop {
            match self.peek_at(i) {
                PTok::LBracket | PTok::LParen => (depth, after_sep) = (depth + 1, false),
                PTok::RBracket | PTok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    after_sep = false;
                }
                PTok::Comma if depth == 1 => {
                    if after_sep {
                        return Err(self.err(
                            "a skipped slot in a foreach destructure (`[, $x]`) has no lift — phorj \
                             has no wildcard binder; name the slot",
                        ));
                    }
                    after_sep = true;
                }
                PTok::Eof => break,
                _ => after_sep = false,
            }
            i += 1;
        }
        let pat = self.parse_expr()?;
        match destructure_binders(&pat)? {
            Some(names) if names.len() >= 2 => Ok(PhpForeachValue::Tuple(names)),
            Some(_) => Err(self.err(
                "a foreach destructure needs two or more binders — iterate the element itself",
            )),
            None => Err(self.err(
                "a foreach destructure binds plain variables only — a nested or keyed element has \
                 no flat phorj form",
            )),
        }
    }
}
