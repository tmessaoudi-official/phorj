//! Offset REBASING for injected `Core.*` preludes. Split out of `pipeline.rs` (Invariant 13); the
//! reason it exists is documented on [`lex_parse_injected`] and is worth reading before touching any
//! prelude source.

use super::*;

/// The first `Span.start` handed to an INJECTED prelude — chosen far above any real file size so a
/// prelude offset can never equal a user-file offset. See [`lex_parse_injected`].
pub(super) const INJECTED_SPAN_BASE: usize = 1 << 32;

/// The offset room reserved per injected module. Every shipped prelude is well under a megabyte, so
/// 16 MiB of headroom keeps each module's range disjoint from every other's.
pub(super) const INJECTED_SPAN_STRIDE: usize = 1 << 24;

/// lex + parse an INJECTED `Core.*` prelude, **rebasing its byte offsets** into a range that cannot
/// collide with the user program's.
///
/// **Why this exists (a verified divergence, not a precaution).** The checker records several
/// post-check rewrites in side tables keyed by `Span.start` alone — `ufcs_resolutions`,
/// `html_resolutions`, the reflect/cast substitutions, `for_bind_resolutions`, `for_iter_lowerings`.
/// Their stated justification is that "each call site's `(` token is at a unique byte offset", which
/// is true *within one source string* — and an injected prelude is a SEPARATE string, whose offsets
/// restart at 0 and therefore overlap the user file's one-for-one. When a prelude call site and a
/// user call site landed on the same offset, the prelude's recorded rewrite was applied to the USER's
/// node: `phg check` stayed clean, the tree-walker (which re-checks nothing) ran correctly, and only
/// the VM compile failed — an Invariant 1 divergence produced by nothing but the byte LENGTH of a
/// prelude line.
///
/// [Verified 2026-07-31: adding one `import` line to the `Core.Database` prelude made
/// `examples/database/transaction-closure.phg` fail on the VM with "`transaction` is not a function,
/// variant, or class" while `check` and `--tree-walker` both passed; adding a single trailing SPACE to
/// that same line made it pass again.]
///
/// Rebasing `start` only — `line`/`col` are untouched, so any diagnostic a prelude does raise still
/// points at the right place in the prelude source.
pub(super) fn lex_parse_injected(src: &str, module_index: usize) -> Result<Program, String> {
    let mut tokens = lex(src).map_err(|e| e.render(src))?;
    let base = INJECTED_SPAN_BASE + module_index * INJECTED_SPAN_STRIDE;
    for t in &mut tokens {
        t.span.start += base;
    }
    Parser::new(tokens)
        .parse_program()
        .map_err(|e| e.render(src))
}
