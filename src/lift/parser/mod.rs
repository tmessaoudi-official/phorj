//! M-Lift L2 — a recursive-descent + precedence-climbing parser for the **Tier-1 PHP** subset,
//! turning the [`super::lexer`] token stream into a [`super::ast::PhpProgram`].
//!
//! Mirrors the house parser style (`src/parser/`): cursor helpers, precedence climbing via
//! [`infix_op`], a `depth` guard against [`MAX_NEST_DEPTH`] (the input is untrusted PHP). Errors are
//! line-numbered `lift parse error:` strings, like the lexer — and anything outside Tier-1 is
//! rejected *loudly* rather than represented and guessed at (the never-guess contract).
//!
//! Precedence follows **PHP 8**: concatenation `.` binds *looser* than `+`/`-` but *tighter* than the
//! comparison operators — a real 8.0 change, pinned by tests.

use super::ast::{
    PhpArrayElem, PhpBinOp, PhpClass, PhpEnum, PhpEnumCase, PhpExpr, PhpFunction, PhpItem,
    PhpMatchArm, PhpMember, PhpMethod, PhpParam, PhpProgram, PhpStmt, PhpStrPart, PhpType, PhpUnOp,
    PhpVisibility,
};
use super::lexer::{lex_php, PTok, PTokenSpanned};
use crate::limits::MAX_NEST_DEPTH;

/// Keywords that exist in PHP but are outside the Tier-1 subset. Encountered in statement-leading
/// position they produce a clear "not supported" error rather than being misread as an expression.
const UNSUPPORTED_KW: &[&str] = &[
    "try",
    "catch",
    "finally",
    "switch",
    "throw",
    "do",
    "namespace",
    "use",
    "trait",
    "interface",
    "global",
    "goto",
    "declare",
    "const",
    "static",
    "function", // a *nested* function is a closure-ish construct; top-level fns are caught earlier
    "fn",
];

/// PHP cast type names (`(int)$x`). Detected to reject casts loudly (Tier-2) instead of misparsing.
const CAST_TYPES: &[&str] = &[
    "int", "integer", "float", "double", "string", "bool", "boolean", "array", "object",
];

struct PParser {
    toks: Vec<PTokenSpanned>,
    pos: usize,
    /// Live expression-nesting depth, checked in [`PParser::parse_unary`] (every operand passes
    /// through it once per level) to bound recursion on pathologically nested input.
    depth: usize,
}

/// Parse a Tier-1 PHP token stream into a [`PhpProgram`]. The stream must end in [`PTok::Eof`]
/// (the lexer guarantees this).
pub fn parse_php(toks: Vec<PTokenSpanned>) -> Result<PhpProgram, String> {
    let mut p = PParser {
        toks,
        pos: 0,
        depth: 0,
    };
    p.parse_program()
}

mod exprs;
mod items;
mod stmts;
