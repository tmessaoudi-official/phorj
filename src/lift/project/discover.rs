//! Directory lift — DISCOVERY: which `.php` files are the app's, and which symbols are vendor's.
//!
//! Both answers come from `composer.json`, which is machine-readable, so nothing here is a heuristic:
//!
//! * `autoload.psr-4` maps a namespace prefix to a directory (`"App\\": "src/"`). Those directories are
//!   the app; everything else — `vendor/` above all — is not.
//! * `require` (and `vendor/composer/installed.json` when present) names the dependencies, which is what
//!   lets a vendor symbol be reported under the package that actually ships it rather than under a bare
//!   namespace segment.
//!
//! When a project has no `composer.json` the whole tree is treated as app code minus the always-excluded
//! directories — the same answer a developer would give by hand for a plain PHP folder.

use crate::pm::json::Json;
use std::path::{Path, PathBuf};

/// What `composer.json` told us about the project.
#[derive(Debug, Default)]
pub(super) struct Composer {
    /// `autoload.psr-4` (and `psr-0`): (namespace prefix without the trailing `\`, relative dir). Sorted
    /// LONGEST-PREFIX-FIRST, because PSR-4 resolution is longest-match and `App\Domain\` must win over `App\`.
    ///
    /// `autoload-dev` is deliberately NOT merged in here — see [`Composer::dev_psr4`].
    pub(super) psr4: Vec<(String, String)>,
    /// `autoload-dev.psr-4` / `psr-0`, kept SEPARATE from [`Composer::psr4`] (DEC-439 part 3).
    ///
    /// Test code is the app's own, so its namespaces still count as app namespaces — a reference into them
    /// must not be reported as a composer dependency. But it is not LIFTED, because phorj has its own
    /// `phg test` surface and a lifted PHPUnit class would reference a framework that will never be ported.
    /// Two different questions, so two different lists.
    pub(super) dev_psr4: Vec<(String, String)>,
    /// Every directory and file declared by `autoload-dev` — its `psr-4`/`psr-0` targets, `classmap` and
    /// `files`. Path membership is how a file is recognized as test code.
    ///
    /// This is composer's OWN declaration, not a guess: nothing here matches a directory called `tests/`, so
    /// the no-hardcoded-framework-paths rule is intact. The honest limit is the converse — test code in a
    /// project that declares no `autoload-dev` is indistinguishable from application code, and is lifted.
    pub(super) dev_paths: Vec<String>,
    /// Dependency package names (`vendor/name`) from `require`, sorted. Used to attribute a vendor symbol
    /// to a package in the report.
    pub(super) requires: Vec<String>,
    /// `installed.json`'s namespace-prefix → package-name map, when a `vendor/` tree is present. This is
    /// what makes attribution exact rather than a guess from the first namespace segment.
    pub(super) installed_psr4: Vec<(String, String)>,
    /// `autoload.classmap` + `autoload-dev.classmap` — arbitrary directories and files, which is where a
    /// project's `migrations/` and legacy non-PSR-4 code is typically declared. Ignoring this key was the
    /// single largest reason app-owned code went unexamined.
    pub(super) classmap: Vec<String>,
    /// `autoload.files` — always-included files (helper/function libraries).
    pub(super) files: Vec<String>,
    /// The top-level `bin` key — declared executables.
    ///
    /// Deliberately NOT part of the app-code surface [`app_php_files`] walks: `autoload` says "this is my
    /// code", while `bin` says "this is a command", and the two are different claims. Feeding a console
    /// script to the lifter produced `lift parse error: require is Tier-2` on Symfony's `bin/console` —
    /// a refusal where the right answer was "this is a bootstrap script, here is the phorj entry that
    /// replaces it". The key is still read so a declared executable is CLASSIFIED even when the content
    /// sniff cannot see it.
    pub(super) bin: Vec<String>,
}

impl Composer {
    /// Read `<root>/composer.json` (absent = an empty map, not an error — a plain PHP folder is a valid
    /// input) plus `<root>/vendor/composer/installed.json` when it exists.
    pub(super) fn read(root: &Path) -> Result<Composer, String> {
        let mut out = Composer::default();
        let path = root.join("composer.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let j = Json::parse(&text).map_err(|e| format!("composer.json: {e}"))?;
            for key in ["autoload", "autoload-dev"] {
                let Some(section) = j.get(key) else { continue };
                let dev = key == "autoload-dev";
                let prefixes = if dev {
                    &mut out.dev_psr4
                } else {
                    &mut out.psr4
                };
                if let Some(psr4) = section.get("psr-4") {
                    collect_psr4(psr4, prefixes);
                }
                // PSR-0 is the legacy prefix scheme and still present in older projects; it maps a prefix
                // to a directory exactly like PSR-4 for the purposes of "which directories are the app's".
                if let Some(psr0) = section.get("psr-0") {
                    collect_psr4(psr0, prefixes);
                }
                // `classmap` and `files` are PATHS, not namespace prefixes — they say "this code is mine"
                // without saying what it is called, which is exactly the case a psr-4-only reading missed.
                if dev {
                    // Every dev target lands in one list: the only question asked of it is "is this file
                    // test code", and a prefix's directory answers that as well as a `classmap` entry does.
                    for (_, dir) in out.dev_psr4.clone() {
                        out.dev_paths.push(dir);
                    }
                    collect_paths(section.get("classmap"), &mut out.dev_paths);
                    collect_paths(section.get("files"), &mut out.dev_paths);
                } else {
                    collect_paths(section.get("classmap"), &mut out.classmap);
                    collect_paths(section.get("files"), &mut out.files);
                }
            }
            collect_paths(j.get("bin"), &mut out.bin);
            if let Some(req) = j.get("require").and_then(Json::as_obj) {
                for (name, _) in req {
                    // `php` and `ext-*` are platform requirements, not packages.
                    if name != "php" && !name.starts_with("ext-") {
                        out.requires.push(name.clone());
                    }
                }
            }
        }
        // Longest prefix first (PSR-4 is longest-match); ties broken by name so the order is total and
        // the walk is deterministic (Invariant 10).
        for prefixes in [&mut out.psr4, &mut out.dev_psr4] {
            prefixes.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
        }
        out.requires.sort();
        out.requires.dedup();
        for list in [
            &mut out.classmap,
            &mut out.files,
            &mut out.bin,
            &mut out.dev_paths,
        ] {
            list.sort();
            list.dedup();
        }
        out.installed_psr4 = read_installed(root)?;
        Ok(out)
    }

    /// The composer package that ships `fqn`, when `installed.json` says so — else `None`, and the report
    /// falls back to the namespace root. Longest-prefix match, same rule PHP's autoloader uses.
    pub(super) fn package_of(&self, fqn: &str) -> Option<&str> {
        self.installed_psr4
            .iter()
            .filter(|(prefix, _)| fqn.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, pkg)| pkg.as_str())
    }

    /// Is this fully-qualified class name part of the APP (i.e. under one of its PSR-4 prefixes)?
    ///
    /// **Dev prefixes count.** Test code is the app's own even though it is not lifted, so a reference into
    /// the test namespace is a sibling reference, not a composer dependency.
    ///
    /// With no `composer.json` there are no prefixes, so nothing is "app" by prefix — the caller falls
    /// back to "was it declared by a file we lifted", which is the only sound answer available then.
    pub(super) fn is_app_namespace(&self, fqn: &str) -> bool {
        self.psr4
            .iter()
            .chain(self.dev_psr4.iter())
            .any(|(prefix, _)| fqn.starts_with(prefix.as_str()))
    }

    /// Is `path` inside something composer declared under `autoload-dev` — i.e. is it TEST code?
    ///
    /// Checked before content classification, and it has to be: a PHPUnit class declares a class, so content
    /// alone calls it application code and lifts it.
    pub(super) fn is_dev_path(&self, root: &Path, path: &Path) -> bool {
        self.dev_paths.iter().any(|rel| {
            let declared = root.join(rel);
            path == declared || path.starts_with(&declared)
        })
    }
}

/// `{"App\\": "src/", "Test\\": ["a/", "b/"]}` — PSR-4 allows a string OR an array of dirs.
fn collect_psr4(psr4: &Json, out: &mut Vec<(String, String)>) {
    let Some(entries) = psr4.as_obj() else { return };
    for (prefix, target) in entries {
        // A PSR-4 prefix is stored WITH its trailing `\` in composer.json; drop it so prefixes compare
        // against a plain FQN.
        let prefix = prefix.trim_end_matches('\\').to_string();
        match target {
            Json::Str(dir) => out.push((prefix, dir.clone())),
            _ => {
                if let Some(dirs) = target.as_arr() {
                    for d in dirs {
                        if let Some(d) = d.as_str() {
                            out.push((prefix.clone(), d.to_string()));
                        }
                    }
                }
            }
        }
    }
}

/// A composer key holding either a single path or an array of paths (`classmap`, `files`, `bin`).
fn collect_paths(node: Option<&Json>, out: &mut Vec<String>) {
    let Some(node) = node else { return };
    if let Some(one) = node.as_str() {
        out.push(one.to_string());
        return;
    }
    if let Some(items) = node.as_arr() {
        for i in items {
            if let Some(p) = i.as_str() {
                out.push(p.to_string());
            }
        }
    }
}

/// `vendor/composer/installed.json` → (namespace prefix, package name), so a vendor symbol can be
/// attributed to the package that ships it. Absent or unparseable = an empty map: attribution degrades to
/// the namespace root, which is a weaker report but never a wrong one.
fn read_installed(root: &Path) -> Result<Vec<(String, String)>, String> {
    let path = root.join("vendor/composer/installed.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let j = Json::parse(&text).map_err(|e| format!("vendor/composer/installed.json: {e}"))?;
    // Composer 2 wraps the list in `{"packages": [...]}`; Composer 1 is a bare array.
    let packages = j
        .get("packages")
        .and_then(Json::as_arr)
        .or_else(|| j.as_arr())
        .unwrap_or(&[]);
    let mut out = Vec::new();
    for pkg in packages {
        let Some(name) = pkg.get("name").and_then(Json::as_str) else {
            continue;
        };
        if let Some(psr4) = pkg.get("autoload").and_then(|a| a.get("psr-4")) {
            let mut prefixes = Vec::new();
            collect_psr4(psr4, &mut prefixes);
            for (prefix, _) in prefixes {
                out.push((prefix, name.to_string()));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Every `.php` file that belongs to the app, in sorted order (Invariant 10 — a lift must not vary run to
/// run, and the reports are keyed on this order).
///
/// With PSR-4 prefixes the walk is restricted to their directories; without, the whole tree minus
/// the never-walked set (`super::walk`). Either way `vendor/` is never entered.
pub(super) fn app_php_files(root: &Path, composer: &Composer) -> Result<Vec<PathBuf>, String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut direct: Vec<PathBuf> = Vec::new();
    // Every part of composer's AUTOLOAD surface, not just `psr-4`: a `classmap` entry may be a directory OR
    // a single file, and `files` is always files. Reading all of them is what brings a project's legacy
    // non-PSR-4 code into scope without naming any framework's directories. `bin` is excluded on purpose —
    // see the field's own note.
    let declared = composer
        .psr4
        .iter()
        .map(|(_, d)| d.as_str())
        .chain(composer.classmap.iter().map(String::as_str))
        .chain(composer.files.iter().map(String::as_str));
    for rel in declared {
        let p = root.join(rel);
        if super::walk::is_real_dir(&p) {
            if !roots.contains(&p) {
                roots.push(p);
            }
        } else if p.is_file() && !direct.contains(&p) {
            direct.push(p);
        }
    }
    if roots.is_empty() && direct.is_empty() {
        roots.push(root.to_path_buf());
    }
    let mut out = direct;
    for r in &roots {
        super::walk::walk(r, &mut out, 0)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Every file in the tree that IS PHP, whatever it is called and wherever it sits (minus
/// the never-walked set) — the denominator [`app_php_files`] alone cannot provide.
///
/// PHP-ness is decided by CONTENT, not extension, because the files this catches are exactly the ones an
/// extension filter cannot: `bin/console` and Laravel's `artisan` have no extension at all. A PHP file
/// starts with `<?php`, optionally after a `#!` line, so reading the first bytes is an exact test rather
/// than a guess.
pub(super) fn all_php_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    super::walk::walk_any(root, &mut candidates, 0)?;
    let mut out: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| super::walk::is_php_file(p))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}
