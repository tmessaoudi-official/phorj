//! M-Lift L3 — a Phorj AST → `.phg` source **pretty-printer**, the inverse of what the transpiler
//! does for PHP. Scoped to the **subset the L4 lifter emits** (functions/classes/enums + the Tier-1
//! statement and expression set); any node outside that subset returns a clear `Err` rather than
//! guessing at syntax. (Growing this into a full `phg format` is a later, independent expansion.)
//!
//! Correctness discipline: strings are escaped (incl. `{`/`}` → `\{`/`\}`, since a bare `{` opens a
//! Phorj interpolation) and binary/unary expressions are parenthesized **only where precedence or
//! associativity requires it** (C-5/6) — `~a`, `a + b * c`, `(a + b) * c` — mirroring the Phorj
//! parser's binding-power table so the printed text re-parses to the *same* AST. The round-trip
//! tests assert that fixed point directly.

use crate::ast::{
    BinaryOp, ClassDecl, ClassMember, CtorParam, EnumDecl, Expr, FunctionDecl, Item, Modifier,
    Param, Pattern, Program, Stmt, StrPart, Type, UnaryOp,
};

mod exprs;
mod items;
use self::exprs::*;
use self::items::*;
pub use items::print_program;
