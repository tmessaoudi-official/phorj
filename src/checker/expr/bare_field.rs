//! `E-BARE-FIELD` — the diagnostic for a bare field name (Phorj has no bare field access), with the
//! trait-body form that names the using class (plan row 4b; moved out of `core.rs`, Invariant 13).

use super::*;

impl Checker {
    /// `E-BARE-FIELD`: Phorj has no bare field access; a field is always `this.x`.
    /// In a trait body whose field belongs to a USING class, `this.{name}` does not check (the
    /// trait declares no such field), so the advice there is a rename (plan row 4b, panel round 1).
    pub(super) fn bare_field_err(&mut self, span: Span, name: &str) -> Ty {
        let owner = self.cur_field_owner(name);
        if let Some(owner) = owner.filter(|o| Some(o) != self.cur_class.as_ref()) {
            // A trait reaches only the fields it declares. With a top-level function of the same
            // name the bare name is ambiguous, so the advice is a rename; without one, declare it.
            let advice = if self.funcs.contains_key(name) {
                format!("rename the function `{name}` or the field")
            } else {
                format!(
                    "pass the value in, or require it with an abstract method (`abstract function {name}(): …;`) \
                     that the using class implements"
                )
            };
            return self.err_coded(
                span,
                format!("bare name `{name}` matches the field `{name}` of `{owner}`, which uses this trait"),
                "E-BARE-FIELD",
                Some(format!(
                    "the trait body runs with `this` = `{owner}`, where the bare name would read that field; \
                     a trait reaches only the fields it declares: {advice}"
                )),
            );
        }
        self.err_coded(
            span,
            format!("bare field reference `{name}` — write `this.{name}`"),
            "E-BARE-FIELD",
            Some(format!(
                "Phorj has no bare field access (like PHP's `$this->`): qualify it as `this.{name}`"
            )),
        )
    }
}
