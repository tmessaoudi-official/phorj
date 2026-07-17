//! CLI pipeline helpers, kept in the library so they are unit-testable without
//! spawning the binary. `main.rs` is a thin dispatcher over these. Each command
//! is `fn(&str) -> Result<String, String>`: `Ok` is text to print verbatim
//! (newline-terminated where appropriate), `Err` is a rendered error message.

use crate::ast::Program;
use crate::chunk::{BytecodeProgram, Chunk, Op};
use crate::compiler::compile_with;
use crate::interpreter::{interpret, interpret_main};
use crate::parser::Parser;
use crate::tokenizer::lex;
use crate::vm::Vm;

// Self-contained command groups (M-Decomp W1.2): the `explain` diagnostic-code table and the
// `bench` profiling suite. Re-exported so callers keep referring to `cli::cmd_explain` etc.
mod benchmark;
mod debug_repl;
mod explain;
mod format_cmd;
mod rewrite_new;
mod test_runner;
pub use benchmark::{
    cmd_benchmark, cmd_benchmark_json, cmd_benchmark_vs_php, cmd_benchmark_vs_php_json,
};
pub use debug_repl::run_repl;
pub use explain::{cmd_explain, explain_text};
pub use format_cmd::{cmd_format, format_source};
pub use rewrite_new::cmd_rewrite_new;
pub use test_runner::cmd_test;

mod help;
mod pipeline;
pub(crate) mod preludes;

pub use self::help::*;
pub use self::pipeline::*;
pub(crate) use self::preludes::*;
// Public seam for the test harnesses: which feature-gated Core modules are absent in this build
// (differential/example sweeps skip their examples loudly instead of failing E-EXTENSION-DISABLED).
pub use self::preludes::unavailable_gated_modules;

#[cfg(test)]
mod tests;
