//! `E-AMBIGUOUS-ATTRIBUTE` — a user attribute written by its leaf (`#[Marker]`) while TWO imported
//! packages each declare an `#[Attribute] class Marker`. `attr_path_matches` accepts any suffix on a
//! segment boundary, so both canonical paths match and the checker must refuse rather than pick one.
//! Needs two packages, hence a project fixture rather than a one-file checker test. The second
//! package is imported as a PACKAGE (`import Acme.Two;`), not by leaf — importing both leaves is
//! already `E-IMPORT-CONFLICT` at the loader, one layer earlier; the attribute check sees every
//! loaded user-attribute class regardless of how its name was bound. `#[Two.Marker]` on a second
//! function gives the package import its use (`E-UNUSED-IMPORT` otherwise, also earlier) and is
//! itself unambiguous; only the bare `#[Marker]` matches both canonical paths.
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use phorj::{cli, loader};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let unique = N.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("phorj_attr_ambig_{}_{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, contents).unwrap();
        p
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_attribute_leaf_matching_two_imported_packages_is_ambiguous() {
    let tmp = TempDir::new();
    tmp.write(
        "src/Acme/One/Marker.phg",
        "package Acme.One;\nimport Core.Runtime.Attribute;\n#[Attribute]\npublic class Marker { constructor() {} }",
    );
    tmp.write(
        "src/Acme/Two/Marker.phg",
        "package Acme.Two;\nimport Core.Runtime.Attribute;\n#[Attribute]\npublic class Marker { constructor() {} }",
    );
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.One.Marker;\nimport Acme.Two;\n\
         #[Two.Marker]\nfunction qualified() -> int { return 2; }\n\
         #[Marker]\nfunction tagged() -> int { return 1; }\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { }",
    );
    let diags = match loader::load(&entry) {
        Ok(unit) => cli::front_end_diagnostics(&unit.program),
        Err(msg) => vec![phorj::diagnostic::Diagnostic::new(
            phorj::diagnostic::Stage::Type,
            msg,
            1,
            1,
        )],
    };
    assert!(
        diags
            .iter()
            .any(|d| d.code == Some("E-AMBIGUOUS-ATTRIBUTE")),
        "expected E-AMBIGUOUS-ATTRIBUTE, got {diags:?}"
    );
}
