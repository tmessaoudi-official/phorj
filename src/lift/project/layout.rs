//! Directory lift — the output LAYOUT: where each lifted draft lands, and the generated manifest.
//!
//! Split out of `mod.rs` by cohesion (Invariant 13 — that file crossed the 500-line hard cap). Everything
//! here answers one question: given a lifted draft and the file it came from, what path does it get and
//! what does the project around it look like. The two subtle rules both live here — the ENTRY is
//! re-packaged as `Main` (a dotted package must sit in a matching subdirectory, so an entry elsewhere makes
//! the project fail to LOAD), and a destination COLLISION renames rather than overwrites (two sources
//! mapping to one path silently destroyed a file before this).

use std::path::{Path, PathBuf};

/// The `// lifted (verify)` banner every draft carries, plus the SOURCE path it came from — a directory
/// lift produces many files, and a reviewer needs to know which PHP each one answers to.
pub(super) fn draft_header(rel: &str, phg: &str) -> String {
    format!("// lifted (verify) from `{rel}` — a best-effort PHP->Phorj draft; review before trusting it.\n{phg}")
}

/// Where a lifted file goes: `src/<package path>/<Stem>.phg`, which is the layout the loader enforces
/// (`E-PKG-PATH` — a dotted package needs a matching subdirectory). `package Main;` is exempt and lands at
/// the source root, exactly as the loader allows.
///
/// The package is read back out of the LIFTED text rather than re-derived from the PHP namespace, so the
/// path can never disagree with the `package` line the file actually declares.
pub(super) fn destination(out: &Path, phg: &str, source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Lifted");
    let mut dir = out.join("src");
    if let Some(pkg) = package_of(phg) {
        if pkg != "Main" {
            for seg in pkg.split('.') {
                dir = dir.join(seg);
            }
        }
    }
    dir.join(format!("{stem}.phg"))
}

/// Rewrite a draft's `package …;` line to `package Main;` so it can be the project ENTRY.
///
/// Sound because nothing imports the entry: its package name is never referenced, while `package Main` is the
/// one package the loader lets live at the source root. Only the FIRST `package` line is touched (it is the
/// declaration; a later occurrence would be inside a string or comment).
pub(super) fn repackage_as_main(phg: &str) -> String {
    let mut done = false;
    phg.lines()
        .map(|l| {
            if !done && l.trim_start().starts_with("package ") {
                done = true;
                "package Main;"
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// A destination nobody has taken yet, disambiguated by walking UP the source path.
///
/// Renaming rather than refusing is sound because a phorj package directory may hold any number of files
/// under any names — the loader maps a PACKAGE to a directory, and a file's own name carries no meaning
/// (`examples/project/shapes/src/Acme/Geometry/` holds four differently-named files in one package). So the
/// content is preserved exactly and only the file name changes.
///
/// The rename is RECORDED and reported: a developer looking for `Helper.php`'s draft needs to know it is
/// now `A_Helper.phg`. Silence here is what made the original bug invisible.
pub(super) fn unique_destination(
    preferred: PathBuf,
    source: &Path,
    written: &mut Vec<PathBuf>,
    rel: &str,
    renames: &mut Vec<(String, String)>,
) -> PathBuf {
    if !written.contains(&preferred) {
        written.push(preferred.clone());
        return preferred;
    }
    let dir = preferred.parent().unwrap_or(Path::new(".")).to_path_buf();
    let stem = preferred
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Lifted")
        .to_string();
    // Prefix with the source's own parent directories, nearest first — `src/A/Helper.php` becomes
    // `A_Helper.phg`, which is both unique and traceable back to where it came from.
    let mut parents: Vec<String> = source
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => s.to_str().map(String::from),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    parents.reverse();
    let mut prefix = String::new();
    for seg in parents {
        prefix = format!("{seg}_{prefix}");
        let candidate = dir.join(format!("{prefix}{stem}.phg"));
        if !written.contains(&candidate) {
            written.push(candidate.clone());
            renames.push((rel.to_string(), file_label(&candidate)));
            return candidate;
        }
    }
    // Exhausted the path (two sources with identical full paths is impossible, so this is unreachable in
    // practice) — fall back to a counter rather than overwriting anything.
    for n in 2.. {
        let candidate = dir.join(format!("{stem}_{n}.phg"));
        if !written.contains(&candidate) {
            written.push(candidate.clone());
            renames.push((rel.to_string(), file_label(&candidate)));
            return candidate;
        }
    }
    unreachable!("the counter is unbounded")
}

/// The trailing `src/…` portion of a destination, for reporting.
fn file_label(path: &Path) -> String {
    let full = path.to_string_lossy().replace('\\', "/");
    match full.rfind("/src/") {
        Some(i) => full[i + 1..].to_string(),
        None => full,
    }
}

/// The `package X.Y;` a lifted draft declares.
fn package_of(phg: &str) -> Option<&str> {
    phg.lines()
        .find_map(|l| l.trim().strip_prefix("package "))
        .and_then(|rest| rest.split(';').next())
        .map(str::trim)
}

/// A minimal `phorj.json`. The name comes from `composer.json`'s own if it is a legal phorj package name,
/// else from the directory — a lifted project should not fail to load on its manifest.
pub(super) fn manifest_json(root: &Path) -> String {
    let name = composer_name(root)
        .filter(|n| crate::pm::manifest::validate_pkg_name(n).is_ok())
        .or_else(|| {
            root.file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
                .filter(|n| crate::pm::manifest::validate_pkg_name(n).is_ok())
        })
        .unwrap_or_else(|| "lifted-app".to_string());
    // DEC-321: the edition field is carried from the first write rather than retrofitted later.
    format!("{{\n  \"name\": \"{name}\",\n  \"version\": \"0.1.0\",\n  \"edition\": \"2026\"\n}}\n")
}

/// composer's package name with its `vendor/` prefix dropped (`acme/blog` → `blog`), since a phorj package
/// name is a single segment.
fn composer_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("composer.json")).ok()?;
    let j = crate::pm::json::Json::parse(&text).ok()?;
    let full = j.get("name")?.as_str()?;
    Some(full.rsplit('/').next().unwrap_or(full).to_string())
}
