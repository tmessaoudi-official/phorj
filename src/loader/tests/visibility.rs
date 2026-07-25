use super::*;

// --- declaration visibility (visibility modifiers) ---------------------

#[test]
fn import_type_of_internal_library_type_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.Hidden;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { Hidden h = Hidden(); }",
    );
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\ninternal class Hidden { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-INTERNAL"), "got: {err}");
}

#[test]
fn import_type_of_public_library_type_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.Shown;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { Shown s = Shown(); }",
    );
    // Public-surface rule: a file with one public type is named after it (`Shown.phg`).
    tmp.write(
        "src/Acme/Geo/Shown.phg",
        "package Acme.Geo;\npublic class Shown { constructor() {} }",
    );
    assert!(load(&entry).is_ok());
}

#[test]
fn private_type_referenced_from_sibling_file_is_rejected() {
    let tmp = TempDir::new();
    // DEC-282: sibling `package Main` files are unreachable (Main = the entry file only), so the
    // cross-FILE private check now lives on package files — same lattice, package-shaped fixture.
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Lib.Helper;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { Helper h = Helper(); }",
    );
    tmp.write(
        "src/Lib/Helper.phg",
        "package Lib;\nprivate class Helper { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-PRIVATE"), "got: {err}");
}

#[test]
fn internal_type_referenced_from_sibling_file_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { Helper h = Helper(); }",
    );
    tmp.write(
        "src/helper.phg",
        "package Main;\ninternal class Helper { constructor() {} }",
    );
    assert!(load(&entry).is_ok());
}

#[test]
fn private_function_called_from_sibling_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Lib;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> int { return Lib.helper(); }",
    );
    tmp.write(
        "src/Lib/util.phg",
        "package Lib;\nprivate function helper() -> int { return 1; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-PRIVATE"), "got: {err}");
}

#[test]
fn internal_function_called_cross_package_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Util;\n#[Entry(kind: EntryKind.Cli)] function main() -> int { return Util.secret(); }",
    );
    tmp.write(
        "src/Acme/Util/util.phg",
        "package Acme.Util;\ninternal function secret() -> int { return 7; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-INTERNAL"), "got: {err}");
}

#[test]
fn internal_function_visible_from_descendant_package_is_allowed() {
    // Q-B DV-1/DV-2: `internal` = this package AND its descendant packages. A child package
    // `Acme.App.Sub` may reach an `internal` member of its ancestor `Acme.App`.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.App.Sub;\n#[Entry(kind: EntryKind.Cli)] function main() -> int { return Sub.run(); }",
    );
    tmp.write(
        "src/Acme/App/app.phg",
        "package Acme.App;\ninternal function secret() -> int { return 7; }",
    );
    tmp.write(
        "src/Acme/App/Sub/sub.phg",
        "package Acme.App.Sub;\nimport Acme.App;\npublic function run() -> int { return App.secret(); }",
    );
    assert!(
        load(&entry).is_ok(),
        "descendant should reach ancestor internal: {:?}",
        load(&entry)
    );
}

#[test]
fn internal_function_not_visible_from_ancestor_package_is_rejected() {
    // Q-B DV-2: `internal` reaches DOWN the subtree, not up. An ancestor package `Acme.App` may
    // NOT reach an `internal` member of its descendant `Acme.App.Sub`.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.App;\n#[Entry(kind: EntryKind.Cli)] function main() -> int { return App.run(); }",
    );
    tmp.write(
        "src/Acme/App/app.phg",
        "package Acme.App;\nimport Acme.App.Sub;\npublic function run() -> int { return Sub.secret(); }",
    );
    tmp.write(
        "src/Acme/App/Sub/sub.phg",
        "package Acme.App.Sub;\ninternal function secret() -> int { return 7; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-INTERNAL"), "got: {err}");
}

#[test]
fn internal_function_not_visible_from_sibling_package_is_rejected() {
    // Q-B DV-2: a sibling package (shared parent, neither an ancestor of the other) is NOT in the
    // subtree — `internal` stays hidden.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Foo;\n#[Entry(kind: EntryKind.Cli)] function main() -> int { return Foo.run(); }",
    );
    tmp.write(
        "src/Acme/Foo/foo.phg",
        "package Acme.Foo;\nimport Acme.Bar;\npublic function run() -> int { return Bar.secret(); }",
    );
    tmp.write(
        "src/Acme/Bar/bar.phg",
        "package Acme.Bar;\ninternal function secret() -> int { return 7; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-INTERNAL"), "got: {err}");
}

#[test]
fn public_function_called_cross_package_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Util;\n#[Entry(kind: EntryKind.Cli)] function main() -> int { return Util.shown(); }",
    );
    tmp.write(
        "src/Acme/Util/util.phg",
        "package Acme.Util;\npublic function shown() -> int { return 7; }",
    );
    assert!(load(&entry).is_ok());
}

#[test]
fn type_alias_does_not_launder_private_type() {
    // A type alias names a type but the *construction* still names the real type directly, so the
    // file-scoped `private` check on `Helper()` fires regardless of the alias (aliases are
    // file-local + erased, so they cannot re-export across files).
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Lib.Helper;\ntype H = Helper;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { H h = Helper(); }",
    );
    tmp.write(
        "src/Lib/Helper.phg",
        "package Lib;\nprivate class Helper { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-PRIVATE"), "got: {err}");
}
