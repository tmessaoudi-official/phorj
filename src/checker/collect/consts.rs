//! Collection pass — a class `const` member's own checks (Feature A; DEC-533 widens the initializer).

use super::*;

impl Checker {
    /// A `const` class constant: compile-time, immutable, class-level, accessed only `ClassName.NAME`.
    /// It needs a literal-const initializer and must not be `mutable`. Disjoint from instance fields
    /// and statics. The caller records the `ConstEntry`.
    pub(super) fn check_const_member(
        &mut self,
        name: &str,
        modifiers: &[crate::ast::Modifier],
        init: Option<&crate::ast::Expr>,
        span: Span,
        fty: &Ty,
    ) {
        use crate::ast::Modifier;
        if modifiers.contains(&Modifier::Mutable) {
            self.err_coded(
                span,
                format!("`const {name}` cannot be `mutable` — a constant is immutable"),
                "E-CONST-MUTABLE",
                Some(
                    "drop `mutable`, or use a `static mutable` field for class-level state".into(),
                ),
            );
        }
        let Some(e) = init else {
            self.err_coded(
                span,
                format!("`const {name}` needs an initializer"),
                "E-CONST-NO-INIT",
                Some("e.g. `const int MAX = 100;`".into()),
            );
            return;
        };
        if crate::value::const_literal(e).is_none() {
            self.err_coded(
                Self::expr_span(e),
                format!("`const {name}` initializer must be a literal constant"),
                "E-CONST-NOT-LITERAL",
                Some("use an int/float/bool/string/null literal".into()),
            );
            return;
        }
        let ity = self.check_expr(e);
        if !self.ty_assignable(&ity, fty) {
            self.err_coded(
                Self::expr_span(e),
                format!("`const {name}: {fty}` initialized with `{ity}`"),
                "E-CONST-INIT-TYPE",
                None,
            );
        }
    }
}
