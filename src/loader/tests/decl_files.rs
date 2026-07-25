use super::*;

// --- M8.5 S3b: `.d.phg` ambient declaration files -----------------------

#[test]
fn decl_file_is_loaded_ambiently_and_not_mangled() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    // A package-free declaration file under the source root — loaded ambiently, never compiled as a
    // package source (so no folder=path / package decl required).
    tmp.write(
        "src/php.d.phg",
        "declare function strtoupper(string s) -> string;",
    );
    let u = load(&entry).unwrap();
    // The foreign function is merged into the unit, with its bare global name (never mangled).
    let foreign_fns: Vec<&str> = u
        .program
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Function(f) if f.foreign => Some(f.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        foreign_fns,
        ["strtoupper"],
        "merged foreign fns: {foreign_fns:?}"
    );
}

#[test]
fn decl_file_with_package_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    tmp.write(
        "src/php.d.phg",
        "package Main;\ndeclare function strtoupper(string s) -> string;",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-DECL-PACKAGE"), "got: {err}");
}

#[test]
fn decl_file_with_nonforeign_item_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    // A real (non-`declare`) function has no place in a declaration file.
    tmp.write(
        "src/php.d.phg",
        "declare function strtoupper(string s) -> string;\nfunction local() -> void {}",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-DECL-NONFOREIGN"), "got: {err}");
}

#[test]
fn decl_file_is_not_counted_as_a_package_source() {
    // A `.d.phg` is excluded from `collect_phg` — it must never be folder=path-validated. Place one
    // directly in the source root (where a real non-`main` `.phg` would be rejected) and confirm load
    // succeeds: only its ambient-merge path ran, not the package-source path.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    tmp.write(
        "src/builtins.d.phg",
        "declare function strlen(string s) -> int;",
    );
    assert!(
        load(&entry).is_ok(),
        "a `.d.phg` in the source root must load fine"
    );
}
