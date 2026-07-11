//! Abstract syntax tree: the parser's output and the shared input to every backend (checker,
//! tree-walking interpreter, bytecode compiler, PHP transpiler). Nodes are **untyped** — the
//! checker validates without annotating, so each backend re-derives the types it needs (see
//! `compiler::CTy`). `token::Span` is carried on nodes for diagnostics.

use crate::token::Span;

// AST analyses live in sibling files (M-Decomp W3.3); re-exported so callers keep
// using `crate::ast::{free_vars, class_implements, ...}` unchanged.
mod classes;
mod walk;
pub use classes::*;
pub use walk::*;

/// Type annotations (e.g. `int`, `List<Shape>`, `T?`).
mod decls;
mod exprs;
mod stmts;
mod types_core;
pub use decls::*;
pub use exprs::*;
pub use stmts::*;
pub use types_core::*;

#[cfg(test)]
mod tests;
