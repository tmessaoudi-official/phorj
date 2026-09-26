//! Row 5v — `foreach ($xs as [$a, $b])` lifts to phorj's own lowering of the tuple loop (DEC-288):
//! `for (var __fortup_N in xs) { var (a, b) = __fortup_N; … }`. That is exactly the shape the phorj
//! parser builds for `for ((a, b) in xs)`, so the checker and every backend already accept it, and
//! the formatter re-collapses it (`format/printer/stmts.rs`) — `cmd_lift` formats its draft, so a
//! shipped draft reads `for ((a, b) in xs)` and never shows the synthetic binder.
//!
//! N is a per-file counter: two sequential loops may share a name, but a NESTED pair may not, and
//! the lifter's spans are all `SP`, so the parser's span-derived suffix is not available here.

use super::*;

thread_local! {
    static NEXT_FORTUP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Start a file's numbering at 0, beside the other per-file resets (`map_consts::begin_file`).
pub(in crate::lift) fn begin_file() {
    NEXT_FORTUP.with(|n| n.set(0));
}

fn next_binder() -> String {
    NEXT_FORTUP.with(|n| {
        let i = n.get();
        n.set(i + 1);
        format!("__fortup_{i}")
    })
}

impl Lifter {
    /// The destructuring `foreach`. The binders are declared by the loop and scoped to it: each one
    /// the enclosing function has not declared is added for the body and removed after it, so a
    /// `$a = …` after the loop — PHP's binders leak, phorj's do not — lifts as a fresh declaration.
    /// Body-declared locals are left in `declared` exactly as the one-variable `foreach` leaves them.
    pub(in crate::lift::lifter) fn lift_tuple_foreach(
        &mut self,
        array: &php::PhpExpr,
        binders: &[String],
        body: &[php::PhpStmt],
        declared: &mut HashSet<String>,
    ) -> Result<Vec<Stmt>, String> {
        // A list the shape registry knows holds NAMED tuples (DEC-515): PHP's `[$a, $b]` reads keys
        // 0 and 1, which a keyed element does not have (null + a warning), while phorj's positional
        // erasure would hand back the field values — same source, different output, no error.
        if let php::PhpExpr::Var(v) = array {
            if super::super::shapes::fields_of(v).1.is_some() {
                return Err(format!(
                    "lift: a positional `foreach` destructure over `${v}`, a list of NAMED tuples, \
                     reads indexes 0, 1, … that its elements do not have (PHP yields null) — read \
                     the fields by name instead"
                ));
            }
        }
        let iter = lift_expr(array)?;
        let tmp = next_binder();
        let scope = super::super::enter_binder(&iter, &tmp);
        let added: Vec<String> = binders
            .iter()
            .filter(|b| declared.insert((*b).clone()))
            .cloned()
            .collect();
        let lifted = self.lift_block(body, declared);
        for b in &added {
            declared.remove(b);
        }
        super::super::leave_binder(scope);
        let mut body = lifted?;
        body.insert(
            0,
            Stmt::Destructure {
                pat: crate::ast::DestructurePat::Tuple {
                    binders: binders.iter().map(|b| (None, b.clone(), SP)).collect(),
                    span: SP,
                },
                init: Expr::Ident(tmp.clone(), SP),
                else_block: None,
                span: SP,
            },
        );
        Ok(vec![Stmt::For {
            ty: Type::Infer(SP),
            name: tmp,
            val: None,
            iter,
            body,
            span: SP,
        }])
    }
}
