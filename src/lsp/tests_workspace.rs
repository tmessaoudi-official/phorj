//! `workspace/symbol` + `textDocument/foldingRange` — end to end through `Server::handle`.
use super::tests::{did_open, PROG};
use super::Server;
use crate::json::Json;

#[test]
fn initialize_advertises_folding_and_workspace_symbols() {
    let mut s = Server::default();
    let out = s.handle(&Json::parse(r#"{"id":1,"method":"initialize"}"#).unwrap());
    assert!(
        out[0].contains("\"foldingRangeProvider\":true"),
        "{}",
        out[0]
    );
    assert!(
        out[0].contains("\"workspaceSymbolProvider\":true"),
        "{}",
        out[0]
    );
}

#[test]
fn workspace_symbols_list_and_filter_across_open_buffers_and_disk() {
    let dir = std::env::temp_dir().join(format!("phorj-lsp-ws-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.phg");
    let entry_src = "package Main;\nfunction helper(int n): int { return n; }\nclass Widget { constructor() {} }\nfunction main(): void { }\n";
    std::fs::write(&entry, entry_src).unwrap();
    std::fs::write(
        dir.join("more.phg"),
        "package Main;\nenum Mode { Fast, Slow }\nfunction helpMe(): int { return 1; }\n",
    )
    .unwrap();
    let mut s = Server::default();
    let uri = format!("file://{}", entry.display());
    s.handle(&did_open(&uri, entry_src));
    let ask = |s: &mut Server, q: &str| {
        s.handle(
            &Json::parse(&format!(
                r#"{{"id":9,"method":"workspace/symbol","params":{{"query":"{q}"}}}}"#
            ))
            .unwrap(),
        )[0]
        .clone()
    };
    // Case-insensitive substring: `help` matches `helper` (open buffer) AND `helpMe` (disk sibling).
    let out = ask(&mut s, "help");
    assert!(out.contains("\"name\":\"helper\""), "{out}");
    assert!(out.contains("\"name\":\"helpMe\""), "{out}");
    assert!(out.contains("more.phg"), "disk sibling missing: {out}");
    assert!(!out.contains("\"name\":\"Widget\""), "{out}");
    // Kinds follow the outline's numbering: class 5, enum 10, function 12.
    let all = ask(&mut s, "");
    assert!(all.contains("\"name\":\"Widget\",\"kind\":5"), "{all}");
    assert!(all.contains("\"name\":\"Mode\",\"kind\":10"), "{all}");
    assert!(all.contains("\"name\":\"main\",\"kind\":12"), "{all}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspace_symbols_with_nothing_open_is_empty() {
    let mut s = Server::default();
    let out = s.handle(
        &Json::parse(r#"{"id":9,"method":"workspace/symbol","params":{"query":"x"}}"#).unwrap(),
    );
    assert!(out[0].contains("\"result\":[]"), "{}", out[0]);
}

#[test]
fn folding_ranges_cover_each_multi_line_declaration_and_its_members() {
    let mut s = Server::default();
    let src = "package Main;\nclass C {\n    constructor() {}\n    function m(): int {\n        return 1;\n    }\n}\nfunction main(): void {\n    var x = 1;\n}\n";
    s.handle(&did_open("file:///f.phg", src));
    let out = s.handle(
        &Json::parse(r#"{"id":3,"method":"textDocument/foldingRange","params":{"textDocument":{"uri":"file:///f.phg"}}}"#).unwrap(),
    );
    let body = &out[0];
    // The class: lines 1..6 (its body), the method inside it: 3..5, main: 7..9.
    assert!(body.contains("{\"startLine\":1,\"endLine\":6}"), "{body}");
    assert!(body.contains("{\"startLine\":3,\"endLine\":5}"), "{body}");
    assert!(body.contains("{\"startLine\":7,\"endLine\":9}"), "{body}");
    // The one-line `constructor() {}` on line 2 folds NOTHING: before the whitespace trim, the
    // indentation preceding the next member gave it a phantom `{2,3}` range.
    assert!(!body.contains("\"startLine\":2,"), "{body}");
    // PROG's one-line declarations fold nothing.
    s.handle(&did_open("file:///x.phg", PROG));
    let out = s.handle(
        &Json::parse(r#"{"id":4,"method":"textDocument/foldingRange","params":{"textDocument":{"uri":"file:///x.phg"}}}"#).unwrap(),
    );
    assert!(out[0].contains("\"result\":[]"), "{}", out[0]);
}
