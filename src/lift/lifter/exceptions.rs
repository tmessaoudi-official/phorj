//! PHP lifter — the EXCEPTION-CLASS analysis behind DEC-421's taxonomy mapping.
//!
//! Three questions are asked of a parsed PHP program, and all three are the SAME walk over the
//! statement tree looking for exception class names:
//!
//!   * [`unmapped_exception_classes`] — which names have no phorj counterpart (→ `// CANNOT LIFT:`);
//!   * [`mapped_error_types`] — which `Core.ErrorModule` types the draft will name (→ its imports);
//!   * [`phorj_error_name`] — what one name becomes.
//!
//! They were three separate recursive walks when this started, which is precisely the shape that
//! rots: the `throw new X` arm was added to two of them and, in one, to the WRONG one — so mapped
//! names were reported as unmappable and the draft carried bogus notes for types it had emitted
//! correctly. A single [`visit_exception_sites`] makes that class of mistake unrepresentable: a new
//! statement form is handled once, for every question at once.

use super::mappings::strip_root_ns;
use crate::lift::ast as php;

/// The phorj type name for a PHP exception class: the DEC-421 mapping when one exists, else the
/// class's own name with PHP's root marker stripped (`\RuntimeException` and `RuntimeException`
/// agree, which is what lets the `catch` and `new` legs of one program name the same type).
pub(in crate::lift) fn phorj_error_name(php: &str) -> String {
    crate::native::error_prelude::phorj_error_for_php_exception(php)
        .map_or_else(|| strip_root_ns(php).to_string(), str::to_string)
}

/// Every PHP exception class in this program that has NO DEC-421 mapping, sorted and deduped
/// (Invariant 10 — a lifted draft must not vary run to run).
///
/// One `// CANNOT LIFT:` note is emitted per entry, so the draft SAYS what it could not do rather
/// than leaving a name that will simply fail `phg check` with no explanation.
pub(in crate::lift) fn unmapped_exception_classes(prog: &php::PhpProgram) -> Vec<String> {
    collect(prog, |class| {
        crate::native::error_prelude::phorj_error_for_php_exception(class)
            .is_none()
            .then(|| strip_root_ns(class).to_string())
    })
}

/// The `Core.ErrorModule` types this program's exception sites actually map onto, sorted and deduped.
///
/// Only what is USED: importing all six when a program names one would put five unused imports in the
/// draft, which `phg check` then flags as `E-UNUSED-IMPORT`. A lift that fails the very check it
/// exists to pass is worse than no mapping.
pub(in crate::lift) fn mapped_error_types(prog: &php::PhpProgram) -> Vec<String> {
    collect(prog, |class| {
        crate::native::error_prelude::phorj_error_for_php_exception(class).map(str::to_string)
    })
}

/// Run `pick` over every exception class name in `prog`, keeping the `Some` answers sorted + deduped.
fn collect(prog: &php::PhpProgram, pick: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut out: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    visit_exception_sites(prog, &mut |class| {
        if let Some(v) = pick(class) {
            out.insert(v);
        }
    });
    out.into_iter().collect()
}

/// Call `f` with every PHP exception class name the program NAMES — each `catch` clause type (one per
/// union member) and each `throw new X(…)`.
///
/// Both positions matter and for the same reason: whichever one the draft prints has to resolve. A
/// program that only THROWS a builtin would otherwise emit the mapped name with nothing importing it
/// (`E-INJECTED-TYPE-BARE`), which is how the throw arm came to be added at all.
fn visit_exception_sites(prog: &php::PhpProgram, f: &mut impl FnMut(&str)) {
    for it in &prog.items {
        match it {
            php::PhpItem::Function(fun) => visit_body(&fun.body, f),
            php::PhpItem::Class(c) => {
                for m in &c.members {
                    // An ABSTRACT/interface method has no body — nothing to scan, not an error.
                    if let php::PhpMember::Method(me) = m {
                        if let Some(b) = &me.body {
                            visit_body(b, f);
                        }
                    }
                }
            }
            php::PhpItem::Stmt(s) => visit_body(std::slice::from_ref(s), f),
            php::PhpItem::Enum(_) => {}
        }
    }
}

/// Recurse through every nested body — an unmapped catch inside a loop inside an `if` is exactly the
/// one a shallow scan would miss. Exhaustive over `PhpStmt` (Invariant 3): a new statement form that
/// can hold a body extends this arm list in the same change, never a `_`.
fn visit_body(body: &[php::PhpStmt], f: &mut impl FnMut(&str)) {
    for s in body {
        match s {
            php::PhpStmt::Try {
                body,
                catches,
                finally_block,
            } => {
                visit_body(body, f);
                for c in catches {
                    for t in &c.types {
                        f(t);
                    }
                    visit_body(&c.body, f);
                }
                if let Some(fin) = finally_block {
                    visit_body(fin, f);
                }
            }
            php::PhpStmt::If {
                then, elifs, els, ..
            } => {
                visit_body(then, f);
                for (_, b) in elifs {
                    visit_body(b, f);
                }
                if let Some(e) = els {
                    visit_body(e, f);
                }
            }
            // `throw new X(…)` names a type the draft will print. A rethrow (`throw $e`) names none —
            // its type comes from the enclosing catch, which this walk has already seen.
            php::PhpStmt::Throw(php::PhpExpr::New { class, .. }) => f(class),
            php::PhpStmt::While { body, .. }
            | php::PhpStmt::For { body, .. }
            | php::PhpStmt::Foreach { body, .. }
            | php::PhpStmt::Block(body) => visit_body(body, f),
            php::PhpStmt::Return(_)
            | php::PhpStmt::Expr(_)
            | php::PhpStmt::Echo(_)
            | php::PhpStmt::Throw(_)
            | php::PhpStmt::Break
            | php::PhpStmt::Continue => {}
        }
    }
}

#[cfg(test)]
#[path = "exceptions_tests.rs"]
mod tests;
