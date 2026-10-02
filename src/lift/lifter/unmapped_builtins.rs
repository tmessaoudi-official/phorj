//! PHP builtins the lifter has no phorj counterpart for.
//!
//! An unmapped `strrev($s)` is copied into the draft verbatim as `strrev(s)`, and `phg check` then reports
//! a bare "unknown function" the reader must trace back to PHP. The lifter's contract is that what it
//! cannot do is said LOUDLY, so each such call is named in a `// CANNOT LIFT:` note (the same discipline
//! as an unmapped exception class, an unresolved attribute or a dropped `implements`).
//!
//! The scan runs over the LIFTED draft rather than the PHP AST, so it needs no knowledge of which builtins
//! the lifter mapped: whatever survives as a free call to a PHP internal function is, by construction,
//! unmapped. `php_internal_functions.txt` is `get_defined_functions()["internal"]` from the php-8.5.11
//! oracle (1637 names, every extension that build loads); a user cannot declare a function with one of
//! these names in PHP, so a free call to one that is not declared in the draft is never a user function.

use crate::token::TokenKind;
use std::collections::HashSet;
use std::sync::OnceLock;

/// PHP internal function names (lowercase), plus the `empty` language construct, which is not a function
/// but lifts to a call of the same shape.
fn internal_functions() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut set: HashSet<&'static str> = include_str!("php_internal_functions.txt")
            .lines()
            .filter(|l| !l.is_empty())
            .collect();
        set.insert("empty");
        set
    })
}

/// Lex `src` into `out`, descending into every `"{…}"` interpolation hole (a lifted `"{strrev(s)}"` holds
/// its call in a string segment, not in the token stream). `false` when the source does not lex.
fn flatten(src: &str, out: &mut Vec<TokenKind>) -> bool {
    let Ok(tokens) = crate::tokenizer::lex(src) else {
        return false;
    };
    for t in tokens {
        if let TokenKind::Str(segs) = &t.kind {
            for seg in segs {
                if let crate::token::StrSeg::Interp(hole, ..) = seg {
                    // A hole that does not lex is skipped, never a reason to drop the whole scan.
                    let _ = flatten(hole, out);
                }
            }
        }
        out.push(t.kind);
    }
    true
}

/// The `// CANNOT LIFT:` notes for every PHP internal function the draft still calls, one per name, in
/// first-seen order (Invariant 10 — a lifted draft must not vary run to run).
pub(in crate::lift::lifter) fn unmapped_builtin_notes(draft: &str) -> String {
    let mut owned: Vec<TokenKind> = Vec::new();
    if !flatten(draft, &mut owned) {
        return String::new();
    }
    let kinds: Vec<&TokenKind> = owned.iter().collect();
    // Names the draft declares itself (`function name(`) are never the PHP builtin.
    let declared: HashSet<&str> = kinds
        .windows(2)
        .filter_map(|w| match (w[0], w[1]) {
            (TokenKind::Function, TokenKind::Ident(n)) => Some(n.as_str()),
            _ => None,
        })
        .collect();
    let known = internal_functions();
    let mut seen: Vec<&str> = Vec::new();
    for (i, w) in kinds.windows(2).enumerate() {
        let (TokenKind::Ident(name), TokenKind::LParen) = (w[0], w[1]) else {
            continue;
        };
        // `x.name(` and `x?.name(` are method calls; `function name(` / `new name(` are not free calls.
        let prev = i.checked_sub(1).map(|p| kinds[p]);
        if matches!(
            prev,
            Some(TokenKind::Dot | TokenKind::QuestionDot | TokenKind::Function | TokenKind::New)
        ) {
            continue;
        }
        if declared.contains(name.as_str()) || !known.contains(name.to_ascii_lowercase().as_str()) {
            continue;
        }
        if !seen.contains(&name.as_str()) {
            seen.push(name.as_str());
        }
    }
    seen.iter()
        .map(|name| {
            format!(
                "// CANNOT LIFT: PHP function `{name}()` has no phorj counterpart, so the call was copied \
                 verbatim and `phg check` reports an unknown function. Port it (a helper function) or use \
                 the `Core.*` equivalent.\n"
            )
        })
        .collect()
}
