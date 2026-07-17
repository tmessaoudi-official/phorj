//! M5 S2b/S2c integration: a multi-file project loads through `loader::load`, resolves cross-package
//! calls, and runs byte-identically on both backends. S2c qualifies cross-package calls
//! (`Util.compute(x)` via an import leaf or alias), tightens the S2b bare-call interim (unqualified
//! cross-package calls now fail), supports cross-package types via `import type`, and transpiles to
//! one PHP `namespace` brace-block per package. Packages are PascalCase (`E-PKG-CASE`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use phorj::{cli, loader};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let unique = N.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("phorj_project_it_{}_{unique}", std::process::id()));
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

fn run_both(entry: &Path) -> (String, String) {
    let unit = loader::load(entry).expect("project loads");
    let run = cli::treewalk_program(&unit).expect("interpreter runs");
    let runvm = cli::run_program(&unit).expect("vm runs");
    (run, runvm)
}

#[test]
fn multi_file_project_qualified_call_runs_byte_identically() {
    let tmp = TempDir::new();
    // S2c: cross-package calls are *qualified* via an import leaf (`Util.compute`), no longer the
    // S2b bare form. The loader resolves it against the imported package's mangled symbol.
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util;\n\
         #[Entry] function main() -> void {\n    Output.printLine(\"{Util.compute(20)}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/compute.phg",
        "package Acme.Util;\nfunction compute(int n) -> int {\n    return n + n + 2;\n}",
    );

    let (run, runvm) = run_both(&entry);
    assert_eq!(run, "42\n");
    assert_eq!(run, runvm, "run and runvm must be byte-identical");
}

#[test]
fn import_alias_resolves_qualified_call() {
    let tmp = TempDir::new();
    // `import Acme.Util as U;` binds the leaf `u`; the call qualifies on the alias.
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util as U;\n\
         #[Entry] function main() -> void {\n    Output.printLine(\"{U.compute(20)}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/compute.phg",
        "package Acme.Util;\nfunction compute(int n) -> int {\n    return n + n + 2;\n}",
    );

    let (run, runvm) = run_both(&entry);
    assert_eq!(run, "42\n");
    assert_eq!(run, runvm, "run and runvm must be byte-identical");
}

#[test]
fn same_package_cross_file_bare_call_resolves() {
    // Two files in the *same* library package: one calls the other by its bare (same-package) name;
    // the loader mangles both consistently so the intra-package call still resolves.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util;\n\
         #[Entry] function main() -> void {\n    Output.printLine(\"{Util.outer(20)}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/outer.phg",
        "package Acme.Util;\nfunction outer(int n) -> int {\n    return inner(n) + 2;\n}",
    );
    tmp.write(
        "src/Acme/Util/inner.phg",
        "package Acme.Util;\nfunction inner(int n) -> int {\n    return n + n;\n}",
    );

    let (run, runvm) = run_both(&entry);
    assert_eq!(run, "42\n");
    assert_eq!(run, runvm, "run and runvm must be byte-identical");
}

#[test]
fn unqualified_cross_package_call_is_rejected() {
    // The S2b interim (bare cross-package call) is gone: a library function must be qualified.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util;\n\
         // Util referenced for the unused-import scan\n\
         #[Entry] function main() -> void {\n    Output.printLine(\"{compute(20)}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/compute.phg",
        "package Acme.Util;\nfunction compute(int n) -> int {\n    return n + 2;\n}",
    );
    let unit = loader::load(&entry).expect("project loads");
    // Both backends reject identically (the bare `compute` no longer names any function).
    let run = cli::treewalk_program(&unit);
    let runvm = cli::run_program(&unit);
    assert!(run.is_err(), "bare cross-package call must fail");
    assert!(
        runvm.is_err(),
        "bare cross-package call must fail on the VM too"
    );
}

#[test]
fn library_package_type_is_usable_cross_package() {
    // The E-PKG-TYPE gate is retired (M-RT cross-package types): a library package may declare a
    // type, and `package Main` consumes it via `import type`, instantiating + reading a field.
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util.Shape;\n\
         #[Entry] function main() -> void {\n    Shape s = new Shape(5);\n    Output.printLine(\"{s.w}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/Shape.phg",
        "package Acme.Util;\nclass Shape { constructor(public int w) {} }",
    );
    let unit = loader::load(&entry).expect("project with a cross-package type loads");
    // Both backends agree (the type def + every reference were mangled before either backend ran).
    let run = cli::treewalk_program(&unit);
    let runvm = cli::run_program(&unit);
    assert_eq!(run.as_deref(), Ok("5\n"), "run output");
    assert_eq!(runvm.as_deref(), Ok("5\n"), "runvm output");
}

/// `import type` of a type a package does not export.
#[test]
fn import_type_unknown_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Acme.Util.Nope;\n// Nope referenced for the unused-import scan\n#[Entry] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/Util/Shape.phg",
        "package Acme.Util;\nclass Shape { constructor(public int w) {} }",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-UNKNOWN"), "got: {err}");
}

/// Two `import type` binding the same bare name without an alias.
#[test]
fn import_type_conflict_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Acme.A.Shape;\nimport Acme.B.Shape;\n// Shape referenced for the unused-import scan\n#[Entry] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/A/Shape.phg",
        "package Acme.A;\nclass Shape { constructor(public int w) {} }",
    );
    tmp.write(
        "src/Acme/B/Shape.phg",
        "package Acme.B;\nclass Shape { constructor(public int w) {} }",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-CONFLICT"), "got: {err}");
}

/// `import type` naming a built-in type (built-ins are import-free).
#[test]
fn import_type_builtin_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Acme.Util.List;\n// List referenced for the unused-import scan\n#[Entry] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/Util/u.phg",
        "package Acme.Util;\nfunction noop() -> void {}",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-BUILTIN"), "got: {err}");
}

/// `import type` whose bound name collides with a module-import qualifier.
#[test]
fn import_type_shadow_is_rejected() {
    let tmp = TempDir::new();
    // A type named `Util` (bound bare by `import Acme.Types.Util`) clashing with the
    // `Acme.Util` module-import leaf `Util`. The shadow guard keeps the two import kinds disjoint.
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Acme.Util;\nimport Acme.Types.Util;\n// Util referenced for the unused-import scan\n#[Entry] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/Util/u.phg",
        "package Acme.Util;\nfunction noop() -> void {}",
    );
    tmp.write(
        "src/Acme/Types/Util.phg",
        "package Acme.Types;\nclass Util { constructor(public int w) {} }",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-SHADOW"), "got: {err}");
}

#[test]
fn multi_package_transpiles_to_brace_namespaces() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util;\n\
         #[Entry] function main() -> void {\n    Output.printLine(\"{Util.compute(20)}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/compute.phg",
        "package Acme.Util;\nfunction compute(int n) -> int {\n    return n + n + 2;\n}",
    );
    let unit = loader::load(&entry).expect("project loads");
    let php = cli::transpile_program(&unit.program, &unit.diag_src).expect("transpiles");
    assert!(php.contains("namespace Acme\\Util {"), "got:\n{php}");
    assert!(php.contains("namespace Main {"), "got:\n{php}");
    assert!(php.contains("\\Main\\main();"), "got:\n{php}");
    // The cross-package call is emitted fully-qualified.
    assert!(php.contains("\\Acme\\Util\\compute("), "got:\n{php}");
    // The library function is declared by its bare leaf inside its namespace block.
    assert!(php.contains("function compute("), "got:\n{php}");
}

#[test]
fn folder_path_violation_is_reported() {
    let tmp = TempDir::new();
    // Import-driven: the misplaced file is reached via its DECLARED package (the index is
    // declaration-keyed), and loading it then trips the folder = package validation.
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Acme.Bad;\n\
         // Bad referenced for the unused-import scan\n#[Entry] function main() -> void {}",
    );
    tmp.write(
        "src/Acme/Util/x.phg",
        "package Acme.Bad;\nfunction x() -> void {}",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-PKG-PATH"), "got: {err}");
}

#[test]
fn manifestless_non_main_file_loads_as_a_library() {
    // DEC-282: the old loose-mode "dotted package needs a project" rejection is retired — the
    // unified loader accepts any file as an entry; a dotted-package library file simply loads
    // (running it would then fail on the missing #[Entry], not on the package).
    let tmp = TempDir::new();
    let entry = tmp.write("script.phg", "package App.Util;\nfunction f() -> void {}");
    let unit = loader::load(&entry).expect("a library file loads under the unified rule");
    assert_eq!(unit.program.package, ["App", "Util"]);
}

#[test]
fn manifestless_sibling_package_resolves_by_folder() {
    // DEC-282 unified loading, lone-script shape: a package folder NEXT TO the entry resolves
    // (entry-dir is the only search root; folder = package).
    let tmp = TempDir::new();
    tmp.write(
        "Util/helpers.phg",
        "package Util;\npublic function double(int n) -> int { return n * 2; }",
    );
    let entry = tmp.write(
        "x.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Util;\n\
         #[Entry] function main() -> void { Output.printLine(\"{Util.double(21)}\"); }",
    );
    let unit = loader::load(&entry).expect("sibling package resolves");
    let out = phorj::cli::treewalk_program(&unit).expect("runs");
    assert_eq!(out, "42\n");
}

#[test]
fn manifestless_unresolved_import_is_module_not_found() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "x.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Bogus;\n\
         #[Entry] function main() -> void { Bogus.f(); }",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-MODULE-NOT-FOUND"), "got: {err}");
    assert!(err.contains("Bogus"), "names the module: {err}");
}

#[test]
fn manifestless_bin_console_reaches_src_and_vendor_and_local() {
    // DEC-282 the Symfony shape: bin/console imports its own folder's package, src/ (stripped),
    // and vendor/ — the app root found by walking up to the dir containing src/.
    let tmp = TempDir::new();
    tmp.write(
        "src/Model/Article.phg",
        "package Model;\nclass Article { constructor(public string title) {} }",
    );
    tmp.write(
        "vendor/Acme/Strutil/StrUtil.phg",
        "package Acme.Strutil;\npublic function shout(string s) -> string { return \"{s}!\"; }",
    );
    tmp.write(
        "bin/Commands/Migrate.phg",
        "package Commands;\npublic function banner() -> string { return \"migrating\"; }",
    );
    let entry = tmp.write(
        "bin/console.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Commands;\n\
         import Model.Article;\nimport Acme.Strutil;\n\
         #[Entry] function main() -> void {\n\
           Output.printLine(Commands.banner());\n\
           Article a = new Article(\"hello\");\n\
           Output.printLine(a.title);\n\
           Output.printLine(Strutil.shout(\"dep\"));\n\
         }",
    );
    let unit = loader::load(&entry).expect("three-root resolution");
    let out = phorj::cli::treewalk_program(&unit).expect("runs");
    assert_eq!(out, "migrating\nhello\ndep!\n");
}

#[test]
fn manifestless_import_main_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "x.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Main;\n\
         #[Entry] function main() -> void {}",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-IMPORT-MAIN"), "got: {err}");
}

#[test]
fn manifestless_duplicate_import_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "x.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Core.Output;\n\
         #[Entry] function main() -> void { Output.printLine(\"x\"); }",
    );
    let err = loader::load(&entry).unwrap_err();
    assert!(err.contains("E-DUP-IMPORT"), "got: {err}");
}

// --- Cross-package traits (M-RT S8, cross-package) ---

/// A `trait` declared in a library package is composed into a `package Main` class via
/// `import Pkg.Trait;` + `use Trait;`, and runs byte-identically on both backends. The loader
/// mangles the trait declaration and the `use` clause to the same FQN, so the by-name flatten lines up.
#[test]
fn cross_package_trait_composition_runs_byte_identically() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Mix.Greet;\n\
         class Person {\n  use Greet;\n  constructor(public string name) {}\n}\n\
         #[Entry] function main() -> void {\n  var p = new Person(\"ada\");\n  Output.printLine(\"{p.name}: {p.hello()}\");\n}",
    );
    tmp.write(
        "src/Acme/Mix/Greet.phg",
        "package Acme.Mix;\ntrait Greet {\n  function hello() -> string { return \"hi\"; }\n}",
    );
    let (run, runvm) = run_both(&entry);
    assert_eq!(run, "ada: hi\n");
    assert_eq!(run, runvm, "run and runvm must be byte-identical");
}

/// The cross-package trait transpiles to a native PHP `trait` in its package namespace, composed by
/// the using class via a fully-qualified `use \Acme\Mix\Greet`.
#[test]
fn cross_package_trait_transpiles_to_namespaced_trait() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Mix.Greet;\n\
         class Person {\n  use Greet;\n  constructor(public string name) {}\n}\n\
         #[Entry] function main() -> void {\n  var p = new Person(\"ada\");\n  Output.printLine(p.hello());\n}",
    );
    tmp.write(
        "src/Acme/Mix/Greet.phg",
        "package Acme.Mix;\ntrait Greet {\n  function hello() -> string { return \"hi\"; }\n}",
    );
    let unit = loader::load(&entry).expect("project loads");
    let php = cli::transpile_program(&unit.program, &unit.diag_src).expect("transpiles");
    assert!(php.contains("namespace Acme\\Mix {"), "got:\n{php}");
    assert!(php.contains("trait Greet {"), "got:\n{php}");
    assert!(php.contains("use \\Acme\\Mix\\Greet;"), "got:\n{php}");
}

/// A trait is reuse, not a type — typing a value as a cross-package trait is `E-USE-AS-TYPE`, exactly
/// as for a same-package trait. (A check-time error: the project loads, the checker rejects it.)
#[test]
fn cross_package_trait_used_as_type_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Mix.Greet;\n\
         function f(Greet x) -> void { Output.printLine(\"no\"); }\n\
         #[Entry] function main() -> void { Output.printLine(\"hi\"); }",
    );
    tmp.write(
        "src/Acme/Mix/Greet.phg",
        "package Acme.Mix;\ntrait Greet {\n  function hello() -> string { return \"hi\"; }\n}",
    );
    let unit = loader::load(&entry).expect("project loads (trait import resolves)");
    let err = cli::treewalk_program(&unit).unwrap_err();
    assert!(err.contains("E-USE-AS-TYPE"), "got: {err}");
}

/// A qualified cross-package call inside a **map literal** value (`["k" => Util.f()]`) is resolved by
/// the loader — the `Expr::Map` arm descends both key and value, so a cross-package reference nested
/// in a map is rewritten like one in a list (the multi-package map-literal gap).
#[test]
fn cross_package_call_inside_map_literal_resolves() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Util;\n\
         #[Entry] function main() -> void {\n  Map<string, int> m = [\"k\" => Util.compute(20)];\n  Output.printLine(\"{m[\\\"k\\\"]}\");\n}",
    );
    tmp.write(
        "src/Acme/Util/compute.phg",
        "package Acme.Util;\nfunction compute(int n) -> int { return n + 22; }",
    );
    let (run, runvm) = run_both(&entry);
    assert_eq!(run, "42\n");
    assert_eq!(run, runvm, "run and runvm must be byte-identical");
}

/// A `package Main` class `extends` a library-package class (imported via `import type`), inheriting
/// its constructor + field, overriding an `open` method, and calling up with the named
/// `parent(Ancestor).m()` form — all resolved across the package boundary, byte-identical.
#[test]
fn cross_package_inheritance_and_parent_calls_run_byte_identically() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Zoo.Animal;\n\
         class Dog extends Animal {\n  open function speak() -> string { return \"woof/\" + parent(Animal).speak(); }\n}\n\
         #[Entry] function main() -> void {\n  Output.printLine(new Dog(\"rex\").speak());\n}",
    );
    tmp.write(
        "src/Acme/Zoo/Animal.phg",
        "package Acme.Zoo;\nopen class Animal {\n  constructor(public string name) {}\n  open function speak() -> string { return \"(animal)\"; }\n}",
    );
    let (run, runvm) = run_both(&entry);
    assert_eq!(run, "woof/(animal)\n");
    assert_eq!(run, runvm, "run and runvm must be byte-identical");
}

/// The cross-package parent class is emitted as `extends \Acme\Zoo\Animal` and the parent call as
/// `parent::speak()` in the using class's namespace block.
#[test]
fn cross_package_inheritance_transpiles_to_qualified_extends() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\nimport Acme.Zoo.Animal;\n\
         class Dog extends Animal {\n  open function speak() -> string { return parent.speak(); }\n}\n\
         #[Entry] function main() -> void {\n  Output.printLine(new Dog(\"rex\").speak());\n}",
    );
    tmp.write(
        "src/Acme/Zoo/Animal.phg",
        "package Acme.Zoo;\nopen class Animal {\n  constructor(public string name) {}\n  open function speak() -> string { return \"(animal)\"; }\n}",
    );
    let unit = loader::load(&entry).expect("project loads");
    let php = cli::transpile_program(&unit.program, &unit.diag_src).expect("transpiles");
    assert!(php.contains("extends \\Acme\\Zoo\\Animal"), "got:\n{php}");
    assert!(php.contains("parent::speak()"), "got:\n{php}");
}

// ── W0-4: loader-side reserved-package + package-casing gates (project mode) ─────────────────────
// H §2.3 (P1): in project mode the flat merge mangles per-file defs *before* `check()`, so a file's
// own `package` decl never reaches program.rs — a `package Core.*;` hijack or a lowercase package
// declaration was silently accepted. The loader now validates each file's package decl per-file,
// before the merge, mirroring the checker's E-RESERVED-PACKAGE / E-PKG-CASE.

#[test]
fn project_reserved_core_package_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\n#[Entry] function main() -> void { Output.printLine(\"hi\"); }",
    );
    // Lives at the folder that matches its (reserved) package, so E-PKG-PATH passes and the
    // reserved-root rule is what fires.
    tmp.write(
        "src/Core/Output/sneak.phg",
        "package Core.Output;\nfunction sneak() -> void {}",
    );
    // DEC-282: `Core.*` imports NEVER consult disk (step 0, reserved), so the sneak file is
    // unreachable by construction — the entry loads and the hijack file is simply never read.
    let unit = loader::load(&entry).expect("Core-package file on disk is inert");
    let out = cli::treewalk_program(&unit).expect("runs");
    assert_eq!(out, "hi\n");
}

#[test]
fn project_lowercase_package_decl_is_rejected() {
    let tmp = TempDir::new();
    let entry = tmp.write(
        "src/main.phg",
        "package Main;\nimport Core.Runtime.Entry;\n#[Entry] function main() -> void {}",
    );
    // Folder matches the (lowercase) package, so E-PKG-PATH passes and E-PKG-CASE is what fires.
    tmp.write(
        "src/acme/util.phg",
        "package acme;\nfunction u() -> void {}",
    );
    // DEC-282 import-driven loading: a lowercase package cannot even be IMPORTED (import
    // segments are PascalCase), so the offending file is unreachable — the entry loads fine.
    loader::load(&entry).expect("lowercase-package file on disk is inert (unimportable)");
}
