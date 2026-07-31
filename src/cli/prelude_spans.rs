//! Offset REBASING for injected `Core.*` preludes. Split out of `pipeline.rs` (Invariant 13); the
//! reason it exists is documented on [`lex_parse_injected`] and is worth reading before touching any
//! prelude source.

use super::*;

/// The first `Span.start` handed to an INJECTED prelude — far above any real file size, so a prelude
/// offset can never equal a user-file offset. See [`lex_parse_injected`].
///
/// **256 MiB, not 4 GiB, and that is not arbitrary.** This was `1 << 32`, which is a compile ERROR on a
/// 32-bit target: `usize` is 32 bits on `wasm32-unknown-unknown`, so the shift overflows during
/// const-eval and the WASM playground build fails outright (`error[E0080]: attempt to shift left by
/// 32_i32, which would overflow`). The local quality gate never caught it because it only builds for the
/// 64-bit host — the playground workflow was the sole wasm32 compile, so the break sat red for six
/// consecutive runs. `scripts/wasm-check.sh` now compiles for wasm32 locally so this class cannot hide
/// again.
///
/// 256 MiB is still absurdly beyond any `.phg` source: the whole example corpus is a few hundred KiB,
/// and a single source file approaching this would be pathological long before offsets mattered.
pub(crate) const INJECTED_SPAN_BASE: usize = 1 << 28;

/// The offset room reserved per injected module. Every shipped prelude is well under a megabyte, so
/// 16 MiB of headroom keeps each module's range disjoint from every other's.
pub(super) const INJECTED_SPAN_STRIDE: usize = 1 << 24;

/// Headroom the range must accommodate, in prelude FRAGMENTS. The shipped registry has **22** (21 rows
/// with sources, one carrying two), so 128 is ~6x room — generous without pretending to a figure the
/// 32-bit budget cannot fund.
const INJECTED_SPAN_FRAGMENT_HEADROOM: usize = 128;

/// The rebasing scheme must fit a 32-bit `usize`, asserted AT COMPILE TIME on the actual target — the
/// only way this stays honest, since the overflow is invisible on a 64-bit host and cost six red
/// playground runs before anyone compiled for wasm32.
///
/// `checked_mul` FIRST: the product itself overflows before any add, which is how the first draft of
/// this very assertion failed (`attempt to compute 256_usize * 16777216_usize, which would overflow`).
const _: () = assert!(
    match INJECTED_SPAN_STRIDE.checked_mul(INJECTED_SPAN_FRAGMENT_HEADROOM) {
        Some(room) => INJECTED_SPAN_BASE.checked_add(room).is_some(),
        None => false,
    },
    "the injected-span range overflows `usize` on this target — lower INJECTED_SPAN_BASE/STRIDE"
);

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
