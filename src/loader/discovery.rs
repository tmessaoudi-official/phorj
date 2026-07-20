//! Project-source discovery — the DEC-282 manifest-less search roots + the cheap package-declaration
//! index. Extracted from `loader/mod.rs` (M-Decomp, Invariant 13) to keep that file under cap and to
//! give the LSP a clean project-package enumeration accessor ([`project_packages`]) without growing the
//! load core. Load/resolution semantics are unchanged: `discover_roots` still returns exactly the three
//! ordered roots (entry-local + `src/` + `vendor/`); the `views/` scan lives ONLY in the LSP accessor
//! (completion listing), so it can never pull template dirs into a build.
use super::fs::{collect_phg, read_file};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// DEC-282 — the manifest-less search roots, discovered by walking UP from the entry file to the
/// nearest directory containing `src/` or `vendor/` (git-style, nearest wins; `src/` itself is the
/// root marker — no config, no sentinel file). No marker above → the entry's own directory is the
/// only root (lone scripts). The entry's own directory is ALWAYS the first (most specific) root.
pub(super) struct SearchRoots {
    /// Search root 1 — the entry file's own directory (entry-local packages, e.g. `bin/Commands/`).
    pub(super) entry_local: PathBuf,
    /// Search root 2 — `<approot>/src/` (the shared application code; package names strip `src/`).
    pub(super) src_root: Option<PathBuf>,
    /// Search root 3 — `<approot>/vendor/` (offline deps; folder = package, PascalCase mirror).
    pub(super) vendor_root: Option<PathBuf>,
}

pub(super) fn discover_roots(entry: &Path) -> SearchRoots {
    let entry_local = entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let mut cur: Option<&Path> = Some(&entry_local);
    while let Some(dir) = cur {
        let src = dir.join("src");
        let vend = dir.join("vendor");
        if src.is_dir() || vend.is_dir() {
            let src_root = src.is_dir().then_some(src);
            // The entry may itself live inside `src/` — the two roots collapse into one.
            let src_root = src_root.filter(|s| s != &entry_local);
            return SearchRoots {
                entry_local,
                src_root,
                vendor_root: vend.is_dir().then_some(vend),
            };
        }
        cur = dir.parent();
    }
    SearchRoots {
        entry_local,
        src_root: None,
        vendor_root: None,
    }
}

/// Cheap package-declaration peek — the declaration index reads only this, never full-parses a
/// file that is not imported. Skips a byte-0 shebang, whitespace, and `//`/`/* … */` comments,
/// then reads `package <dotted>;`. `None` for a file with no (or malformed) package line — such a
/// file is simply not indexable; if it is genuinely broken AND imported, the full parse of its
/// package's other files never claims it, and an import that needed it reports not-found.
pub(super) fn peek_package(src: &str) -> Option<String> {
    let mut s = src;
    if let Some(stripped) = s.strip_prefix("#!") {
        s = stripped.split_once('\n').map(|(_, rest)| rest)?;
    }
    loop {
        s = s.trim_start();
        if let Some(rest) = s.strip_prefix("//") {
            s = rest.split_once('\n').map(|(_, r)| r)?;
        } else if let Some(rest) = s.strip_prefix("/*") {
            s = rest.split_once("*/").map(|(_, r)| r)?;
        } else {
            break;
        }
    }
    let rest = s.strip_prefix("package")?;
    let (decl, _) = rest.split_once(';')?;
    let pkg = decl.trim();
    (!pkg.is_empty()).then(|| pkg.to_string())
}

/// Build one search root's declaration index: package name → its files (sorted — `collect_phg`
/// walks deterministically). `exclude` prunes subtrees owned by OTHER roots (when the app root IS
/// the entry's directory, `src/` and `vendor/` below it belong to roots 2/3, not root 1).
pub(super) fn index_packages(root: &Path, exclude: &[&Path]) -> BTreeMap<String, Vec<PathBuf>> {
    let mut idx: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let Ok(files) = collect_phg(root) else {
        return idx;
    };
    for f in files {
        if exclude.iter().any(|e| f.starts_with(e)) {
            continue;
        }
        let Ok(src) = read_file(&f) else { continue };
        if let Some(pkg) = peek_package(&src) {
            if pkg != "Main" {
                idx.entry(pkg).or_default().push(f);
            }
        }
    }
    idx
}

/// LSP project-package enumeration (2026-07-20 alignment pass): every user package name importable
/// from `entry`'s project — the DEC-282 search roots (entry-local + `src/` + `vendor/`) PLUS a sibling
/// `views/` dir if present. **Completion-only**: it does NOT change `discover_roots`, so listing a
/// `views/` package can never pull it into a build. Sorted + deduped; `Main` is already excluded by
/// `index_packages`. Errors (unreadable dirs) degrade to fewer results, never a failure.
pub(crate) fn project_packages(entry: &Path) -> Vec<String> {
    let roots = discover_roots(entry);
    let mut pkgs: BTreeSet<String> = BTreeSet::new();
    let mut add = |root: &Path| {
        for k in index_packages(root, &[]).into_keys() {
            pkgs.insert(k);
        }
    };
    add(&roots.entry_local);
    if let Some(s) = &roots.src_root {
        add(s);
    }
    if let Some(v) = &roots.vendor_root {
        add(v);
    }
    // `views/` sibling of the app root (parent of `src/`, else the entry's own dir) — completion-only.
    let approot = roots
        .src_root
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(&roots.entry_local);
    let views = approot.join("views");
    if views.is_dir() {
        add(&views);
    }
    pkgs.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::project_packages;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn scratch() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "phorj_discovery_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn project_packages_finds_src_and_views_excludes_main() {
        let root = scratch();
        std::fs::create_dir_all(root.join("src/App")).unwrap();
        std::fs::create_dir_all(root.join("views")).unwrap();
        std::fs::write(
            root.join("src/App/Models.phg"),
            "package App.Models;\nclass User {}\n",
        )
        .unwrap();
        std::fs::write(root.join("views/Home.phg"), "package Views.Home;\n").unwrap();
        std::fs::write(root.join("main.phg"), "package Main;\n").unwrap();

        let pkgs = project_packages(&root.join("main.phg"));
        assert!(
            pkgs.contains(&"App.Models".to_string()),
            "want App.Models (src/) in {pkgs:?}"
        );
        assert!(
            pkgs.contains(&"Views.Home".to_string()),
            "want Views.Home (views/) in {pkgs:?}"
        );
        assert!(
            !pkgs.contains(&"Main".to_string()),
            "package Main must be excluded: {pkgs:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}
