//! M-Lift L7 — **directory lift**: a PHP tree → a phorj PROJECT (DEC-439, developer-ruled).
//!
//! # Why a directory lift is the fix, not a convenience
//!
//! `phg lift <file>` produced a draft that could not resolve anything it referenced. Both halves of the
//! lift chain failed for the SAME reason — one file cannot see its siblings:
//!
//! * LIFT-NS emitted `import App.Support.Helper;` and the checker said `E-MODULE-NOT-FOUND`;
//! * LIFT-ATTR emitted `#[App.Meta.Audited]` and the checker said `E-UNKNOWN-ATTRIBUTE`.
//!
//! Lifting the whole tree in ONE pass fixes both at once, because the files that declare those symbols are
//! now in the project beside the files that use them.
//!
//! # What it does with what it cannot lift (developer-ruled)
//!
//! **Lift what lifts; name the rest.** A real Symfony/Laravel app contains plenty of Tier-2 PHP, so an
//! all-or-nothing lift would produce nothing at all on any real input. Every file that fails is recorded in
//! `LIFT-REPORT.md` with its reason — which doubles as the ranked worklist of what the lifter still cannot
//! do. Nothing is faked and nothing is silently skipped.
//!
//! # What it does with composer VENDOR (developer-ruled)
//!
//! **Report by default.** `VENDOR-REPORT.md` lists every vendor symbol the app references, attributed to
//! the composer package that ships it (exactly, via `installed.json`, when a `vendor/` tree is present),
//! with a per-symbol reference count. That report is the migration worklist.
//!
//! Foreign `declare` stubs are the ruled OPT-IN (`--vendor=stub`) and are NOT built in this slice — see the
//! "not yet" note on [`VendorMode`]. The reason they must stay opt-in is measured, not stylistic: a program
//! carrying foreign declarations cannot run on either phorj engine (`E-FOREIGN-RUNTIME`), so stubs trade the
//! VM, the JIT and the byte-identity spine for a draft that checks. Invariant 14 forbids making that trade
//! silently.

use std::path::{Path, PathBuf};

mod classify;
mod discover;
mod layout;
mod report;

/// What to do about composer dependencies (DEC-439).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorMode {
    /// The default: every vendor symbol is listed in `VENDOR-REPORT.md` and nothing is synthesized.
    Report,
    /// `--vendor=stub`: additionally generate foreign `declare` stubs from the vendor's own type hints.
    ///
    /// **Not implemented in this slice.** Accepted by the CLI and refused with the reason, rather than
    /// silently behaving like [`VendorMode::Report`] — a flag that quietly does something else is worse
    /// than one that says "not yet".
    Stub,
}

/// Why a `phg lift <dir>` invocation was rejected. Split from a plain string so the caller can map
/// "your command line is wrong" (usage, exit 2) and "the lift failed" (exit 1) onto different exits
/// without string-sniffing.
pub enum CliError {
    /// Print usage and exit 2.
    Usage,
    Failed(String),
}

/// `phg lift <dir> [-o <out>] [--vendor=report|--vendor=stub]` — argument parsing plus the lift.
///
/// Lives here rather than in `main.rs` because that file is a grandfathered size-gate breach Invariant 13
/// forbids growing; keeping the parsing beside the thing it configures is also where it belongs.
///
/// `args` starts at the DIRECTORY argument.
pub fn cli_lift_directory(args: &[String]) -> Result<String, CliError> {
    let root = std::path::PathBuf::from(args.first().ok_or(CliError::Usage)?);
    let mut out: Option<String> = None;
    let mut vendor = VendorMode::Report;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                out = Some(args.get(i + 1).ok_or(CliError::Usage)?.clone());
                i += 2;
            }
            "--vendor=report" => i += 1,
            "--vendor=stub" => {
                vendor = VendorMode::Stub;
                i += 1;
            }
            _ => return Err(CliError::Usage),
        }
    }
    // `-o` is REQUIRED: a directory lift writes a whole tree, so where it lands is never implied.
    let out = out.ok_or(CliError::Usage)?;
    lift_directory(&root, std::path::Path::new(&out), vendor).map_err(CliError::Failed)
}

/// One file that could not be lifted, and why.
struct Failure {
    /// Path relative to the input root, so the report is portable between machines.
    rel: String,
    reason: String,
}

/// A PHP file that is NOT the app's own code — a framework bootstrap or a PHP configuration file. It has no
/// phorj translation; it has a phorj replacement, which [`classify::Role::phorj_counterpart`] names.
struct Glue {
    rel: String,
    role: classify::Role,
}

/// A vendor symbol the app references.
struct VendorRef {
    /// The PHP fully-qualified name, as resolved from the referencing file.
    fqn: String,
    /// The composer package that ships it, when `installed.json` says so.
    package: Option<String>,
    /// How many app files reference it — the report sorts by this, so the worklist is ranked.
    refs: usize,
}

/// Lift `root` into a new phorj project at `out`.
///
/// `out` must not already contain files: a lift writes a whole tree, and overwriting an existing project
/// is exactly the kind of unrecoverable surprise the caller should have to ask for explicitly.
pub fn lift_directory(root: &Path, out: &Path, vendor: VendorMode) -> Result<String, String> {
    if vendor == VendorMode::Stub {
        return Err(
            "lift: `--vendor=stub` (foreign `declare` stubs for composer dependencies) is \
                    ruled but not implemented yet (DEC-439). Re-run without it to get \
                    `VENDOR-REPORT.md`, which lists every vendor symbol the app references."
                .to_string(),
        );
    }
    if !root.is_dir() {
        return Err(format!("lift: `{}` is not a directory", root.display()));
    }
    if out.exists()
        && std::fs::read_dir(out)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!(
            "lift: output directory `{}` is not empty — a directory lift writes a whole project tree, \
             so it will not overwrite one. Choose an empty path.",
            out.display()
        ));
    }

    let composer = discover::Composer::read(root)?;
    let mut files = discover::app_php_files(root, &composer)?;
    // Every PHP file in the TREE — by CONTENT rather than extension, so `bin/console` and Laravel's
    // `artisan` are seen at all — then CLASSIFIED rather than ignored or guessed at by path (DEC-439
    // part 2). A file that declares types is the app's own code however composer maps it (Doctrine's
    // `migrations/` above all), so it joins the lift; a framework bootstrap or a PHP config file has no
    // phorj translation at all, only a phorj REPLACEMENT, and the report names it.
    //
    // Before this existed the reports counted "files I looked at" and called it "files that exist" — the
    // silent-omission failure this lifter exists to avoid. [Verified on a Symfony-shaped fixture: 8 PHP
    // files present, 4 examined.]
    let mut glue: Vec<Glue> = Vec::new();
    let mut candidates = discover::all_php_files(root)?;
    // A declared executable is classified even when the content sniff cannot see it — a script written with
    // PHP's short tags has no `<?php` to find, and composer already told us the file exists.
    for rel in &composer.bin {
        let p = root.join(rel);
        if p.is_file() && !candidates.contains(&p) {
            candidates.push(p);
        }
    }
    candidates.sort();
    for path in candidates {
        if files.contains(&path) {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap_or_default();
        match classify::classify(&src) {
            classify::Role::Code => files.push(path),
            role => glue.push(Glue {
                rel: relative(root, &path),
                role,
            }),
        }
    }
    files.sort();
    glue.sort_by(|a, b| a.rel.cmp(&b.rel));
    if files.is_empty() {
        // Two different empty results, and reporting them the same way would be a lie: a tree with no PHP
        // at all is a wrong argument, while a tree whose PHP is ALL bootstrap and configuration is a real
        // PHP app that simply has no own-code to lift — the developer needs to know which one they hit.
        if glue.is_empty() {
            return Err(format!(
                "lift: no `.php` files found under `{}` (searched {} autoload root(s); `vendor/` is never \
                 walked)",
                root.display(),
                composer.psr4.len() + composer.classmap.len() + composer.files.len()
            ));
        }
        return Err(format!(
            "lift: found {} PHP file(s) under `{}`, but every one is framework bootstrap or PHP \
             configuration — none declares a class, interface, trait, enum or function, so there is no \
             application code to lift. Point the lift at the directory holding your own classes (for a \
             Symfony/Laravel layout that is `src/`, or whatever `composer.json`'s `autoload` maps).",
            glue.len(),
            root.display()
        ));
    }

    let mut lifted = 0usize;
    let mut failures: Vec<Failure> = Vec::new();
    // Every class/function the lift DECLARED, so a reference to one is not reported as vendor. Needed
    // because a project with no `composer.json` has no PSR-4 prefixes to test against.
    let mut declared: Vec<String> = Vec::new();
    let mut vendor_refs: Vec<VendorRef> = Vec::new();
    // The source file that became `src/main.phg`, and any further entry-shaped files after it.
    let mut entry_taken: Option<String> = None;
    let mut entry_conflicts: Vec<String> = Vec::new();
    // Destinations already written. Without this, two source files that map to the SAME package and file
    // stem silently overwrote each other and the report still said "lifted 2/2" — measured, not
    // hypothetical: `src/A/Helper.php` + `src/B/Helper.php` (both `namespace App`) produced ONE file and
    // lost a class. Legacy PHP hits this constantly, because every namespace-LESS file lands in
    // `package Main` and collides on its bare stem.
    let mut written: Vec<PathBuf> = Vec::new();
    let mut renames: Vec<(String, String)> = Vec::new();

    for path in &files {
        let rel = relative(root, path);
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(Failure {
                    rel,
                    reason: format!("cannot read the file: {e}"),
                });
                continue;
            }
        };
        // The vendor scan runs on the PHP source and is INDEPENDENT of whether the lift succeeds — which
        // matters most precisely where the lift could NOT finish: a file the lifter refuses still tells us
        // which vendor packages the app depends on. That independence needs the token-level fallback in
        // `scan_vendor`, because a Tier-2 construct anywhere in the file fails the whole PARSE, not just
        // the lift. [Verified on a fixture: `use Doctrine\ORM\EntityManager;` in a file containing a
        // `switch` was missing from the report until the fallback existed.]
        scan_vendor(&src, &composer, &mut vendor_refs, &mut declared);
        match crate::lift::lifter::lift_source(&src) {
            Ok(phg) => {
                // A draft the lifter gave an `#[Entry]` to came from PHP with top-level code (or a
                // `main()`), i.e. a SCRIPT. phorj's entry convention is `package Main;` at the source
                // root, and that is not cosmetic: a dotted package must sit in a matching subdirectory
                // (`E-PKG-PATH`), while `package Main` is exempt and runnable anywhere — so an entry left
                // in its namespace package makes the whole project fail to load. [Verified: the same tree
                // with the entry as `package Acme.Blog;` under `src/Acme/Blog/` is `E-PKG-PATH`; as
                // `package Main;` at `src/main.phg` it reports "whole project type-checks clean".]
                let is_entry = phg.contains("#[Entry(");
                if is_entry && entry_taken.is_none() {
                    entry_taken = Some(rel.clone());
                    let as_main = layout::repackage_as_main(&phg);
                    let main_dest = out.join("src").join("main.phg");
                    written.push(main_dest.clone());
                    write_file(&main_dest, &layout::draft_header(&rel, &as_main))?;
                } else {
                    if is_entry {
                        // PHP allows any number of scripts with top-level code; a phorj project has ONE
                        // entry per role, so a second is a decision only the developer can make. Emitted
                        // at its namespace path and REPORTED — never silently demoted or dropped.
                        entry_conflicts.push(rel.clone());
                    }
                    let dest = layout::unique_destination(
                        layout::destination(out, &phg, path),
                        path,
                        &mut written,
                        &rel,
                        &mut renames,
                    );
                    write_file(&dest, &layout::draft_header(&rel, &phg))?;
                }
                lifted += 1;
            }
            Err(reason) => failures.push(Failure { rel, reason }),
        }
    }

    // A vendor symbol the app itself declares is not vendor — drop those now that every file is scanned.
    vendor_refs.retain(|v| !declared.contains(&v.fqn));
    vendor_refs.sort_by(|a, b| b.refs.cmp(&a.refs).then(a.fqn.cmp(&b.fqn)));

    write_file(
        out.join("phorj.json").as_path(),
        &layout::manifest_json(root),
    )?;
    write_file(
        out.join("LIFT-REPORT.md").as_path(),
        &report::lift_report(
            root,
            &report::Outcome {
                total: files.len(),
                lifted,
                failures: &failures,
                entry: entry_taken.as_deref(),
                entry_conflicts: &entry_conflicts,
                glue: &glue,
                renames: &renames,
            },
        ),
    )?;
    write_file(
        out.join("VENDOR-REPORT.md").as_path(),
        &report::vendor_report(root, &composer, &vendor_refs),
    )?;

    Ok(report::summary(
        out,
        &report::Counts {
            total: files.len(),
            lifted,
            failed: failures.len(),
            vendor: vendor_refs.len(),
            entry: entry_taken,
            entry_conflicts: entry_conflicts.len(),
            glue: glue.len(),
        },
    ))
}

/// Collect this file's VENDOR references (and the symbols it declares).
///
/// Deliberately driven by the PHP `use` list and attribute names rather than by type analysis: a `use` IS
/// the file's own statement of what it depends on, so the answer is exact and needs no resolution of
/// expression types. A reference count is accumulated per FQN so the report can rank the worklist.
fn scan_vendor(
    src: &str,
    composer: &discover::Composer,
    out: &mut Vec<VendorRef>,
    declared: &mut Vec<String>,
) {
    let Ok((toks, docs)) = crate::lift::lexer::lex_php_with_docs(src) else {
        return;
    };
    let uses = match crate::lift::parser::parse_php_with_docs(toks.clone(), docs) {
        Ok(prog) => {
            let ns = prog.namespace.join("\\");
            for item in &prog.items {
                if let Some(name) = crate::lift::ast::php_item_name(item) {
                    declared.push(if ns.is_empty() {
                        name.to_string()
                    } else {
                        format!("{ns}\\{name}")
                    });
                }
            }
            prog.uses.iter().map(|u| u.path.join("\\")).collect()
        }
        // The file is outside the Tier-1 subset — but its DEPENDENCIES are still knowable, and this is the
        // case where knowing them matters most. Read the file-level `use` block off the token stream.
        Err(_) => file_level_uses(&toks),
    };
    for fqn in uses {
        // An app-namespace `use` is a sibling reference — exactly what the directory lift fixes.
        if composer.is_app_namespace(&fqn) {
            continue;
        }
        match out.iter_mut().find(|v| v.fqn == fqn) {
            Some(v) => v.refs += 1,
            None => out.push(VendorRef {
                package: composer.package_of(&fqn).map(str::to_string),
                fqn,
                refs: 1,
            }),
        }
    }
}

/// The `use A\B\C;` paths in a file's FILE-LEVEL import block, read straight off the tokens so a file the
/// parser rejects still reports its dependencies.
///
/// Scanning STOPS at the first `class`/`interface`/`trait`/`enum`/`function` keyword, which is what keeps a
/// class-body `use SomeTrait;` (trait composition — a different meaning of the same keyword) out of the
/// import list. PSR-12 puts every import above the first declaration, so nothing real is missed.
fn file_level_uses(toks: &[crate::lift::lexer::PTokenSpanned]) -> Vec<String> {
    use crate::lift::lexer::PTok;
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i].tok {
            PTok::Ident(k)
                if matches!(
                    k.as_str(),
                    "class" | "interface" | "trait" | "enum" | "function"
                ) =>
            {
                break;
            }
            PTok::Ident(k) if k == "use" => {
                i += 1;
                // `use function f;` / `use const K;` import a symbol, not a type — not a class dependency.
                if matches!(&toks[i].tok, PTok::Ident(k) if k == "function" || k == "const") {
                    while i < toks.len() && !matches!(toks[i].tok, PTok::Semi) {
                        i += 1;
                    }
                    continue;
                }
                let mut segs: Vec<String> = Vec::new();
                while i < toks.len() {
                    match &toks[i].tok {
                        PTok::Ident(seg) if seg == "as" => {
                            // The alias is a local name, not part of the path.
                            while i < toks.len() && !matches!(toks[i].tok, PTok::Semi) {
                                i += 1;
                            }
                            break;
                        }
                        PTok::Ident(seg) => segs.push(seg.clone()),
                        PTok::Backslash => {}
                        _ => break,
                    }
                    i += 1;
                }
                if !segs.is_empty() {
                    out.push(segs.join("\\"));
                }
            }
            _ => i += 1,
        }
    }
    out
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|e| format!("cannot write `{}`: {e}", path.display()))
}
