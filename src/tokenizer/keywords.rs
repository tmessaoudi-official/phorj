//! The reserved-word table (split out of `mod.rs` for DEC-531, scout row 5k).
//!
//! [`keyword`] maps source text to its token on the lexer's hot path, so it stays a `match`.
//! [`RESERVED_WORDS`] lists the same words for the two REVERSE questions DEC-531 needs — "which word
//! is this keyword token?" (a keyword in member position is a name) and "is this text reserved?" (the
//! lifter renames a keyword-named local). `tests` pins that the two agree in both directions.

use crate::token::TokenKind;

/// Every word [`keyword`] reserves, in its arm order. Contextual words (`var`, `foreach`, `as`,
/// `when`, `using`, …) are NOT here: they lex as identifiers and are already legal names.
pub const RESERVED_WORDS: [&str; 43] = [
    "function",
    "class",
    "enum",
    "constructor",
    "trait",
    "const",
    "open",
    "abstract",
    "sealed",
    "public",
    "private",
    "protected",
    "internal",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "break",
    "continue",
    "in",
    "match",
    "import",
    "package",
    "this",
    "true",
    "false",
    "null",
    "new",
    "instanceof",
    "interface",
    "implements",
    "extends",
    "mutable",
    "static",
    "with",
    "type",
    "throw",
    "try",
    "catch",
    "finally",
    "throws",
];

/// The source spelling of a reserved-word token, or `None` for any other token.
pub fn reserved_word(kind: &TokenKind) -> Option<&'static str> {
    RESERVED_WORDS
        .iter()
        .copied()
        .find(|w| keyword(w).as_ref() == Some(kind))
}

/// True when `name` is a reserved word — it cannot name a local, a parameter or a top-level item.
pub fn is_reserved_word(name: &str) -> bool {
    keyword(name).is_some()
}

pub(super) fn keyword(s: &str) -> Option<TokenKind> {
    use TokenKind::*;
    Some(match s {
        "function" => Function,
        "class" => Class,
        "enum" => Enum,
        "constructor" => Constructor,
        "trait" => Trait,
        "const" => Const,
        "open" => Open,
        "abstract" => Abstract,
        "sealed" => Sealed,
        "public" => Public,
        "private" => Private,
        "protected" => Protected,
        "internal" => Internal,
        "return" => Return,
        "if" => If,
        "else" => Else,
        "for" => For,
        "while" => While,
        "do" => Do,
        "break" => Break,
        "continue" => Continue,
        "in" => In,
        "match" => Match,
        "import" => Import,
        "package" => Package,
        "this" => This,
        "true" => True,
        "false" => False,
        "null" => Null,
        "new" => New,
        "instanceof" => Instanceof,
        "interface" => Interface,
        "implements" => Implements,
        "extends" => Extends,
        // `var` is a CONTEXTUAL keyword (like `foreach`/`as`/`when`): it stays an ordinary identifier
        // in the token stream and is recognized as the inference-binding keyword only at a
        // declaration/binding start by the parser (`Parser::at_var_decl`). This frees `var` to be a
        // value / parameter / field name (it maps to a legal PHP `$var` / `->var`).
        "mutable" => Mutable,
        "static" => Static,
        "with" => With,
        "type" => TypeKw,
        "throw" => Throw,
        "try" => Try,
        "catch" => Catch,
        "finally" => Finally,
        "throws" => Throws,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_word_is_reserved_and_round_trips() {
        for w in RESERVED_WORDS {
            let k = keyword(w).unwrap_or_else(|| panic!("`{w}` is listed but not reserved"));
            assert_eq!(reserved_word(&k), Some(w), "`{w}` does not round-trip");
        }
    }

    #[test]
    fn every_keyword_arm_is_listed() {
        // The third direction: a word added to `keyword` alone — in neither list — would be
        // silently un-nameable as a member. Read the arms from this file's own source.
        let src = include_str!("keywords.rs");
        let body = &src[src.find("pub(super) fn keyword").unwrap()..];
        let body = &body[..body.find("_ => return None").unwrap()];
        let mut arms: Vec<&str> = body
            .split('"')
            .collect::<Vec<_>>()
            .chunks(2)
            .filter_map(|c| c.get(1).copied())
            .collect();
        let mut listed = RESERVED_WORDS.to_vec();
        arms.sort_unstable();
        listed.sort_unstable();
        assert_eq!(arms, listed);
    }

    #[test]
    fn every_reserved_word_the_editor_knows_is_listed() {
        // Drift guard: a word added to `keyword` (and, per Invariant 17, to the LSP keyword list)
        // but not to `RESERVED_WORDS` could never be a member name. Every LSP keyword that lexes
        // as a keyword token must be listed here.
        for w in crate::lsp::keywords::KEYWORDS {
            if keyword(w).is_some() {
                assert!(
                    RESERVED_WORDS.contains(w),
                    "`{w}` is reserved but not listed"
                );
            }
        }
    }
}
