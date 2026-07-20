//! The registry client (DEC-316). The central "registry" is deliberately just a **name→git-URL
//! index** (crates.io-index / Go-proxy shape), so registry deps reduce to git deps and the compiler
//! needs no tarball decompression (std-only). The index is a single JSON doc; a hosted registry can
//! serve it over http(s) later, and `file://`/local paths drive hermetic tests — mirroring
//! `bundle::cross.rs`'s fetch. `PHORJ_REGISTRY` selects the index source.

use crate::pm::json::Json;
use crate::pm::semver::{Version, VersionReq};

/// One published version of a package: its semver + the git tag that materializes it.
#[derive(Debug, Clone)]
pub struct RegistryVersion {
    pub version: Version,
    pub tag: String,
}

/// A package's registry entry: the git repo + its published versions.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub git: String,
    pub versions: Vec<RegistryVersion>,
}

/// The parsed registry index.
#[derive(Debug, Clone, Default)]
pub struct RegistryIndex {
    entries: Vec<(String, RegistryEntry)>,
}

impl RegistryIndex {
    pub fn parse(src: &str) -> Result<RegistryIndex, String> {
        let j = Json::parse(src).map_err(|e| format!("registry index: {e}"))?;
        let pkgs = j
            .get("packages")
            .and_then(Json::as_obj)
            .ok_or("registry index: missing `packages` object")?;
        let mut entries = Vec::new();
        for (name, spec) in pkgs {
            let git = spec
                .get("git")
                .and_then(Json::as_str)
                .ok_or_else(|| format!("registry index: `{name}` missing `git` url"))?
                .to_string();
            let mut versions = Vec::new();
            let vs = spec
                .get("versions")
                .and_then(Json::as_arr)
                .ok_or_else(|| format!("registry index: `{name}` missing `versions` array"))?;
            for v in vs {
                let version =
                    Version::parse(v.get("version").and_then(Json::as_str).ok_or_else(|| {
                        format!("registry index: `{name}` version entry missing `version`")
                    })?)?;
                let tag = match v.get("tag").and_then(Json::as_str) {
                    Some(t) => t.to_string(),
                    None => format!("v{version}"),
                };
                versions.push(RegistryVersion { version, tag });
            }
            entries.push((name.clone(), RegistryEntry { git, versions }));
        }
        Ok(RegistryIndex { entries })
    }

    pub fn entry(&self, name: &str) -> Option<&RegistryEntry> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, e)| e)
    }

    /// The highest published version of `name` satisfying `req`; errors (listing the candidates) if
    /// none match, so a bad constraint fails loudly.
    pub fn resolve(&self, name: &str, req: &VersionReq) -> Result<(&str, RegistryVersion), String> {
        let entry = self
            .entry(name)
            .ok_or_else(|| format!("package `{name}` not found in the registry index"))?;
        let best = entry
            .versions
            .iter()
            .filter(|rv| req.matches(&rv.version))
            .max_by(|a, b| a.version.cmp(&b.version));
        match best {
            Some(rv) => Ok((&entry.git, rv.clone())),
            None => {
                let mut have: Vec<String> = entry
                    .versions
                    .iter()
                    .map(|v| v.version.to_string())
                    .collect();
                have.sort();
                Err(format!(
                    "no version of `{name}` satisfies the constraint (available: {})",
                    have.join(", ")
                ))
            }
        }
    }
}

/// Read the registry index from `PHORJ_REGISTRY` (default canonical URL). `file://` or a bare path is
/// read directly (hermetic tests); http(s) shells to `curl` into a temp file (like `bundle::cross`).
/// A bare/`file://` path may name the index file directly or a directory containing `index.json`.
pub fn fetch_index() -> Result<RegistryIndex, String> {
    let base = std::env::var("PHORJ_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY.to_string());
    let text = read_source(&base)?;
    RegistryIndex::parse(&text)
}

/// The canonical hosted index (not yet stood up — a registry dep with no `PHORJ_REGISTRY` set will
/// fail loudly against this until it exists; git/path deps work with no registry at all).
const DEFAULT_REGISTRY: &str = "https://packages.phorj.dev/index.json";

fn read_source(src: &str) -> Result<String, String> {
    // Local file or directory (file:// or bare path with no scheme).
    let local = if let Some(p) = src.strip_prefix("file://") {
        Some(p.to_string())
    } else if src.contains("://") {
        None
    } else {
        Some(src.to_string())
    };
    if let Some(p) = local {
        let mut path = std::path::PathBuf::from(&p);
        if path.is_dir() {
            path = path.join("index.json");
        }
        return std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read registry index {}: {e}", path.display()));
    }

    // http(s): shell to curl into a temp file, then read (std has no TLS — the cross.rs exemption).
    let curl = std::env::var("PHORJ_CURL").unwrap_or_else(|_| "curl".into());
    let tmp = std::env::temp_dir().join(format!("phorj_registry_{}.json", std::process::id()));
    let status = std::process::Command::new(&curl)
        .args(["-fSL", "--proto", "=https,http", "-o"])
        .arg(&tmp)
        .arg(src)
        .status()
        .map_err(|e| format!("cannot run `{curl}` to fetch the registry index: {e}"))?;
    if !status.success() {
        return Err(format!("registry index fetch failed ({status}) for {src}"));
    }
    let text = std::fs::read_to_string(&tmp)
        .map_err(|e| format!("cannot read fetched registry index: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDX: &str = r#"{
      "packages": {
        "Acme/Json": {
          "git": "https://example.test/acme/json.git",
          "versions": [
            { "version": "1.0.0", "tag": "v1.0.0" },
            { "version": "1.2.0", "tag": "v1.2.0" },
            { "version": "2.0.0", "tag": "v2.0.0" }
          ]
        }
      }
    }"#;

    #[test]
    fn resolves_highest_matching() {
        let idx = RegistryIndex::parse(IDX).unwrap();
        let (git, rv) = idx
            .resolve("Acme/Json", &VersionReq::parse("^1.0").unwrap())
            .unwrap();
        assert_eq!(git, "https://example.test/acme/json.git");
        assert_eq!(rv.version.to_string(), "1.2.0"); // highest under ^1.0
        assert_eq!(rv.tag, "v1.2.0");
    }

    #[test]
    fn unmatched_and_unknown_error_loudly() {
        let idx = RegistryIndex::parse(IDX).unwrap();
        assert!(idx
            .resolve("Acme/Json", &VersionReq::parse("^3.0").unwrap())
            .is_err());
        assert!(idx
            .resolve("No/Such", &VersionReq::parse("*").unwrap())
            .is_err());
    }

    #[test]
    fn read_source_reads_local_dir_and_file() {
        let dir = std::env::temp_dir().join(format!("phorj_reg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.json"), IDX).unwrap();
        // directory → index.json
        let txt = read_source(dir.to_str().unwrap()).unwrap();
        assert!(RegistryIndex::parse(&txt)
            .unwrap()
            .entry("Acme/Json")
            .is_some());
        // file:// direct
        let file_url = format!("file://{}", dir.join("index.json").display());
        assert!(read_source(&file_url).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
