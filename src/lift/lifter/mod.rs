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
mod array_fns;
mod attrs;
mod const_infer;
mod decls;
mod division;
mod enums;
mod exceptions;
mod exceptions_exprs;
mod exprs;
mod keyword_locals;
mod leaves;
mod magic;
mod map_consts;
mod mappings;
mod matches;
mod shapes;
mod spread;
mod throw_expr;
use attrs::AttrCtx;
use const_infer::const_type;
pub use decls::*;
use division::float_division;
pub(super) use enums::{enum_names_of, set_project_enum_names, EnumSymbols};
use exprs::*;
use leaves::*;
use mappings::*;
use matches::*;
use shapes::{enter_binder, is_tuple_field, leave_binder, set_tuple_fields};

// DEC-312: the Core modules referenced by builtin→native resolutions during one lift, drained into
// `import` items at assembly. Thread-local (the lifter is stateless free functions; a lift runs on
// one thread) — reset at the start of every `lift_program` so runs never leak into each other.
thread_local! {
    static CONSOLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LIFTED_NATIVE_MODULES: std::cell::RefCell<std::collections::BTreeSet<&'static str>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

/// `echo` was lifted somewhere in this run, so the draft needs `import Core.Output`. A flag rather
/// than a `Lifter` field since 2026-09-07: a block-closure body is lifted through the FREE
/// `lift_expr`, which has no `Lifter` to set, and an `echo` inside one would otherwise lose its
/// import silently.
pub(super) fn note_console() {
    CONSOLE.with(|c| c.set(true));
}

pub(super) fn reset_console() {
    CONSOLE.with(|c| c.set(false));
}

pub(super) fn took_console() -> bool {
    CONSOLE.with(|c| c.get())
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

/// The text of a string literal that has no interpolation holes, else `None`.
///
/// An array key written with interpolation (`$row["$k"]`) is not a statically known field name, so
/// it is left as an index read rather than guessed at.
pub(super) fn str_literal_text(parts: &[StrPart]) -> Option<String> {
    match parts {
        [] => Some(String::new()),
        [StrPart::Literal(t)] => Some(t.clone()),
        _ => None,
    }
}
