//! DEC-197 — member-imported module FUNCTION bindings (`import Core.Output.printLine [as p];`).
//!
//! The two-mode import discipline already shipped for types (`import Core.Http.Router;`), variants
//! (`import Core.Result.Success;`) and intrinsics (`import Core.Abort.panic;`) is extended here to
//! module FUNCTIONS: a whole-module import (`import Core.Output;`) keeps the QUALIFIED call form
//! (`Output.printLine(x)`, unchanged) while a member import (`import Core.Output.printLine;`) enables
//! the BARE form (`printLine(x)`). The two are strict — a member import does NOT enable a qualified
//! sibling (`Output.print(x)` still needs `import Core.Output;`), exactly like the intrinsic model.
//!
//! This module is the single source of truth for *which* member imports name a stdlib function, shared
//! by the checker's bare-call resolution (the `fn_imports` map, [`super::Checker`]) and the collision
//! check (`check_function_import_collisions` in `program.rs`) so the two never diverge. The actual
//! bare→qualified rewrite is recorded by `check_named_call` (scope-aware, so `local > user fn >
//! imported native` holds) and applied by `rewrite_ufcs`, reusing the proven qualified-call path.

use crate::ast::Item;
use crate::token::Span;

/// For each member import `import <Module>.<fn> [as bound];` whose (`Module`, `fn`) names a real
/// stdlib native, one `(bound, module, real, span)`: `bound` is the call-site name (the `as` alias
/// else the function leaf), `module` the full dotted native module (`Core.Output`), `real` the native
/// leaf (`printLine`). A whole-module import (`import Core.Output;` — no native `Core.Output`), a
/// variant/type member import (Pascal leaf, not a native function), and an unknown leaf all yield
/// nothing here (handled by the other import maps / their own diagnostics).
pub(crate) fn function_import_bindings(items: &[Item]) -> Vec<(String, String, String, Span)> {
    let mut out = Vec::new();
    for it in items {
        let Item::Import {
            path, alias, span, ..
        } = it
        else {
            continue;
        };
        // A member function import is `Module.leaf` (≥ 2 segments); `module` is everything but the leaf.
        if path.len() < 2 {
            continue;
        }
        let module = path[..path.len() - 1].join(".");
        let real = &path[path.len() - 1];
        if crate::native::index_of(&module, real).is_some() {
            let bound = alias.clone().unwrap_or_else(|| real.clone());
            out.push((bound, module, real.clone(), *span));
        }
    }
    out
}
