//! Symbol resolution for LSP hover + go-to-definition (Item D). A lightweight, robust v1: rather than
//! a full position→type index over the checker, it finds the **identifier token** at the cursor (via
//! the lexer's spans) and resolves that *name* to a top-level declaration (function / class / enum /
//! interface / trait / type alias). This covers the high-value cases — jump to / hover a named symbol
//! — without instrumenting the checker. Locals and field-vs-name collisions are a v2 refinement.
//!
//! Hover renders the declaration's signature by **slicing the source** from the decl's span to the
//! first `{`/`;`, so it shows exactly what the user wrote (no AST type renderer needed).

use crate::ast::{Item, Program};
use crate::lexer::lex;
use crate::token::{Span, TokenKind};

/// Convert an LSP 0-based `(line, character)` to a byte offset into `text`. `character` is counted in
/// Unicode scalars (correct for ASCII/BMP code; astral-plane columns may differ from a UTF-16 client,
/// a documented v1 simplification). Clamps to the line end; `None` if the line is out of range.
pub fn offset_at(text: &str, line: u32, character: u32) -> Option<usize> {
    let mut byte = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i as u32 == line {
            for (chars, (boff, _)) in l.char_indices().enumerate() {
                if chars as u32 == character {
                    return Some(byte + boff);
                }
            }
            return Some(byte + l.trim_end_matches(['\r', '\n']).len());
        }
        byte += l.len();
    }
    None
}

/// The identifier name whose token span contains `offset`, if any. Lexes `text` (a lex error ⇒ no
/// symbol — the diagnostics path already reports it).
pub fn ident_at(text: &str, offset: usize) -> Option<String> {
    let tokens = lex(text).ok()?;
    for t in &tokens {
        if let TokenKind::Ident(name) = &t.kind {
            if offset >= t.span.start && offset < t.span.start + t.span.len {
                return Some(name.clone());
            }
        }
    }
    None
}

/// The declaration of a top-level item named `name`: its kind label, name, and span. `None` if no such
/// top-level symbol (a local/parameter/field is not resolved in v1).
pub fn definition_of<'a>(program: &'a Program, name: &str) -> Option<(&'a str, Span)> {
    for item in &program.items {
        let hit = match item {
            Item::Function(f) if f.name == name => Some(("function", f.span)),
            Item::Class(c) if c.name == name => Some(("class", c.span)),
            Item::Enum(e) if e.name == name => Some(("enum", e.span)),
            Item::Interface(i) if i.name == name => Some(("interface", i.span)),
            Item::Trait(t) if t.name == name => Some(("trait", t.span)),
            Item::TypeAlias { name: n, span, .. } if n == name => Some(("type", *span)),
            _ => None,
        };
        if hit.is_some() {
            return hit;
        }
    }
    None
}

/// Render the one-line signature of a declaration by slicing the source from the decl's start to the
/// first `{` (body open) or `;` (type alias) — exactly the user's written signature.
pub fn signature_text(text: &str, span: Span) -> String {
    // The decl's `span` covers only its keyword/name, so scan forward from its start to the body open
    // `{` (or `;` for a type alias / expression-body fn) — that delimiter always appears within a
    // short distance for these item kinds. Char-safe and defensively capped so a malformed buffer
    // can't slice the whole file or a non-char boundary.
    let start = span.start.min(text.len());
    let rest = &text[start..];
    let mut cut = rest.len();
    for (i, c) in rest.char_indices() {
        if c == '{' || c == ';' || i > 500 {
            cut = i;
            break;
        }
    }
    rest[..cut].split_whitespace().collect::<Vec<_>>().join(" ")
}
