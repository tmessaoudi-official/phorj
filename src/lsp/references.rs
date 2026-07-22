//! DEC-327 — the project-wide references leg (split from `mod.rs` per Invariant 13): scan every
//! project `.phg` file (disk + other open buffers) for occurrences of a TOP-LEVEL symbol that are
//! not shadowed by a local in their own file. Deterministic (sorted paths); parse failures skip the
//! file, so a mid-edit buffer never breaks the query.

use super::*;

impl Server {
    /// The DEC-327 cross-file leg (b): scan every project file (disk + other open buffers) for
    /// occurrences of `name` that are NOT shadowed by a local in their own file. Deterministic
    /// (sorted paths); parse failures skip the file (a mid-edit buffer never breaks the query).
    pub(super) fn cross_file_references(&self, current_uri: &str, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        // Project files from the current document's path; non-file URIs degrade to open buffers.
        let mut files: Vec<String> = uri_to_path(current_uri)
            .map(|p| crate::loader::project_phg_files(&p))
            .unwrap_or_default()
            .into_iter()
            .map(|p| format!("file://{}", p.display()))
            .collect();
        for open_uri in self.docs.keys() {
            if !files.contains(open_uri) {
                files.push(open_uri.clone());
            }
        }
        files.sort();
        for furi in files {
            if furi == current_uri {
                continue;
            }
            let text = match self.docs.get(&furi) {
                Some(t) => t.clone(),
                None => match uri_to_path(&furi).and_then(|p| std::fs::read_to_string(p).ok()) {
                    Some(t) => t,
                    None => continue,
                },
            };
            let Some(program) = crate::tokenizer::lex(&text)
                .ok()
                .and_then(|t| crate::parser::Parser::new(t).parse_program().ok())
            else {
                continue;
            };
            for sp in symbols::all_ident_spans(&text, name) {
                // Keep the span unless it resolves to a LOCAL in its own file (a shadow).
                if matches!(
                    Self::resolve_decl(sp.start, name, &program),
                    Some((_, true))
                ) {
                    continue;
                }
                let (sl, sc) = scope::position_at(&text, sp.start);
                let (el, ec) = scope::position_at(&text, sp.start + sp.len);
                out.push(format!(
                    "{{\"uri\":\"{}\",\"range\":{}}}",
                    escape(&furi),
                    range_json(sl, sc, el, ec)
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::lsp::tests::{did_open, req_at, req_at_uri, PROG};
    use crate::lsp::Server;

    #[test]
    fn references_cross_open_buffers_for_top_level_symbols() {
        let mut s = Server::default();
        s.handle(&did_open("file:///x.phg", PROG));
        // A second open buffer using `helper` (same-project sibling; a local `helper` would be excluded).
        s.handle(&did_open(
            "file:///b.phg",
            "package Main;\nfunction other(): int { return helper(2); }\n",
        ));
        let out = s.handle(&req_at("references", 2, 35)); // cursor on the call in x.phg
        let body = &out[0];
        assert!(body.contains("file:///x.phg"), "{body}");
        assert!(
            body.contains("file:///b.phg"),
            "cross-file use missing: {body}"
        );
        assert_eq!(body.matches("\"uri\"").count(), 3, "{body}");
    }

    #[test]
    fn references_reach_unopened_project_files_on_disk() {
        let dir = std::env::temp_dir().join(format!("phorj-lsp-refs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("main.phg");
        let sibling = dir.join("uses.phg");
        let entry_src = "package Main;\nfunction helper(int n): int { return n; }\nfunction main(): void { discard helper(1); }\n";
        std::fs::write(&entry, entry_src).unwrap();
        std::fs::write(
            &sibling,
            "package Main;\nfunction other(): int { return helper(2); }\n",
        )
        .unwrap();
        let mut s = Server::default();
        let uri = format!("file://{}", entry.display());
        s.handle(&did_open(&uri, entry_src));
        // Cursor on the `helper` declaration name (line 1, col 9).
        let out = s.handle(&req_at_uri(&uri, "references", 1, 9));
        let body = &out[0];
        assert!(body.contains("uses.phg"), "disk sibling missing: {body}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn references_for_locals_stay_single_buffer() {
        let mut s = Server::default();
        s.handle(&did_open("file:///x.phg", PROG));
        s.handle(&did_open(
            "file:///c.phg",
            "package Main;\nfunction f(): int { int x = 1; return x; }\n",
        ));
        // Cursor on the local `x` in c.phg — the other buffer must not be scanned.
        let out = s.handle(&req_at_uri("file:///c.phg", "references", 1, 24));
        let body = &out[0];
        assert!(!body.contains("file:///x.phg"), "{body}");
    }
}
