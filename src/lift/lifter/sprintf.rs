//! Row 4i — `sprintf` with a LITERAL format of `%s` / `%%` directives only.
//!
//! `%s` formats its argument with PHP's string conversion — the same conversion `.` applies — so
//! `sprintf('%s de « %s »', $a, $b)` IS `$a . ' de « ' . $b . ' »'`, which DEC-511 already lifts to an
//! interpolation. The call is rebuilt as that `.` chain and lifted through the one DEC-511 path; the
//! arguments keep their order, because sequential `%s` directives consume them left to right.
//!
//! Everything else is refused by name rather than approximated: `%d` equals `%s` only for an `int`
//! (it truncates a float and parses a numeric string), which the lifter cannot see; a width, flag,
//! precision or positional (`%1$s`) directive has no interpolation form; a computed format cannot be
//! read at lift time; and an argument count that disagrees with the directives is a PHP
//! `ArgumentCountError` (or silently ignored extra values), not a string.

use super::*;

/// The literal runs and `%s` holes of `fmt`, or the refusal naming the first directive that is not
/// `%s` / `%%`.
fn pieces(fmt: &str) -> Result<(Vec<String>, usize), String> {
    let mut runs = vec![String::new()];
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            runs.last_mut().expect("never empty").push(c);
            continue;
        }
        match chars.next() {
            Some('%') => runs.last_mut().expect("never empty").push('%'),
            Some('s') => runs.push(String::new()),
            other => {
                let mut dir = String::from("%");
                let mut next = other;
                while let Some(d) = next {
                    dir.push(d);
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                    next = chars.next();
                }
                let hint = if dir == "%d" {
                    " — `%d` equals `%s` only for an int (it truncates a float and parses a numeric \
                     string); if the value is an int, write `%s`"
                } else {
                    ""
                };
                return Err(format!(
                    "lift: `sprintf` directive `{dir}` has no interpolation form — only `%s` and `%%` \
                     lift{hint}"
                ));
            }
        }
    }
    let holes = runs.len() - 1;
    Ok((runs, holes))
}

/// The text of a literal format — a string literal, or a `.` chain of them (`'a %s ' . 'b'`, the way a
/// long format is wrapped across lines).
fn literal_text(e: &php::PhpExpr) -> Option<String> {
    match e {
        php::PhpExpr::Str(s) => Some(s.clone()),
        php::PhpExpr::Binary {
            op: php::PhpBinOp::Concat,
            left,
            right,
        } => Some(literal_text(left)? + &literal_text(right)?),
        _ => None,
    }
}

pub(super) fn lift_sprintf(
    callee: &php::PhpExpr,
    args: &[php::PhpExpr],
) -> Option<Result<Expr, String>> {
    let (php::PhpExpr::Name(n), [fmt, values @ ..]) = (callee, args) else {
        return None;
    };
    // A spread or named value is left to `lift_expr`, which refuses it by name on every path.
    let plain =
        |a: &php::PhpExpr| !matches!(a, php::PhpExpr::Spread(_) | php::PhpExpr::NamedArg { .. });
    if n != "sprintf" || !args.iter().all(plain) {
        return None;
    }
    let Some(fmt) = literal_text(fmt) else {
        return Some(Err(
            "lift: `sprintf` needs a literal format string to lift — the directives are read at lift time"
                .into(),
        ));
    };
    Some((|| {
        let (runs, holes) = pieces(&fmt)?;
        if holes != values.len() {
            return Err(format!(
                "lift: `sprintf` format has {holes} `%s` directive(s) but {} value(s) were passed",
                values.len()
            ));
        }
        let concat = |l: php::PhpExpr, r: php::PhpExpr| php::PhpExpr::Binary {
            op: php::PhpBinOp::Concat,
            left: Box::new(l),
            right: Box::new(r),
        };
        let mut chain = php::PhpExpr::Str(runs[0].clone());
        for (value, run) in values.iter().zip(&runs[1..]) {
            chain = concat(chain, value.clone());
            chain = concat(chain, php::PhpExpr::Str(run.clone()));
        }
        lift_expr(&chain)
    })())
}
