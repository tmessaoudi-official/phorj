//! Scout row 4l-b2 (DEC-554) — a `preg_match` that fills a captures array lifts onto `Regex.first`.
//!
//! `if (1 !== preg_match('/…/', $s, $m)) { … }` becomes
//!
//! ```text
//! RegexMatch? m = Regex.first(Regex.compile("…"), s);
//! if (m is null) { … }
//! ```
//!
//! The `RegexMatch?` is HOISTED in front of the statement, so the lift only fires where the
//! comparison is the statement's FIRST evaluated test ([`take_site`]): the leftmost operand of an `if`
//! condition, a `return`, or an assignment's value. Anywhere else — behind a `||`, in a loop
//! condition — hoisting would evaluate the subject earlier than PHP does, so it is refused by name.
//!
//! Reads of `$m` follow PHP's array exactly ([`read`]): `$m[0]` is `m.full()`, `$m[k]` is
//! `m.at(k) ?? ""` (a group that sat out is `""` in the middle and unset at the end, and PHP reads
//! both as `""` in a string), `$m['name']` is `m.group("name") ?? ""`. A fallback other than `''`
//! and `isset` tell a MIDDLE group (`""`, set) from a TRAILING one (unset), which `at(k)` cannot, so
//! they lift only on the LAST group, where the two cannot differ. Any other use of `$m` is refused.

use super::*;
use crate::ext::regex::engine::{compiled, Engine};
use std::collections::HashMap;

/// A variable name no PHP source can spell: the placeholder the rewritten comparison tests.
const MARK: &str = "\u{0}capture:";

thread_local! {
    /// Captures variable → its pattern's group names (index 1.. at position 0..; `""` = unnamed).
    /// `None` once two sites fill the same variable from patterns with different groups.
    static VARS: std::cell::RefCell<HashMap<String, Option<Vec<String>>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// The registry as it stands — saved around a closure body and a file-scope statement.
pub(in crate::lift) type Snapshot = HashMap<String, Option<Vec<String>>>;

pub(in crate::lift) fn snapshot() -> Snapshot {
    VARS.with(|v| v.borrow().clone())
}

pub(in crate::lift) fn restore(s: Snapshot) {
    VARS.with(|v| *v.borrow_mut() = s);
}

/// A function body starts with no captures variables.
pub(in crate::lift) fn reset() {
    VARS.with(|v| v.borrow_mut().clear());
}

/// Does `var` already hold a captures value?
pub(super) fn holds(var: &str) -> bool {
    groups_of(var).is_some()
}

fn groups_of(name: &str) -> Option<Option<Vec<String>>> {
    VARS.with(|v| v.borrow().get(name).cloned())
}

/// The three-argument call and the integer it is compared with.
pub(super) struct Site {
    pat: php::PhpExpr,
    subject: php::PhpExpr,
    var: String,
    /// `true` when the comparison asks "did it match".
    matched: bool,
}

/// `preg_match(P, S, $m)` compared with `0` / `1` by `===`, `!==`, `==` or `!=`, either order.
fn site_of(op: php::PhpBinOp, left: &php::PhpExpr, right: &php::PhpExpr) -> Option<Site> {
    use php::PhpBinOp::{Eq, Identical, NotEq, NotIdentical};
    if !matches!(op, Identical | NotIdentical | Eq | NotEq) {
        return None;
    }
    let call = |e: &php::PhpExpr| match e {
        php::PhpExpr::Call { callee, args } => match (&**callee, args.as_slice()) {
            (php::PhpExpr::Name(n), [pat, subject, php::PhpExpr::Var(var)])
                if n.trim_start_matches('\\') == "preg_match"
                    && ![pat, subject].iter().any(|a| {
                        matches!(a, php::PhpExpr::Spread(_) | php::PhpExpr::NamedArg { .. })
                    }) =>
            {
                Some((pat.clone(), subject.clone(), var.clone()))
            }
            _ => None,
        },
        _ => None,
    };
    let int = |e: &php::PhpExpr| match e {
        php::PhpExpr::Int(n @ (0 | 1)) => Some(*n),
        _ => None,
    };
    let ((pat, subject, var), n) = match (call(left), int(right), call(right), int(left)) {
        (Some(c), Some(n), _, _) | (_, _, Some(c), Some(n)) => (c, n),
        _ => return None,
    };
    let equal = matches!(op, Identical | Eq);
    Some(Site {
        pat,
        subject,
        var,
        matched: equal == (n == 1),
    })
}

/// Find the comparison at the head of `e`'s evaluation order and replace it, in place, with the
/// placeholder null test. Only the LEFT spine is walked: every node on it is evaluated first.
fn take_site(e: &mut php::PhpExpr) -> Option<Site> {
    if let php::PhpExpr::Binary { op, left, right } = e {
        if let Some(site) = site_of(*op, left, right) {
            *e = php::PhpExpr::Binary {
                op: if site.matched {
                    php::PhpBinOp::NotIdentical
                } else {
                    php::PhpBinOp::Identical
                },
                left: Box::new(php::PhpExpr::Var(format!("{MARK}{}", site.var))),
                right: Box::new(php::PhpExpr::Null),
            };
            return Some(site);
        }
    }
    match e {
        php::PhpExpr::Binary { left, .. } => take_site(left),
        php::PhpExpr::Unary { expr, .. } => take_site(expr),
        php::PhpExpr::Ternary { cond, .. } => take_site(cond),
        php::PhpExpr::Cast { value, .. } => take_site(value),
        php::PhpExpr::Assign { target, value } if matches!(**target, php::PhpExpr::Var(_)) => {
            take_site(value)
        }
        _ => None,
    }
}

/// A statement whose first test is a captures `preg_match`: the statement with that test replaced,
/// and the hoisted `RegexMatch?` declaration (or assignment) to put in front of it.
pub(in crate::lift) fn hoist(
    s: &php::PhpStmt,
    declared: &mut std::collections::HashSet<String>,
) -> Option<Result<(Stmt, php::PhpStmt), String>> {
    let mut s = s.clone();
    let site = match &mut s {
        php::PhpStmt::If { cond, .. } => take_site(cond),
        php::PhpStmt::Return(Some(e)) | php::PhpStmt::Expr(e) => take_site(e),
        _ => None,
    }?;
    Some(declare(site, declared).map(|decl| (decl, s)))
}

fn declare(site: Site, declared: &mut std::collections::HashSet<String>) -> Result<Stmt, String> {
    let t = translate(&pattern_text(&site.pat)?)?;
    let engine = if t.backtracking {
        Engine::Backtracking
    } else {
        Engine::Linear
    };
    let names = compiled(&t.pattern, engine)
        .map_err(|e| format!("lift: `preg_match` pattern: {e}"))?
        .group_names();
    if super::match_all::holds(&site.var) {
        return Err(super::match_all::shared(&site.var));
    }
    VARS.with(|v| {
        let mut v = v.borrow_mut();
        let entry = v.entry(site.var.clone()).or_insert(Some(names.clone()));
        if entry.as_ref() != Some(&names) {
            *entry = None;
        }
    });
    let init = regex_call("first", vec![compile_call(t), lift_expr(&site.subject)?]);
    record_native_module("Core.Regex");
    Ok(if declared.insert(site.var.clone()) {
        Stmt::VarDecl {
            ty: Type::Optional {
                inner: Box::new(Type::Named {
                    name: "RegexMatch".to_string(),
                    args: Vec::new(),
                    span: SP,
                }),
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

/// Why a captures `preg_match` outside a hoistable position is refused.
const POSITION: &str =
    "lift: `preg_match` with a captures array lifts only as the FIRST test of an \
                        `if` condition, a `return` or an assignment — anywhere else the hoisted \
                        `Regex.first` would read the subject before PHP does (DEC-554)";

/// Does this expression belong to the captures lift?
pub(super) fn owns(e: &php::PhpExpr) -> bool {
    match e {
        php::PhpExpr::Binary {
            op: php::PhpBinOp::Coalesce,
            left,
            ..
        } => capture_index(left).is_some(),
        php::PhpExpr::Binary { op, left, right } => {
            site_of(*op, left, right).is_some() || mark(left).is_some()
        }
        php::PhpExpr::Index { .. } => capture_index(e).is_some(),
        php::PhpExpr::Var(n) => groups_of(n).is_some(),
        php::PhpExpr::Call { callee, args } => {
            matches!(&**callee, php::PhpExpr::Name(n) if n == "isset")
                && args.iter().any(|a| capture_index(a).is_some())
        }
        _ => false,
    }
}

fn mark(e: &php::PhpExpr) -> Option<&str> {
    match e {
        php::PhpExpr::Var(n) => n.strip_prefix(MARK),
        _ => None,
    }
}

/// `$m[<index>]` where `$m` holds captures: the variable and the index expression.
fn capture_index(e: &php::PhpExpr) -> Option<(&str, &php::PhpExpr)> {
    match e {
        php::PhpExpr::Index { base, index } => match &**base {
            php::PhpExpr::Var(n) if groups_of(n).is_some() => Some((n.as_str(), &**index)),
            _ => None,
        },
        _ => None,
    }
}

/// Lift an expression [`owns`] accepted.
pub(super) fn lift(e: &php::PhpExpr) -> Result<Expr, String> {
    match e {
        php::PhpExpr::Binary {
            op: php::PhpBinOp::Coalesce,
            left,
            right,
        } => {
            let (var, index) = capture_index(left).expect("guarded by owns");
            if matches!(&**right, php::PhpExpr::Str(s) if s.is_empty()) {
                return read(var, index);
            }
            let k = group(var, index)?;
            if k == 0 {
                return read(var, index);
            }
            last_group(var, k, "a `??` fallback other than `''`")?;
            Ok(Expr::Binary {
                op: BinaryOp::Coalesce,
                lhs: Box::new(call_at(var, k)),
                rhs: Box::new(lift_expr(right)?),
                span: SP,
            })
        }
        php::PhpExpr::Binary { op, left, .. } => match mark(left) {
            Some(var) => {
                let test = Expr::InstanceOf {
                    value: Box::new(Expr::Ident(var.to_string(), SP)),
                    type_name: "null".to_string(),
                    span: SP,
                };
                Ok(if *op == php::PhpBinOp::Identical {
                    test
                } else {
                    not(test)
                })
            }
            None => Err(POSITION.into()),
        },
        php::PhpExpr::Index { .. } => {
            let (var, index) = capture_index(e).expect("guarded by owns");
            read(var, index)
        }
        php::PhpExpr::Call { args, .. } => {
            let [arg] = args.as_slice() else {
                return Err("lift: `isset` over a captures array takes one `$m[…]` argument".into());
            };
            let (var, index) = capture_index(arg).ok_or(
                "lift: `isset` over a captures array takes one `$m[…]` argument",
            )?;
            let k = group(var, index)?;
            let absent = if k == 0 {
                Expr::Ident(var.to_string(), SP)
            } else {
                last_group(var, k, "`isset`")?;
                call_at(var, k)
            };
            Ok(not(Expr::InstanceOf {
                value: Box::new(absent),
                type_name: "null".to_string(),
                span: SP,
            }))
        }
        php::PhpExpr::Var(n) => Err(format!(
            "lift: `${n}` holds `preg_match` captures — only `${n}[<literal>]`, `${n}[<literal>] ?? …` \
             and `isset(${n}[<literal>])` read it on `RegexMatch` (DEC-554)"
        )),
        _ => unreachable!("guarded by owns"),
    }
}

/// The group an index names: `0`, a group number, or a group name.
fn group(var: &str, index: &php::PhpExpr) -> Result<usize, String> {
    let names = groups_of(var).flatten();
    let count = names.as_ref().map(Vec::len);
    match index {
        php::PhpExpr::Int(k) => {
            let k = usize::try_from(*k).map_err(|_| format!("lift: `${var}[{k}]` is not a group"))?;
            match count {
                Some(c) if k > c => Err(format!(
                    "lift: `${var}[{k}]` — the pattern has {c} group(s), so PHP reads null here"
                )),
                _ => Ok(k),
            }
        }
        php::PhpExpr::Str(name) => names
            .and_then(|ns| ns.iter().position(|n| n == name))
            .map(|i| i + 1)
            .ok_or_else(|| {
                format!("lift: `${var}['{name}']` — no group of that name in every pattern `${var}` holds")
            }),
        _ => Err(format!(
            "lift: `${var}[…]` reads captures only with a literal group number or name (DEC-554)"
        )),
    }
}

/// Refuse unless `k` is the last group of the (single) pattern `var` holds.
fn last_group(var: &str, k: usize, what: &str) -> Result<(), String> {
    match groups_of(var).flatten() {
        Some(ns) if ns.len() == k => Ok(()),
        _ => Err(format!(
            "lift: `preg_match` captures: {what} on `${var}[{k}]` — PHP leaves a MIDDLE group that sat out as `\"\"` and \
             unsets a TRAILING one, and `RegexMatch.at` answers null for both; it lifts only on the \
             pattern's last group, where the two cannot differ"
        )),
    }
}

fn read(var: &str, index: &php::PhpExpr) -> Result<Expr, String> {
    let k = group(var, index)?;
    let recv = Expr::Ident(var.to_string(), SP);
    if k == 0 {
        return Ok(method(recv, "full", Vec::new()));
    }
    let text = match index {
        php::PhpExpr::Str(name) => method(
            recv,
            "group",
            vec![Expr::Str(vec![StrPart::Literal(name.clone())], SP)],
        ),
        _ => call_at(var, k),
    };
    Ok(Expr::Binary {
        op: BinaryOp::Coalesce,
        lhs: Box::new(text),
        rhs: Box::new(Expr::Str(vec![StrPart::Literal(String::new())], SP)),
        span: SP,
    })
}

fn call_at(var: &str, k: usize) -> Expr {
    let k = i64::try_from(k).unwrap_or(i64::MAX);
    method(
        Expr::Ident(var.to_string(), SP),
        "at",
        vec![Expr::Int(k, SP)],
    )
}

pub(super) fn method(recv: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(recv),
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

fn not(e: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Not,
        expr: Box::new(e),
        span: SP,
    }
}
