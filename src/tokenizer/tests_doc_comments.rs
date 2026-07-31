//! Doc-comment CLASSIFICATION tests (DEC-419) — a sibling of `tests.rs` rather than an addition to it,
//! because that file is a grandfathered Invariant-13 breach and the size gate fails when it grows.

use crate::token::CommentKind;

/// DEC-419: `/** … */` lexes as [`CommentKind::Doc`], `/* … */` as `Block`, and `/**/` as `Block` —
/// the empty-block corner. Classification is the tokenizer's half of the doc-comment feature; the LSP
/// reads the same rule via `token::opens_doc_comment`, so this test pins the shared contract.
#[test]
fn doc_comments_lex_as_a_distinct_kind() {
    let src = "package Main;\n/** doc */\n/* plain */\n/**/\n/*** odd */\nfunction f(): void {}\n";
    let (_toks, comments) = crate::tokenizer::lex_with_comments(src).expect("lexes");
    let kinds: Vec<(&str, CommentKind)> =
        comments.iter().map(|c| (c.text.as_str(), c.kind)).collect();
    assert_eq!(
        kinds,
        vec![
            ("/** doc */", CommentKind::Doc),
            ("/* plain */", CommentKind::Block),
            // `/**/` is `/*` + `*/` — an EMPTY block comment, not an unterminated doc comment.
            ("/**/", CommentKind::Block),
            // `/***/`-style: `/**` not followed by `/`, so it IS a doc comment (body `* odd`). A corner
            // recorded as a decision rather than left to be discovered.
            ("/*** odd */", CommentKind::Doc),
        ],
        "got {kinds:?}"
    );
}
