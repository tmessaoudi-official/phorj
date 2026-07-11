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
pub mod checker;
pub mod chunk;
pub mod cli;
pub mod compiler;
pub mod dap;
pub mod debug;
pub mod diagnostic;
pub mod dispatch;
pub mod dump;
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
pub mod lock;
pub mod lsp;
pub mod manifest;
pub mod mem;
pub mod native;
pub mod parser;
pub mod phstr;
pub mod profile;
pub mod serve;
pub mod token;
pub mod tokenizer;
pub mod transpile;
pub mod types;
pub mod value;
pub mod vendor;
pub mod vm;
