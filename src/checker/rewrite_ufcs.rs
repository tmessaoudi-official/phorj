use super::*;

/// Rewrite every resolved UFCS member call `x.f(a)` into the ordinary free/native call `f(x, a)` the
/// checker chose (keyed by the enclosing `Call` node's `Span.start`), so the interpreter, compiler,
/// and transpiler never see a UFCS-shaped `Member` call — the same "compile-time sugar, erased before
/// any backend" treatment as `type` aliases / generics / `html"…"` (Slice 6, F-001). Runs last in
/// [`crate::cli::check_and_expand`], after the other front-end sugar is gone, so the receiver and
/// arguments it relocates are already fully de-sugared. A recorded replacement embeds the original
/// receiver/argument subtrees, which may themselves contain UFCS (`xs.filter(p).map(g)`), so the
/// rewrite re-walks each substituted subtree — but never re-matches the replacement's own root span
/// (which equals the key), which would loop. When no UFCS was recorded the program is returned
/// untouched, so programs without UFCS are byte-for-byte identical to the pre-Slice-6 AST.
#[path = "rewrite_ufcs_walk.rs"]
mod walk;
use walk::*;

pub fn rewrite_ufcs(program: Program, ufcs: &HashMap<usize, crate::ast::Expr>) -> Program {
    use crate::ast::{ClassMember, Item};
    if ufcs.is_empty() {
        return program;
    }

    // Apply a recorded replacement, re-walking its children for nested sugar but reconstructing the
    // root directly (its span is the original key / synthetic, so it is never re-matched → no loop).
    // `#[inline(never)]` + a separate function keep these (relatively large) arms off `rexpr`'s
    // frame: `rexpr` recurses once per expression-tree level, so bloating its frame overflows the
    // stack on a deeply-nested program (a regression the differential's example sweep catches).

    let items = program
        .items
        .into_iter()
        .map(|item| match item {
            Item::Function(mut f) => {
                f.body = rblock(f.body, ufcs);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                for m in &mut c.members {
                    match m {
                        ClassMember::Method(f) => {
                            let body = std::mem::take(&mut f.body);
                            f.body = rblock(body, ufcs);
                        }
                        ClassMember::Constructor { body, .. } => {
                            let b = std::mem::take(body);
                            *body = rblock(b, ufcs);
                        }
                        ClassMember::Hook { get, set, .. } => {
                            if let Some(e) = get.take() {
                                *get = Some(rexpr(e, ufcs));
                            }
                            if let Some((p, body)) = set.take() {
                                *set = Some((p, rblock(body, ufcs)));
                            }
                        }
                        // A field initializer (Feature B) may contain UFCS — rewrite it (resolve_html
                        // skips fields, but the checker checks field-init expressions, so a recorded
                        // UFCS site here must be applied or the backend would see the raw member call).
                        ClassMember::Field { init, .. } => {
                            if let Some(e) = init.take() {
                                *init = Some(rexpr(e, ufcs));
                            }
                        }
                    }
                }
                Item::Class(c)
            }
            other => other,
        })
        .collect();

    Program {
        package: program.package,
        items,
        span: program.span,
    }
}
