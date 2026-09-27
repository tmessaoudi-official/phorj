//! Scout row 4l-a (DEC-540) — PHP's `preg_match` EXISTENCE test lifts onto `Core.Regex`.
//!
//! `1 === preg_match('/…/', $s)` (and `!==`, `==`, `!=`, either operand order, `0` as well as `1`)
//! becomes `Regex.matches(Regex.compile("…"), s)`, negated where the comparison asks. The pattern is
//! read at LIFT time — a literal, or a string constant of the class being lifted (`self::X`) — and
//! translated by [`translate`]; anything whose meaning would change is refused by name, never guessed.
//!
//! PHP returns `false` on a PCRE error, which `=== 1` reads as "no match"; phorj faults instead. phorj
//! strings are valid UTF-8, so only the backtracking step budget can raise it — a louder failure, not
//! a silent one. A captures array (`preg_match($p, $s, $m)`) is row 4l-b2, in [`captures`]; every
//! other `preg_*` shape (`preg_match_all`, `preg_replace`, …) is rows 4l-b3 and 4l-c.

use super::*;
use std::collections::HashMap;

mod captures;
mod scan;
#[cfg(test)]
mod tests;
mod translate;

pub(in crate::lift) use captures::{hoist, reset as reset_captures, restore as restore_captures};
pub(in crate::lift) use captures::{snapshot as snapshot_captures, Snapshot as CapturesScope};
pub(in crate::lift) use translate::translate;

/// Does this expression belong to the `preg_*` lift — an existence test (row 4l-a) or a captures
/// read (row 4l-b2)?
pub(super) fn owns(e: &php::PhpExpr) -> bool {
    match e {
        php::PhpExpr::Binary { op, left, right } if is_match_test(*op, left, right) => true,
        _ => captures::owns(e),
    }
}

/// Lift an expression [`owns`] accepted.
pub(super) fn lift(e: &php::PhpExpr) -> Result<Expr, String> {
    match e {
        php::PhpExpr::Binary { op, left, right } if is_match_test(*op, left, right) => {
            lift_match_test(*op, left, right)
        }
        _ => captures::lift(e),
    }
}

thread_local! {
    /// `(class, constant)` → value, for every string or int class constant of the file being lifted.
    static CONSTS: std::cell::RefCell<HashMap<(String, String), Folded>> =
        std::cell::RefCell::new(HashMap::new());
}

/// A pattern piece known at lift time.
#[derive(Clone)]
enum Folded {
    Str(String),
    Int(i64),
}

/// Begin one file: record its string and int class constants (reset per file, like `map_consts`).
pub(in crate::lift) fn begin_file(prog: &php::PhpProgram) {
    let mut found = HashMap::new();
    for item in &prog.items {
        let php::PhpItem::Class(c) = item else {
            continue;
        };
        for m in &c.members {
            if let php::PhpMember::Const { name, value, .. } = m {
                let v = match value {
                    php::PhpExpr::Str(v) => Folded::Str(v.clone()),
                    php::PhpExpr::Int(n) => Folded::Int(*n),
                    _ => continue,
                };
                found.insert((c.name.clone(), name.clone()), v);
            }
        }
    }
    CONSTS.with(|m| *m.borrow_mut() = found);
}

/// The two-argument `preg_match` call and the integer it is compared with, whichever side each is on.
fn split_test<'a>(
    left: &'a php::PhpExpr,
    right: &'a php::PhpExpr,
) -> Option<(&'a php::PhpExpr, &'a php::PhpExpr, i64)> {
    let call = |e: &'a php::PhpExpr| match e {
        php::PhpExpr::Call { callee, args } => match (&**callee, args.as_slice()) {
            (php::PhpExpr::Name(n), [pat, subject])
                if n.trim_start_matches('\\') == "preg_match"
                    && ![pat, subject].iter().any(|a| {
                        matches!(a, php::PhpExpr::Spread(_) | php::PhpExpr::NamedArg { .. })
                    }) =>
            {
                Some((pat, subject))
            }
            _ => None,
        },
        _ => None,
    };
    let int = |e: &php::PhpExpr| match e {
        php::PhpExpr::Int(n @ (0 | 1)) => Some(*n),
        _ => None,
    };
    match (call(left), int(right), call(right), int(left)) {
        (Some((p, s)), Some(n), _, _) | (_, _, Some((p, s)), Some(n)) => Some((p, s, n)),
        _ => None,
    }
}

/// Is `left <op> right` an existence test this row lifts?
fn is_match_test(op: php::PhpBinOp, left: &php::PhpExpr, right: &php::PhpExpr) -> bool {
    use php::PhpBinOp::{Eq, Identical, NotEq, NotIdentical};
    matches!(op, Identical | NotIdentical | Eq | NotEq) && split_test(left, right).is_some()
}

/// Lift the existence test [`is_match_test`] accepted.
fn lift_match_test(
    op: php::PhpBinOp,
    left: &php::PhpExpr,
    right: &php::PhpExpr,
) -> Result<Expr, String> {
    let (pat, subject, n) = split_test(left, right).expect("guarded by is_match_test");
    let equal = matches!(op, php::PhpBinOp::Identical | php::PhpBinOp::Eq);
    let literal = pattern_text(pat)?;
    let t = translate(&literal)?;
    let subject = lift_expr(subject)?;
    record_native_module("Core.Regex");
    let test = regex_call("matches", vec![compile_call(t), subject]);
    // `=== 1` and `!== 0` ask "does it match"; `!== 1` and `=== 0` ask the opposite.
    Ok(if equal == (n == 1) {
        test
    } else {
        Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(test),
            span: SP,
        }
    })
}

/// `Regex.<name>(args)`.
fn regex_call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident("Regex".to_string(), SP)),
            name: name.to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args,
        type_args: Vec::new(),
        span: SP,
    }
}

/// `Regex.compile("…")`, or `Regex.compileBacktracking("…")` when the translation needs it.
fn compile_call(t: translate::Translated) -> Expr {
    let ctor = if t.backtracking {
        "compileBacktracking"
    } else {
        "compile"
    };
    regex_call(ctor, vec![Expr::Str(vec![StrPart::Literal(t.pattern)], SP)])
}

/// The pattern's text, read at lift time — or why it cannot be. A literal, a constant of this class,
/// and `.` / `+` / `-` / `*` over them fold (`'/^[0-9a-f]{' . (2 * self::RAW_BYTES) . '}$/'`): a class
/// constant is fixed, so PHP builds the same string on every run.
fn pattern_text(pat: &php::PhpExpr) -> Result<String, String> {
    match fold(pat)? {
        Folded::Str(s) => Ok(s),
        Folded::Int(_) => Err("lift: `preg_match` pattern is an integer, not a string".into()),
    }
}

fn fold(e: &php::PhpExpr) -> Result<Folded, String> {
    use php::PhpBinOp::{Add, Concat, Mul, Sub};
    match e {
        php::PhpExpr::Str(s) => Ok(Folded::Str(s.clone())),
        php::PhpExpr::Int(n) => Ok(Folded::Int(*n)),
        php::PhpExpr::ClassConst { class, name } => class_const(class, name),
        php::PhpExpr::Binary {
            op: op @ (Concat | Add | Sub | Mul),
            left,
            right,
        } => {
            let (l, r) = (fold(left)?, fold(right)?);
            let text = |f: Folded| match f {
                Folded::Str(s) => s,
                Folded::Int(n) => n.to_string(),
            };
            match (*op, l, r) {
                (Concat, l, r) => Ok(Folded::Str(text(l) + &text(r))),
                (_, Folded::Int(a), Folded::Int(b)) => match op {
                    Add => a.checked_add(b),
                    Sub => a.checked_sub(b),
                    _ => a.checked_mul(b),
                }
                .map(Folded::Int)
                .ok_or_else(|| DYNAMIC.to_string()),
                // Arithmetic on a string follows PHP's numeric-string rules: not folded.
                _ => Err(DYNAMIC.into()),
            }
        }
        _ => Err(DYNAMIC.into()),
    }
}

const DYNAMIC: &str = "lift: `preg_match` needs a pattern known at lift time — a literal, a constant \
                       of this class, or `.` / `+` / `-` / `*` over them; a pattern built at run time \
                       is not translated (DEC-540)";

fn class_const(class: &str, name: &str) -> Result<Folded, String> {
    let own = super::current_class();
    let leaf = class.rsplit('\\').next().unwrap_or(class);
    if class != "self" && leaf != own {
        return Err(format!(
            "lift: `preg_match` pattern `{class}::{name}` — a constant of another class is not read; \
             only the lifted class's own (`self::{name}`)"
        ));
    }
    CONSTS
        .with(|m| m.borrow().get(&(own.clone(), name.to_string())).cloned())
        .ok_or_else(|| format!("lift: `preg_match` pattern `self::{name}` is not a string or int constant of `{own}`"))
}
