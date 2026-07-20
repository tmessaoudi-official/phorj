//! Vendor materialization + integrity verification (DEC-316, phase 3). Copies resolved packages into
//! `vendor/<Publisher>/<Name>/` — the read-only third search root the DEC-282 loader already consumes —
//! and turns the resolution into a `phorj.lock`. A later offline `phg install` re-verifies each
//! vendored tree's SHA-256 against the lock (the `bundle::sha256` "real security boundary"): a mismatch
//! (tampered or stale `vendor/`) is a hard refusal.

use crate::pm::fetch::{copy_tree, tree_hash};
use crate::pm::lockfile::LockFile;
use crate::pm::resolve::Resolved;
use std::path::Path;

/// Copy every resolved package's staged tree into `vendor/<Publisher>/<Name>/` (replacing any prior
/// copy). The staged trees have already had `.git` stripped and been integrity-hashed by `pm::fetch`.
pub fn materialize(resolved: &[Resolved], vendor_dir: &Path) -> Result<(), String> {
    for r in resolved {
        let dst = pkg_dir(vendor_dir, &r.locked.name)?;
        let _ = std::fs::remove_dir_all(&dst);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("vendor mkdir: {e}"))?;
        }
        copy_tree(&r.dir, &dst)?;
    }
    Ok(())
}

/// The lock record for a completed resolution.
pub fn build_lock(resolved: &[Resolved]) -> LockFile {
    LockFile {
        packages: resolved.iter().map(|r| r.locked.clone()).collect(),
    }
}

/// Verify every locked package is present in `vendor/` and its tree hash still matches the lock — the
/// reproducible-offline integrity gate. A mismatch is a hard error (never a silent re-fetch).
pub fn verify(vendor_dir: &Path, lock: &LockFile) -> Result<(), String> {
    for p in &lock.packages {
        let dir = pkg_dir(vendor_dir, &p.name)?;
        if !dir.is_dir() {
            return Err(format!(
                "locked package `{}` is not vendored (expected {}) — run `phg install`",
                p.name,
                dir.display()
            ));
        }
        let got = tree_hash(&dir)?;
        if got != p.hash {
            return Err(format!(
                "integrity check failed for `{}`: expected {}, got {} — the vendored tree was modified \
                 or is stale; refusing (re-run `phg install` to re-fetch)",
                p.name, p.hash, got
            ));
        }
    }
    Ok(())
}

fn pkg_dir(vendor_dir: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let (publisher, pkg) = name
        .split_once('/')
        .ok_or_else(|| format!("bad package name `{name}`"))?;
    Ok(vendor_dir.join(publisher).join(pkg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pm::manifest::Manifest;
    use crate::pm::resolve::resolve;
    use crate::pm::MANIFEST_FILE;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("phorj_pm_vendor_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn write_pkg(dir: &Path, pkg: &str, manifest: Option<&str>) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("mod.phg"), format!("package {pkg};").as_bytes()).unwrap();
        if let Some(m) = manifest {
            std::fs::write(dir.join(MANIFEST_FILE), m).unwrap();
        }
    }

    #[test]
    fn materialize_lock_and_verify_roundtrip() {
        let base = tmp("proj");
        write_pkg(&base.join("pkgs/B"), "Acme.B", None);
        write_pkg(
            &base.join("pkgs/A"),
            "Acme.A",
            Some(r#"{"name":"Acme/A","require":{"Acme/B":{"path":"../B"}}}"#),
        );
        let root = Manifest::parse(
            r#"{"name":"Acme/App","require":{"Acme/A":{"path":"pkgs/A"},"Acme/B":{"path":"pkgs/B"}}}"#,
        )
        .unwrap();
        let resolved = resolve(&root, &base, &base.join("stage")).unwrap();

        let vendor = base.join("vendor");
        materialize(&resolved, &vendor).unwrap();
        assert!(vendor.join("Acme/A/mod.phg").exists());
        assert!(vendor.join("Acme/B/mod.phg").exists());

        let lock = build_lock(&resolved);
        assert_eq!(lock.packages.len(), 2);
        // A fresh vendor tree verifies clean.
        verify(&vendor, &lock).unwrap();

        // Tampering a vendored file is caught.
        std::fs::write(
            vendor.join("Acme/B/mod.phg"),
            b"package Acme.B; // tampered",
        )
        .unwrap();
        let err = verify(&vendor, &lock).unwrap_err();
        assert!(err.contains("integrity check failed"), "got: {err}");

        // A missing package is caught.
        std::fs::remove_dir_all(vendor.join("Acme/B")).unwrap();
        assert!(verify(&vendor, &lock).unwrap_err().contains("not vendored"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
