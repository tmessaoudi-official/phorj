//! M-Lift L4 — the **lifter**: PHP AST ([`super::ast`]) → Phorj AST ([`crate::ast`]). The lossy
//! half of the bridge. PHP is the floor, not the ceiling: lifted Phorj is *idiomatic* (PHP `.`
//! concat → `+`, `===` → `==`, top-level code → a `main()`, PHP fields → `mutable`) and never mirrors
//! a wart. The contract is a **draft you verify**, so the output is annotated `// lifted (verify)` by
//! the CLI (L6); anything that has no faithful Phorj form is a **loud lift error**, never a guess.
//!
//! Tier-1 core: typed functions, classes (typed props, ctor promotion, methods), pure enums, and the
//! plain statement/expression set. The Tier-2 frontier (`array`→List/Map/Set inference, default
//! params, backed enums, key-foreach, …) errors clearly here and is built out in later L4 slices.

use super::ast as php;
use super::lexer::lex_php;
use super::parser::parse_php;
use super::printer::print_program;
use crate::ast::{
    BinaryOp, ClassDecl, ClassMember, CtorParam, EnumDecl, EnumVariant, Expr, FunctionDecl, Item,
    MatchArm, Modifier, Param, Pattern, Program, Stmt, StrPart, Type, UnaryOp,
};
use crate::token::Span;
use std::collections::HashSet;

/// A zero span for synthesized nodes. The lift output is re-parsed (which re-derives real spans), and
/// the printer ignores spans, so a dummy is sound here.
const SP: Span = Span {
    start: 0,
    len: 0,
    line: 0,
    col: 0,
};

/// End-to-end convenience: PHP source → Phorj `.phg` source. Lexes (L1), parses (L2), lifts (L4),
/// and prints (L3). Any stage's error propagates as a `lift …` / `printer: …` string.
mod decls;
mod exprs;
mod magic;
pub use decls::*;
use exprs::*;

// DEC-312: the Core modules referenced by builtin→native resolutions during one lift, drained into
// `import` items at assembly. Thread-local (the lifter is stateless free functions; a lift runs on
// one thread) — reset at the start of every `lift_program` so runs never leak into each other.
thread_local! {
    static LIFTED_NATIVE_MODULES: std::cell::RefCell<std::collections::BTreeSet<&'static str>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

pub(super) fn record_native_module(module: &'static str) {
    LIFTED_NATIVE_MODULES.with(|m| {
        m.borrow_mut().insert(module);
    });
}

pub(super) fn drain_native_modules() -> Vec<&'static str> {
    LIFTED_NATIVE_MODULES.with(|m| {
        let mut set = m.borrow_mut();
        let out: Vec<&'static str> = set.iter().copied().collect();
        set.clear();
        out
    })
}
