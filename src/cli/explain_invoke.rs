//! `phg explain` sub-catalog: the DEC-331 D9 `#[Invoke]` / `#[ToString]` codes (split from
//! `explain.rs` per Invariant 13 — consulted from its fallback arm, same contract as `explain_config`).

/// The `explain_text` fallback chain over the M-Decomp sub-catalogs (Invariant 13): first `Some`
/// wins. Called from `explain.rs`'s `_` arm so each catalog owns its own codes.
pub(super) fn sub_catalog(code: &str) -> Option<&'static str> {
    text(code).or_else(|| super::explain_config::text(code))
}

/// The explanation text for a DEC-331 D9 code, or `None` when `code` is not ours.
pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-ATTRIBUTE-TARGET" => {
            "E-ATTRIBUTE-TARGET — `#[Invoke]` or `#[ToString]` on an illegal target (DEC-331 D9).\n\n\
             Both mark an INSTANCE method of a class: `#[Invoke]` makes the instance callable\n\
             (`x(args)`), `#[ToString]` gives the class its one stringification. A free function or a\n\
             `static` method has no receiver to make callable/stringify, and a constructor already has\n\
             a role — so those are rejected. Move the attribute onto a non-static method of the class.\n"
        }
        "E-INVOKE-DUPLICATE" => {
            "E-INVOKE-DUPLICATE — two `#[Invoke]` methods share a parameter signature (DEC-331 D9a).\n\n\
             A call `x(args)` resolves to a `#[Invoke]` method by arity and parameter types, so two\n\
             `#[Invoke]` methods with the SAME parameter signature would be ambiguous. Give them\n\
             distinct signatures (different arities or parameter types); a class may have any number of\n\
             `#[Invoke]` methods as long as each is distinguishable at the call site.\n"
        }
        "E-TOSTRING-DUPLICATE" => {
            "E-TOSTRING-DUPLICATE — a class declares more than one `#[ToString]` method (DEC-331 D9b).\n\n\
             A class has a single stringification, so exactly one method may carry `#[ToString]`. Keep\n\
             one; an overriding subclass method inherits the role rather than adding a second.\n"
        }
        "E-TOSTRING-SIGNATURE" => {
            "E-TOSTRING-SIGNATURE — a `#[ToString]` method has the wrong signature (DEC-331 D9b).\n\n\
             The stringify method must take NO parameters and return `string` — it is called with no\n\
             arguments whenever the object appears in string context. Declare it\n\
             `#[ToString] function toString(): string { … }`.\n"
        }
        "E-NO-TOSTRING" => {
            "E-NO-TOSTRING — an object reached string context without a `#[ToString]` method (DEC-331 D9b).\n\n\
             Interpolating an object (`\"{obj}\"`) or passing it to `Conversion.toString` stringifies it\n\
             through its `#[ToString]` method — stricter than PHP's runtime warning, the error is at\n\
             compile time. Give the class `#[ToString] function toString(): string { … }`, or\n\
             interpolate a primitive/string instead. (Primitives auto-stringify with no attribute.)\n"
        }
        "E-INVOKE-DEFAULTS" => {
            "E-INVOKE-DEFAULTS — an `#[Invoke]` method has a default or variadic parameter (DEC-331 D9a).\n\n\
             A call `x(args)` resolves the `#[Invoke]` set by exact arity and parameter types, so a\n\
             default/variadic param (which blurs arity) is rejected in slice 1 — it could otherwise let\n\
             `x(5)` pick a different method than `x.method(5)`. Give the method a fixed parameter list,\n\
             or call it by name (`x.method(…)`) to use its defaults. (Honoring defaults through the\n\
             `x(…)` sugar is a later slice.)\n"
        }
        "E-NOT-CALLABLE" => {
            "E-NOT-CALLABLE — a value was called like a function but is not callable (DEC-331 D9a).\n\n\
             Only a function value (a lambda / function reference) or a class instance whose class has\n\
             an `#[Invoke]` method may be called as `x(args)`. Add an `#[Invoke]` method to make the\n\
             class's instances callable, or call a named method (`x.method(args)`) instead.\n"
        }
        _ => return None,
    })
}
