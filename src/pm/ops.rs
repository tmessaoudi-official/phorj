//! High-level package-manager operations (DEC-316) — the verbs `phg add/install/update/remove` drive
//! (`crate::pm::ops`). These are the ONLY network-capable package paths; `phg run/check/transpile` stay
//! offline (Invariant 10). Each resolves from `phorj.json`, materializes `vendor/`, and writes the
//! reproducible `phorj.lock`.

use crate::pm::manifest::{validate_pkg_name, Dependency, Manifest, SourceSpec};
use crate::pm::resolve::resolve;
use crate::pm::vendor::{build_lock, materialize, verify};
use crate::pm::{LockFile, LOCK_FILE, MANIFEST_FILE};
use std::path::Path;

/// Outcome of an install-type operation, for the CLI to print.
#[derive(Debug)]
pub struct InstallReport {
    /// `Publisher/Name version`, sorted.
    pub installed: Vec<String>,
}

/// Resolve `phorj.json` → fetch + materialize `vendor/` → write `phorj.lock`, reproducing an
/// existing lock: every git/registry dependency it still governs must come back at its locked commit
/// and tree hash, or the install fails before `vendor/` or the lock is touched (audit A4; `pm::pin`).
/// A dependency whose spec changed in `phorj.json` re-resolves on its own. `add` and `remove` go
/// through here, so they keep every other pin too.
pub fn install(root: &Path) -> Result<InstallReport, String> {
    let lock = read_lock(root)?;
    resolve_and_vendor(root, lock.as_ref())
}

/// `phg update`: re-resolve from `phorj.json` ignoring the old lock, taking the newest satisfying
/// versions, and rewrite `phorj.lock` + `vendor/`. The only verb that accepts a moved tag.
pub fn update(root: &Path) -> Result<InstallReport, String> {
    resolve_and_vendor(root, None)
}

/// The existing lock, if any. One that exists but cannot be read is an error, never "no lock":
/// treating it as absent would silently re-resolve everything it pinned.
fn read_lock(root: &Path) -> Result<Option<LockFile>, String> {
    match std::fs::read_to_string(root.join(LOCK_FILE)) {
        Ok(txt) => LockFile::parse(&txt)
            .map(Some)
            .map_err(|e| format!("{e} — fix {LOCK_FILE}, or run `phg update` to rewrite it")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {LOCK_FILE}: {e}")),
    }
}

fn resolve_and_vendor(root: &Path, lock: Option<&LockFile>) -> Result<InstallReport, String> {
    let manifest = read_manifest(root)?;
    let stage = crate::tempdir::private_temp_dir("phorj-pm-stage")?;
    let vendored = vendor_from(&manifest, root, &stage, lock);
    let _ = std::fs::remove_dir_all(&stage); // on every path: the stage is ours alone
    let resolved = vendored?;
    let mut installed: Vec<String> = resolved
        .iter()
        .map(|r| format!("{} {}", r.locked.name, r.locked.version))
        .collect();
    installed.sort();
    Ok(InstallReport { installed })
}

fn vendor_from(
    manifest: &Manifest,
    root: &Path,
    stage: &Path,
    lock: Option<&LockFile>,
) -> Result<Vec<crate::pm::resolve::Resolved>, String> {
    let resolved = resolve(manifest, root, stage, lock)?;
    let vendor = root.join("vendor");
    materialize(&resolved, &vendor)?;
    let lock = build_lock(&resolved);
    std::fs::write(root.join(LOCK_FILE), lock.to_pretty())
        .map_err(|e| format!("cannot write {LOCK_FILE}: {e}"))?;
    verify(&vendor, &lock)?; // freshly materialized tree must match its own lock
    Ok(resolved)
}

/// Add (or replace) a dependency in `phorj.json`, then install. Creates a minimal manifest if none
/// exists so `phg add` works in a fresh project.
pub fn add(root: &Path, name: &str, source: SourceSpec) -> Result<InstallReport, String> {
    validate_pkg_name(name)?;
    let mut manifest = match std::fs::read_to_string(root.join(MANIFEST_FILE)) {
        Ok(txt) => Manifest::parse(&txt)?,
        Err(_) => Manifest {
            name: None,
            version: None,
            description: None,
            // A FRESH manifest is stamped with the current edition (DEC-321); existing
            // pre-edition manifests are never rewritten to add it.
            edition: Some(crate::pm::manifest::KNOWN_EDITIONS[0].to_string()),
            require: Vec::new(),
        },
    };
    manifest.require.retain(|d| d.name != name);
    manifest.require.push(Dependency {
        name: name.to_string(),
        source,
    });
    write_manifest(root, &manifest)?;
    install(root)
}

/// Remove a dependency from `phorj.json`, drop its vendored tree, then re-resolve.
pub fn remove(root: &Path, name: &str) -> Result<InstallReport, String> {
    let mut manifest = read_manifest(root)?;
    let before = manifest.require.len();
    manifest.require.retain(|d| d.name != name);
    if manifest.require.len() == before {
        return Err(format!("`{name}` is not a dependency in {MANIFEST_FILE}"));
    }
    write_manifest(root, &manifest)?;
    if let Some((publisher, pkg)) = name.split_once('/') {
        let _ = std::fs::remove_dir_all(root.join("vendor").join(publisher).join(pkg));
    }
    install(root)
}

/// Offline integrity check of `vendor/` against `phorj.lock` (no network) — the reproducible gate.
pub fn verify_locked(root: &Path) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join(LOCK_FILE))
        .map_err(|e| format!("no {LOCK_FILE} to verify against: {e}"))?;
    let lock = LockFile::parse(&txt)?;
    verify(&root.join("vendor"), &lock)
}

fn read_manifest(root: &Path) -> Result<Manifest, String> {
    let txt = std::fs::read_to_string(root.join(MANIFEST_FILE))
        .map_err(|e| format!("no {MANIFEST_FILE} in {}: {e}", root.display()))?;
    Manifest::parse(&txt)
}

fn write_manifest(root: &Path, m: &Manifest) -> Result<(), String> {
    std::fs::write(root.join(MANIFEST_FILE), m.to_pretty())
        .map_err(|e| format!("cannot write {MANIFEST_FILE}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("phorj_pm_ops_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn install_add_remove_flow_with_path_deps() {
        let root = tmp("app");
        std::fs::create_dir_all(&root).unwrap();
        // a local package to depend on
        let libdir = root.join("libs/Util");
        std::fs::create_dir_all(&libdir).unwrap();
        std::fs::write(libdir.join("mod.phg"), b"package Acme.Util;").unwrap();
        // app manifest requiring it by path
        std::fs::write(
            root.join(MANIFEST_FILE),
            r#"{"name":"Acme/App","require":{"Acme/Util":{"path":"libs/Util"}}}"#,
        )
        .unwrap();

        let rep = install(&root).unwrap();
        assert_eq!(rep.installed, vec!["Acme/Util path".to_string()]);
        assert!(root.join("vendor/Acme/Util/mod.phg").exists());
        assert!(root.join(LOCK_FILE).exists());
        // offline verify passes on a fresh install
        verify_locked(&root).unwrap();

        // add a second path dep via the API
        std::fs::create_dir_all(root.join("libs/Extra")).unwrap();
        std::fs::write(root.join("libs/Extra/mod.phg"), b"package Acme.Extra;").unwrap();
        add(&root, "Acme/Extra", SourceSpec::Path("libs/Extra".into())).unwrap();
        assert!(root.join("vendor/Acme/Extra/mod.phg").exists());
        let manifest = std::fs::read_to_string(root.join(MANIFEST_FILE)).unwrap();
        assert!(manifest.contains("Acme/Extra"));

        // remove it
        remove(&root, "Acme/Extra").unwrap();
        assert!(!root.join("vendor/Acme/Extra").exists());
        assert!(!std::fs::read_to_string(root.join(MANIFEST_FILE))
            .unwrap()
            .contains("Acme/Extra"));

        // removing a non-dependency errors
        assert!(remove(&root, "No/Such").is_err());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_refuses_an_unreadable_lock_and_update_rewrites_it() {
        let root = tmp("badlock");
        std::fs::create_dir_all(root.join("libs/Util")).unwrap();
        std::fs::write(root.join("libs/Util/mod.phg"), b"package Acme.Util;").unwrap();
        std::fs::write(
            root.join(MANIFEST_FILE),
            r#"{"name":"Acme/App","require":{"Acme/Util":{"path":"libs/Util"}}}"#,
        )
        .unwrap();
        std::fs::write(root.join(LOCK_FILE), "not a lock").unwrap();
        let err = install(&root).unwrap_err();
        assert!(
            err.contains("phorj.lock") && err.contains("phg update"),
            "{err}"
        );
        update(&root).unwrap();
        let lock = std::fs::read_to_string(root.join(LOCK_FILE)).unwrap();
        assert!(LockFile::parse(&lock).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_without_manifest_errors() {
        let root = tmp("empty");
        std::fs::create_dir_all(&root).unwrap();
        assert!(install(&root).unwrap_err().contains("no phorj.json"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
