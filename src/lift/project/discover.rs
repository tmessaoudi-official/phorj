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

/// Directories never walked, whatever `composer.json` says. `vendor/` is the important one: lifting a
/// dependency tree is explicitly NOT what a directory lift does (DEC-439 — vendor is reported, and
/// optionally stubbed, never forked).
const NEVER_WALK: &[&str] = &["vendor", "node_modules", ".git", ".github", "var", "cache"];

/// Directory-nesting bound for both walks — a backstop only. The real cycle defence is [`is_real_dir`]:
/// a depth cap alone does NOT save you, because a symlinked cycle re-walks the whole subtree at every
/// level, so 64 levels is exponential rather than merely deep. [Verified: with the cap alone and
/// `src/up -> ..`, the lift had to be killed at 30s.] 64 is far past any real source layout.
const MAX_WALK_DEPTH: usize = 64;

/// A directory that is NOT a symlink.
///
/// Symlinked directories are skipped rather than followed, which is what makes a cyclic tree
/// (`src/up -> ..`) terminate at all. It also avoids lifting the same file twice under two paths — a
/// symlinked source tree is ordinary in PHP projects (shared library dirs, `vendor` overlays), and the
/// duplicate would then collide on its destination and be renamed for no reason.
fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

/// What `composer.json` told us about the project.
#[derive(Debug, Default)]
pub(super) struct Composer {
    /// `autoload.psr-4` + `autoload-dev.psr-4`: (namespace prefix without the trailing `\`, relative dir).
    /// Sorted LONGEST-PREFIX-FIRST, because PSR-4 resolution is longest-match and `App\Domain\` must win
    /// over `App\`.
    pub(super) psr4: Vec<(String, String)>,
    /// Dependency package names (`vendor/name`) from `require`, sorted. Used to attribute a vendor symbol
    /// to a package in the report.
    pub(super) requires: Vec<String>,
    /// `installed.json`'s namespace-prefix → package-name map, when a `vendor/` tree is present. This is
    /// what makes attribution exact rather than a guess from the first namespace segment.
    pub(super) installed_psr4: Vec<(String, String)>,
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
                if let Some(psr4) = j.get(key).and_then(|a| a.get("psr-4")) {
                    collect_psr4(psr4, &mut out.psr4);
                }
            }
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
        out.psr4
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
        out.requires.sort();
        out.requires.dedup();
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
    /// With no `composer.json` there are no prefixes, so nothing is "app" by prefix — the caller falls
    /// back to "was it declared by a file we lifted", which is the only sound answer available then.
    pub(super) fn is_app_namespace(&self, fqn: &str) -> bool {
        self.psr4
            .iter()
            .any(|(prefix, _)| fqn.starts_with(prefix.as_str()))
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
/// [`NEVER_WALK`]. Either way `vendor/` is never entered.
pub(super) fn app_php_files(root: &Path, composer: &Composer) -> Result<Vec<PathBuf>, String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for (_, dir) in &composer.psr4 {
        let d = root.join(dir);
        if d.is_dir() && !roots.contains(&d) {
            roots.push(d);
        }
    }
    if roots.is_empty() {
        roots.push(root.to_path_buf());
    }
    let mut out = Vec::new();
    for r in &roots {
        walk(r, &mut out, 0)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Every file in the tree that IS PHP, whatever it is called and wherever it sits (minus
/// [`NEVER_WALK`]) — the denominator [`app_php_files`] alone cannot provide.
///
/// PHP-ness is decided by CONTENT, not extension, because the files this catches are exactly the ones an
/// extension filter cannot: `bin/console` and Laravel's `artisan` have no extension at all. A PHP file
/// starts with `<?php`, optionally after a `#!` line, so reading the first bytes is an exact test rather
/// than a guess.
pub(super) fn all_php_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    walk_any(root, &mut candidates, 0)?;
    let mut out: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("php") || starts_with_php_open_tag(p)
        })
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

/// Does this file open with `<?php` (allowing a leading `#!` shebang line)? Reads a small prefix only.
fn starts_with_php_open_tag(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let head = &bytes[..bytes.len().min(256)];
    let Ok(text) = std::str::from_utf8(head) else {
        return false;
    };
    // A shebang may precede the open tag (`#!/usr/bin/env php`), so look within the prefix rather than
    // requiring the very first bytes.
    text.contains("<?php")
}

/// Like [`walk`] but collects EVERY file, not just `.php` ones — the caller decides PHP-ness by content.
fn walk_any(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?;
    let mut names: Vec<PathBuf> = Vec::new();
    for e in entries {
        names.push(
            e.map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?
                .path(),
        );
    }
    names.sort();
    for path in names {
        if is_real_dir(&path) {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| NEVER_WALK.contains(&n));
            if !skip {
                walk_any(&path, out, depth + 1)?;
            }
        } else if !path.is_dir() {
            // A symlink TO a directory lands here and is correctly not collected as a file.
            out.push(path);
        }
    }
    Ok(())
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?;
    // Collected then sorted: `read_dir` order is filesystem-dependent, and a lift that varies with it
    // would produce different reports on different machines for the same input.
    let mut names: Vec<PathBuf> = Vec::new();
    for e in entries {
        names.push(
            e.map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?
                .path(),
        );
    }
    names.sort();
    for path in names {
        if is_real_dir(&path) {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| NEVER_WALK.contains(&n));
            if !skip {
                walk(&path, out, depth + 1)?;
            }
        } else if !path.is_dir() && path.extension().and_then(|e| e.to_str()) == Some("php") {
            out.push(path);
        }
    }
    Ok(())
}
