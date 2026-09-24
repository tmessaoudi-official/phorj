//! DEC-532 (scout row 5l): the checked-exception rules for a thrown value, shared by the `throw e;`
//! STATEMENT and the `throw e` EXPRESSION — the value must implement `Error` (`E-THROW-TYPE`) and be
//! discharged in context: caught by an enclosing `try` or declared in the enclosing `throws`
//! (`E-THROW-UNDECLARED`, or `E-UNCAUGHT-THROW` inside `main`). A throw-expression is typed `never`,
//! the bottom: the `??` / `if` / `match` joins let the non-throwing side decide the result type.

use super::*;

impl Checker {
    pub(in crate::checker) fn check_thrown(&mut self, value: &crate::ast::Expr, span: Span) {
        let e = self.check_expr(value);
        if matches!(e, Ty::Error) {
            // poison — an earlier error already reported
        } else if !self.is_error_type(&e) {
            self.err_coded(
                span,
                format!("can only `throw` a value whose type implements `Error`, found `{e}`"),
                "E-THROW-TYPE",
                Some("define the thrown type as `class Foo implements Error { … }`".into()),
            );
        } else if !self.covered_by_try(&e) && !self.throws_declared(&e) {
            if self.cur_is_main {
                self.err_coded(
                    span,
                    format!("`{e}` thrown in `main` escapes the program entry point"),
                    "E-UNCAUGHT-THROW",
                    Some("wrap it in `try { … } catch (… e) { … }` — `main` may not let an exception escape".into()),
                );
            } else {
                self.err_coded(
                    span,
                    format!("`{e}` is thrown here but neither caught nor declared"),
                    "E-THROW-UNDECLARED",
                    Some(format!(
                        "add `throws {e}` to the enclosing function, or wrap this in `try`/`catch`"
                    )),
                );
            }
        }
    }

    pub(in crate::checker) fn check_throw_expr(
        &mut self,
        value: &crate::ast::Expr,
        span: Span,
    ) -> Ty {
        self.check_thrown(value, span);
        Ty::Never
    }
}
