//! Which tokens are NAMES (DEC-531, scout row 5k).
//!
//! An identifier is always a name. A reserved word is a name only where the parser accepts it as a
//! member name: after `.`/`?.`/`::`/`function`, or in a declaration, `with`-field or named-argument
//! slot. Deciding by POSITION is what keeps a `match (x) { … }` expression from being read as a use
//! of a method named `match`. `constructor` is never a member name (the parser keeps it reserved).
//!
//! Deliberately NOT used by `symbols::all_ident_spans` (references/rename): those resolve top-level
//! and local names only, and a reserved word can be neither.

use crate::token::{Token, TokenKind};

/// The name `tokens[i]` spells, if it is one.
pub(super) fn name_at(tokens: &[Token], i: usize) -> Option<&str> {
    let tok = tokens.get(i)?;
    if let TokenKind::Ident(n) = &tok.kind {
        return Some(n);
    }
    let word = crate::tokenizer::reserved_word(&tok.kind)?;
    if word == "constructor" {
        return None;
    }
    let prev = i
        .checked_sub(1)
        .and_then(|j| tokens.get(j))
        .map(|t| &t.kind);
    let next = tokens.get(i + 1).map(|t| &t.kind);
    use TokenKind as K;
    // The end of a declared type (`int`, `List<T>`, `string?`, `T[]`) directly before the name.
    let after_type = matches!(prev, Some(K::Ident(_) | K::Gt | K::Question | K::RBracket));
    let is_name = match (prev, next) {
        (Some(K::Dot | K::QuestionDot | K::ColonColon | K::Function), _) => true,
        // A property hook `int type { … }` — but `x with { … }` is the functional update.
        (_, Some(K::LBrace)) => after_type && word != "with",
        // A field or a promoted parameter: `int type;` / `,` / `)`.
        (_, Some(K::Semicolon | K::Comma | K::RParen)) => after_type,
        // A field initializer, or a `with { type = … }` field.
        (_, Some(K::Eq)) => after_type || matches!(prev, Some(K::LBrace | K::Comma)),
        // A named argument: `(type: …` / `, type: …`.
        (_, Some(K::Colon)) => matches!(prev, Some(K::LParen | K::Comma)),
        _ => false,
    };
    is_name.then_some(word)
}
