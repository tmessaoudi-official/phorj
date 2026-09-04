// First-party `unsafe` is confined to the JIT island (`src/jit/`, the `jit` feature) — everywhere
// else it is a compile error. `deny` (not `forbid`) is deliberate: `forbid` cannot be locally
// overridden, so it would reject even the audited unsafe-code allow-island the Cranelift JIT requires
// for its `finalize -> transmute -> call` path (perf mandate G-8, dep-policy domain #7, 2026-07-06
// amendment). `deny` keeps the invariant crate-wide while permitting that one scoped override; the CI
// `unsafe-island` gate fails the build if an unsafe-code allow-attribute appears anywhere outside
// `src/jit/`, so "first-party unsafe lives only in the JIT" is machine-enforced, not a convention.
// (Wording avoids the literal attribute token on purpose — that grep would otherwise match this very
// comment.) M2 P3.5 Wave 0 Task 0.5 locked the original `forbid`; this relaxes it by one audited module.
#![deny(unsafe_code)]

pub mod ast;
pub mod bundle;
// The charset transcoding kernel (DEC-468/DEC-494) — a shared leaf like `phstr`/`json`, read by
// `ext::encoding`'s natives AND by `transpile::charset_php`, which formats its tables straight into
// the emitted PHP helper. Feature-gated with its consumers.
#[cfg(feature = "encoding")]
pub mod charset;
pub mod checker;
pub mod chunk;
pub mod cli;
pub mod compiler;
pub mod dap;
pub mod debug;
pub mod diagnostic;
pub mod dispatch;
pub mod doc_comment;
pub mod dump;
pub mod ext;
// The accent-folding table behind `Core.String.foldAccents` (DEC-468) — a shared leaf like
// `charset`, read by the native AND by `transpile::fold_php`, which formats it into the emitted
// `__phorj_fold_accents` helper so the two legs cannot drift.
pub mod fold_accents;
pub mod format;
pub mod green;
pub mod inspect;
pub mod interpreter;
#[cfg(feature = "jit")]
pub mod jit;
pub mod json;
pub mod lift;
pub mod limits;
pub mod loader;
pub mod lsp;
pub mod mem;
pub mod native;
pub mod parser;
pub mod php_names;
pub mod phstr;
pub mod pm;
pub mod profile;
pub mod serve;
// The process-wide single ctrlc registration (DEC-204/DEC-487) — shared by `serve`'s accept loop
// and `Time.sleep`'s interruptibility, because `ctrlc::set_handler` may only be called once.
pub mod shutdown;
pub mod token;
pub mod tokenizer;
pub mod transpile;
pub mod types;
pub mod value;
pub mod vm;
