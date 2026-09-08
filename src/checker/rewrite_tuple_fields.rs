//! DEC-504 — rewrite every resolved named-tuple field read `t.bp` into the positional `t[1]` it
//! denotes, BEFORE the tuple erasure runs.
//!
//! Ordering is the whole design. This pass runs immediately before [`super::erase_tuples`], which
//! turns `Expr::Tuple` into `Expr::List`; by the time any backend looks, a named tuple is a list
//! and a field read is an index read. That is why the interpreter, the VM, the JIT and the
//! transpiler need NO changes for this feature, and why the developer's positional-PHP-leg ruling
//! (2026-09-08) costs nothing to honour: `$t[1]` is what the transpiler already emits for a tuple.
//!
//! The rewrite is applied to the LIVE tree, keyed by the `Member`'s `span.start` — never to a
//! check-time clone spliced back later. A clone would go stale the moment another pass edited that
//! subtree, which is the rewrite-clone-staleness class that produced three real bugs on 2026-07-16.
//!
//! **The index is a literal `Expr::Int`, and its span is the Member's own.** Reusing the span keeps
//! diagnostics and the reified-operand side-table (both keyed by `span.start`) pointing at the
//! source the user actually wrote.

use crate::ast::{Expr, Program};
use std::collections::HashMap;

/// Rewrite each recorded `t.field` into `t[index]`. A no-op for a program with no named tuples, so
/// every existing program is byte-identical to before.
pub fn rewrite_tuple_fields(mut program: Program, fields: &HashMap<usize, usize>) -> Program {
    if fields.is_empty() {
        return program;
    }
    super::rewrite_pipe::walk::visit_exprs_mut(&mut program, &mut |e| {
        let Expr::Member { object, span, .. } = e else {
            return;
        };
        let Some(&i) = fields.get(&span.start) else {
            return;
        };
        let span = *span;
        let object = std::mem::replace(object.as_mut(), Expr::Null(span));
        *e = Expr::Index {
            object: Box::new(object),
            index: Box::new(Expr::Int(i as i64, span)),
            span,
        };
    });
    program
}
