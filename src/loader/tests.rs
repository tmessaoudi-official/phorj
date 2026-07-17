use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let unique = N.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("phorj_loader_test_{}_{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn path(&self) -> &Path {
        &self.0
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

// --- loose mode --------------------------------------------------------

#[test]
fn loose_main_is_accepted() {
    let u = load_loose_src(
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
    )
    .unwrap();
    assert_eq!(u.program.package, ["Main"]);
    assert_eq!(
        u.diag_src,
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}"
    );
}

#[test]
fn loose_non_main_is_rejected() {
    let err = load_loose_src("package app.util;\nfunction f() -> void {}").unwrap_err();
    assert!(err.contains("cannot run from stdin/-e"), "got: {err}");
}

#[test]
fn loose_empty_package_defers_to_checker() {
    // No package decl — loader stays silent (checker reports E-NO-PACKAGE downstream).
    let u = load_loose_src("#[Entry] function main() -> void {}").unwrap();
    assert!(u.program.package.is_empty());
}

// --- project mode ------------------------------------------------------

#[test]
fn project_merges_files_flat() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Acme.Util;\n// Util referenced for the unused-import scan\n#[Entry] function main() -> void {}\nfunction local() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Util;\n// Util referenced for the unused-import scan\n#[Entry] function main() -> void {}\nclass C {}",
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
fn loose_load_has_no_stats() {
    let u = load_loose_src(
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
    )
    .unwrap();
    assert!(u.stats.is_none(), "loose mode reports no project stats");
}

#[test]
fn project_main_is_folder_exempt_at_root() {
    let tmp = TempDir::new();
    // main lives at the project root, outside src/ — allowed.
    let entry = tmp.write(
        "main.phg",
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
    );
    let u = load(&entry).unwrap();
    assert_eq!(u.program.package, ["Main"]);
}

#[test]
fn folder_path_mismatch_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Acme.Wrong;\n\
         // Wrong referenced for the unused-import scan\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\nimport App;\n\
         // App referenced for the unused-import scan\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Util;\n\
         // Util referenced for the unused-import scan\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Lib;\n\
         // Lib referenced for the unused-import scan\n#[Entry] function main() -> void {}",
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

// --- declaration visibility (visibility modifiers) ---------------------

#[test]
fn import_type_of_internal_library_type_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.Hidden;\n#[Entry] function main() -> void { Hidden h = Hidden(); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.Shown;\n#[Entry] function main() -> void { Shown s = Shown(); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Lib.Helper;\n\
         #[Entry] function main() -> void { Helper h = Helper(); }",
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
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void { Helper h = Helper(); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Lib;\n\
         #[Entry] function main() -> int { return Lib.helper(); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Util;\n#[Entry] function main() -> int { return Util.secret(); }",
    );
    tmp.write(
        "src/Acme/Util/util.phg",
        "package Acme.Util;\ninternal function secret() -> int { return 7; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-INTERNAL"), "got: {err}");
}

#[test]
fn public_function_called_cross_package_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Acme.Util;\n#[Entry] function main() -> int { return Util.shown(); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Lib.Helper;\ntype H = Helper;\n\
         #[Entry] function main() -> void { H h = Helper(); }",
    );
    tmp.write(
        "src/Lib/Helper.phg",
        "package Lib;\nprivate class Helper { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-VIS-PRIVATE"), "got: {err}");
}

// --- public-surface file-naming rule (E-FILE-*) -----------------------

#[test]
fn public_type_in_mismatched_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Ui.Widget;\n\
         #[Entry] function main() -> void { Widget w = Widget(); }",
    );
    // A file declaring one public type must be named after it; `widget.phg` ≠ `Widget`.
    tmp.write(
        "src/Ui/widget.phg",
        "package Ui;\npublic class Widget { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-FILE-NAME"), "got: {err}");
}

#[test]
fn public_type_in_matching_file_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void { Widget w = Widget(); }",
    );
    tmp.write(
        "src/Widget.phg",
        "package Main;\npublic class Widget { constructor() {} }",
    );
    assert!(load(&entry).is_ok());
}

#[test]
fn two_public_types_in_one_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Lib.A;\n\
         #[Entry] function main() -> void { A a = A(); }",
    );
    tmp.write(
        "src/Lib/A.phg",
        "package Lib;\npublic class A { constructor() {} }\npublic class B { constructor() {} }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-FILE-MULTI-PUBLIC"), "got: {err}");
}

#[test]
fn public_type_plus_public_fn_in_one_file_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Lib.Box;\n\
         #[Entry] function main() -> void { Box b = Box(); }",
    );
    tmp.write(
        "src/Lib/Box.phg",
        "package Lib;\npublic class Box { constructor() {} }\npublic function helper() -> int { return 1; }",
    );
    let err = load(&entry).unwrap_err();
    assert!(err.contains("E-FILE-MIXED-PUBLIC"), "got: {err}");
}

#[test]
fn private_helper_type_rides_along_in_a_type_module() {
    // A type module may carry private/internal helper types + functions — they ride along free.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void { Widget w = Widget(); }",
    );
    tmp.write(
        "src/Widget.phg",
        "package Main;\npublic class Widget { constructor() {} }\nprivate class Cache { constructor() {} }\nprivate function tweak() -> int { return 1; }",
    );
    assert!(load(&entry).is_ok(), "private helpers should ride along");
}

#[test]
fn main_file_with_multiple_public_types_is_exempt() {
    let tmp = TempDir::new();
    // The entry file declares `main` → exempt: multiple public types + functions are fine, any name.
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\npublic class A { constructor() {} }\npublic class B { constructor() {} }\n#[Entry] function main() -> void { A a = A(); B b = B(); }",
    );
    assert!(
        load(&entry).is_ok(),
        "a main file is exempt from the public-surface rule"
    );
}

#[test]
fn forward_and_cross_file_type_references_resolve() {
    // Order-independence (the prebind pre-pass): `Order` references `OrderLine`, which sorts/merges
    // AFTER it — and a forward reference within the entry file — both resolve.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void { B b = makeB(); }\nfunction makeB() -> B { return B(7); }",
    );
    // Two library files; `Order.phg` (merged first, alphabetically) references `OrderLine` (later).
    tmp.write(
        "src/Acme/Lib/Order.phg",
        "package Acme.Lib;\npublic class Order { constructor(public OrderLine line) {} }",
    );
    tmp.write(
        "src/Acme/Lib/OrderLine.phg",
        "package Acme.Lib;\npublic class OrderLine { constructor(public int qty) {} }",
    );
    tmp.write(
        "src/B.phg",
        "package Main;\npublic class B { constructor(public int x) {} }",
    );
    assert!(
        load(&entry).is_ok(),
        "forward + cross-file type refs must resolve"
    );
}

// --- M8.5 S3b: `.d.phg` ambient declaration files -----------------------

#[test]
fn decl_file_is_loaded_ambiently_and_not_mangled() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
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

// --- DEC-197: member FUNCTION imports (bare cross-package function calls) -----------------------

#[test]
fn import_function_bare_from_library_is_allowed() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.area;\n#[Entry] function main() -> void { int a = area(3); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.area as size;\n#[Entry] function main() -> void { int a = size(3); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.area;\n#[Entry] function main() -> void { int a = area(3); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.area;\nimport Acme.Alt.area;\n#[Entry] function main() -> void { int a = area(3); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo;\n#[Entry] function main() -> void { int a = Geo.area(3); }",
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
        "package Main; import Core.Runtime.Entry;\nimport Acme.Geo.area;\n#[Entry] function main() -> void { var f = area; int a = f(3); }",
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
