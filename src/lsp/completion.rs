//! Parse-tolerant completion (2026-07-20 alignment pass). Completion is invoked mid-edit, when the
//! buffer almost never parses (`Output.` with nothing after it is a parse error) — so this works from
//! the raw text + cursor offset and treats a successful parse as a best-effort bonus, never a
//! requirement. Before this, `completion()` bailed to `[]` the instant the buffer didn't parse, i.e.
//! exactly while the user was typing a member access. Contexts inferred from the text before the cursor:
//!   * `import Core.` → importable module paths (from the catalog)
//!   * `Qualifier.`   → that Core module's native members (PascalCase qualifier ⇒ a module, not a var)
//!   * otherwise      → top-level symbols (when the buffer parses) + enclosing locals + keywords
//!
//! Instance/type-aware member completion (`myVar.` → the class's methods) needs the checker's resolved
//! type index and is a documented follow-up; the PascalCase-qualifier gate deliberately avoids emitting
//! wrong module members for a lowercase variable receiver.
use super::catalog;
use super::KEYWORDS;
use crate::ast::Program;

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
    /// Completing a member after `Qualifier.`: the PascalCase qualifier before the dot.
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
        Ctx::Member(qual) => catalog::module_members(&qual)
            .into_iter()
            .map(|m| completion_item(&m, 3 /* Function */, "member"))
            .collect(),
        Ctx::General => general_items(offset, program),
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

    // `Qualifier.<partial-member>` immediately before the cursor: scan back over the partial member
    // (ident chars), require a `.`, then read the qualifier ident. A PascalCase qualifier ⇒ a Core
    // module receiver (List/Output/Map); a lowercase receiver is an instance (type-aware — deferred).
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
        if !qual.is_empty() && qual.starts_with(|c: char| c.is_ascii_uppercase()) {
            return Ctx::Member(qual.to_string());
        }
    }

    Ctx::General
}

/// General completion: top-level declarations (when the buffer parsed), the enclosing callable's
/// in-scope locals/params, and the language keywords. Mirrors the pre-2026-07-20 behaviour, but the
/// parse is now optional — keywords are always offered even on a buffer that does not parse.
fn general_items(offset: usize, program: Option<&Program>) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    if let Some(prog) = program {
        for (name, kind) in super::symbols::top_level_symbols(prog) {
            items.push(completion_item(&name, kind, "phorj symbol"));
        }
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (name, _span) in super::scope::enclosing_bindings(prog, offset) {
            if seen.insert(name.clone()) {
                items.push(completion_item(&name, 6 /* Variable */, "local"));
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

/// Convert a `file://` document URI to an on-disk path — the SAME minimal handling `diagnostics_for_uri`
/// uses (strip the scheme; no percent-decoding, matching the existing code). `None` for a non-file URI
/// or a path that is not a real file, so the project scan simply doesn't apply (untitled/virtual buffers).
fn uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri));
    p.is_file().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::complete;

    /// Extract every `"label":"…"` value from a completion response (assert on CONTENT, not just count).
    fn labels(resp: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = resp;
        while let Some(i) = rest.find("\"label\":\"") {
            rest = &rest[i + 9..];
            if let Some(end) = rest.find('"') {
                out.push(rest[..end].to_string());
                rest = &rest[end..];
            }
        }
        out
    }

    // The key regression this slice fixes: completion on an INCOMPLETE buffer (parse fails) must still
    // work — before 2026-07-20 every case below returned `[]` because `symbol_at` required a parse.

    #[test]
    fn import_context_lists_core_modules() {
        let src = "package Main;\nimport Core.\n";
        let offset = src.find("Core.").unwrap() + "Core.".len(); // right after the dot
        let got = labels(&complete(src, offset, None, None));
        assert!(
            got.iter().any(|l| l == "Core.Json"),
            "want Core.Json in {got:?}"
        );
        assert!(
            got.iter().any(|l| l == "Core.Http"),
            "want Core.Http in {got:?}"
        );
        // Raw `Core.Native.*` twins are excluded (users import the friendly module).
        assert!(
            !got.iter().any(|l| l.starts_with("Core.Native.")),
            "raw twins leaked: {got:?}"
        );
    }

    #[test]
    fn import_context_filters_by_prefix() {
        let src = "import Core.J";
        let got = labels(&complete(src, src.len(), None, None));
        assert!(
            got.iter().any(|l| l == "Core.Json"),
            "want Core.Json in {got:?}"
        );
        assert!(
            got.iter().all(|l| l.starts_with("Core.J")),
            "prefix not applied: {got:?}"
        );
    }

    #[test]
    fn member_context_lists_module_natives_on_incomplete_buffer() {
        // `Output.` with nothing after ⇒ the buffer does NOT parse; member completion must still fire.
        let src = "package Main;\nfunction main(): void {\n  Output.\n}\n";
        let offset = src.find("Output.").unwrap() + "Output.".len();
        let got = labels(&complete(src, offset, None, None));
        assert!(
            got.iter().any(|l| l == "printLine"),
            "want printLine in {got:?}"
        );
        assert!(!got.is_empty());
    }

    #[test]
    fn lowercase_receiver_is_not_module_member_completion() {
        // A lowercase receiver is an instance (type-aware completion is deferred), NOT a Core module —
        // must fall to general completion, never emit module members for the wrong qualifier.
        let src = "function main(): void {\n  myvar.\n}\n";
        let offset = src.find("myvar.").unwrap() + "myvar.".len();
        let got = labels(&complete(src, offset, None, None));
        // General context always includes keywords; it must NOT be a members list.
        assert!(
            got.iter().any(|l| l == "function"),
            "want keywords in general ctx: {got:?}"
        );
    }

    #[test]
    fn general_context_offers_keywords_without_a_parse() {
        // Even a buffer that does not parse yields keywords (never a bare `[]`).
        let got = labels(&complete("packag", 6, None, None));
        assert!(
            got.iter().any(|l| l == "package"),
            "want keyword 'package' in {got:?}"
        );
    }
}
