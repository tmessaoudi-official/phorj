//! `check_resolutions` — the checked-program + rewrite-decision entry for the backend
//! pipeline (split from `mod.rs` per Invariant 13). Same contract; see `check`/`check_and_expand`.
use super::*;

/// Like [`check`], but on success also returns the `html"…"` desugarings keyed by literal
/// `Span.start` — fed to [`resolve_html`] so the backend-facing program is `Expr::Html`-free. Used
/// by the interp/VM/transpile pipeline ([`crate::cli::check_and_expand`]); plain [`check`] (e.g.
/// `phg check`) ignores the map since it never reaches a backend.
/// The checker's full resolution output — twelve side-tables the front end threads into the
/// rewriters and the VM compiler. Named so the two entry points below can share one signature
/// instead of repeating it (and drifting apart).
pub type ResolutionsOut = Result<
    (
        Vec<Diagnostic>,
        HashMap<usize, crate::ast::Expr>,
        HashMap<usize, crate::ast::Expr>,
        HashMap<usize, String>,
        HashMap<usize, Ty>,
        HashMap<usize, Ty>,
        HashMap<usize, crate::ast::Expr>,
        std::collections::HashSet<usize>,
        HashMap<usize, (Option<Ty>, Option<Ty>)>,
        HashMap<usize, Vec<Ty>>,
        HashMap<usize, String>,
        // DEC-331 D9: (invoke call targets, tostring string-context targets) for `resolve_invoke_tostring`.
        (HashMap<usize, String>, HashMap<usize, String>),
    ),
    Vec<Diagnostic>,
>;

pub fn check_resolutions(program: &Program) -> ResolutionsOut {
    check_resolutions_mode(program, false)
}

/// Like [`check_resolutions`], but in **test mode**: `test "name" { … }` items are accepted and their
/// bodies type-checked.
///
/// This exists so `phg test` can run the SAME front end as `phg check` and the LSP (DEC-252's
/// chokepoint) instead of calling the raw checker directly. It used to call `checker::check_tests`,
/// which sees no prelude injection, so every injected-prelude symbol surfaced as the user's own
/// `E-UNKNOWN-IDENT` — `phg check` accepted programs `phg test` rejected (KNOWN_ISSUES
/// §TEST-RAW-CHECKER). The only difference between the two entries is this flag; routing test mode
/// through a separate pipeline would recreate the very divergence being closed.
pub fn check_resolutions_mode(program: &Program, test_mode: bool) -> ResolutionsOut {
    let c = crate::checker::run_checker_mode(program, test_mode);
    if c.errors.is_empty() {
        // Merge the Reflect `typeName` substitutions into the call-rewrite map applied by
        // `rewrite_ufcs`. Keys are disjoint (a `typeName` site is a native member call, never UFCS);
        // ONE combined pass makes the two sugars compose when nested either way — the single walker
        // re-resolves embedded original subtrees regardless of nesting direction.
        let mut calls = c.ufcs_resolutions;
        calls.extend(c.reflect_resolutions);
        // M4/DEC-249 default-parameter fills return SEPARATELY (never merged into the rewrite_ufcs
        // map): a fill is a CHECK-TIME clone spliced back FIRST — before resolve_html/unwrap_new
        // erase nodes — or a lambda argument's already-erased nodes restore stale (the
        // db.transaction(fn) regression). `apply_default_fills` runs ahead of every other rewrite.
        // M4 as-matrix: primitive-cast → native-conversion-call substitutions, keyed by the `Cast`
        // node's span (the `as` token — disjoint from every call/UFCS/fill/reflect span). Applied by
        // the same `rewrite_ufcs` walker (its `Cast` arm now consults this map).
        calls.extend(c.cast_resolutions);
        // M-RT Slice C1: resolved overload-selector call-site rewrites join the same call-rewrite map
        // (keys are the `OverloadSelect` node spans — disjoint from every call/UFCS/fill/reflect/cast
        // span). The definition renames are returned separately (they rename items, not call sites).
        calls.extend(c.overload_resolutions);
        Ok((
            c.warnings,
            c.html_resolutions,
            calls,
            c.overload_def_renames,
            c.reified_operands,
            // DEC-239: contextual pipe-lambda param resolutions, materialized into the AST by
            // `materialize_pipe_params` (LAST in the pipeline's rewrite chain).
            c.pipe_param_resolutions,
            c.default_fills,
            // DEC-257: foreach-over-Iterator spans, lowered to while-pulls by `lower_foreach_iter`.
            c.for_iter_lowerings,
            // DEC-280: inferred foreach-binding types, written into the AST by `materialize_for_binds`.
            c.for_bind_resolutions,
            // DEC-288: inferred tuple-destructure binder types, written by `materialize_tuple_binds`.
            c.tuple_bind_resolutions,
            c.variant_resolutions, // DEC-329.3 (see field doc)
            // DEC-331 D9: the two live-node rewrite decision maps, applied by `resolve_invoke_tostring`.
            (c.invoke_call_targets, c.to_string_targets),
        ))
    } else {
        Err(c.errors)
    }
}
