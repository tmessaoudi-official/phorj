//! The `phorj.json` manifest model (DEC-316) — composer.json-style: a package's identity + its
//! `require` map. Each dependency names a `Publisher/Name` and a source: a registry semver constraint
//! (`"^1.2"`), a git repo (`{ "git": url, "ref": tag }`), or a local path (`{ "path": "../pkg" }`).
//!
//! `Publisher/Name` mirrors the loader's `vendor/<Publisher>/<Name>/` layout (PascalCase segments,
//! `src/loader/fs.rs`), so a resolved dependency drops straight into a tree the DEC-282 loader accepts.

use crate::pm::json::Json;
use crate::pm::semver::{Version, VersionReq};

/// Where a dependency comes from. Registry entries resolve a constraint to a concrete version via the
/// registry index; git/path fetch directly (all three land in `vendor/` — see `pm::fetch`).
#[derive(Debug, Clone, PartialEq)]
pub enum SourceSpec {
    /// Resolved through the registry index by semver constraint.
    Registry(VersionReq),
    /// A git repository at a specific ref (tag/branch/commit).
    Git { url: String, git_ref: String },
    /// A local directory (dev dependency), relative to the manifest.
    Path(String),
}

/// One `require` entry: a `Publisher/Name` and its source.
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    pub name: String,
    pub source: SourceSpec,
}

/// A parsed `phorj.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub name: Option<String>,
    pub version: Option<Version>,
    pub description: Option<String>,
    pub require: Vec<Dependency>,
}

impl Manifest {
    /// Parse a `phorj.json` document. Validates each dependency name and source shape; unknown top-level
    /// keys are ignored (forward-compat), unknown source keys are an error (typo protection).
    pub fn parse(src: &str) -> Result<Manifest, String> {
        let j = Json::parse(src).map_err(|e| format!("phorj.json: {e}"))?;
        if j.as_obj().is_none() {
            return Err("phorj.json: top level must be a JSON object".to_string());
        }
        let name = j.get("name").and_then(Json::as_str).map(str::to_string);
        if let Some(n) = &name {
            validate_pkg_name(n)?;
        }
        let version = match j.get("version").and_then(Json::as_str) {
            Some(v) => Some(Version::parse(v).map_err(|e| format!("phorj.json version: {e}"))?),
            None => None,
        };
        let description = j
            .get("description")
            .and_then(Json::as_str)
            .map(str::to_string);

        let mut require = Vec::new();
        if let Some(req) = j.get("require") {
            let pairs = req
                .as_obj()
                .ok_or("phorj.json: `require` must be an object")?;
            for (dep_name, spec) in pairs {
                validate_pkg_name(dep_name)?;
                require.push(Dependency {
                    name: dep_name.clone(),
                    source: parse_source(dep_name, spec)?,
                });
            }
        }
        Ok(Manifest {
            name,
            version,
            description,
            require,
        })
    }

    /// Serialize back to a `phorj.json` (used by `phg add`/`remove` to rewrite the manifest). Preserves
    /// the canonical key order (name, version, description, require) and sorts `require` by name for a
    /// stable diff.
    pub fn to_pretty(&self) -> String {
        let mut top: Vec<(String, Json)> = Vec::new();
        if let Some(n) = &self.name {
            top.push(("name".into(), Json::Str(n.clone())));
        }
        if let Some(v) = &self.version {
            top.push(("version".into(), Json::Str(v.to_string())));
        }
        if let Some(d) = &self.description {
            top.push(("description".into(), Json::Str(d.clone())));
        }
        let mut deps = self.require.clone();
        deps.sort_by(|a, b| a.name.cmp(&b.name));
        let req: Vec<(String, Json)> = deps
            .iter()
            .map(|d| (d.name.clone(), source_to_json(&d.source)))
            .collect();
        top.push(("require".into(), Json::Obj(req)));
        Json::Obj(top).to_pretty()
    }
}

fn parse_source(dep: &str, spec: &Json) -> Result<SourceSpec, String> {
    match spec {
        Json::Str(s) => Ok(SourceSpec::Registry(
            VersionReq::parse(s).map_err(|e| format!("require `{dep}`: {e}"))?,
        )),
        Json::Obj(_) => {
            if let Some(path) = spec.get("path").and_then(Json::as_str) {
                return Ok(SourceSpec::Path(path.to_string()));
            }
            if let Some(url) = spec.get("git").and_then(Json::as_str) {
                let git_ref = spec
                    .get("ref")
                    .and_then(Json::as_str)
                    .ok_or_else(|| format!("require `{dep}`: git source needs a `ref`"))?;
                return Ok(SourceSpec::Git {
                    url: url.to_string(),
                    git_ref: git_ref.to_string(),
                });
            }
            Err(format!(
                "require `{dep}`: object source must have `path` or `git`"
            ))
        }
        _ => Err(format!(
            "require `{dep}`: source must be a version string or an object"
        )),
    }
}

fn source_to_json(s: &SourceSpec) -> Json {
    match s {
        SourceSpec::Registry(req) => Json::Str(req_to_string(req)),
        SourceSpec::Git { url, git_ref } => Json::Obj(vec![
            ("git".into(), Json::Str(url.clone())),
            ("ref".into(), Json::Str(git_ref.clone())),
        ]),
        SourceSpec::Path(p) => Json::Obj(vec![("path".into(), Json::Str(p.clone()))]),
    }
}

fn req_to_string(req: &VersionReq) -> String {
    match req {
        VersionReq::Any => "*".to_string(),
        VersionReq::Exact(v) => v.to_string(),
        VersionReq::Caret(v) => format!("^{v}"),
        VersionReq::Tilde(v, _) => format!("~{v}"),
    }
}

/// A dependency/package name is `Publisher/Name`, each a PascalCase segment (matches the loader's
/// `vendor/<Publisher>/<Name>/` + `Core.` naming laws in `src/loader/fs.rs`). `Core` is reserved.
pub fn validate_pkg_name(name: &str) -> Result<(), String> {
    let (publisher, pkg) = name
        .split_once('/')
        .ok_or_else(|| format!("package name `{name}` must be `Publisher/Name`"))?;
    if pkg.contains('/') {
        return Err(format!("package name `{name}` has too many `/` segments"));
    }
    for seg in [publisher, pkg] {
        if seg.is_empty() {
            return Err(format!("package name `{name}` has an empty segment"));
        }
        let mut chars = seg.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_uppercase() {
            return Err(format!(
                "package segment `{seg}` in `{name}` must be PascalCase (start uppercase)"
            ));
        }
        if !seg.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!(
                "package segment `{seg}` in `{name}` must be alphanumeric"
            ));
        }
    }
    if publisher == "Core" {
        return Err(format!(
            "`{name}`: the `Core` publisher is reserved for first-party packages"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_three_source_kinds() {
        let src = r#"{
          "name": "Acme/App",
          "version": "0.1.0",
          "require": {
            "Acme/Json": "^1.2",
            "Foo/Bar": { "git": "https://example.test/foo/bar.git", "ref": "v2.1.0" },
            "Dev/Local": { "path": "../local" }
          }
        }"#;
        let m = Manifest::parse(src).unwrap();
        assert_eq!(m.name.as_deref(), Some("Acme/App"));
        assert_eq!(m.version.unwrap().to_string(), "0.1.0");
        assert_eq!(m.require.len(), 3);
        let by = |n: &str| {
            m.require
                .iter()
                .find(|d| d.name == n)
                .unwrap()
                .source
                .clone()
        };
        assert!(matches!(by("Acme/Json"), SourceSpec::Registry(_)));
        assert!(matches!(by("Foo/Bar"), SourceSpec::Git { .. }));
        assert!(matches!(by("Dev/Local"), SourceSpec::Path(_)));
    }

    #[test]
    fn roundtrips_and_sorts_require() {
        let m =
            Manifest::parse(r#"{"name":"Acme/App","require":{"Zed/Z":"1.0.0","Acme/A":"^1.0"}}"#)
                .unwrap();
        let out = m.to_pretty();
        let m2 = Manifest::parse(&out).unwrap();
        assert_eq!(m2.require.len(), 2);
        // require serialized in sorted order
        assert!(out.find("Acme/A").unwrap() < out.find("Zed/Z").unwrap());
    }

    #[test]
    fn rejects_bad_names_and_sources() {
        assert!(Manifest::parse(r#"{"require":{"bad":"1.0"}}"#).is_err()); // not Publisher/Name
        assert!(Manifest::parse(r#"{"require":{"acme/x":"1.0"}}"#).is_err()); // lowercase publisher
        assert!(Manifest::parse(r#"{"require":{"Core/X":"1.0"}}"#).is_err()); // reserved Core
        assert!(Manifest::parse(r#"{"require":{"Foo/Bar":{"git":"u"}}}"#).is_err()); // git w/o ref
        assert!(Manifest::parse(r#"{"require":{"Foo/Bar":{}}}"#).is_err()); // empty source
        assert!(Manifest::parse(r#"[]"#).is_err()); // non-object top level
    }

    #[test]
    fn empty_manifest_is_valid() {
        let m = Manifest::parse(r#"{"name":"Acme/App"}"#).unwrap();
        assert!(m.require.is_empty());
    }
}
