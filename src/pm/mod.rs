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
pub mod registry;
pub mod resolve;
pub mod semver;

pub use lockfile::{LockFile, LockedPackage};
pub use manifest::{Dependency, Manifest, SourceSpec};
pub use semver::{Version, VersionReq};

/// Canonical on-disk filenames.
pub const MANIFEST_FILE: &str = "phorj.json";
pub const LOCK_FILE: &str = "phorj.lock";
