//! Scout row 4l-b3 (DEC-554) — `preg_match_all` lifts onto `Regex.all`.
//!
//! `if (preg_match_all('/…/u', $s, $m) < 1) { … }` becomes
//!
//! ```text
//! List<RegexMatch> m = Regex.all(Regex.compile("…"), s);
//! if (m.length() < 1) { … }
//! ```
//!
//! Hoisted exactly like the `preg_match` captures ([`super::captures`]): the lift fires only where
//! the call is the statement's FIRST evaluated test, is compared with an integer, is an assignment's
//! value, or is the whole statement. The return is the match COUNT, so `=== false` (a PCRE error)
//! is not an integer comparison and stays refused.
//!
//! Reads follow PHP's default `PREG_PATTERN_ORDER` array: `$m[k]` is the COLUMN of group `k` across
//! every match — `m.map(r => r.at(k) ?? "")`, since PATTERN_ORDER fills a group that sat out with
//! `""` in every position. With `PREG_OFFSET_CAPTURE` the column holds `[text, byte offset]` pairs —
//! `(r.at(k) ?? "", r.startOf(k) ?? -1)`, where PHP reports `-1` for a group that sat out. Both
//! column forms are the faithful total answers; `PREG_SET_ORDER` (rows, not columns) and
//! `PREG_UNMATCHED_AS_NULL` are refused by name, as is any other use of `$m`.

use super::captures::method;
use super::*;
use crate::ext::regex::engine::{compiled, Engine};
use std::collections::HashMap;

/// The lambda parameter of a column read: a name a PHP function of the same file will not take
/// (a local that shadows a function is `E-SHADOW-FN`).
const PARAM: &str = "regexMatch";

/// A variable name no PHP source can spell: the placeholder the rewritten count test reads.
const MARK: &str = "\u{0}matches:";

/// What a matches variable holds: its pattern's group names and whether offsets were asked for.
#[derive(Clone, PartialEq)]
pub(in crate::lift) struct Info {
    names: Vec<String>,
    offset: bool,
}

thread_local! {
    /// Matches variable → what it holds. `None` once two sites fill the same variable with
    /// different patterns' groups or flags.
    static VARS: std::cell::RefCell<HashMap<String, Option<Info>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// The registry as it stands — saved around a closure body and a file-scope statement.
pub(in crate::lift) type Snapshot = HashMap<String, Option<Info>>;

pub(in crate::lift) fn snapshot() -> Snapshot {
    VARS.with(|v| v.borrow().clone())
}

pub(in crate::lift) fn restore(s: Snapshot) {
    VARS.with(|v| *v.borrow_mut() = s);
}

/// A function body starts with no matches variables.
pub(in crate::lift) fn reset() {
    VARS.with(|v| v.borrow_mut().clear());
}

/// Does `var` already hold matches?
pub(super) fn holds(var: &str) -> bool {
    info_of(var).is_some()
}

/// Why one variable cannot hold both a `preg_match` captures value and `preg_match_all` matches.
pub(super) fn shared(var: &str) -> String {
    format!(
        "lift: `${var}` receives both a `preg_match` captures array and `preg_match_all` matches — \
         they lift to different types (`RegexMatch?` and `List<RegexMatch>`); use two variables (DEC-554)"
    )
}

fn info_of(name: &str) -> Option<Option<Info>> {
    VARS.with(|v| v.borrow().get(name).cloned())
}

/// A `preg_match_all($p, $s, $m[, flags])` call.
struct Site {
    pat: php::PhpExpr,
    subject: php::PhpExpr,
    var: String,
    offset: bool,
}

/// Why a `preg_match_all` outside a hoistable position is refused.
const POSITION: &str = "lift: `preg_match_all` lifts only as the FIRST test of an `if` condition, \
                        compared with an integer, as an assignment's value, or as a statement of its \
                        own — anywhere else the hoisted `Regex.all` would read the subject before PHP \
                        does (DEC-554)";

/// The flags argument: `PREG_PATTERN_ORDER` and `PREG_OFFSET_CAPTURE`, joined by `|`.
fn flags(e: &php::PhpExpr, offset: &mut bool) -> Result<(), String> {
    match e {
        php::PhpExpr::Binary {
            op: php::PhpBinOp::BitOr,
            left,
            right,
        } => {
            flags(left, offset)?;
            flags(right, offset)
        }
        php::PhpExpr::Name(n) => match n.trim_start_matches('\\') {
            "PREG_PATTERN_ORDER" => Ok(()),
            "PREG_OFFSET_CAPTURE" => {
                *offset = true;
                Ok(())
            }
            n @ ("PREG_SET_ORDER" | "PREG_UNMATCHED_AS_NULL") => Err(format!(
                "lift: `preg_match_all` with `{n}` — the lift reads PHP's default PATTERN_ORDER \
                 columns (and `PREG_OFFSET_CAPTURE`) only; `{n}` has no lifted form (DEC-554)"
            )),
            n => Err(format!(
                "lift: `preg_match_all` flag `{n}` is not lifted (DEC-554)"
            )),
        },
        _ => Err(
            "lift: `preg_match_all` flags must be `PREG_` constants joined by `|` (DEC-554)".into(),
        ),
    }
}

/// `preg_match_all(P, S, $m)` or `preg_match_all(P, S, $m, FLAGS)`.
fn call_of(e: &php::PhpExpr) -> Option<Result<Site, String>> {
    let php::PhpExpr::Call { callee, args } = e else {
        return None;
    };
    let php::PhpExpr::Name(n) = &**callee else {
        return None;
    };
    if n.trim_start_matches('\\') != "preg_match_all" {
        return None;
    }
    let (pat, subject, var, rest) = match args.as_slice() {
        [pat, subject, php::PhpExpr::Var(var), rest @ ..] if rest.len() <= 1 => {
            (pat, subject, var, rest)
        }
        _ => return Some(Err(POSITION.to_string())),
    };
    if [pat, subject]
        .iter()
        .any(|a| matches!(a, php::PhpExpr::Spread(_) | php::PhpExpr::NamedArg { .. }))
    {
        return Some(Err(POSITION.to_string()));
    }
    let mut offset = false;
    if let [f] = rest {
        if let Err(e) = flags(f, &mut offset) {
            return Some(Err(e));
        }
    }
    Some(Ok(Site {
        pat: pat.clone(),
        subject: subject.clone(),
        var: var.clone(),
        offset,
    }))
}

fn mark_var(var: &str) -> php::PhpExpr {
    php::PhpExpr::Var(format!("{MARK}{var}"))
}

/// Replace the call in `slot` with the count placeholder.
fn replace_call(slot: &mut php::PhpExpr) -> Option<Result<Site, String>> {
    let site = call_of(slot)?;
    Some(site.inspect(|s| *slot = mark_var(&s.var)))
}

fn is_cmp(op: php::PhpBinOp) -> bool {
    use php::PhpBinOp::{Eq, Ge, Gt, Identical, Le, Lt, NotEq, NotIdentical};
    matches!(
        op,
        Eq | Identical | NotEq | NotIdentical | Lt | Le | Gt | Ge
    )
}

/// Find the call at the head of `e`'s evaluation order. Only the LEFT spine is walked.
fn take(e: &mut php::PhpExpr) -> Option<Result<Site, String>> {
    match e {
        php::PhpExpr::Binary { op, left, right } => {
            if is_cmp(*op) {
                if matches!(**right, php::PhpExpr::Int(_)) {
                    if let Some(r) = replace_call(left) {
                        return Some(r);
                    }
                } else if matches!(**left, php::PhpExpr::Int(_)) {
                    if let Some(r) = replace_call(right) {
                        return Some(r);
                    }
                }
            }
            take(left)
        }
        php::PhpExpr::Unary { expr, .. } => take(expr),
        php::PhpExpr::Ternary { cond, .. } => take(cond),
        php::PhpExpr::Cast { value, .. } => take(value),
        php::PhpExpr::Assign { target, value } if matches!(**target, php::PhpExpr::Var(_)) => {
            replace_call(value).or_else(|| take(value))
        }
        _ => None,
    }
}

/// A statement whose first test is a `preg_match_all`: the statement with that call replaced, and
/// the hoisted `List<RegexMatch>` declaration (or assignment) to put in front of it.
pub(in crate::lift) fn hoist(
    s: &php::PhpStmt,
    declared: &mut std::collections::HashSet<String>,
) -> Option<Result<(Stmt, php::PhpStmt), String>> {
    let mut s = s.clone();
    let site = match &mut s {
        php::PhpStmt::If { cond, .. } => take(cond),
        php::PhpStmt::Return(Some(e)) => replace_call(e).or_else(|| take(e)),
        php::PhpStmt::Expr(e) => match replace_call(e) {
            // A bare statement: nothing is left of it once the matches are hoisted.
            Some(site) => {
                s = php::PhpStmt::Block(Vec::new());
                Some(site)
            }
            None => take(e),
        },
        _ => None,
    }?;
    Some(
        site.and_then(|site| declare(site, declared))
            .map(|decl| (decl, s)),
    )
}

/// `translate` promises the same EXISTENCE answer; `preg_match_all` reads EVERY match, so the pattern
/// must also agree with PCRE on where each match starts and ends. Three things make it differ, and each
/// is refused by name rather than lifted (DEC-554; Invariant 14):
/// an empty match (PCRE retries at the same position, phorj's engine steps past it), a byte atom
/// in a pattern without `u` (PCRE takes one BYTE, phorj one character), and a rewritten `$` (the consuming
/// `\n?\z` eats the final newline a match must stop before).
fn all_matches_faithful(t: &translate::Translated) -> Result<(), String> {
    if t.byte_atom {
        return Err(
            "lift: `preg_match_all` has a `.` or a negated class without the `u` modifier — PCRE matches one BYTE \
                    per `.` or negated class and phorj one character, so the matches (and their \
                    offsets) differ; add `u` to the PHP pattern if the subject is text (DEC-554)"
                .into(),
        );
    }
    if t.dollar_rewritten {
        return Err(
            "lift: `preg_match_all` with a `$` or `\\Z` — PHP's `$` is zero-width before a final \
                    newline, and the lifted `\\n?\\z` consumes it, so a match's text and the next \
                    match differ; use `\\z` or the `D` modifier where the subject has no final \
                    newline (DEC-554)"
                .into(),
        );
    }
    if !t.nonempty {
        return Err(
            "lift: `preg_match_all` with a pattern that may match the empty string — PCRE \
                    reports the empty match and retries at the same position, phorj's engine skips \
                    it, so the COUNT differs (`/\\d*/` on `a1b22c`: 6 in PHP, 4 lifted); give the \
                    pattern a part every match must consume, outside any group (DEC-554)"
                .into(),
        );
    }
    Ok(())
}

fn declare(site: Site, declared: &mut std::collections::HashSet<String>) -> Result<Stmt, String> {
    let t = translate(&pattern_text(&site.pat)?)?;
    all_matches_faithful(&t)?;
    let engine = if t.backtracking {
        Engine::Backtracking
    } else {
        Engine::Linear
    };
    let names = compiled(&t.pattern, engine)
        .map_err(|e| format!("lift: `preg_match_all` pattern: {e}"))?
        .group_names();
    let info = Info {
        names,
        offset: site.offset,
    };
    if super::captures::holds(&site.var) {
        return Err(shared(&site.var));
    }
    VARS.with(|v| {
        let mut v = v.borrow_mut();
        let entry = v.entry(site.var.clone()).or_insert(Some(info.clone()));
        if entry.as_ref() != Some(&info) {
            *entry = None;
        }
    });
    let init = regex_call("all", vec![compile_call(t), lift_expr(&site.subject)?]);
    record_native_module("Core.Regex");
    Ok(if declared.insert(site.var.clone()) {
        Stmt::VarDecl {
            ty: Type::Named {
                name: "List".to_string(),
                args: vec![named("RegexMatch")],
                span: SP,
            },
            name: site.var,
            init,
            mutable: true,
            span: SP,
        }
    } else {
        Stmt::Assign {
            target: Expr::Ident(site.var, SP),
            value: init,
            span: SP,
        }
    })
}

/// Does this expression belong to the `preg_match_all` lift?
pub(super) fn owns(e: &php::PhpExpr) -> bool {
    match e {
        php::PhpExpr::Var(n) => n.starts_with(MARK) || info_of(n).is_some(),
        php::PhpExpr::Index { base, .. } => {
            matches!(&**base, php::PhpExpr::Var(n) if info_of(n).is_some())
        }
        php::PhpExpr::Call { .. } => call_of(e).is_some(),
        _ => false,
    }
}

/// Lift an expression [`owns`] accepted.
pub(super) fn lift(e: &php::PhpExpr) -> Result<Expr, String> {
    match e {
        php::PhpExpr::Var(n) => match n.strip_prefix(MARK) {
            Some(var) => {
                record_native_module("Core.List");
                Ok(method(
                    Expr::Ident(var.to_string(), SP),
                    "length",
                    Vec::new(),
                ))
            }
            None => Err(format!(
                "lift: `${n}` holds `preg_match_all` matches — only `${n}[<literal>]` (a column of \
                 every match's group) reads it (DEC-554)"
            )),
        },
        php::PhpExpr::Index { base, index } => {
            let php::PhpExpr::Var(var) = &**base else {
                unreachable!("guarded by owns")
            };
            column(var, index)
        }
        _ => Err(POSITION.into()),
    }
}

/// `$m[k]` — the column of group `k`.
fn column(var: &str, index: &php::PhpExpr) -> Result<Expr, String> {
    let info = info_of(var).flatten().ok_or_else(|| {
        format!("lift: `${var}` holds `preg_match_all` matches of patterns with different groups")
    })?;
    let k = match index {
        php::PhpExpr::Int(k) => {
            let k =
                usize::try_from(*k).map_err(|_| format!("lift: `${var}[{k}]` is not a group"))?;
            if k > info.names.len() {
                return Err(format!(
                    "lift: `${var}[{k}]` — the pattern has {} group(s), so PHP has no such column",
                    info.names.len()
                ));
            }
            k
        }
        php::PhpExpr::Str(name) => info
            .names
            .iter()
            .position(|n| n == name)
            .map(|i| i + 1)
            .ok_or_else(|| format!("lift: `${var}['{name}']` — no group of that name"))?,
        _ => {
            return Err(format!(
                "lift: `${var}[…]` reads matches only with a literal group number or name (DEC-554)"
            ))
        }
    };
    let r = || Expr::Ident(PARAM.to_string(), SP);
    let int = |n: i64| Expr::Int(n, SP);
    let text = if k == 0 {
        method(r(), "full", Vec::new())
    } else {
        Expr::Binary {
            op: BinaryOp::Coalesce,
            lhs: Box::new(match index {
                php::PhpExpr::Str(name) => method(
                    r(),
                    "group",
                    vec![Expr::Str(vec![StrPart::Literal(name.clone())], SP)],
                ),
                _ => method(r(), "at", vec![int(i64::try_from(k).unwrap_or(i64::MAX))]),
            }),
            rhs: Box::new(Expr::Str(vec![StrPart::Literal(String::new())], SP)),
            span: SP,
        }
    };
    let (body, ret) = if info.offset {
        let at = if k == 0 {
            method(r(), "start", Vec::new())
        } else {
            Expr::Binary {
                op: BinaryOp::Coalesce,
                lhs: Box::new(method(
                    r(),
                    "startOf",
                    vec![int(i64::try_from(k).unwrap_or(i64::MAX))],
                )),
                rhs: Box::new(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(int(1)),
                    span: SP,
                }),
                span: SP,
            }
        };
        (
            Expr::Tuple(vec![text, at], None, SP),
            // The printed `function(…): (string, int)` form does not parse: the tuple is inferred.
            None,
        )
    } else {
        (text, Some(named("string")))
    };
    record_native_module("Core.List");
    let lambda = Expr::Lambda {
        params: vec![crate::ast::Param {
            ty: named("RegexMatch"),
            name: PARAM.to_string(),
            default: None,
            variadic: false,
            span: SP,
        }],
        ret,
        throws: Vec::new(),
        body: crate::ast::LambdaBody::Expr(Box::new(body)),
        span: SP,
    };
    Ok(method(
        Expr::Ident(var.to_string(), SP),
        "map",
        vec![lambda],
    ))
}
