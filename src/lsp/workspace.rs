//! `workspace/symbol` and `textDocument/foldingRange` — two capabilities every LSP client offers a
//! key for (Ctrl+T "go to symbol in workspace"; the gutter fold arrows) and that the server did not
//! advertise. Both are pure front-end, off the byte-identity spine, and both reuse machinery that
//! already exists: DEC-327's project-file scan and `document_symbols_json`'s item ranges.
//!
//! `workspace/symbol` answers over the PROJECT, not just the open buffers: every `.phg` the loader
//! would see from the first open document's app root (`loader::project_phg_files`) plus any other
//! open buffer, sorted by URI so the result is deterministic (Invariant 10). Matching is a
//! case-insensitive substring of the query, which is what every client's fuzzy picker degrades to
//! when the server does the filtering; an empty query lists everything.
//!
//! `foldingRange` folds each top-level item from its own start line to the line before the next
//! item — the same span `document_symbols_json` reports as the symbol's range — and each class
//! member likewise. Line-based, so a client that folds by line (all of them) gets exactly the
//! declaration bodies.

use super::{scope, uri_to_path, Server};
use crate::ast::{ClassMember, Item};
use crate::json::Json;
use crate::parser::Parser;
use crate::tokenizer::lex;

/// LSP `SymbolKind` for a top-level item — the SAME numbers `document_symbols_json` uses, so a
/// symbol found by the workspace picker carries the icon its outline entry has.
fn kind_of(item: &Item) -> Option<(u32, &str)> {
    match item {
        Item::Function(f) => Some((12, &f.name)),
        Item::Class(c) => Some((5, &c.name)),
        Item::Interface(i) => Some((11, &i.name)),
        Item::Trait(t) => Some((23, &t.name)),
        Item::Enum(e) => Some((10, &e.name)),
        Item::TypeAlias { name, .. } => Some((26, name)),
        _ => None,
    }
}

/// The byte a class/trait member's declaration starts at — the same anchor `class_children` uses
/// for the outline, so a fold and its outline entry begin on the same line.
fn member_start(m: &ClassMember) -> usize {
    match m {
        ClassMember::Field { span, .. }
        | ClassMember::Constructor { span, .. }
        | ClassMember::Hook { span, .. } => span.start,
        ClassMember::Method(f) => f.span.start,
    }
}

impl Server {
    /// `workspace/symbol` — `SymbolInformation[]` for every top-level declaration in the project
    /// whose name contains `params.query` (case-insensitive); `[]` when nothing is open.
    pub(super) fn workspace_symbols(&self, msg: &Json) -> String {
        let query = msg
            .get("params")
            .and_then(|p| p.get("query"))
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_lowercase();
        // The project root is discovered from the first open document (sorted, so it is stable);
        // with nothing open there is no project to scan.
        let mut uris: Vec<String> = self.docs.keys().cloned().collect();
        uris.sort();
        let Some(first) = uris.first() else {
            return "[]".to_string();
        };
        let mut files: Vec<String> = uri_to_path(first)
            .map(|p| crate::loader::project_phg_files(&p))
            .unwrap_or_default()
            .into_iter()
            .map(|p| format!("file://{}", p.display()))
            .collect();
        for u in &uris {
            if !files.contains(u) {
                files.push(u.clone());
            }
        }
        files.sort();
        files.dedup();
        let mut out = Vec::new();
        for furi in files {
            let text = match self.docs.get(&furi) {
                Some(t) => t.clone(),
                None => match uri_to_path(&furi).and_then(|p| std::fs::read_to_string(p).ok()) {
                    Some(t) => t,
                    None => continue,
                },
            };
            let Some(program) = lex(&text)
                .ok()
                .and_then(|t| Parser::new(t).parse_program().ok())
            else {
                continue;
            };
            for it in &program.items {
                let Some((kind, name)) = kind_of(it) else {
                    continue;
                };
                if !query.is_empty() && !name.to_lowercase().contains(&query) {
                    continue;
                }
                let sp = scope::item_span(it);
                let (sl, sc) = scope::position_at(&text, sp.start);
                let (el, ec) = scope::position_at(&text, sp.start + sp.len);
                out.push(format!(
                    "{{\"name\":\"{}\",\"kind\":{kind},\"location\":{{\"uri\":\"{}\",\"range\":{{\"start\":{{\"line\":{sl},\"character\":{sc}}},\"end\":{{\"line\":{el},\"character\":{ec}}}}}}}}}",
                    super::escape(name),
                    super::escape(&furi)
                ));
            }
        }
        format!("[{}]", out.join(","))
    }

    /// `textDocument/foldingRange` — one `FoldingRange` per top-level item and per class/trait
    /// member, from the declaration's start line to the line before the next declaration starts
    /// (the last one runs to the end of the buffer). Single-line declarations fold nothing and are
    /// skipped, as the LSP spec expects.
    pub(super) fn folding_ranges(&self, msg: &Json) -> String {
        let Some(uri) = super::doc_uri(msg) else {
            return "[]".to_string();
        };
        let Some(text) = self.docs.get(&uri) else {
            return "[]".to_string();
        };
        let Some(program) = lex(text)
            .ok()
            .and_then(|t| Parser::new(t).parse_program().ok())
        else {
            return "[]".to_string();
        };
        let mut out = Vec::new();
        let items = &program.items;
        let bytes = text.as_bytes();
        let mut push = |start: usize, end: usize| {
            let (sl, _) = scope::position_at(text, start);
            // `end` is exclusive — the byte the NEXT declaration (or the enclosing `}`) starts at.
            // Walk back over the whitespace before it so the fold ends on the declaration's own
            // closing brace, not on the blank line or indentation that precedes its successor.
            let mut e = end.min(text.len());
            while e > start && bytes[e - 1].is_ascii_whitespace() {
                e -= 1;
            }
            let (el, _) = scope::position_at(text, e.saturating_sub(1).max(start));
            if el > sl {
                out.push(format!("{{\"startLine\":{sl},\"endLine\":{el}}}"));
            }
        };
        for (i, it) in items.iter().enumerate() {
            let start = scope::item_span(it).start;
            let end = items
                .get(i + 1)
                .map_or(text.len(), |n| scope::item_span(n).start);
            push(start, end);
            let members = match it {
                Item::Class(c) => Some(&c.members),
                Item::Trait(t) => Some(&t.members),
                _ => None,
            };
            if let Some(members) = members {
                let spans: Vec<usize> = members.iter().map(member_start).collect();
                // The LAST member ends before the class's own closing brace — the last `}` before
                // the next item — not at the next item, or its fold would swallow that brace.
                let close = text[..end].rfind('}').unwrap_or(end);
                for (j, s) in spans.iter().enumerate() {
                    let e = spans.get(j + 1).copied().unwrap_or(close);
                    push(*s, e);
                }
            }
        }
        format!("[{}]", out.join(","))
    }
}
