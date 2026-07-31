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
//! `Acme\Geometry\Point`) and a consuming file binds it with a unified `import a.b.C [as D];`; the same
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

// Cohesion split (M-Decomp): resolution walkers + fs helpers + the load pipeline / two-pass
// assembly / data-type impls / visibility lattice live in sibling files. This root keeps only the
// shared type definitions, the `mangle`/`pascal` name helpers, and the module wiring.
mod assemble;
mod discovery;
mod entry;
mod fs;
mod import_hygiene;
mod imports;
mod resolve;
mod resolve_stmts;
mod unit;
mod visibility;

use assemble::*;
use discovery::{discover_roots, index_packages, SearchRoots};
/// Project-package enumeration for the LSP import-path completion (the `discovery` module is private).
pub(crate) use discovery::{project_packages, project_phg_files};
/// The loader's public entry points (defined in `entry`) re-exported at `crate::loader::…`.
pub use entry::{discover_phg, load, load_loose_src, load_with_buffer};
use fs::*;
use import_hygiene::*;
use imports::*;
use resolve::*;
use visibility::*;

/// Provenance for one top-level definition: where it was declared and how visible it is. Built in
/// Pass 1 (which still has per-file information) and consumed by the visibility lattice during Pass 2.
#[derive(Clone)]
struct DefInfo {
    file: PathBuf,
    package: String,
    vis: Visibility,
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

/// Counts of what a project load assembled and handed to the checker — every `.phg` under the source
/// root (first-party + vendored), merged and validated as one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadStats {
    pub files: usize,
    pub packages: usize,
    pub defs: usize,
}

/// One resolved import: (winning root index, root label, root path, the package's files, the
/// package name) — the search loop's carrier (a named alias keeps clippy's type-complexity
/// lint honest).
type RootHit = (usize, &'static str, PathBuf, Vec<PathBuf>, String);

/// One indexed search root: (human label, root path, package → files declaration index).
type SearchIndex = (&'static str, PathBuf, BTreeMap<String, Vec<PathBuf>>);

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
