//! Dependency resolution (DEC-316): walk the root manifest's `require` transitively, fetch each dep
//! (path / git / registry→git) into a staging tree, dedupe by name, and detect conflicts. Produces the
//! resolved set that `pm::vendor` (phase 3) materializes into `vendor/` + records in `phorj.lock`.
//!
//! v1 policy (honest, documented): **first-come version wins**; a later, incompatible constraint on an
//! already-resolved package is a hard conflict error (align the constraints) rather than a full
//! back-tracking SAT solve. Cycles terminate (dedupe by name). Registry constraint intersection across
//! multiple requirers is a documented follow-up.

use crate::pm::fetch::{fetch_git, fetch_path, Fetched};
use crate::pm::lockfile::LockedPackage;
use crate::pm::manifest::{Dependency, Manifest, SourceSpec};
use crate::pm::registry::RegistryIndex;
use crate::pm::semver::Version;
use crate::pm::MANIFEST_FILE;
use std::path::{Path, PathBuf};

/// A resolved package: its lock record + the staged tree ready to be vendored.
#[derive(Debug)]
pub struct Resolved {
    pub locked: LockedPackage,
    pub dir: PathBuf,
    /// Physical dedup key (canonical path for a path dep; `git:url@ref`; `registry:Name`) — two
    /// requirers reaching the SAME package via different relative paths share this, so they dedup
    /// rather than false-conflict.
    ident: String,
}

#[derive(Clone)]
struct Item {
    dep: Dependency,
    /// The directory of the manifest that declared this dep (path deps resolve relative to it).
    from_dir: PathBuf,
}

/// Resolve `root`'s dependencies transitively, staging fetched trees under `stage`.
pub fn resolve(root: &Manifest, root_dir: &Path, stage: &Path) -> Result<Vec<Resolved>, String> {
    let mut queue: Vec<Item> = root
        .require
        .iter()
        .map(|d| Item {
            dep: d.clone(),
            from_dir: root_dir.to_path_buf(),
        })
        .collect();
    let mut out: Vec<Resolved> = Vec::new();
    let mut registry: Option<RegistryIndex> = None;

    let mut head = 0;
    while head < queue.len() {
        let Item { dep, from_dir } = queue[head].clone();
        head += 1;

        // Physical identity for dedup/conflict (path deps by canonical dir, not rel string).
        let ident = match &dep.source {
            SourceSpec::Path(rel) => {
                let c = from_dir
                    .join(rel)
                    .canonicalize()
                    .map_err(|e| format!("path dependency `{rel}` not found: {e}"))?;
                format!("path:{}", c.display())
            }
            SourceSpec::Git { url, git_ref } => format!("git:{url}@{git_ref}"),
            SourceSpec::Registry(_) => format!("registry:{}", dep.name),
        };

        if let Some(existing) = out.iter().find(|r| r.locked.name == dep.name) {
            check_compat(&dep, existing, &ident)?;
            continue; // already resolved (dedupe / cycle break)
        }

        let dest = pkg_stage_dir(stage, &dep.name)?;
        let _ = std::fs::remove_dir_all(&dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("stage mkdir: {e}"))?;
        }

        let (fetched, version, source): (Fetched, String, String) = match &dep.source {
            SourceSpec::Path(rel) => (
                fetch_path(&from_dir, rel, &dest)?,
                "path".to_string(),
                format!("path:{rel}"),
            ),
            SourceSpec::Git { url, git_ref } => (
                fetch_git(url, git_ref, &dest)?,
                git_ref.clone(),
                format!("git:{url}@{git_ref}"),
            ),
            SourceSpec::Registry(req) => {
                if registry.is_none() {
                    registry = Some(crate::pm::registry::fetch_index()?);
                }
                let idx = registry.as_ref().unwrap();
                let (git, rv) = idx.resolve(&dep.name, req)?;
                (
                    fetch_git(git, &rv.tag, &dest)?,
                    rv.version.to_string(),
                    "registry".to_string(),
                )
            }
        };

        // Recurse into the fetched package's own manifest (if any). A path dep declared inside this
        // package is relative to its ORIGINAL directory, not the staged copy — so for a path source the
        // children resolve against the original tree; for git/registry the repo root IS the staged dir.
        let children_base = match &dep.source {
            SourceSpec::Path(rel) => from_dir
                .join(rel)
                .canonicalize()
                .unwrap_or_else(|_| dest.clone()),
            _ => dest.clone(),
        };
        let sub_manifest = dest.join(MANIFEST_FILE);
        if sub_manifest.is_file() {
            let txt = std::fs::read_to_string(&sub_manifest)
                .map_err(|e| format!("read {}'s manifest: {e}", dep.name))?;
            let sub = Manifest::parse(&txt).map_err(|e| format!("{}: {e}", dep.name))?;
            for d in sub.require {
                queue.push(Item {
                    dep: d,
                    from_dir: children_base.clone(),
                });
            }
        }

        out.push(Resolved {
            locked: LockedPackage {
                name: dep.name.clone(),
                version,
                source,
                commit: fetched.commit.clone(),
                hash: fetched.hash.clone(),
            },
            dir: fetched.dir,
            ident,
        });
    }
    Ok(out)
}

/// Staging dir for `Publisher/Name` under `stage` (mirrors the eventual `vendor/` layout).
fn pkg_stage_dir(stage: &Path, name: &str) -> Result<PathBuf, String> {
    let (publisher, pkg) = name
        .split_once('/')
        .ok_or_else(|| format!("bad package name `{name}`"))?;
    Ok(stage.join(publisher).join(pkg))
}

/// Is `dep` compatible with the already-resolved `existing`? First-come wins; an incompatible re-require
/// is a hard error naming the clash. Registry deps re-check the constraint against the resolved version;
/// git/path deps compare physical identity (`ident`), so the same package reached two ways deduplicates.
fn check_compat(dep: &Dependency, existing: &Resolved, ident: &str) -> Result<(), String> {
    let conflict = |want: &str| {
        Err(format!(
            "dependency conflict on `{}`: already resolved to `{}` ({}), but also required as {want} \
             — align the constraints",
            dep.name, existing.locked.version, existing.locked.source
        ))
    };
    match &dep.source {
        SourceSpec::Registry(req) => {
            let v = Version::parse(&existing.locked.version)
                .map_err(|e| format!("`{}` resolved version unparseable: {e}", dep.name))?;
            if req.matches(&v) {
                Ok(())
            } else {
                conflict("an incompatible registry constraint")
            }
        }
        SourceSpec::Git { .. } => {
            if existing.ident == ident {
                Ok(())
            } else {
                conflict("a different git source")
            }
        }
        SourceSpec::Path(_) => {
            if existing.ident == ident {
                Ok(())
            } else {
                conflict("a different path source")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("phorj_pm_res_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn write_pkg(dir: &Path, pkg: &str, manifest: Option<&str>) {
        std::fs::create_dir_all(dir).unwrap();
        // a minimal valid .phg so the tree is real
        std::fs::write(dir.join("mod.phg"), format!("package {pkg};").as_bytes()).unwrap();
        if let Some(m) = manifest {
            std::fs::write(dir.join(MANIFEST_FILE), m).unwrap();
        }
    }

    #[test]
    fn resolves_transitive_path_deps_and_dedupes() {
        let base = tmp("proj");
        // Acme/A depends on Acme/B; root depends on both A and B (B appears twice → dedupe).
        let a = base.join("pkgs/A");
        let b = base.join("pkgs/B");
        write_pkg(&b, "Acme.B", None);
        write_pkg(
            &a,
            "Acme.A",
            Some(r#"{"name":"Acme/A","require":{"Acme/B":{"path":"../B"}}}"#),
        );
        let root = Manifest::parse(
            r#"{"name":"Acme/App","require":{"Acme/A":{"path":"pkgs/A"},"Acme/B":{"path":"pkgs/B"}}}"#,
        )
        .unwrap();
        let stage = base.join("stage");

        let resolved = resolve(&root, &base, &stage).unwrap();
        let names: Vec<&str> = resolved.iter().map(|r| r.locked.name.as_str()).collect();
        assert!(names.contains(&"Acme/A"));
        assert!(names.contains(&"Acme/B"));
        assert_eq!(names.iter().filter(|n| **n == "Acme/B").count(), 1); // deduped
        for r in &resolved {
            assert_eq!(r.locked.hash.len(), 64);
            assert!(r.dir.join("mod.phg").exists());
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn conflicting_path_sources_error() {
        let base = tmp("conflict");
        write_pkg(&base.join("x1"), "Acme.X", None);
        write_pkg(&base.join("x2"), "Acme.X", None);
        // A requires X via x1; root requires X via x2 → conflict.
        write_pkg(
            &base.join("a"),
            "Acme.A",
            Some(r#"{"name":"Acme/A","require":{"Acme/X":{"path":"../x1"}}}"#),
        );
        let root = Manifest::parse(
            r#"{"name":"Acme/App","require":{"Acme/A":{"path":"a"},"Acme/X":{"path":"x2"}}}"#,
        )
        .unwrap();
        let err = resolve(&root, &base, &base.join("stage")).unwrap_err();
        assert!(err.contains("conflict"), "got: {err}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
