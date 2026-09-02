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
pub(crate) use crate::token::INJECTED_SPAN_BASE;

/// The offset room reserved per injected module. Every shipped prelude is well under a megabyte, so
/// 16 MiB of headroom keeps each module's range disjoint from every other's.
pub(super) const INJECTED_SPAN_STRIDE: usize = 1 << 24;

/// Appended to every prelude-declared `Core.Native.*` alias at injection (DEC-459). `#` is not an
/// identifier character, so no user token can ever spell the isolated name.
pub(crate) const PRELUDE_ALIAS_SUFFIX: &str = "#prelude";

/// Every alias the Core preludes bind a raw `Core.Native.*` module under (`import Core.Native.Http
/// as NativeHttp;` → `NativeHttp`), scanned once over all `CORE_MODULES` fragment sources.
pub(crate) fn prelude_native_aliases() -> &'static std::collections::HashSet<String> {
    use crate::token::TokenKind;
    static SET: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        let mut out = std::collections::HashSet::new();
        for m in super::preludes::CORE_MODULES {
            for src in m.srcs {
                let Ok(tokens) = lex(src) else {
                    continue; // unreachable: registry preludes are valid
                };
                let mut i = 0;
                while i < tokens.len() {
                    if matches!(tokens[i].kind, TokenKind::Import) {
                        // `import Core . Native . X as A ;` — collect the path idents up to `as`.
                        let mut path: Vec<&str> = Vec::new();
                        let mut j = i + 1;
                        while j < tokens.len() {
                            match &tokens[j].kind {
                                TokenKind::Ident(s) if s == "as" => break,
                                TokenKind::Ident(s) => path.push(s.as_str()),
                                TokenKind::Semicolon => break,
                                _ => {}
                            }
                            j += 1;
                        }
                        let is_as = matches!(tokens.get(j).map(|t| &t.kind), Some(TokenKind::Ident(s)) if s == "as");
                        if is_as && path.len() >= 3 && path[0] == "Core" && path[1] == "Native" {
                            if let Some(TokenKind::Ident(a)) = tokens.get(j + 1).map(|t| &t.kind) {
                                out.insert(a.clone());
                            }
                        }
                        i = j;
                    }
                    i += 1;
                }
            }
        }
        out
    })
}

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
    let isolated = prelude_native_aliases();
    for t in &mut tokens {
        t.span.start += base;
        // DEC-459: a prelude's raw-native qualifier (`NativeHttp`, `NativeInput`, …) is rebound under
        // a spelling no user identifier can take, so a user alias can neither collide with it (the
        // injection used to drop the prelude's import on a same-module user import) nor capture it,
        // and the name is not "in the wind" for user code (panel F6). The set is computed over EVERY
        // prelude source because a fragment may use an alias a sibling fragment declares (the serve
        // fragment calls `NativeHttp.registerServe` through the request fragment's import).
        if let crate::token::TokenKind::Ident(name) = &mut t.kind {
            if isolated.contains(name.as_str()) {
                name.push_str(PRELUDE_ALIAS_SUFFIX);
            }
        }
    }
    Parser::new(tokens)
        .parse_program()
        .map_err(|e| e.render(src))
}
