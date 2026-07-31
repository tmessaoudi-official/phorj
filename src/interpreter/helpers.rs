//! Stateless interpreter helpers — statement line lookup, the `Signal`/diagnostic constructors, and
//! the small coercions. Split out of `mod.rs` when DEC-364 pushed that file past Invariant 13's cap;
//! none of these touch `Interp` state, which is exactly why they belong outside it.
//!
//! `stmt_line` is exhaustive over `Stmt` (Invariant 3 / DEC-356) — every statement has a span and a
//! trace frame needs it, so a new variant must be considered here rather than defaulting to line 0.

use super::*;

/// The source line of a statement, for runtime trace frames (error-handling slice 1).
pub(super) fn stmt_line(s: &Stmt) -> u32 {
    match s {
        Stmt::VarDecl { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::If { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::CFor { span, .. }
        | Stmt::Throw { span, .. }
        | Stmt::Try { span, .. }
        | Stmt::Destructure { span, .. }
        | Stmt::Using { span, .. } => span.line,
        Stmt::Break(s)
        | Stmt::Continue(s)
        | Stmt::Block(_, s)
        | Stmt::Expr(_, s)
        | Stmt::Discard(_, s) => s.line,
    }
}

pub(super) fn rt<T>(msg: impl Into<String>) -> R<T> {
    Err(Signal::Runtime(Diagnostic::runtime(msg)))
}

/// Flatten a runtime `Signal` to its message body for the higher-order-native callback boundary (a
/// [`crate::native::ClosureInvoker`] returns `Result<_, String>`, the backend-shared fault contract).
/// A `Return` escaping `call_closure` would be an interpreter bug — a closure's return value is
/// consumed inside the call, never surfaced — so it maps to a defensive internal-error string.
pub(super) fn signal_msg(sig: Signal) -> String {
    match sig {
        Signal::Runtime(d) => d.message,
        Signal::Return(_) => "internal error: closure return escaped".to_string(),
        Signal::Break | Signal::Continue => "internal error: loop control escaped".to_string(),
        // A `Throw` is intercepted before this point at the native boundary (it becomes the
        // sentinel + `pending_throw`); reaching here would be an interpreter bug.
        Signal::Throw(_) => "internal error: throw escaped to native boundary".to_string(),
    }
}

/// The literal text of a fault intrinsic's string-literal message argument (M-faults 2a). The checker
/// guarantees it is a single `StrPart::Literal`; defaults to empty (e.g. a bare `assert(cond)`).
pub(super) fn lit_msg(e: Option<&Expr>) -> String {
    if let Some(Expr::Str(parts, _)) = e {
        if let [crate::ast::StrPart::Literal(s)] = &parts[..] {
            return s.clone();
        }
    }
    String::new()
}

pub(super) fn as_bool(v: &Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => rt(format!("expected bool, got {}", other.type_name())),
    }
}
