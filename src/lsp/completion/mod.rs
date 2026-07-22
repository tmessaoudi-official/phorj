//! Parse-tolerant completion (2026-07-20 alignment pass). Completion is invoked mid-edit, when the
//! buffer almost never parses (`Output.` with nothing after it is a parse error) — so this works from
//! the raw text + cursor offset and treats a successful parse as a best-effort bonus, never a
//! requirement. Before this, `completion()` bailed to `[]` the instant the buffer didn't parse, i.e.
//! exactly while the user was typing a member access. Contexts inferred from the text before the cursor:
//!   * `import X.` — importable Core modules + the user's own project packages.
//!   * `<receiver>.` — Core-module natives (`List.`), or an instance/`this` receiver's declared-type class members + inherited (via a repaired parse of the mid-edit buffer).
//!   * otherwise — top-level symbols (this file + open sibling buffers) + enclosing locals + keywords.
//!
//! Type-aware member completion is DECLARED-type only (params, `Type x` locals, fields, ctor-promoted
//! params, `this`); an inferred `var x = …` receiver or a method-chain resolves to nothing — the
//! conservative gate (a wrong member list is worse than none). Prelude-class members (Date/Uri…) need
//! the injected prelude program, a documented follow-up.
use super::catalog;
use super::keywords::KEYWORDS;
use crate::ast::Program;
use crate::parser::Parser;
use crate::tokenizer::lex;
use std::collections::HashMap;

const EMPTY: &str = "{\"isIncomplete\":false,\"items\":[]}";

/// A `CompletionItem` JSON object: a label, an LSP `CompletionItemKind`, and a short detail string.
/// (Moved here from `mod.rs` — a completion concern, and it keeps `mod.rs` under its Invariant-13 cap.)
fn completion_item(label: &str, kind: u32, detail: &str) -> String {
    format!(
        "{{\"label\":\"{}\",\"kind\":{kind},\"detail\":\"{}\"}}",
        super::escape(label),
        super::escape(detail)
    )
}

/// The completion context inferred from the text immediately before the cursor.
enum Ctx {
    /// Completing an import path: the partial dotted text after `import `.
    Import(String),
    /// Completing a member after `<receiver>.`: the receiver ident before the dot — a Core-module
    /// qualifier (`List`) or an instance/`this` (resolved to its declared class in `complete`).
    Member(String),
    /// No special context — general symbol/keyword completion.
    General,
}

/// Build the completion response for a cursor at byte `offset` in `text`. `program` is the parsed
/// buffer *when it happened to parse* — best-effort, never required (completion runs on broken input).
pub(super) fn complete(
    text: &str,
    offset: usize,
    program: Option<&Program>,
    uri: Option<&str>,
    docs: &HashMap<String, String>,
) -> String {
    let items: Vec<String> = match context(text, offset) {
        Ctx::Import(prefix) => {
            // Core modules (from the registry) + the user's own project packages (from the loader's
            // project scan of src/vendor/entry-local/views) — the single-source-of-truth enumeration
            // for `import X.`. Project scan runs only for `file://` URIs; anything else degrades to Core.
            let mut items: Vec<String> = catalog::core_module_paths()
                .into_iter()
                .filter(|p| prefix.is_empty() || p.starts_with(&prefix))
                .map(|p| completion_item(&p, 9 /* Module */, "core module"))
                .collect();
            let project_pkgs = uri
                .and_then(uri_to_path)
                .map(|p| crate::loader::project_packages(&p))
                .unwrap_or_default();
            for p in &project_pkgs {
                if prefix.is_empty() || p.starts_with(prefix.as_str()) {
                    items.push(completion_item(p, 9 /* Module */, "project package"));
                }
            }
            items
        }
        Ctx::Member(recv) => {
            // Core-module qualifier first (`List.`/`Output.` → native members).
            let mods = catalog::module_members(&recv);
            if !mods.is_empty() {
                mods.into_iter()
                    .map(|m| completion_item(&m, 3 /* Function */, "member"))
                    .collect()
            } else {
                // Instance receiver (`this.`/`myVar.`): resolve its declared type → the class's members
                // (+ inherited). The LIVE buffer usually does NOT parse (the trailing `receiver.` is a
                // syntax error), so fall back to a repaired parse that blanks the cursor's line — the
                // receiver's declaration lives on other lines and survives. Emits nothing for
                // untyped/inferred receivers (the conservative gate).
                let repaired = match program {
                    Some(_) => None,
                    None => parse_repaired(text, offset),
                };
                let prog = program.or(repaired.as_ref());
                match prog.and_then(|p| {
                    super::scope::receiver_type_name(p, offset, &recv).map(|ty| (p, ty))
                }) {
                    Some((p, ty)) => catalog::class_members(p, &ty)
                        .into_iter()
                        .map(|(m, kind)| completion_item(&m, kind, "member"))
                        .collect(),
                    None => Vec::new(),
                }
            }
        }
        Ctx::General => general_items(text, offset, program, docs, uri),
    };
    if items.is_empty() {
        return EMPTY.to_string();
    }
    format!("{{\"isIncomplete\":false,\"items\":[{}]}}", items.join(","))
}

/// Infer the completion context from the text preceding `offset`. Purely lexical (no parse) so it is
/// robust to the incomplete buffers completion always sees.
fn context(text: &str, offset: usize) -> Ctx {
    let end = offset.min(text.len());
    let before = &text[..end];

    // `import <partial>` — the current line, trimmed, begins with `import ` and the remainder is a
    // dotted path fragment (letters/digits/`_`/`.`). Handles `import ` (empty) through `import Core.J`.
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = before[line_start..].trim_start();
    if let Some(rest) = line.strip_prefix("import ") {
        if rest
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_')
        {
            return Ctx::Import(rest.trim().to_string());
        }
    }

    // `<receiver>.<partial-member>` immediately before the cursor: scan back over the partial member
    // (ident chars), require a `.`, then read the receiver ident. The receiver may be a Core-module
    // qualifier (`List`/`Output` → module natives) OR an instance / `this` (→ the declared class's
    // members); `complete` disambiguates. A receiver itself preceded by `.` (a dotted chain like
    // `Core.List.` or `a.b.`) is skipped — not a single typeable receiver.
    let b = before.as_bytes();
    let mut i = b.len();
    while i > 0 && is_ident_byte(b[i - 1]) {
        i -= 1;
    }
    if i > 0 && b[i - 1] == b'.' {
        let dot = i - 1;
        let mut j = dot;
        while j > 0 && is_ident_byte(b[j - 1]) {
            j -= 1;
        }
        let qual = &before[j..dot];
        let in_dotted_chain = j > 0 && b[j - 1] == b'.';
        if !qual.is_empty() && !in_dotted_chain {
            return Ctx::Member(qual.to_string());
        }
    }

    Ctx::General
}

/// General completion: top-level declarations, the enclosing callable's in-scope locals/params, the
/// buffer's imported module qualifiers (`import Core.Output;` → `Output`), and the language keywords.
/// The buffer almost never parses at this moment — a half-typed identifier IS a parse error — so when
/// the live parse failed, retry on a repaired buffer (cursor line blanked, same trick as member
/// completion). Before this, an unparseable buffer dropped every symbol/local and the user saw ONLY
/// keywords — i.e. "no autocomplete" for the most common action, typing a name.
fn general_items(
    text: &str,
    offset: usize,
    program: Option<&Program>,
    docs: &HashMap<String, String>,
    exclude_uri: Option<&str>,
) -> Vec<String> {
    let repaired = match program {
        Some(_) => None,
        None => parse_repaired(text, offset),
    };
    let program = program.or(repaired.as_ref());
    let mut items: Vec<String> = Vec::new();
    let mut seen_top: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(prog) = program {
        for (name, kind) in super::symbols::top_level_symbols(prog) {
            if seen_top.insert(name.clone()) {
                items.push(completion_item(&name, kind, "phorj symbol"));
            }
        }
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (name, _span) in super::scope::enclosing_bindings(prog, offset) {
            if seen.insert(name.clone()) {
                items.push(completion_item(&name, 6 /* Variable */, "local"));
            }
        }
    }
    // Imported module qualifiers (`import Core.Output;` → `Output`) — the receiver names the user
    // types before a `.`. Lexical scan (never needs a parse) so they survive any mid-edit state.
    for q in imported_qualifiers(text) {
        if seen_top.insert(q.clone()) {
            items.push(completion_item(&q, 9 /* Module */, "imported module"));
        }
    }
    // Top-level symbols from the OTHER open project buffers (same-project siblings) — bounded to open
    // files (no disk scan → perf-safe), so a function/class defined in another open file completes too.
    // Sorted-uri iteration keeps the output deterministic (Invariant 10). Whole-project (unopened-file)
    // symbol completion needs a cached index to stay perf-safe and is a documented follow-up.
    let mut uris: Vec<&String> = docs.keys().collect();
    uris.sort();
    for uri in uris {
        if Some(uri.as_str()) == exclude_uri {
            continue;
        }
        if let Some(p) = lex(&docs[uri])
            .ok()
            .and_then(|t| Parser::new(t).parse_program().ok())
        {
            for (name, kind) in super::symbols::top_level_symbols(&p) {
                if seen_top.insert(name.clone()) {
                    items.push(completion_item(&name, kind, "project symbol"));
                }
            }
        }
    }
    for kw in KEYWORDS {
        items.push(completion_item(kw, 14 /* Keyword */, "keyword"));
    }
    items
}

/// A byte that may appear inside an identifier (ASCII letters/digits/`_`). Byte-level is safe: a
/// multibyte UTF-8 continuation byte is never one of these, so we never split a codepoint.
fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The module qualifiers this buffer imports (`import Core.Output;` → `Output`), by lexical line scan
/// — parse-free so half-typed buffers still surface them. Sorted + deduped (Invariant 10).
fn imported_qualifiers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("import "))
        .filter_map(|rest| {
            let path = rest.trim_end().trim_end_matches(';').trim();
            (!path.is_empty()
                && path
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '_'))
            .then(|| path.rsplit('.').next().unwrap_or(path).to_string())
        })
        .filter(|q| !q.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Parse a completion buffer whose cursor line is a syntax error (the dangling `receiver.`), by
/// **blanking that line** with spaces (byte-length-preserving, so scope/offset lookups still align)
/// and parsing the rest. The receiver's declaration (its `var`/param/field, and the enclosing class)
/// lives on other lines and survives, which is all instance member completion needs. `None` if even
/// the repaired buffer doesn't lex/parse.
fn parse_repaired(text: &str, offset: usize) -> Option<Program> {
    let end = offset.min(text.len());
    let line_start = text[..end].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[line_start..]
        .find('\n')
        .map(|i| line_start + i)
        .unwrap_or(text.len());
    let mut buf = String::with_capacity(text.len());
    buf.push_str(&text[..line_start]);
    buf.push_str(&" ".repeat(line_end - line_start));
    buf.push_str(&text[line_end..]);
    let tokens = lex(&buf).ok()?;
    Parser::new(tokens).parse_program().ok()
}

/// Convert a `file://` document URI to an on-disk path — the SAME minimal handling `diagnostics_for_uri`
/// uses (strip the scheme; no percent-decoding, matching the existing code). `None` for a non-file URI
/// or a path that is not a real file, so the project scan simply doesn't apply (untitled/virtual buffers).
fn uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri));
    p.is_file().then_some(p)
}

#[cfg(test)]
mod tests;
