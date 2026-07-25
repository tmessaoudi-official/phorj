use super::*;

// --- project mode ------------------------------------------------------

#[test]
fn project_merges_files_flat() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Util;\n// Util referenced for the unused-import scan\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}\nfunction local() -> void {}",
    );
    tmp.write(
        "src/Acme/Util/parse.phg",
        "package Acme.Util;\nfunction parse() -> void {}",
    );
    let u = load(&entry).unwrap();
    assert_eq!(u.program.package, ["Main"]);
    // Items from both files are merged into one flat program.
    assert!(
        u.program.items.len() >= 3,
        "merged items: {:?}",
        u.program.items.len()
    );
    assert!(u.diag_src.is_empty(), "merged unit has no single source");
}

#[test]
fn project_load_reports_stats() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Util;\n// Util referenced for the unused-import scan\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}\nclass C {}",
    );
    tmp.write(
        "src/Acme/Util/parse.phg",
        "package Acme.Util;\nfunction parse() -> void {}",
    );
    let u = load(&entry).unwrap();
    let stats = u.stats.expect("project mode reports stats");
    assert_eq!(stats.files, 2, "two source files");
    assert_eq!(stats.packages, 2, "main + Acme.Util");
    assert_eq!(stats.defs, 3, "main, C, parse");
    // The human summary mentions the project-wide scope.
    let summary = stats.summary();
    assert!(summary.contains("2 files"), "got: {summary}");
    assert!(summary.contains("whole project"), "got: {summary}");
}

#[test]
fn project_main_is_folder_exempt_at_root() {
    let tmp = TempDir::new();
    // main lives at the project root, outside src/ — allowed.
    let entry = tmp.write(
        "main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    let u = load(&entry).unwrap();
    assert_eq!(u.program.package, ["Main"]);
}

#[test]
fn folder_path_mismatch_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Wrong;\n\
         // Wrong referenced for the unused-import scan\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    // File sits in src/Acme/Util but declares the wrong package — reached via its DECLARED name.
    tmp.write(
        "src/Acme/Util/parse.phg",
        "package Acme.Wrong;\nfunction parse() -> void {}",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-PKG-PATH"), "got: {err}");
    assert!(err.contains("does not match its location"), "got: {err}");
}

#[test]
fn non_main_directly_in_source_root_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport App;\n\
         // App referenced for the unused-import scan\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    tmp.write("src/loose.phg", "package App;\nfunction f() -> void {}");
    let err = load(&entry).unwrap_err();
    assert!(
        err.contains("cannot sit directly in the source root"),
        "got: {err}"
    );
}

#[test]
fn library_package_outside_source_root_is_rejected() {
    let tmp = TempDir::new();
    tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    // A dotted package outside src/ run AS THE ENTRY — legal under DEC-282 (any file may be an
    // entry; the old outside-the-source-root rejection retired with the manifest).
    tmp.write(
        "lib/parse.phg",
        "package Acme.Util;\nfunction parse() -> void {}",
    );
    let u = load(&tmp.path().join("lib/parse.phg")).expect("a library entry loads");
    assert_eq!(u.program.package, ["Acme", "Util"]);
}

#[test]
fn missing_entry_file_errors() {
    let tmp = TempDir::new();
    let err = load(&tmp.path().join("does-not-exist.phg")).unwrap_err();
    assert!(err.contains("cannot read"), "got: {err}");
}

#[test]
fn duplicate_function_in_package_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Util;\n\
         // Util referenced for the unused-import scan\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    // Two files in the same package each define `f` — collides after the flat merge.
    tmp.write(
        "src/Acme/Util/a.phg",
        "package Acme.Util;\nfunction f() -> void {}",
    );
    tmp.write(
        "src/Acme/Util/b.phg",
        "package Acme.Util;\nfunction f() -> void {}",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-DUP-DEF"), "got: {err}");
    assert!(err.contains("duplicate definition of `f`"), "got: {err}");
}

#[test]
fn vendored_package_main_is_inert() {
    // DEC-282: a `package Main` file inside vendor/ is UNREACHABLE (Main is never indexed, never
    // importable) — the old E-VENDOR-MAIN collision cannot occur; the stray file is simply inert.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Lib;\n\
         // Lib referenced for the unused-import scan\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    tmp.write(
        "vendor/Acme/Lib/Real.phg",
        "package Acme.Lib;\npublic function real() -> int { return 1; }",
    );
    tmp.write(
        "vendor/Acme/Lib/oops.phg",
        "package Main;\nfunction stray() -> void {}",
    );
    let u = load(&entry).expect("the stray vendored Main file is inert");
    assert_eq!(u.program.package, ["Main"]);
}
