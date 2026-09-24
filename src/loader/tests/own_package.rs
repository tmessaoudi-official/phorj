//! DEC-525 (scout row 5e) — a file inside a package folder loads its WHOLE package, plus what that
//! package imports, as an entry reaching the package would. Before, a library file loaded only what
//! it imported, so `phg check src/Acme/Core/Classification.phg` reported its package-mate `Tenure`
//! unknown — and so did the editor, which loads through the same `load_with_buffer` seam.

use super::*;

const TENURE: &str = "package Acme.Core;\n\npublic enum Tenure {\n    Short,\n    Long,\n}\n";
const CLASSIFICATION: &str = "package Acme.Core;\n\npublic class Classification {\n    \
    constructor(public Tenure tenure) {}\n    \
    public static function long(): Classification {\n        return new Classification(new Long());\n    }\n}\n";

fn checks(u: &Unit) -> Result<String, String> {
    crate::cli::check_program(&u.program, &u.diag_src)
}

#[test]
fn a_library_file_checks_with_its_package_mates() {
    let tmp = TempDir::new();
    tmp.write("src/Acme/Core/Tenure.phg", TENURE);
    let file = tmp.write("src/Acme/Core/Classification.phg", CLASSIFICATION);
    let u = load(&file).expect("loads");
    checks(&u).expect("the package-mate `Tenure` resolves");
    // The same view through the editor seam (DEC-252), with an unsaved buffer.
    let u = load_with_buffer(&file, CLASSIFICATION).expect("loads with a buffer");
    checks(&u).expect("the editor sees the package-mate too");
}

/// "plus its imports": a package-mate's own import is followed, as it would be from an entry.
#[test]
fn a_package_mates_imports_are_loaded_too() {
    let tmp = TempDir::new();
    tmp.write(
        "src/Acme/Support/Label.phg",
        "package Acme.Support;\n\npublic class Label {\n    constructor(public string text) {}\n}\n",
    );
    tmp.write(
        "src/Acme/Core/Tagged.phg",
        "package Acme.Core;\n\nimport Acme.Support.Label;\n\npublic class Tagged {\n    \
         constructor(public Label label) {}\n}\n",
    );
    let file = tmp.write(
        "src/Acme/Core/Uses.phg",
        "package Acme.Core;\n\npublic class Uses {\n    constructor(public Tagged t) {}\n}\n",
    );
    let u = load(&file).expect("loads");
    checks(&u).expect("Tagged and, through it, Label resolve");
}

/// `package Main` is never seeded: a directory of standalone `Main` scripts (every `examples/*.phg`)
/// must not merge into one program. `index_packages` drops `Main` already; this pins the outcome.
#[test]
fn a_main_file_does_not_load_the_other_main_files_beside_it() {
    let tmp = TempDir::new();
    let a = tmp.write(
        "a.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {}\nfunction onlyInA() -> void {}",
    );
    tmp.write(
        "b.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {}\nfunction onlyInB() -> void {}",
    );
    let u = load(&a).expect("loads");
    let names: Vec<String> = u
        .program
        .items
        .iter()
        .filter_map(|i| match i {
            crate::ast::Item::Function(f) => Some(f.name.clone()),
            _ => None,
        })
        .collect();
    assert!(names.iter().any(|n| n == "onlyInA"), "{names:?}");
    assert!(!names.iter().any(|n| n == "onlyInB"), "{names:?}");
}

/// A dotted package whose folder does not END in the package path (`examples/lift/namespaces.phg`
/// declares `App.CliTools` from a folder called `lift`) has no package root to seed from, so it
/// loads exactly as before — the self-contained fast path, no folder validation.
#[test]
fn a_package_whose_folder_does_not_match_loads_as_before() {
    let tmp = TempDir::new();
    tmp.write("src/.keep.phg", "package Main;\n");
    let file = tmp.write(
        "lift/one.phg",
        "package App.CliTools;\n\nfunction shout(string s): string {\n    return s + \"!\";\n}\n",
    );
    tmp.write(
        "lift/two.phg",
        "package App.CliTools;\n\nfunction other(): string {\n    return \"x\";\n}\n",
    );
    let u = load(&file).expect("loads as before");
    assert!(u.stats.is_none(), "still the self-contained fast path");
}

/// With no `src/` or `vendor/` there is no source root the package folder can sit under, and the
/// folder rule would reject the entry's own directory as one. That layout keeps today's behaviour
/// (disclosed in KNOWN_ISSUES) rather than trading "unknown type" for a misleading `E-PKG-PATH`.
#[test]
fn a_package_with_no_source_root_loads_as_before() {
    let tmp = TempDir::new();
    tmp.write("lib/Acme/Core/Tenure.phg", TENURE);
    let file = tmp.write("lib/Acme/Core/Classification.phg", CLASSIFICATION);
    let u = load(&file).expect("loads as before");
    assert!(u.stats.is_none(), "still the self-contained fast path");
}
