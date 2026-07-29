//! The loader's entry points + unified load pipeline (DEC-282). Split out of `loader/mod.rs`
//! (M-Decomp) to keep the root under the file-size cap; all shared types (`Unit`, `Source`, the
//! `RootHit`/`SearchIndex` aliases, `ResolveCtx`) and the `mangle`/`assemble` helpers stay in the
//! root or its siblings and are reached here via `use super::*`. The public entry points are
//! re-exported by the root, so `crate::loader::load` etc. still resolve.

use super::*;

/// Recursively collect every `*.phg` under `dir` (sorted, deterministic). Public wrapper over the
/// internal walker, used by the `phg test` runner (M-Test T3) to discover test files. An empty Vec
/// for a non-directory or empty tree.
pub fn discover_phg(dir: &Path) -> Result<Vec<PathBuf>, String> {
    collect_phg(dir)
}

/// Load the entry at `path` — DEC-282, the unified manifest-less loader. A `phorj.toml` found by
/// walk-up still selects the legacy project mode (retiring this release); otherwise the unified
/// rule applies: app-root discovery (`src/`/`vendor/` as the walk-up marker), three ordered search
/// roots (entry-local → `src/` → `vendor/`), and import-driven, declaration-indexed lazy loading —
/// only packages the entry's import graph reaches are ever read.
pub fn load(entry: &Path) -> Result<Unit, String> {
    // Canonicalize so walk-up discovery works from a relative entry path; fall back to the raw path
    // when it does not exist yet (the read below then yields the canonical "cannot read" error).
    let canon = entry.canonicalize().ok();
    let probe: &Path = canon.as_deref().unwrap_or(entry);
    load_unified(probe)
}

/// DEC-282 — the unified load: parse the entry, then chase its user imports through the three
/// ordered search roots (first match wins; a later root also holding the package gets a loud
/// shadow warning), transitively, loading ONLY reached packages. The assembled sources then run
/// through the same two-pass mangle/rewrite/merge machinery as before.
fn load_unified(entry: &Path) -> Result<Unit, String> {
    let entry_src = read_file(entry)?;
    load_unified_src(entry, entry_src)
}

/// DEC-282/DEC-252 — the LSP seam: load `entry` under the unified rule but with `entry_src` as the
/// entry's text (the editor's possibly-unsaved buffer) instead of the on-disk bytes; sibling
/// packages still come from disk. This is what makes editor diagnostics ≡ `phg check` for
/// multi-file programs.
pub fn load_with_buffer(entry: &Path, entry_src: &str) -> Result<Unit, String> {
    let canon = entry.canonicalize().ok();
    let probe: &Path = canon.as_deref().unwrap_or(entry);
    load_unified_src(probe, entry_src.to_string())
}

fn load_unified_src(entry: &Path, entry_src: String) -> Result<Unit, String> {
    let entry_prog = parse_at(entry, &entry_src)?;
    check_unused_imports(&entry_prog, &entry_src, entry)?;
    let roots = discover_roots(entry);

    // Fast path: no user imports AND no ambient `*.d.phg` declaration files under the roots →
    // a self-contained script; skip all disk scanning. (An entry using only foreign `declare`s
    // has no user imports but still needs its decl files ambient-merged — the assemble path.)
    let mut queue: Vec<Vec<String>> = user_imports(&entry_prog, entry)?;
    if queue.is_empty() && collect_unified_decls(&roots)?.is_empty() {
        return Ok(Unit {
            program: entry_prog,
            diag_src: entry_src,
            stats: None,
            sources: std::collections::HashMap::new(),
            fn_files: std::collections::HashMap::new(),
            item_files: std::collections::HashMap::new(),
        });
    }

    // The three ordered (name, root, index) search roots. Root 1 excludes root 2/3 subtrees.
    let mut indexed: Vec<SearchIndex> = Vec::new();
    {
        let mut exclude: Vec<&Path> = Vec::new();
        if let Some(s) = &roots.src_root {
            exclude.push(s);
        }
        if let Some(v) = &roots.vendor_root {
            exclude.push(v);
        }
        indexed.push((
            "entry directory",
            roots.entry_local.clone(),
            index_packages(&roots.entry_local, &exclude),
        ));
    }
    if let Some(s) = &roots.src_root {
        indexed.push(("src/", s.clone(), index_packages(s, &[])));
    }
    if let Some(v) = &roots.vendor_root {
        indexed.push(("vendor/", v.clone(), index_packages(v, &[])));
    }

    let mut sources: Vec<Source> =
        vec![Source::first_party(entry.to_path_buf(), &roots.entry_local)];
    let mut loaded: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut parsed_cache: HashMap<PathBuf, Program> = HashMap::new();
    while let Some(path) = queue.pop() {
        // A dotted import names a package, or a member of one — resolve the longest matching
        // prefix as the package (full path first, then the parent for `import Pkg.Member;`).
        let full = path.join(".");
        let parent = path[..path.len().saturating_sub(1)].join(".");
        let mut hit: Option<RootHit> = None;
        'outer: for want in [&full, &parent] {
            if want.is_empty() {
                continue;
            }
            for (i, (label, root, idx)) in indexed.iter().enumerate() {
                if let Some(files) = idx.get(want.as_str()) {
                    hit = Some((i, label, root.clone(), files.clone(), want.clone()));
                    break 'outer;
                }
            }
        }
        let Some((win_i, _label, root, files, pkg)) = hit else {
            let searched: Vec<String> = indexed
                .iter()
                .map(|(label, root, _)| format!("{} ({})", label, root.display()))
                .collect();
            return Err(format!(
                "import `{}` does not resolve: no package `{}` (or `{}`) under any search root\n  searched: {}\n  hint: packages live in folders matching their name (folder = package) under the \
                 entry's directory, `src/`, or `vendor/`; dependencies must already be on disk — \
                 `run`/`check`/`transpile` never download code (`phg install` writes `vendor/`) [E-MODULE-NOT-FOUND]",
                full,
                full,
                if parent.is_empty() { "-" } else { &parent },
                searched.join(", ")
            ));
        };
        // Shadow visibility: the same package in a LATER root too is legal (the specific root
        // wins) but never silent.
        for (label, root2, idx) in indexed.iter().skip(win_i + 1) {
            if idx.contains_key(&pkg) {
                eprintln!(
                    "warning: package `{pkg}` in {} ({}) is shadowed by the more specific {} \
                     ({}) [W-SHADOWED]",
                    label,
                    root2.display(),
                    indexed[win_i].0,
                    root.display()
                );
            }
        }
        if !loaded.insert(pkg.clone()) {
            continue;
        }
        for f in files {
            if parsed_cache.contains_key(&f) || same_file(&f, entry) {
                continue;
            }
            let fsrc = read_file(&f)?;
            let fprog = parse_at(&f, &fsrc)?;
            check_unused_imports(&fprog, &fsrc, &f)?;
            queue.extend(user_imports(&fprog, &f)?);
            parsed_cache.insert(f.clone(), fprog);
            let vendored = roots.vendor_root.as_ref().is_some_and(|v| f.starts_with(v));
            sources.push(if vendored {
                Source::vendored(f, &root)
            } else {
                Source::first_party(f, &root)
            });
        }
    }
    sources.sort_by(|a, b| a.file.cmp(&b.file));
    sources.dedup_by(|a, b| a.file == b.file);
    let decl_files = collect_unified_decls(&roots)?;
    assemble(entry, sources, &decl_files, Some((entry, &entry_src)))
}

/// The unified decl sweep: `*.d.phg` DIRECTLY in the entry's directory (non-recursive — a folder
/// of unrelated scripts must never inhale a nested project's foreign declares) plus everything
/// under `src/` (the app's own ambient declarations), never under `vendor/`.
fn collect_unified_decls(roots: &SearchRoots) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&roots.entry_local) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() && p.to_string_lossy().ends_with(".d.phg") {
                out.push(p);
            }
        }
    }
    if let Some(sr) = &roots.src_root {
        out.extend(collect_decl_phg(sr)?);
    }
    out.sort();
    Ok(out)
}

/// Load a loose-mode program from source text (the `-e`/stdin path, and any single file with no
/// project above it). Enforces the reserved `package Main;` — a dotted package needs a project.
pub fn load_loose_src(src: &str) -> Result<Unit, String> {
    let program = parse_one(src)?;
    enforce_loose_main(&program)?;
    Ok(Unit {
        program,
        diag_src: src.to_string(),
        stats: None,
        sources: std::collections::HashMap::new(),
        fn_files: std::collections::HashMap::new(),
        item_files: std::collections::HashMap::new(),
    })
}
