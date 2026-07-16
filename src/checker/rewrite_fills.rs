//! M4/DEC-236/DEC-249 — splice the recorded default-parameter fills back into the program,
//! FIRST among the post-check rewrites.
//!
//! A fill is recorded at CHECK time as a full replacement `Call` (the provided arguments plus the
//! appended default literals) — a clone of the still-sugared source. It therefore MUST be spliced
//! before `expand_aliases`/`resolve_html`/`unwrap_new`/`rewrite_ufcs` run, so the spliced subtree
//! flows through the whole chain exactly like hand-written code. (The previous design merged fills
//! into the LAST pass's call-rewrite map, which restored already-erased nodes inside cloned
//! arguments — a lambda argument's throws-`?`, erased by `resolve_html`, came back stale and
//! faulted the backends: the `db.transaction(fn)` regression that exposed this.)
//!
//! Spliced content may itself contain further fill sites (a defaulted call inside a defaulted
//! call's arguments — the clone carries the ORIGINAL under-filled inner call, whose own fill is
//! keyed by its preserved span), so the splice is applied TOP-DOWN in one pass: the parent
//! splices first, then the walk descends into the freshly spliced children and fills them.
//! (Bottom-up + fixpoint alternates forever on nesting: the parent splice restores the unfilled
//! child, the child re-fills, the parent sees a mismatch again — the nested `db.transaction`
//! compile hang.)

use super::*;
use crate::ast::Expr;

/// Apply every recorded default fill (span-keyed full-call replacements) to the program, to a
/// fixpoint. A no-op when no call omitted defaulted arguments.
pub fn apply_default_fills(mut program: Program, fills: &HashMap<usize, Expr>) -> Program {
    if fills.is_empty() {
        return program;
    }
    rewrite_pipe::walk::visit_exprs_mut_pre(&mut program, &mut |e| {
        if let Expr::Call { span, .. } = e {
            if let Some(r) = fills.get(&span.start) {
                *e = r.clone();
            }
        }
    });
    program
}
