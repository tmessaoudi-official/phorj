//! Per-file `Span.start` windows for a multi-file project — the user-file axis of the span collision
//! (KNOWN_ISSUES §default_fills, fixed 2026-09-02). Split out of `fs.rs` (Invariant 13). Read the doc
//! on [`SpanWindows`] before adding any parse path that feeds the checker.

use super::*;

/// Disjoint `Span.start` windows for the files of ONE project — the user-file axis of the span
/// collision (KNOWN_ISSUES §default_fills, P0). Every checker rewrite map (`default_fills`,
/// `ufcs_resolutions`, `for_iter_lowerings`, …) is keyed by `Span.start` alone, and each file is
/// lexed from offset 0, so two files of one project could share a key and one file's rewrite was
/// spliced into the other — on every leg, so the byte-identity harness could not see it. The
/// injected-prelude axis was closed the same way (`crate::cli::prelude_spans`: fragments live at
/// `INJECTED_SPAN_BASE` and above); this closes the remaining one below that base.
///
/// The ENTRY file keeps base 0: single-file runs, `phg format`, the LSP's own-buffer handlers and the
/// `rewrite_new` codemod all index the entry text by offset, and none of them ever sees a non-entry
/// span (the LSP's only loader use is diagnostics, which travel as `line`/`col`). Every other file's
/// window starts after the entry's bytes and after the previous window, so no two files overlap.
/// `line`/`col` are never touched — a diagnostic in a rebased file still names its own position.
pub(super) struct SpanWindows {
    next: usize,
}

impl SpanWindows {
    /// Windows start just past the entry file, which owns `[0, entry_len]`.
    pub(super) fn after_entry(entry_len: usize) -> Self {
        Self {
            next: entry_len + 1,
        }
    }

    /// Reserve `len + 1` bytes for `path` and return its base. A project whose source would reach
    /// the injected-prelude range is refused rather than allowed to collide there.
    pub(super) fn reserve(&mut self, path: &Path, len: usize) -> Result<usize, String> {
        let base = self.next;
        let end = base
            .checked_add(len + 1)
            .filter(|e| *e < crate::cli::INJECTED_SPAN_BASE)
            .ok_or_else(|| {
                format!(
                    "{}: the project's source exceeds the {} MiB span-offset budget \
                     (split it into smaller projects)",
                    path.display(),
                    crate::cli::INJECTED_SPAN_BASE >> 20
                )
            })?;
        self.next = end;
        Ok(base)
    }
}

/// [`parse_at`] with every token's `Span.start` shifted by `base` (see [`SpanWindows`]). Base 0 is
/// exactly `parse_at`, so the entry file's parse is byte-for-byte the single-file one.
pub(super) fn parse_at_rebased(path: &Path, src: &str, base: usize) -> Result<Program, String> {
    if base == 0 {
        return parse_at(path, src);
    }
    let mut tokens = lex(src).map_err(|e| format!("{}: {}", path.display(), e.render(src)))?;
    for t in &mut tokens {
        t.span.start += base;
    }
    Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("{}: {}", path.display(), e.render(src)))
}
