//! Call checking — the `Regex.compile` / `Regex.compileBacktracking` LITERAL-pattern gate (DEC-461,
//! REGEX-B), split from `calls/core.rs` per Invariant 13 like `format.rs`.
//!
//! A pattern written as a plain string literal is validated at CHECK time with the very engine that
//! would compile it at run time (`ext::regex::engine::validate`), so a PCRE-only construct on the
//! linear engine (`E-REGEX-UNSUPPORTED`) or a syntax error on either (`E-REGEX-INVALID`) is a
//! compile error on every leg — before this the transpile leg emitted `preg_*` for a pattern the Rust
//! engines refused (panel C2/C5: `a++`, `(?=b)`, `\h`, … were `true` under PHP, a fault natively).
//! A dynamic pattern is left to the runtime, where `compile` faults and the PHP twin
//! `__phorj_regex_compile` faults identically.
use super::*;

impl Checker {
    /// Side-effect only (diagnostics); the call is then type-checked by the ordinary native path.
    pub(in crate::checker) fn gate_regex_literal(&mut self, name: &str, args: &[crate::ast::Expr]) {
        use crate::ast::{Expr, StrPart};
        let Some(Expr::Str(parts, span)) = args.first() else {
            return;
        };
        let mut literal = String::new();
        for part in parts {
            match part {
                StrPart::Literal(s) => literal.push_str(s),
                StrPart::Expr(_) => return, // interpolated → dynamic, runtime-validated
            }
        }
        #[cfg(feature = "regex")]
        {
            use crate::ext::regex::engine::{validate, Engine};
            let engine = if name == "compile" {
                Engine::Linear
            } else {
                Engine::Backtracking
            };
            if let Err(e) = validate(&literal, engine) {
                use crate::ext::regex::reject::RejectKind;
                let (code, hint) = match e.kind {
                    RejectKind::LinearOnly => (
                        "E-REGEX-UNSUPPORTED",
                        "the linear engine is ReDoS-immune and omits PCRE's backtracking-only syntax; \
                         `Regex.compileBacktracking(...)` accepts it under a step budget",
                    ),
                    RejectKind::NotPortable => (
                        "E-REGEX-UNSUPPORTED",
                        "no engine makes this byte-identical with PHP — rewrite with an explicit \
                         class, `\\p{…}`, or the portable spelling named in the message",
                    ),
                    RejectKind::Invalid => (
                        "E-REGEX-INVALID",
                        "fix the pattern — it would fault at run time on every backend",
                    ),
                };
                self.err_coded(*span, e.message, code, Some(hint.to_string()));
            }
        }
        #[cfg(not(feature = "regex"))]
        {
            let _ = (name, literal, span);
        }
    }
}
