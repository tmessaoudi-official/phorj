//! Doc-comment extraction (DEC-419) — THE one reader of `/** … */` documentation.
//!
//! Three consumers, one rule: the LSP (hover + completion `documentation`), and the TRANSPILER, which
//! re-emits the doc as a PHP docblock. Living at crate level rather than under `lsp/` is what keeps
//! those from drifting — a second extractor would let hover and the emitted PHP disagree about what a
//! declaration's documentation even is.
//!
//! A `/** … */` comment immediately above a declaration IS that declaration's documentation. This
//! module finds it in the buffer text and renders it as markdown.
//!
//! **Why text-scanning and not the AST.** Comments are not AST nodes here — the tokenizer collects
//! them into a side channel keyed by span (that is what the formatter consumes), so attaching them to
//! every declaration would mean a field on `Function`/`Class`/`Enum`/`Trait`/`Interface`/`TypeAlias`
//! plus every construction site, and the backends would have to carry a field they can never use.
//! Hover already holds the buffer text and the declaration's span, so the doc is recoverable from what
//! is already in hand, and the byte-identity spine is untouched by construction.
//!
//! The "is this a doc comment" test is NOT re-implemented here — it calls
//! [`crate::token::opens_doc_comment`], the same predicate the tokenizer uses to pick
//! [`crate::token::CommentKind::Doc`]. Two spellings would drift, and the drift would be invisible:
//! highlighted as documentation by the editor while hover showed nothing.

/// The rendered markdown of the doc comment attached to the declaration whose span starts at
/// `decl_start`, or `None` when there is none.
///
/// "Attached" means: reading upwards from the declaration's own line, skipping ATTRIBUTE lines
/// (`#[Entry(…)]` sits between the doc and the declaration in real code) and blank lines, the next
/// thing is a `/** … */` block. A plain `/* … */` or `// …` above a declaration is deliberately NOT
/// documentation — that distinction is the whole point of the ruling.
pub(crate) fn doc_markdown_before(text: &str, decl_start: usize) -> Option<String> {
    let bytes = text.as_bytes();
    // Walk up from the declaration's line to the first line that is neither blank nor an attribute.
    let mut line_start = line_start_of(bytes, decl_start.min(bytes.len()));
    loop {
        if line_start == 0 {
            return None;
        }
        let prev_start = line_start_of(bytes, line_start - 1);
        let prev = text.get(prev_start..line_start.saturating_sub(1))?.trim();
        if prev.is_empty() || prev.starts_with("#[") {
            line_start = prev_start;
            continue;
        }
        // This is the candidate: it must CLOSE a block comment for a doc comment to end here.
        if !prev.ends_with("*/") {
            return None;
        }
        let close = prev_start
            + text
                .get(prev_start..line_start.saturating_sub(1))?
                .rfind("*/")?;
        let open = find_comment_open(bytes, close)?;
        if !crate::token::opens_doc_comment(bytes, open) {
            return None; // a plain `/* … */` — a note, not documentation
        }
        return Some(clean(text.get(open + 3..close)?));
    }
}

/// The byte offset of the `/*` that opens the block comment closed by the `*/` at `close`.
///
/// Scans backwards for the nearest `/*`. Block comments do not nest in phorj (the tokenizer's
/// `skip_block_comment` returns at the FIRST `*/`), so the nearest preceding `/*` is the opener —
/// matching the tokenizer's own behaviour rather than inventing a nesting rule it does not have.
fn find_comment_open(bytes: &[u8], close: usize) -> Option<usize> {
    let mut i = close;
    while i > 0 {
        i -= 1;
        if bytes.get(i) == Some(&b'/') && bytes.get(i + 1) == Some(&b'*') {
            return Some(i);
        }
    }
    None
}

/// Offset of the start of the line containing `at`.
fn line_start_of(bytes: &[u8], at: usize) -> usize {
    let mut i = at.min(bytes.len());
    while i > 0 && bytes[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

/// Strip the PHPDoc furniture: the leading `*` on continuation lines, and blank leading/trailing lines.
/// The body is otherwise passed through UNCHANGED — it is markdown, and rewriting a user's markdown is
/// not this function's business.
fn clean(body: &str) -> String {
    let mut lines: Vec<String> = body
        .lines()
        .map(|l| {
            let t = l.trim_start();
            // `* text` → `text`; a bare `*` → empty. A line NOT starting with `*` is kept verbatim
            // (trimmed), so a doc comment written without the star column still reads correctly.
            match t.strip_prefix('*') {
                Some(rest) => rest
                    .strip_prefix(' ')
                    .unwrap_or(rest)
                    .trim_end()
                    .to_string(),
                None => t.trim_end().to_string(),
            }
        })
        .collect();
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::doc_markdown_before;

    /// Offset of `name` in `src` — the declaration span hover works from points at the name token.
    fn at(src: &str, name: &str) -> usize {
        src.find(name).expect("fixture contains the name")
    }

    #[test]
    fn a_doc_comment_directly_above_a_declaration_is_attached() {
        let src =
            "package Main;\n/** Adds two numbers. */\nfunction add(int a): int { return a; }\n";
        assert_eq!(
            doc_markdown_before(src, at(src, "function add")),
            Some("Adds two numbers.".to_string())
        );
    }

    #[test]
    fn the_star_column_and_blank_edges_are_stripped() {
        let src =
            "package Main;\n/**\n * First line.\n *\n * Second line.\n */\nfunction f(): void {}\n";
        assert_eq!(
            doc_markdown_before(src, at(src, "function f")),
            Some("First line.\n\nSecond line.".to_string())
        );
    }

    /// Attributes sit BETWEEN the doc comment and the declaration in real code (`#[Entry(…)]`), so the
    /// upward walk has to step over them — otherwise every documented entry point loses its docs.
    #[test]
    fn attribute_lines_between_the_doc_and_the_declaration_are_skipped() {
        let src = "package Main;\n/** The entry point. */\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {}\n";
        assert_eq!(
            doc_markdown_before(src, at(src, "function main")),
            Some("The entry point.".to_string())
        );
    }

    /// THE distinction the ruling exists for: a plain block comment is a note to the next reader, not
    /// documentation. If this ever returns `Some`, `/**` has stopped meaning anything.
    #[test]
    fn a_plain_block_comment_is_not_documentation() {
        let src = "package Main;\n/* just a note */\nfunction f(): void {}\n";
        assert_eq!(doc_markdown_before(src, at(src, "function f")), None);
    }

    #[test]
    fn a_line_comment_is_not_documentation() {
        let src = "package Main;\n// just a note\nfunction f(): void {}\n";
        assert_eq!(doc_markdown_before(src, at(src, "function f")), None);
    }

    /// `/**/` is an EMPTY BLOCK comment, not a doc comment — the corner `opens_doc_comment` excludes.
    /// Getting this wrong would attach an empty doc to whatever followed it.
    #[test]
    fn an_empty_block_comment_is_not_documentation() {
        let src = "package Main;\n/**/\nfunction f(): void {}\n";
        assert_eq!(doc_markdown_before(src, at(src, "function f")), None);
    }

    #[test]
    fn no_comment_at_all_yields_none() {
        let src = "package Main;\nfunction f(): void {}\n";
        assert_eq!(doc_markdown_before(src, at(src, "function f")), None);
    }

    /// A doc comment separated from the declaration by a blank line still documents it (the blank is
    /// skipped like an attribute line) — but a doc comment for a DIFFERENT, earlier declaration must
    /// not leak onto this one.
    #[test]
    fn an_earlier_declarations_doc_does_not_leak_onto_the_next() {
        let src =
            "package Main;\n/** Belongs to f. */\nfunction f(): void {}\nfunction g(): void {}\n";
        assert_eq!(
            doc_markdown_before(src, at(src, "function f")),
            Some("Belongs to f.".to_string())
        );
        assert_eq!(doc_markdown_before(src, at(src, "function g")), None);
    }
}
