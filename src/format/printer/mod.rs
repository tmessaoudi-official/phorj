//! `phg format` — a comment-preserving, **full-surface** Phorj AST → `.phg` source printer. Unlike the
//! Tier-1-subset lift printer (`src/lift/printer.rs`), this one covers the *entire* language so it can
//! format any parseable program. Its matches are exhaustive — the Rust compiler proves completeness,
//! so it can never silently mis-handle a node — and the only `Err` arms are for AST shapes that a
//! *parsed* program can never contain (e.g. `Type::Erased`, which is produced only by a post-check
//! pass `phg format` never runs).
//!
//! Correctness discipline (the formatter's one hard rule — meaning preservation): strings are escaped
//! (incl. `{`/`}` → `\{`/`\}`, since a bare `{` opens an interpolation); binary/unary expressions are
//! parenthesized **only where precedence/associativity requires it** mirroring the parser's
//! binding-power table; and every meaning-carrying field (class generics / `use` traits / resolution
//! clauses, function `throws`, …) is printed. The invariants `parse(fmt(x)) ≡ parse(x)` and
//! `fmt(fmt(x)) == fmt(x)` are asserted by the round-trip tests.
//!
//! Comments (which the token stream discards) are carried in via the tokenizer's `lex_with_comments`
//! side-channel (F1) and interleaved by source span (F2b).

use super::doc::{self, Doc};
use crate::ast::{
    BinaryOp, CatchClause, ClassDecl, ClassMember, CtorParam, DestructureField, DestructurePat,
    EnumDecl, Expr, FieldPat, FunctionDecl, Item, LambdaBody, Modifier, Param, Pattern, Program,
    Resolution, Stmt, StrPart, Type, UnaryOp,
};
use crate::token::Comment;

/// Format a whole program (already parsed) to canonical `.phg` source, interleaving `comments`
/// (from [`crate::tokenizer::lex_with_comments`]) by source position. `Err` only for an AST a parsed
/// program cannot contain (see the module docs).
pub fn format_program(p: &Program, comments: &[Comment]) -> Result<String, String> {
    let mut pr = Printer {
        out: String::new(),
        indent: 0,
        comments,
        next_comment: 0,
    };
    pr.program(p)?;
    pr.flush_remaining_comments();
    Ok(pr.out)
}

struct Printer<'a> {
    out: String,
    indent: usize,
    /// Captured comments in source order (F1 side-channel); `next_comment` is the cursor.
    comments: &'a [Comment],
    next_comment: usize,
}

impl Printer<'_> {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// F2b: flush every own-line comment whose source position precedes byte offset `before`, each on
    /// its own indented line, ahead of the node about to be printed. (A trailing comment on the same
    /// line as preceding code is handled separately.) Comment text is emitted verbatim (no reflow).
    /// Whether an own-line comment is still pending before source offset `before` (used to keep a
    /// commented import from being tightly grouped with the previous import).
    fn has_comment_before(&self, before: usize) -> bool {
        self.next_comment < self.comments.len()
            && self.comments[self.next_comment].span.start < before
    }

    fn flush_comments_before(&mut self, before: usize) {
        while self.next_comment < self.comments.len()
            && self.comments[self.next_comment].span.start < before
        {
            let c = self.comments[self.next_comment].clone();
            self.next_comment += 1;
            for cl in c.text.lines() {
                self.line(cl);
            }
        }
    }

    /// Emit any comments that appear after the last printed node (trailing block/footer comments).
    fn flush_remaining_comments(&mut self) {
        let n = self.comments.len();
        while self.next_comment < n {
            let c = self.comments[self.next_comment].clone();
            self.next_comment += 1;
            for cl in c.text.lines() {
                self.line(cl);
            }
        }
    }
}

mod atoms;
mod exprs;
mod items;
mod stmts;

use self::atoms::*;
