//! Filesystem + parse helpers for the loader (M-Decomp W3.2): parse entry points,
//! folder=package validation, and the `.phg` directory walk.

use super::*;

/// lex + parse a single source, rendering any front-end error to one line (no path prefix — used
/// for the loose path so CLI output stays byte-identical to the pre-S2b single-file pipeline).
pub(super) fn parse_one(src: &str) -> Result<Program, String> {
    let tokens = lex(src).map_err(|e| e.render(src))?;
    Parser::new(tokens)
        .parse_program()
        .map_err(|e| e.render(src))
}

/// As [`parse_one`], but prefix errors with the file path (project mode spans many files).
pub(super) fn parse_at(path: &Path, src: &str) -> Result<Program, String> {
    parse_one(src).map_err(|e| format!("{}: {e}", path.display()))
}

/// In loose mode, only the reserved `package Main;` runs. An empty package is left to the checker
/// (`E-NO-PACKAGE`) so the error is not double-reported.
pub(super) fn enforce_loose_main(prog: &Program) -> Result<(), String> {
    if prog.package.is_empty() || prog.package == ["Main"] {
        return Ok(());
    }
    Err(format!(
        "package `{}` requires a phorge.toml project; only `package Main` runs as a loose script \
         (add a phorge.toml above the source root, or declare `package Main`)",
        prog.package.join(".")
    ))
}

/// Validate a file's package against its on-disk location: directory under the source root = the
/// dotted package (folder = path). `package Main` is exempt (runnable anywhere); an empty package
/// is left to the checker.
pub(super) fn validate_folder_path(
    prog: &Program,
    file: &Path,
    source_root: &Path,
) -> Result<(), String> {
    if prog.package.is_empty() || prog.package == ["Main"] {
        return Ok(());
    }
    let Some(rel) = relative_under(file, source_root) else {
        return Err(format!(
            "{}: package `{}` lives outside the source root `{}` — only `package Main` may live \
             outside it [E-PKG-PATH]",
            file.display(),
            prog.package.join("."),
            source_root.display()
        ));
    };
    let expected: Vec<String> = match rel.parent() {
        Some(dir) => dir
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(String::from),
                _ => None,
            })
            .collect(),
        None => Vec::new(),
    };
    if expected.is_empty() {
        return Err(format!(
            "{}: package `{}` cannot sit directly in the source root — a dotted package needs a \
             matching subdirectory (expected under `{}/`) [E-PKG-PATH]",
            file.display(),
            prog.package.join("."),
            prog.package.join("/")
        ));
    }
    if expected != prog.package {
        return Err(format!(
            "{}: package `{}` does not match its location — directory `{}` implies \
             `package {};` (folder = path) [E-PKG-PATH]",
            file.display(),
            prog.package.join("."),
            expected.join("/"),
            expected.join(".")
        ));
    }
    Ok(())
}

/// The path of `file` relative to `source_root`, resolving symlinks/`.`/`..` via canonicalization
/// when possible. Returns `None` when `file` is not under `source_root`.
pub(super) fn relative_under(file: &Path, source_root: &Path) -> Option<PathBuf> {
    if let (Ok(f), Ok(root)) = (file.canonicalize(), source_root.canonicalize()) {
        return f.strip_prefix(&root).ok().map(Path::to_path_buf);
    }
    file.strip_prefix(source_root).ok().map(Path::to_path_buf)
}

/// Two paths refer to the same file (canonicalized; falls back to a raw compare).
pub(super) fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// All `*.phg` files under `dir`, recursively, in a deterministic (sorted) order.
pub(super) fn collect_phg(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(dir, &mut out)?;
    }
    out.sort();
    Ok(out)
}

pub(super) fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let rd = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory {}: {e}", dir.display()))?;
    let mut entries: Vec<PathBuf> = Vec::new();
    for e in rd {
        let e = e.map_err(|e| format!("cannot read an entry in {}: {e}", dir.display()))?;
        entries.push(e.path());
    }
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out)?;
        } else if p.extension().and_then(|s| s.to_str()) == Some("phg") {
            out.push(p);
        }
    }
    Ok(())
}

pub(super) fn read_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}
