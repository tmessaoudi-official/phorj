//! DEC-208 slice F — the compile-time SQL-injection lint (`W-SQL-INJECTION`).

use super::*;

impl Checker {
    /// DEC-208 slice F — the SQL-injection lint. Fires `W-SQL-INJECTION` when a `Core.DatabaseModule` `Database.prepare`
    /// receives a string-INTERPOLATED literal whose hole splices a NON-constant value into the SQL text.
    ///
    /// Type-directed and import-gated ("nothing in the wind"): the receiver must type to the `Database` class
    /// AND the program must import `Core.DatabaseModule` (module or member form), so a user class happening to be
    /// named `Database` with a `prepare` method is never hijacked. A fully-constant interpolation (every hole a
    /// literal) does NOT warn; a plain non-interpolated literal has no hole so never warns. This is a
    /// non-fatal lint — the program still compiles (the deliberately-built-query escape hatch stays open).
    pub(in crate::checker) fn lint_sql_injection(
        &mut self,
        base: &Ty,
        name: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) {
        if name != "prepare" {
            return;
        }
        // Receiver must be the `Database` class …
        if !matches!(base, Ty::Named(cls, _) if cls == "Database") {
            return;
        }
        // … and it must be Core.DatabaseModule's `Database` (imported — module `Core.DatabaseModule` or a member `Core.DatabaseModule.X`), never a
        // coincidental user class named `Database`.
        if !self
            .imports
            .values()
            .any(|m| m == "Core.DatabaseModule" || m.starts_with("Core.DatabaseModule."))
        {
            return;
        }
        // The SQL argument must be a string LITERAL with at least one NON-constant interpolation hole.
        let crate::ast::Expr::Str(parts, _) = (match args.first() {
            Some(a) => a,
            None => return,
        }) else {
            return;
        };
        let has_nonconst_hole = parts.iter().any(|p| match p {
            crate::ast::StrPart::Literal(_) => false,
            crate::ast::StrPart::Expr(inner) => !expr_is_const_sql(inner),
        });
        if !has_nonconst_hole {
            return;
        }
        self.warn_coded(
            span,
            "interpolating a value into SQL risks injection — use a `?` placeholder and `.bind(...)`",
            "W-SQL-INJECTION",
            Some(
                "replace the interpolated `{…}` with a `?` placeholder and pass the value to `.bind(...)` \
                 (or a `:name` placeholder + `.bindNamed(...)`) — the value is then sent separately from the \
                 SQL text and can never be parsed as SQL"
                    .to_string(),
            ),
        );
    }
}

/// True iff `e` is a compile-time constant for the SQL-injection lint (DEC-208 slice F): a literal
/// scalar, or a string literal whose every interpolation hole is itself constant (recursively). Any
/// other form — a variable, field access, call, index, arithmetic, cast, … — is NON-constant: it may
/// carry user data, so splicing it into SQL text is the injection risk the lint flags. Conservative by
/// design (a named/class `const` interpolated into SQL still warns — steering it to a bind is harmless
/// and keeps the rule simple); the escape hatch is that it is only a warning, never an error.
fn expr_is_const_sql(e: &crate::ast::Expr) -> bool {
    use crate::ast::{Expr, StrPart};
    match e {
        Expr::Int(..)
        | Expr::Float(..)
        | Expr::Bool(..)
        | Expr::Null(..)
        | Expr::Bytes(..)
        | Expr::Decimal { .. } => true,
        Expr::Str(parts, _) => parts.iter().all(|p| match p {
            StrPart::Literal(_) => true,
            StrPart::Expr(inner) => expr_is_const_sql(inner),
        }),
        _ => false,
    }
}
