//! DEC-439 — the DIRECTORY lift: a PHP tree → a phorj project.
//!
//! Every test here is filesystem-driven, so they live in an integration test rather than a unit module.
//! Three of them exist because a review round found a real defect that no unit test would have caught:
//! silent overwrite on a name collision, a symlink cycle that never terminated, and PHP files invisible
//! to the report because they sat outside composer's autoload map.

use phorj::lift::project::{lift_directory, VendorMode};
use std::path::{Path, PathBuf};

/// A scratch directory that cleans itself up, pid-scoped so concurrent test binaries cannot collide
/// (DEC-378's root cause was fixed-path fixtures).
struct Tmp(PathBuf);

impl Tmp {
    fn new(label: &str) -> Tmp {
        let dir =
            std::env::temp_dir().join(format!("phorj_liftproj_{label}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        Tmp(dir)
    }

    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir -p");
        std::fs::write(&path, contents).expect("write");
        path
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// THE acceptance test for the whole slice: a cross-file `use` resolves once the tree is lifted as a
/// PROJECT. Single-file lifting could not do this — `import App.Support.Money;` was `E-MODULE-NOT-FOUND`
/// because one file cannot see its siblings.
#[test]
fn a_lifted_project_resolves_its_own_cross_file_imports() {
    let t = Tmp::new("xfile");
    t.write(
        "composer.json",
        r#"{ "name": "acme/blog", "autoload": { "psr-4": { "Acme\\Blog\\": "src/" } } }"#,
    );
    t.write(
        "src/Support/Money.php",
        "<?php\nnamespace Acme\\Blog\\Support;\nclass Money { public function show(): string { return \"m\"; } }\n",
    );
    t.write(
        "src/Entity/Post.php",
        "<?php\nnamespace Acme\\Blog\\Entity;\nuse Acme\\Blog\\Support\\Money;\nclass Post { public function d(Money $m): string { return $m->show(); } }\n",
    );
    t.write(
        "src/index.php",
        "<?php\nnamespace Acme\\Blog;\nuse Acme\\Blog\\Entity\\Post;\n$p = new Post();\necho \"ok\\n\";\n",
    );
    let out = t.path("out");
    lift_directory(&t.0, &out, VendorMode::Report).expect("the directory lifts");

    // The entry becomes `package Main;` at the source root. Not cosmetic: a dotted package must sit in a
    // matching subdirectory (`E-PKG-PATH`), so an entry left in its namespace package makes the whole
    // project fail to LOAD.
    let main = out.join("src/main.phg");
    assert!(main.is_file(), "expected src/main.phg");
    assert!(read(&main).contains("package Main;"), "{}", read(&main));

    // …and the whole project type-checks, which is the thing single-file lifting could not achieve.
    let unit = phorj::loader::load(&main).expect("the lifted project loads");
    phorj::cli::check_and_expand(&unit.program, &unit.diag_src)
        .expect("the lifted project type-checks");
}

/// `vendor/` is never walked — lifting a dependency tree is explicitly not what a directory lift does.
#[test]
fn vendor_is_never_walked() {
    let t = Tmp::new("vendor");
    t.write("src/Own.php", "<?php\nclass Own {}\n");
    t.write("vendor/acme/lib/Dep.php", "<?php\nclass Dep {}\n");
    let out = t.path("out");
    lift_directory(&t.0, &out, VendorMode::Report).expect("lifts");
    let report = read(&out.join("LIFT-REPORT.md"));
    assert!(
        !report.contains("Dep.php"),
        "a vendor file must not even be listed:\n{report}"
    );
}

/// Files outside composer's autoload map must be NAMED, not silently ignored. Before this, the report
/// counted "files I looked at" and called it "files that exist" — on a Symfony-shaped tree, 8 PHP files
/// present and 4 examined.
///
/// `bin/console` is the sharp case: it has NO extension, so only a content check can see it at all.
#[test]
fn files_outside_the_autoload_map_are_reported_including_extensionless_ones() {
    let t = Tmp::new("outside");
    t.write(
        "composer.json",
        r#"{ "name": "acme/app", "autoload": { "psr-4": { "App\\": "src/" } } }"#,
    );
    t.write("src/Thing.php", "<?php\nnamespace App;\nclass Thing {}\n");
    t.write("public/index.php", "<?php\nrequire __DIR__.'/x.php';\n");
    t.write("bin/console", "#!/usr/bin/env php\n<?php\necho 1;\n");
    let out = t.path("out");
    lift_directory(&t.0, &out, VendorMode::Report).expect("lifts");
    let report = read(&out.join("LIFT-REPORT.md"));
    assert!(report.contains("public/index.php"), "{report}");
    assert!(
        report.contains("bin/console"),
        "an extensionless PHP file must be found by CONTENT:\n{report}"
    );
}

/// Two sources mapping to the same package AND stem used to overwrite each other, and the summary still
/// said "lifted 2/2" — a silent data loss. Legacy PHP hits this constantly: every namespace-less file
/// lands in `package Main` and collides on its bare stem.
#[test]
fn a_destination_collision_renames_rather_than_overwriting() {
    let t = Tmp::new("collide");
    t.write(
        "composer.json",
        r#"{ "name": "acme/coll", "autoload": { "psr-4": { "App\\": "src/" } } }"#,
    );
    t.write(
        "src/A/Helper.php",
        "<?php\nnamespace App;\nclass FromA { public function w(): string { return \"A\"; } }\n",
    );
    t.write(
        "src/B/Helper.php",
        "<?php\nnamespace App;\nclass FromB { public function w(): string { return \"B\"; } }\n",
    );
    let out = t.path("out");
    lift_directory(&t.0, &out, VendorMode::Report).expect("lifts");

    // BOTH classes must survive somewhere under the package directory.
    let dir = out.join("src/App");
    let mut bodies = String::new();
    for e in std::fs::read_dir(&dir).expect("read_dir") {
        bodies.push_str(&read(&e.expect("entry").path()));
    }
    assert!(bodies.contains("class FromA"), "FromA was lost:\n{bodies}");
    assert!(bodies.contains("class FromB"), "FromB was lost:\n{bodies}");
    // …and the rename is reported, because the draft is not where a reader would look for it.
    let report = read(&out.join("LIFT-REPORT.md"));
    assert!(
        report.contains("renamed to avoid a collision"),
        "the rename must be disclosed:\n{report}"
    );
}

/// A symlinked cycle must terminate. A depth cap alone does NOT achieve that — the cycle re-walks the whole
/// subtree at every level, so bounded depth is still exponential; directory symlinks are skipped instead.
/// [Measured: with the cap alone this ran until killed at 30s and reported 41 files for a 1-file tree.]
#[test]
#[cfg(unix)]
fn a_symlink_cycle_terminates() {
    let t = Tmp::new("cycle");
    t.write("src/X.php", "<?php\nclass X {}\n");
    std::os::unix::fs::symlink("..", t.path("src/up")).expect("symlink");
    let out = t.path("out");
    let summary = lift_directory(&t.0, &out, VendorMode::Report).expect("lifts");
    assert!(
        summary.contains("lifted 1/1"),
        "the cycle inflated the file count:\n{summary}"
    );
}

/// `--vendor=stub` is RULED but not implemented. It must refuse with the reason rather than silently
/// behaving like the default — a flag that quietly does something else is worse than one that says
/// "not yet".
#[test]
fn vendor_stub_refuses_with_a_reason_rather_than_silently_reporting() {
    let t = Tmp::new("stub");
    t.write("src/X.php", "<?php\nclass X {}\n");
    let err = lift_directory(&t.0, &t.path("out"), VendorMode::Stub)
        .expect_err("--vendor=stub must refuse");
    assert!(err.contains("not implemented yet"), "{err}");
    assert!(err.contains("DEC-439"), "{err}");
}

/// A directory lift writes a whole tree, so it must not overwrite an existing one.
#[test]
fn a_non_empty_output_directory_is_refused() {
    let t = Tmp::new("nonempty");
    t.write("src/X.php", "<?php\nclass X {}\n");
    t.write("out/keep.txt", "important");
    let err = lift_directory(&t.0, &t.path("out"), VendorMode::Report)
        .expect_err("a non-empty output must be refused");
    assert!(err.contains("not empty"), "{err}");
}

/// A tree with no PHP at all is a loud error, not an empty project.
#[test]
fn a_tree_with_no_php_is_refused() {
    let t = Tmp::new("nophp");
    t.write("README.md", "# nothing here");
    let err = lift_directory(&t.0, &t.path("out"), VendorMode::Report)
        .expect_err("no PHP must be refused");
    assert!(err.contains("no `.php` files found"), "{err}");
}
