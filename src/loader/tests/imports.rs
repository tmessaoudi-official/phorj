use super::*;

// --- Q-A wildcard imports ---------------------------------------------

#[test]
fn wildcard_expands_public_cross_package_members() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Shapes.*;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { int a = area(); int p = perimeter(); }",
    );
    tmp.write(
        "src/Acme/Shapes/shapes.phg",
        "package Acme.Shapes;\nfunction area() -> int { return 1; }\nfunction perimeter() -> int { return 2; }",
    );
    let u = load(&entry).expect("wildcard should expand + both members resolve");
    assert_eq!(u.program.package, ["Main"]);
}

#[test]
fn wildcard_core_submodule_is_deferred() {
    // Q-A: Core-submodule wildcards are a deferred follow-up (parser-rejected for now — see spec
    // "Core wildcard" PENDING). Import Core members explicitly meanwhile.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Core.Output.*;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("not yet supported"), "got: {err}");
}

#[test]
fn wildcard_bare_core_root_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Core.*;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("standard library"), "got: {err}");
}

#[test]
fn wildcard_except_removing_all_is_empty() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Shapes.* except { area, perimeter };\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/Shapes/shapes.phg",
        "package Acme.Shapes;\nfunction area() -> int { return 1; }\nfunction perimeter() -> int { return 2; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-WILDCARD-EMPTY"), "got: {err}");
}

#[test]
fn wildcard_except_unknown_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Shapes.* except { Nope };\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { int a = area(); }",
    );
    tmp.write(
        "src/Acme/Shapes/shapes.phg",
        "package Acme.Shapes;\nfunction area() -> int { return 1; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-EXCEPT-UNKNOWN"), "got: {err}");
}

#[test]
fn wildcard_ambiguous_collision_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.A.*;\nimport Acme.B.*;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/A/a.phg",
        "package Acme.A;\nfunction thing() -> void {}",
    );
    tmp.write(
        "src/Acme/B/b.phg",
        "package Acme.B;\nfunction thing() -> void {}",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-AMBIGUOUS"), "got: {err}");
}

#[test]
fn import_unknown_member_is_rejected() {
    // Q-A step 3 (G6): a member import naming nothing the package exports → E-IMPORT-UNKNOWN
    // (used here so the earlier unused-import scan passes and we reach member validation).
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Shapes.bogus;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { bogus(); }",
    );
    tmp.write(
        "src/Acme/Shapes/shapes.phg",
        "package Acme.Shapes;\nfunction area() -> int { return 1; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-UNKNOWN"), "got: {err}");
}

#[test]
fn import_valid_member_is_accepted() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Acme.Shapes.area;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { int a = area(); }",
    );
    tmp.write(
        "src/Acme/Shapes/shapes.phg",
        "package Acme.Shapes;\nfunction area() -> int { return 1; }",
    );
    load(&entry).expect("a valid member import must load");
}

/// The `html"…"` LITERAL counts as a use of `import Core.Html`.
///
/// Reproduced before the fix: `var a = html"<p>{name}</p>";` under `import Core.Html;` reported
/// `E-UNUSED-IMPORT` ("nothing in this file references `Html` — remove the import, or use it") while
/// REMOVING the import reported `E-HTML-IMPORT` ("`html"…"` requires the Core.Html module"). Two
/// diagnostics instructing opposite actions, with no way to write the program in that shape — the only
/// working form was an explicit `Html a = …` annotation, which happens to spell the type name.
///
/// The cause: the hygiene scan is TEXTUAL and case-sensitive, so the lowercase literal prefix `html"`
/// never matched the whole word `Html`. The import GATES the literal, so the literal is a use by
/// definition.
#[test]
fn an_html_literal_counts_as_a_use_of_its_import() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         import Core.Output;\nimport Core.Html;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { string n = \"x\"; \
             var a = html\"<p>{n}</p>\"; Output.printLine(\"{a}\"); }",
    );
    load(&entry).expect("an `html\"…\"` literal must count as a use of `import Core.Html`");
}

/// The guard's other half: an import with NO use at all still fails. Without this, "count the literal"
/// could degrade into "never report Core.Html unused".
#[test]
fn an_import_with_no_use_at_all_is_still_unused() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         import Core.Output;\nimport Core.Html;\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void { Output.printLine(\"no html here\"); }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-UNUSED-IMPORT"), "got: {err}");
}
