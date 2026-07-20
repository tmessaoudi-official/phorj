//! The `phorj.lock` model (DEC-316) — the reproducible, integrity-pinned resolution result.
//!
//! Written by `phg install`/`update` after resolution; read by a later `phg install` to reproduce the
//! exact tree offline and verify each vendored package's **tree SHA-256** (`bundle::sha256`) — a
//! mismatch is a hard error (the `sha256.rs` "real security boundary"). JSON, deterministic (packages
//! sorted by name), so the lock diffs cleanly in review.

use crate::pm::json::Json;

/// One resolved, materialized package.
#[derive(Debug, Clone, PartialEq)]
pub struct LockedPackage {
    /// `Publisher/Name`.
    pub name: String,
    /// The concrete resolved version (registry/git tag) or `"path"` for a local dependency.
    pub version: String,
    /// Human-readable origin: `registry`, `git:<url>@<ref>`, or `path:<dir>`.
    pub source: String,
    /// The git commit SHA a git/registry package resolved to (absent for path deps).
    pub commit: Option<String>,
    /// SHA-256 over the sorted (relative-path, bytes) list of the materialized tree — the integrity pin.
    pub hash: String,
}

/// A parsed `phorj.lock`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LockFile {
    pub packages: Vec<LockedPackage>,
}

const LOCK_VERSION: &str = "1";

impl LockFile {
    pub fn parse(src: &str) -> Result<LockFile, String> {
        let j = Json::parse(src).map_err(|e| format!("phorj.lock: {e}"))?;
        let mut packages = Vec::new();
        if let Some(arr) = j.get("packages").and_then(Json::as_arr) {
            for p in arr {
                let field = |k: &str| p.get(k).and_then(Json::as_str).map(str::to_string);
                packages.push(LockedPackage {
                    name: field("name").ok_or("phorj.lock: package missing `name`")?,
                    version: field("version").ok_or("phorj.lock: package missing `version`")?,
                    source: field("source").unwrap_or_default(),
                    commit: field("commit"),
                    hash: field("hash").ok_or("phorj.lock: package missing `hash`")?,
                });
            }
        }
        Ok(LockFile { packages })
    }

    pub fn to_pretty(&self) -> String {
        let mut pkgs = self.packages.clone();
        pkgs.sort_by(|a, b| a.name.cmp(&b.name));
        let items: Vec<Json> = pkgs
            .iter()
            .map(|p| {
                let mut o = vec![
                    ("name".into(), Json::Str(p.name.clone())),
                    ("version".into(), Json::Str(p.version.clone())),
                    ("source".into(), Json::Str(p.source.clone())),
                ];
                if let Some(c) = &p.commit {
                    o.push(("commit".into(), Json::Str(c.clone())));
                }
                o.push(("hash".into(), Json::Str(p.hash.clone())));
                Json::Obj(o)
            })
            .collect();
        Json::Obj(vec![
            ("lockVersion".into(), Json::Str(LOCK_VERSION.into())),
            ("packages".into(), Json::Arr(items)),
        ])
        .to_pretty()
    }

    pub fn get(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_sorted() {
        let lf = LockFile {
            packages: vec![
                LockedPackage {
                    name: "Zed/Z".into(),
                    version: "1.0.0".into(),
                    source: "registry".into(),
                    commit: Some("abc123".into()),
                    hash: "deadbeef".into(),
                },
                LockedPackage {
                    name: "Acme/A".into(),
                    version: "path".into(),
                    source: "path:../a".into(),
                    commit: None,
                    hash: "cafe".into(),
                },
            ],
        };
        let out = lf.to_pretty();
        assert!(out.find("Acme/A").unwrap() < out.find("Zed/Z").unwrap()); // sorted
        let back = LockFile::parse(&out).unwrap();
        assert_eq!(back.get("Acme/A").unwrap().source, "path:../a");
        assert_eq!(back.get("Zed/Z").unwrap().commit.as_deref(), Some("abc123"));
        assert!(back.get("Acme/A").unwrap().commit.is_none());
    }

    #[test]
    fn empty_lock_parses() {
        assert!(LockFile::parse(r#"{"lockVersion":"1","packages":[]}"#)
            .unwrap()
            .packages
            .is_empty());
        assert!(LockFile::parse(r#"{"packages":[{"name":"A/B"}]}"#).is_err()); // missing hash
    }
}
