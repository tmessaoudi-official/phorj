//! `phg explain`: the diagnostic-code explanation catalog. Per-family code tables live in
//! sibling submodules (M-Decomp, Invariant 13); this module chains them and renders the command.

mod attrs_faults;
mod db_generics;
mod declfile_enums;
mod imports_casts;
mod match_overloads;
mod members_destructure;
mod names_types;
mod serve_tls;
mod transpile_di;
mod types_traits;

/// The prose explanation for a diagnostic `code`, or `None` if the code is unknown. The codes are
/// the stable identifiers carried by [`crate::diagnostic::Diagnostic::code`] and shown in `[…]`
/// beneath a rendered error.
pub fn explain_text(code: &str) -> Option<String> {
    names_types::text(code)
        .or_else(|| imports_casts::text(code))
        .or_else(|| types_traits::text(code))
        .or_else(|| match_overloads::text(code))
        .or_else(|| attrs_faults::text(code))
        .or_else(|| members_destructure::text(code))
        .or_else(|| transpile_di::text(code))
        .or_else(|| db_generics::text(code))
        .or_else(|| declfile_enums::text(code))
        .or_else(|| serve_tls::text(code))
        .or_else(|| super::explain_invoke::sub_catalog(code))
        .map(str::to_string)
}

/// `explain <code>`: print the explanation for a diagnostic code, or error on an unknown one.
pub fn cmd_explain(code: &str) -> Result<String, String> {
    explain_text(code).ok_or_else(|| {
        // Every code Phorj emits carries a `[CODE]` in its rendered diagnostic — pass that code here.
        // (Historically this listed all known codes inline; that list drifted, so it was removed in
        // favor of the `every_emitted_diagnostic_code_has_an_explanation` coverage ratchet, which
        // guarantees every emitted code is explainable.)
        format!(
            "unknown diagnostic code `{code}` — pass a code exactly as it appears in a `[…]` diagnostic \
             (e.g. `phg explain E-UNKNOWN-IDENT`)"
        )
    })
}
