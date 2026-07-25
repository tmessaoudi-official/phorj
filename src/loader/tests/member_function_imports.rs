use super::*;

// --- DEC-197: member FUNCTION imports (bare cross-package function calls) -----------------------

#[test]
fn import_function_bare_from_library_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.area;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { int a = area(3); }",
    );
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\npublic function area(int r) -> int { return r + r; }",
    );
    assert!(
        load(&entry).is_ok(),
        "a bare member-imported public library function must resolve"
    );
}

#[test]
fn import_function_aliased_from_library_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.area as size;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { int a = size(3); }",
    );
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\npublic function area(int r) -> int { return r + r; }",
    );
    assert!(
        load(&entry).is_ok(),
        "an `as`-aliased function import must resolve under the alias"
    );
}

#[test]
fn import_private_function_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.area;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { int a = area(3); }",
    );
    // `private` = file-scoped; a cross-package member import cannot reach it.
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\nprivate function area(int r) -> int { return r + r; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-PRIVATE"), "got: {err}");
}

#[test]
fn duplicate_function_import_conflicts() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.area;\nimport Acme.Alt.area;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { int a = area(3); }",
    );
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\npublic function area(int r) -> int { return r; }",
    );
    tmp.write(
        "src/Acme/Alt/alt.phg",
        "package Acme.Alt;\npublic function area(int r) -> int { return r + 1; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-CONFLICT"), "got: {err}");
}

#[test]
fn qualified_call_still_works_alongside_function_imports() {
    // A whole-module import keeps the qualified form; regression that slice 2 does not break it.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { int a = Geo.area(3); }",
    );
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\npublic function area(int r) -> int { return r + r; }",
    );
    assert!(
        load(&entry).is_ok(),
        "the qualified cross-package call form must still resolve"
    );
}

#[test]
fn import_function_used_as_value_resolves() {
    // DEC-197: a bare member-imported function referenced as a first-class VALUE (not just called
    // directly) resolves in value position too (`resolve_expr`'s Ident arm), not only at a call site.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Geo.area;\n#[Entry(kind: EntryKind.Cli)] function main() -> void { var f = area; int a = f(3); }",
    );
    tmp.write(
        "src/Acme/Geo/geo.phg",
        "package Acme.Geo;\npublic function area(int r) -> int { return r + r; }",
    );
    assert!(
        load(&entry).is_ok(),
        "a bare member-imported function must resolve as a first-class value"
    );
}
