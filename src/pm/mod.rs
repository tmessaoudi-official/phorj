//! The phorj package manager (DEC-316) — the tool that POPULATES `vendor/<Publisher>/<Name>/`, the
//! read-only third search root the DEC-282 loader already consumes (`crate::loader`).
//!
//! Off the byte-identity spine (pure tooling — no transpile/lift). Third-party packages are userland
//! `.phg` source (DEC-315); this fetches + pins them. Design (dev-ruled, DEC-316):
//! - **Manifest** `phorj.json` — composer.json-style ([`manifest`]).
//! - **Three source kinds** unified so the compiler stays std-only (no `serde_json`/`flate2`): every
//!   fetch is a `git` checkout or a filesystem copy, and the central **registry is a name→git-URL
//!   index**. Registry (semver) / git (url+ref) / path (local) all land in `vendor/` ([`fetch`],
//!   [`registry`], [`resolve`]).
//! - **Lockfile** `phorj.lock` — reproducible, tree-SHA-256-pinned ([`lockfile`], reusing
//!   `bundle::sha256`).
//!
//! `phg` itself never networks for run/check/transpile (Invariant 10); only the `phg add/install/
//! update/remove` commands (`cli`) fetch, and only from an explicit source.

pub mod fetch;
pub mod json;
pub mod lockfile;
pub mod manifest;
pub mod ops;
pub(crate) mod pin;
pub mod registry;
pub mod resolve;
pub mod semver;
pub mod vendor;

pub use lockfile::{LockFile, LockedPackage};
pub use manifest::{Dependency, Manifest, SourceSpec};
pub use semver::{Version, VersionReq};

/// Canonical on-disk filenames.
pub const MANIFEST_FILE: &str = "phorj.json";
pub const LOCK_FILE: &str = "phorj.lock";

/// A fresh scratch directory under the system temp dir, for staging fetched packages (audit A4).
/// `create_dir`, never `create_dir_all`: an existing path, a symlink another user planted at a
/// guessed name included, is never reused — the name is retried instead. Owner-only on unix.
pub(crate) fn private_temp_dir(prefix: &str) -> Result<std::path::PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    #[cfg(unix)]
    let builder = {
        let mut b = std::fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
        b
    };
    #[cfg(not(unix))]
    let builder = std::fs::DirBuilder::new();
    // The clock only makes names harder to guess; uniqueness and safety come from the exclusive
    // create below, so a clock set before 1970 just yields a guessable name that still never reuses.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for _ in 0..16 {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("{prefix}-{}-{nanos:x}-{seq}", std::process::id()));
        match builder.create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot create a temp dir: {e}")),
        }
    }
    Err(format!(
        "cannot create a temp dir: 16 candidate names under {} already exist",
        std::env::temp_dir().display()
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_temp_dirs_are_fresh_and_owner_only() {
        let a = super::private_temp_dir("phorj-pm-test").unwrap();
        let b = super::private_temp_dir("phorj-pm-test").unwrap();
        assert_ne!(a, b, "every call gets its own directory");
        assert!(a.is_dir() && b.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&a).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "owner-only, got {mode:o}");
        }
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }
}
