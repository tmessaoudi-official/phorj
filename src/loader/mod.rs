//! The unified, manifest-less multi-file loader + cross-package name resolution (DEC-282).
//!
//! Turns an entry source into a single [`Unit`] (one [`Program`] ready for check + run). ONE rule
//! everywhere — no manifest, no modes:
//!
//! - **App root**: the nearest ancestor of the entry containing `src/` or `vendor/` (git-style
//!   walk-up; `src/` itself is the marker); with neither, the entry's own directory.
//! - **Three ordered search roots**: the entry file's directory (entry-local packages, e.g.
//!   `bin/Commands/`), then `<approot>/src/` (shared code — package names strip `src/`), then
//!   `<approot>/vendor/` (offline deps; the compiler NEVER touches the network). First match
//!   wins; a later root also holding the package warns `W-SHADOWED`.
//! - **Import-driven, declaration-indexed lazy loading**: only packages the entry's import graph
//!   reaches are ever read (`peek_package` indexes cheaply; unreached/broken strangers are inert).
//!   Folder = package (`E-PKG-PATH`) and the public-surface file rules validate per loaded file;
//!   `package Main` is entry-only, location-free, and unimportable (`E-IMPORT-MAIN`). Import
//!   hygiene is Go-maximal: `E-DUP-IMPORT` and `E-UNUSED-IMPORT` are hard errors and an
//!   unresolvable import is `E-MODULE-NOT-FOUND` listing the searched roots.
//!
//! Loaded files then run the same two-pass assembly as always: every non-`Main` definition is
//! mangled to a globally-unique name (`Acme.Util` + `compute` ⇒ `Acme\Util\compute`), call/type
//! sites rewrite per file against its import map, and all items merge into one flat [`Program`]
//! — so the checker/interpreter/compiler/VM are unchanged (interp ≡ VM is structural) and only the
//! transpiler de-mangles into PHP `namespace` blocks.
//!
//! Enforcement and resolution live here (path-aware), never in the type checker, so
//! `cli::cmd_treewalk(&str)`, the differential harness, and the checker's package-agnostic tests are
//! untouched. Library packages export **functions and types** (M-RT cross-package types): a non-`main`
//! `class`/`enum`/`interface` is mangled like a function (`acme.geometry` + `Point` ⇒
//! `Acme\Geometry\Point`) and a consuming file binds it with `import type a.b.C [as D];`; the same
//! Pass-2 rewrite that mangles call sites also rewrites every type-name position (annotations,
//! instantiation, `instanceof`, enum access) to the mangled FQN, so the backends see fully-resolved
//! names and only the transpiler de-mangles into PHP `namespace` blocks.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::ast::{
    ClassMember, Expr, Item, LambdaBody, MatchArm, Param, Program, Stmt, StrPart, Type, Visibility,
};
use crate::diagnostic::Diagnostic;
use crate::parser::Parser;
use crate::token::Span;
use crate::tokenizer::lex;

// Cohesion split (M-Decomp W3.2): resolution walkers + fs helpers in sibling files.
mod discovery;
mod fs;
mod import_hygiene;
mod imports;
mod resolve;

use discovery::{discover_roots, index_packages, SearchRoots};
/// Project-package enumeration for the LSP import-path completion (the `discovery` module is private).
pub(crate) use discovery::{project_packages, project_phg_files};
use fs::*;
use import_hygiene::*;
use imports::*;
use resolve::*;

/// Provenance for one top-level definition: where it was declared and how visible it is. Built in
/// Pass 1 (which still has per-file information) and consumed by the visibility lattice during Pass 2.
#[derive(Clone)]
struct DefInfo {
    file: PathBuf,
    package: String,
    vis: Visibility,
}

/// The visibility lattice check. `None` ⇒ the reference is legal; `Some(code)` ⇒ the diagnostic code
/// to report. Same file → always legal. Same package, different file → legal unless `private`.
/// Different package → legal only if `public`.
fn vis_violation(info: &DefInfo, referrer_file: &Path, referrer_pkg: &str) -> Option<&'static str> {
    if info.file == referrer_file {
        return None;
    }
    if info.package == referrer_pkg {
        return if info.vis == Visibility::Private {
            Some("E-VIS-PRIVATE")
        } else {
            None
        };
    }
    match info.vis {
        Visibility::Public => None,
        Visibility::Internal => Some("E-VIS-INTERNAL"),
        Visibility::Private => Some("E-VIS-PRIVATE"),
    }
}

/// Render the visibility keyword for a diagnostic.
fn vis_word(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "public",
        Visibility::Internal => "internal",
        Visibility::Private => "private",
    }
}

/// A loaded compilation unit: the (possibly merged) program plus the source text used to render
/// type-error carets. `diag_src` is the single file's source in loose mode (full carets) or empty
/// for a merged multi-file unit, where no single source aligns — diagnostics then print message +
/// position without a source line (a deliberate flat-merge limitation; richer multi-file carets are
/// a later slice).
#[derive(Debug, Clone)]
pub struct Unit {
    pub program: Program,
    pub diag_src: String,
    /// Project-load statistics (project mode only; `None` in loose mode). Lets `phg check` report the
    /// *scope* it validated — proving the whole project (every file, including code no route reaches,
    /// plus vendored deps) was type-checked, the PHP-absent superpower of whole-program checking.
    pub stats: Option<LoadStats>,
    /// Per-file source text (project mode), for runtime stack-trace carets. Empty in loose mode (the
    /// single source rides on `diag_src`). Keyed by the file path shown in a `Frame.file`.
    pub sources: std::collections::HashMap<PathBuf, String>,
    /// Function (compiled/mangled) name → origin file, for attributing trace frames to a file
    /// (error-handling slice 1). Covers free functions (incl. `main`); methods/ctors — whose frame
    /// names are backend-synthesized (`Class::m`) — are not keyed here and show line-only.
    pub fn_files: std::collections::HashMap<String, PathBuf>,
    /// DEC-320: EVERY top-level definition's origin — mangled name → declaring `.phg` file, types
    /// AND functions (Pass 1 knows both; `fn_files` above stays the trace-frame subset). Drives the
    /// `phg build --php` sibling emit's item→file routing. Empty in loose mode.
    pub item_files: std::collections::HashMap<String, PathBuf>,
}

impl Unit {
    /// Attribute each runtime trace frame to its origin file via [`Unit::fn_files`] (no-op in loose
    /// mode / for backend-synthesized method frames). Returns the source text to render the fault
    /// caret against — the innermost frame's file source in project mode, else `diag_src`.
    #[must_use]
    pub fn attribute_frames(&self, diag: &mut Diagnostic) -> String {
        for f in &mut diag.frames {
            if f.file.is_none() {
                f.file = self.fn_files.get(&f.function).cloned();
            }
        }
        diag.frames
            .first()
            .and_then(|f| f.file.as_ref())
            .and_then(|p| self.sources.get(p))
            .cloned()
            .unwrap_or_else(|| self.diag_src.clone())
    }
}

/// Counts of what a project load assembled and handed to the checker — every `.phg` under the source
/// root (first-party + vendored), merged and validated as one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadStats {
    pub files: usize,
    pub packages: usize,
    pub defs: usize,
}

impl LoadStats {
    /// A one-line human summary for `phg check`'s success message.
    pub fn summary(&self) -> String {
        format!(
            "OK — whole project type-checks clean: {} file{}, {} package{}, {} definition{} \
             validated (every file + vendored deps)\n",
            self.files,
            plural(self.files),
            self.packages,
            plural(self.packages),
            self.defs,
            plural(self.defs),
        )
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

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

/// One resolved import: (winning root index, root label, root path, the package's files, the
/// package name) — the search loop's carrier (a named alias keeps clippy's type-complexity
/// lint honest).
type RootHit = (usize, &'static str, PathBuf, Vec<PathBuf>, String);

/// One indexed search root: (human label, root path, package → files declaration index).
type SearchIndex = (&'static str, PathBuf, BTreeMap<String, Vec<PathBuf>>);

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
                 phg never downloads code [E-MODULE-NOT-FOUND]",
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

/// The shared two-pass assembly (DEC-282 factored it out of `load_project` so the unified loader
/// reuses it verbatim): parse + validate every source, mangle non-`Main` definitions to globally
/// unique names, rewrite call/type sites per file, merge into one flat [`Program`].
/// `decl_files` is the pre-collected ambient `*.d.phg` set (the CALLER owns the sweep scope —
/// the unified loader deliberately keeps the entry-local sweep NON-recursive so a directory of
/// unrelated scripts never inhales a distant project's foreign declares).
fn assemble(
    entry: &Path,
    sources: Vec<Source>,
    decl_files: &[PathBuf],
    buffer: Option<(&Path, &str)>,
) -> Result<Unit, String> {
    // Pass 1 — parse, validate, and index every top-level definition by (package, name) ⇒ mangled
    // global name. Functions and types live in separate symbol tables (PHP namespaces functions and
    // classes separately), so a `compute` function and a `Compute` type never collide. Library
    // packages may now declare types (the old `E-PKG-TYPE` gate is retired — cross-package types).
    let mut parsed: Vec<(PathBuf, Program)> = Vec::with_capacity(sources.len());
    let mut defined: HashMap<(String, String), String> = HashMap::new();
    let mut types: HashMap<(String, String), String> = HashMap::new();
    // Declaration-visibility provenance (visibility modifiers): where each definition lives + its
    // visibility, keyed by (package, name) like the rename tables. Consumed by the lattice in Pass 2.
    let mut prov_fns: HashMap<(String, String), DefInfo> = HashMap::new();
    let mut prov_types: HashMap<(String, String), DefInfo> = HashMap::new();
    // Whole-project scope counters for `phg check`'s success summary.
    let mut pkgset: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut defs: usize = 0;
    // Trace-attribution maps (error-handling slice 1): per-file source + function → file.
    let mut src_map: HashMap<PathBuf, String> = HashMap::new();
    let mut fn_files: HashMap<String, PathBuf> = HashMap::new();
    let mut item_files: HashMap<String, PathBuf> = HashMap::new();
    for src_entry in &sources {
        let file = &src_entry.file;
        // The LSP buffer override (DEC-252): the entry's text may be the editor's unsaved buffer.
        let src = match buffer {
            Some((p, b)) if same_file(p, file) => b.to_string(),
            _ => read_file(file)?,
        };
        src_map.insert(file.clone(), src.clone());
        let prog = parse_at(file, &src)?;
        validate_folder_path(&prog, file, &src_entry.root)?;
        validate_package_decl(&prog, file)?;
        validate_public_surface(&prog, file)?;
        if src_entry.vendored && (prog.package.is_empty() || prog.package == ["Main"]) {
            return Err(format!(
                "{}: a vendored dependency is a library and cannot declare `package Main` \
                 (it would collide with the consumer's entry) [E-VENDOR-MAIN]",
                file.display()
            ));
        }
        let pkg = prog.package.join(".");
        pkgset.insert(if pkg.is_empty() {
            "main".to_string()
        } else {
            pkg.clone()
        });
        for item in &prog.items {
            let (name, is_type, vis) = match item {
                Item::Function(f) => (&f.name, false, f.vis),
                Item::Class(c) => (&c.name, true, c.vis),
                Item::Enum(e) => (&e.name, true, e.vis),
                Item::Interface(i) => (&i.name, true, i.vis),
                // A trait is a public named symbol in the type namespace (it carries no visibility
                // modifier — always public reuse). Register it so a cross-package `import type` +
                // `use T;` can resolve and mangle it to its FQN, exactly like a class/interface.
                Item::Trait(t) => (&t.name, true, crate::ast::Visibility::Public),
                _ => continue,
            };
            let table = if is_type { &mut types } else { &mut defined };
            if table
                .insert((pkg.clone(), name.clone()), mangle(&prog.package, name))
                .is_some()
            {
                return Err(format!(
                    "{}: duplicate definition of `{}` in package `{}` \
                     (a name must be unique within its package) [E-DUP-DEF]",
                    file.display(),
                    name,
                    if pkg.is_empty() { "main" } else { &pkg }
                ));
            }
            let prov = if is_type {
                &mut prov_types
            } else {
                &mut prov_fns
            };
            prov.insert(
                (pkg.clone(), name.clone()),
                DefInfo {
                    file: file.clone(),
                    package: pkg.clone(),
                    vis,
                },
            );
            // A free function's trace frame is keyed by its compiled (mangled) name — map it to its
            // file so a runtime trace can show `file:line` (methods/ctors are synthesized elsewhere).
            if !is_type {
                fn_files.insert(mangle(&prog.package, name), file.clone());
            }
            // DEC-320: every top-level definition (types too) → its declaring file, for the
            // `phg build --php` sibling emit's per-item routing.
            item_files.insert(mangle(&prog.package, name), file.clone());
            defs += 1;
        }
        parsed.push((file.clone(), prog));
    }

    // Q-A — expand wildcard imports (`import X.*;`) to per-member imports NOW, using the Pass-1 index
    // (user/vendored, gated by `vis_violation`) or the native registry (`Core.*`). Compile-time sugar
    // (Inv 5): Pass-2 and every backend see only plain per-symbol `Item::Import`s.
    expand_wildcard_imports(&mut parsed, &prov_fns, &prov_types)?;

    // M8.5 S3b — ambient `*.d.phg` declaration files: a file of foreign `declare`s carrying no package,
    // loaded into the project (the `.d.ts` analog). Parsed + validated (no package, all foreign) but
    // NOT folder=path-validated and NOT indexed as package definitions; their foreign items merge
    // ambiently into the unit (the checker's prebind makes merge order irrelevant) and are emitted by
    // the transpiler as global `\Name` symbols. First-party only — vendored decl bundling is deferred.
    // Excluded from `collect_phg`, so a decl file is never compiled as a package source.
    let mut decl_items: Vec<Item> = Vec::new();
    let mut decl_count = 0usize;
    let mut decl_seen: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for f in decl_files {
        if !decl_seen.insert(f.clone()) {
            continue;
        }
        let src = read_file(f)?;
        let prog = parse_at(f, &src)?;
        validate_decl_file(&prog, f)?;
        src_map.insert(f.clone(), src);
        decl_items.extend(prog.items);
        decl_count += 1;
    }

    let stats = LoadStats {
        files: sources.len() + decl_count,
        packages: pkgset.len(),
        defs,
    };

    // Pass 2 — resolve call sites per file, then flat-merge.
    let mut merged_items: Vec<Item> = Vec::new();
    // The merged unit runs as the entry's package (normally `main`); its span anchors any
    // program-level diagnostic.
    let mut unit_package: Vec<String> = vec!["Main".to_string()];
    let mut unit_span = Span {
        start: 0,
        len: 0,
        line: 0,
        col: 0,
    };

    for (file, prog) in parsed {
        if same_file(&file, entry) {
            unit_package = prog.package.clone();
            unit_span = prog.span;
        }
        // Q-A step 3 (G6): reject member imports naming a non-existent member (E-IMPORT-UNKNOWN).
        validate_member_imports(&prog, &defined, &types, &pkgset, &file)?;
        let user_imports = user_import_map(&prog.items, &types, &defined);
        let type_imports = build_type_imports(&prog, &types, &prov_types, &user_imports, &file)?;
        let function_imports =
            build_function_imports(&prog, &defined, &prov_fns, &user_imports, &file)?;
        let ctx = ResolveCtx {
            package: prog.package.clone(),
            user_imports,
            defined: &defined,
            types: &types,
            type_imports,
            function_imports,
            file: &file,
            prov_types: &prov_types,
            prov_fns: &prov_fns,
            violations: RefCell::new(Vec::new()),
        };
        for item in prog.items {
            merged_items.push(resolve_item(item, &ctx));
        }
        // Surface the first visibility violation collected while resolving this file (the
        // infallible `resolve_*` chain buffers them).
        if let Some(first) = ctx.violations.into_inner().into_iter().next() {
            return Err(first);
        }
    }

    // Ambient foreign declarations merge unmangled (they are global PHP symbols — never namespaced).
    merged_items.extend(decl_items);

    Ok(Unit {
        program: Program {
            package: unit_package,
            items: merged_items,
            span: unit_span,
        },
        diag_src: String::new(),
        stats: Some(stats),
        sources: src_map,
        fn_files,
        item_files,
    })
}

/// One source file in a project load, paired with the folder=path root it validates against and
/// whether it came from the vendor tree (a vendored file must be a library — never `package Main`).
struct Source {
    file: PathBuf,
    root: PathBuf,
    vendored: bool,
}

impl Source {
    fn first_party(file: PathBuf, source_root: &Path) -> Source {
        Source {
            file,
            root: source_root.to_path_buf(),
            vendored: false,
        }
    }
    fn vendored(file: PathBuf, dep_root: &Path) -> Source {
        Source {
            file,
            root: dep_root.to_path_buf(),
            vendored: true,
        }
    }
}

/// The globally-unique name for a top-level definition. `package Main` (and the malformed empty
/// package) keep the bare name — so the entry stays byte-identical to a single-file program; any
/// other package is mangled to a PHP-FQN-shaped key (`acme.util` + `compute` ⇒ `Acme\Util\compute`),
/// which the transpiler later splits back into a `namespace Acme\Util` block.
fn mangle(package: &[String], name: &str) -> String {
    if package.is_empty() || package == ["Main"] {
        return name.to_string();
    }
    let ns = package
        .iter()
        .map(|s| pascal(s))
        .collect::<Vec<_>>()
        .join("\\");
    format!("{ns}\\{name}")
}

/// PascalCase one package segment (`util` ⇒ `Util`) for the PHP namespace mapping (M5-2).
fn pascal(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The resolution context for one file: its package (caller side of a bare call), its user-import
/// map (for qualified calls), and the shared global symbol table.
struct ResolveCtx<'a> {
    package: Vec<String>,
    user_imports: HashMap<String, Vec<String>>,
    defined: &'a HashMap<(String, String), String>,
    /// Global type symbol table `(package, type) ⇒ mangled FQN` — for resolving a same-package
    /// sibling type reference inside a library package.
    types: &'a HashMap<(String, String), String>,
    /// This file's terminal type imports: bare name (or `as` alias) ⇒ mangled FQN.
    type_imports: HashMap<String, String>,
    /// DEC-197: this file's member FUNCTION imports: bare name (or `as` alias) ⇒ mangled FQN of a
    /// cross-package function, resolved by `resolve_call` after a same-package function of that name.
    function_imports: HashMap<String, String>,
    /// The file currently being resolved (the referrer side of the visibility lattice).
    file: &'a Path,
    /// Visibility provenance for type and function definitions (visibility modifiers).
    prov_types: &'a HashMap<(String, String), DefInfo>,
    prov_fns: &'a HashMap<(String, String), DefInfo>,
    /// Visibility violations collected while resolving this file's references (the `resolve_*` chain
    /// is infallible, so violations are buffered here and surfaced after the file is resolved).
    violations: RefCell<Vec<String>>,
}

#[cfg(test)]
mod tests;
