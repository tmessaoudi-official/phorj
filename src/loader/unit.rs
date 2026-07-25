//! Impl blocks for the loader's data types (`Unit`, `LoadStats`). Split out of `loader/mod.rs`
//! (M-Decomp) to keep the root under the file-size cap; the struct DEFINITIONS stay in the root and
//! are reached here via `use super::*`. The methods keep their original visibility (`pub` inherent
//! methods resolve through the type regardless of which module the `impl` lives in).

use super::*;

impl Unit {
    /// Attribute each runtime trace frame to its origin file via [`Unit::fn_files`] (no-op in loose
    /// mode / for backend-synthesized method frames). Returns the source text to render the fault
    /// caret against — the innermost frame's file source in project mode, else `diag_src`.
    #[must_use]
    pub fn attribute_frames(&self, diag: &mut Diagnostic) -> String {
        for f in &mut diag.frames {
            if f.file.is_none() {
                f.file = self.fn_files.get(&f.function).cloned();
            }
        }
        diag.frames
            .first()
            .and_then(|f| f.file.as_ref())
            .and_then(|p| self.sources.get(p))
            .cloned()
            .unwrap_or_else(|| self.diag_src.clone())
    }
}

impl LoadStats {
    /// A one-line human summary for `phg check`'s success message.
    pub fn summary(&self) -> String {
        format!(
            "OK — whole project type-checks clean: {} file{}, {} package{}, {} definition{} \
             validated (every file + vendored deps)\n",
            self.files,
            plural(self.files),
            self.packages,
            plural(self.packages),
            self.defs,
            plural(self.defs),
        )
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
