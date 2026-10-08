//! Lock pinning (audit 2026-10-07, A4): `phg install` reproduces `phorj.lock` instead of re-resolving.
//! A git or registry dependency the lock still governs is fetched at its locked version and must come
//! back with the locked commit and tree hash; anything else is a hard error that only `phg update`
//! (which resolves with no lock) accepts. Path dependencies are the developer's own working trees and
//! are never pinned. Before this, `install` re-resolved every time and then verified the tree against
//! the lock it had just written, so a moved tag or a changed index silently produced a new lock.

use crate::pm::lockfile::{LockFile, LockedPackage};
use crate::pm::manifest::{Dependency, SourceSpec};
use crate::pm::registry::{RegistryIndex, RegistryVersion};
use crate::pm::semver::Version;
use crate::pm::LOCK_FILE;

/// The lock entry that still governs `dep`: same name, and a source the manifest has not changed —
/// the same `git:url@ref`, or a registry version the constraint still accepts. A dependency whose
/// spec changed re-resolves on its own; every other pin holds.
pub(crate) fn governing<'l>(
    lock: Option<&'l LockFile>,
    dep: &Dependency,
) -> Option<&'l LockedPackage> {
    let locked = lock?.get(&dep.name)?;
    let unchanged = match &dep.source {
        SourceSpec::Path(_) => false,
        SourceSpec::Git { url, git_ref } => locked.source == format!("git:{url}@{git_ref}"),
        SourceSpec::Registry(req) => {
            locked.source == "registry"
                && Version::parse(&locked.version).is_ok_and(|v| req.matches(&v))
        }
    };
    unchanged.then_some(locked)
}

/// The index's record of a locked registry version. A version the index no longer lists is a hard
/// error, never a silent move to another one.
pub(crate) fn locked_version<'i>(
    idx: &'i RegistryIndex,
    locked: &LockedPackage,
) -> Result<(&'i str, RegistryVersion), String> {
    idx.entry(&locked.name)
        .and_then(|entry| {
            entry
                .versions
                .iter()
                .find(|rv| rv.version.to_string() == locked.version)
                .map(|rv| (entry.git.as_str(), rv.clone()))
        })
        .ok_or_else(|| {
            format!(
                "`{}` is locked at {} in {LOCK_FILE}, but the registry index no longer lists that \
                 version — run `phg update` to re-resolve",
                locked.name, locked.version
            )
        })
}

/// A pinned dependency must come back exactly as locked: same commit, same tree hash.
pub(crate) fn check_fetched(
    locked: &LockedPackage,
    commit: Option<&str>,
    hash: &str,
) -> Result<(), String> {
    if locked.commit.as_deref() == commit && locked.hash == hash {
        return Ok(());
    }
    Err(format!(
        "`{}` no longer matches {LOCK_FILE}: locked {} at commit {} (tree {}), fetched commit {} \
         (tree {hash}) — its tag moved or its source changed; run `phg update` to accept the new tree",
        locked.name,
        locked.version,
        locked.commit.as_deref().unwrap_or("-"),
        locked.hash,
        commit.unwrap_or("-"),
    ))
}
