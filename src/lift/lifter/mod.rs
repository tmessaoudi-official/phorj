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
mod attrs;
mod decls;
mod enums;
mod exceptions;
mod exprs;
mod leaves;
mod magic;
mod mappings;
mod matches;
use attrs::AttrCtx;
pub use decls::*;
pub(super) use enums::{enum_names_of, set_project_enum_names, EnumSymbols};
use exprs::*;
use leaves::*;
use mappings::*;
use matches::*;

// DEC-312: the Core modules referenced by builtin→native resolutions during one lift, drained into
// `import` items at assembly. Thread-local (the lifter is stateless free functions; a lift runs on
// one thread) — reset at the start of every `lift_program` so runs never leak into each other.
thread_local! {
    static CONSOLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// DEC-504 — the NAMED-TUPLE fields of each variable in the function being lifted, keyed by
    /// variable name. Populated from the lifted parameters (a `@param array{bp: int, …} $row`
    /// becomes a `Type::Tuple` carrying its labels), and read by the `Index` arm so `$row['bp']`
    /// lifts to `row.bp` rather than to a string index a tuple cannot answer.
    ///
    /// Thread-local for the same reason as the two above: the lifter is stateless free functions, so
    /// a fact learned in the signature has no other route to the body. FUNCTION-scoped — cleared at
    /// the start of every function lift, because `$row` in one function is not `$row` in the next.
    static TUPLE_FIELDS: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
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

/// Record the named-tuple fields of every parameter, replacing the previous function's map
/// (DEC-504). A parameter whose lifted type is a POSITIONAL tuple contributes nothing — there are
/// no field names to read by.
pub(super) fn set_tuple_fields(params: &[Param]) {
    TUPLE_FIELDS.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for p in params {
            if let Type::Tuple(_, Some(labels), _) = &p.ty {
                m.insert(
                    p.name.clone(),
                    labels.iter().map(|l| l.name.clone()).collect(),
                );
            }
        }
    });
}

/// Whether `var` is a named tuple with a field called `field` — the test that turns `$row['bp']`
/// into `row.bp`. A key the shape does NOT declare answers `false` and stays an index read: the
/// lifter invents no field, and the checker is the right place for that mistake to surface.
pub(super) fn is_tuple_field(var: &str, field: &str) -> bool {
    TUPLE_FIELDS.with(|m| {
        m.borrow()
            .get(var)
            .is_some_and(|fs| fs.iter().any(|f| f == field))
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
