//! The pattern translation behind row 4l-a — a delimited PCRE pattern as PHP writes it, into the bare
//! pattern `Core.Regex` compiles, with the same existence answer on every subject.
//!
//! The delimiters and modifiers are PHP's and are consumed here; the body goes through
//! [`super::scan`]. The result is checked by `ext::regex::engine::validate` — the very gate the
//! checker applies to a literal pattern — so a draft never carries a pattern `phg check` would refuse,
//! and PCRE-only or divergent syntax is refused with that gate's own words.

use super::scan::{scan, Opts};
use crate::ext::regex::engine::{validate, Engine};
use crate::ext::regex::reject::linear_unsupported;

/// A translated pattern and whether it needs the backtracking engine.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::lift) struct Translated {
    pub(in crate::lift) pattern: String,
    pub(in crate::lift) backtracking: bool,
    /// See [`super::scan::Facts`]: a `.` or negated class that PCRE reads per byte.
    pub(in crate::lift) byte_atom: bool,
    /// See [`super::scan::Facts`]: no match can be empty.
    pub(in crate::lift) nonempty: bool,
    /// See [`super::scan::Facts`]: `$` / `\Z` became a consuming `\n?\z`.
    pub(in crate::lift) dollar_rewritten: bool,
}

/// Translate a PHP `preg_match` pattern literal (delimiters and modifiers included).
pub(in crate::lift) fn translate(literal: &str) -> Result<Translated, String> {
    let fail = |why: &str| format!("lift: `preg_match` pattern `{literal}`: {why}");
    let (body, mods, delim) = split(literal).map_err(|w| fail(&w))?;
    let (mut unicode, mut dollar_end, mut flags) = (false, false, String::new());
    for m in mods.chars() {
        match m {
            'u' => unicode = true,
            'D' => dollar_end = true,
            'i' | 's' if !flags.contains(m) => flags.push(m),
            'i' | 's' | 'S' | ' ' | '\n' | '\r' => {} // `S` is a no-op since PHP 7.3
            'm' => {
                return Err(fail(
                    "the `m` modifier — PCRE's multiline `^` does not match after a newline that \
                     ends the subject, and phorj's does",
                ))
            }
            'x' => {
                return Err(fail(
                    "the `x` modifier — PCRE keeps whitespace inside a character class, and \
                     phorj's extended mode drops it",
                ))
            }
            other => return Err(fail(&format!("the `{other}` modifier has no phorj form"))),
        }
    }
    if flags.contains('i') && !unicode {
        return Err(fail(
            "the `i` modifier without `u` — PCRE folds ASCII case only, and phorj folds Unicode \
             case (U+212A KELVIN SIGN matches `k`); add `u` to the PHP pattern if the subject is \
             text, then lift again",
        ));
    }
    if !unicode && !body.is_ascii() {
        return Err(fail(
            "a non-ASCII character without the `u` modifier — PCRE reads it as separate bytes; add \
             `u` to the PHP pattern, then lift again",
        ));
    }
    let o = Opts {
        unicode,
        dollar_end,
        delim,
    };
    let (core, lookahead, facts) = scan(body, &o).map_err(|w| fail(&w))?;
    let pattern = if flags.is_empty() {
        core
    } else {
        format!("(?{flags}){core}")
    };
    let backtracking = lookahead || linear_unsupported(&pattern).is_some();
    let engine = if backtracking {
        Engine::Backtracking
    } else {
        Engine::Linear
    };
    validate(&pattern, engine).map_err(|e| fail(&e.message))?;
    Ok(Translated {
        pattern,
        backtracking,
        byte_atom: facts.byte_atom,
        nonempty: facts.nonempty,
        dollar_rewritten: facts.dollar_rewritten,
    })
}

/// Split `/body/mods` as PHP does: optional leading whitespace, a non-alphanumeric, non-backslash
/// delimiter, and — for `(`, `[`, `{`, `<` — its closing bracket, found by nesting. Returns the body,
/// the modifiers, and the delimiter an escape in the body can name (none for a bracket pair).
fn split(literal: &str) -> Result<(&str, &str, Option<char>), String> {
    let s = literal.trim_start();
    let Some(open) = s.chars().next() else {
        return Err("an empty pattern has no delimiter".into());
    };
    if open.is_ascii_alphanumeric() || open == '\\' || open.is_whitespace() {
        return Err(format!(
            "`{open}` cannot be a delimiter — PHP needs a non-alphanumeric, non-backslash one"
        ));
    }
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        c => c,
    };
    let mut depth = 0usize;
    let mut chars = s.char_indices().skip(1);
    while let Some((at, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == close && depth == 0 {
            let body = &s[open.len_utf8()..at];
            let mods = &s[at + close.len_utf8()..];
            return Ok((body, mods, (open == close).then_some(open)));
        } else if c == close {
            depth -= 1;
        } else if c == open && open != close {
            depth += 1;
        }
    }
    Err(format!("no closing delimiter `{close}`"))
}
