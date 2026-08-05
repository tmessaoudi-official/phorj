//! Directory lift — the two REPORTS and the stdout summary (DEC-439).
//!
//! Both reports exist for the same reason the lifter refuses loudly elsewhere: a migration tool that
//! quietly does 80% of the job leaves the developer with no way to see the other 20%. `LIFT-REPORT.md`
//! answers "what could the lifter not do", `VENDOR-REPORT.md` answers "what does this app depend on that
//! is not its own code" — and both are ranked worklists rather than prose.

use super::{Failure, VendorRef};
use std::path::Path;

/// `LIFT-REPORT.md` — every file, lifted or not, with the reason for each failure.
/// Everything one directory lift has to say about the FILES it saw. A struct rather than eight
/// positional parameters — four of them are lists of strings, and a transposed pair would be invisible.
pub(super) struct Outcome<'a> {
    pub(super) total: usize,
    pub(super) lifted: usize,
    pub(super) failures: &'a [Failure],
    /// The source file that became the entry, if any.
    pub(super) entry: Option<&'a str>,
    /// Further scripts with top-level code — only one file can be the entry.
    pub(super) entry_conflicts: &'a [String],
    /// PHP files present in the tree but outside composer's autoload map.
    pub(super) unexamined: &'a [String],
    /// (source, destination) for each draft renamed to avoid a collision.
    pub(super) renames: &'a [(String, String)],
}

pub(super) fn lift_report(root: &Path, o: &Outcome<'_>) -> String {
    let (total, lifted, failures) = (o.total, o.lifted, o.failures);
    let mut out = String::new();
    out.push_str("# Lift report\n\n");
    out.push_str(&format!(
        "Source: `{}`\n\nLifted **{lifted} of {total}** PHP file(s).\n\n",
        root.display()
    ));
    out.push_str(&entry_section(o.entry, o.entry_conflicts));
    out.push_str(&unexamined_section(o.unexamined));
    out.push_str(&rename_section(o.renames));
    if failures.is_empty() {
        out.push_str(
            "## Every file lifted\n\nThat is not the same as every file being CORRECT: a lifted draft is \
             `// lifted (verify)` — the lifter cannot prove it preserved the original's behaviour, only \
             that it had a faithful form for everything it saw.\n",
        );
        return out;
    }
    out.push_str(&format!(
        "## {} file(s) not lifted\n\nEach is refused with a reason rather than half-lifted — the lifter \
         never guesses (DEC-166). This list is also the ranked worklist for the lifter itself: the \
         reasons that repeat are the constructs worth supporting next.\n\n\
         | File | Why not |\n|---|---|\n",
        failures.len()
    ));
    for f in failures {
        // A reason can contain `|` (a PHP union type in a message), which would break the table.
        out.push_str(&format!(
            "| `{}` | {} |\n",
            f.rel,
            f.reason.replace('|', "\\|").replace('\n', " ")
        ));
    }
    out.push_str(
        "\n### What to do with these\n\n\
         A refusal names the construct it could not map. Rewrite that construct in the PHP and re-run the \
         lift, or lift the file by hand — the drafts already produced are unaffected either way, since \
         each file is lifted independently.\n",
    );
    out
}

/// The ENTRY section: which PHP script became `src/main.phg`, and any further scripts that cannot also be
/// entries.
///
/// This is not bookkeeping. A PHP script with top-level code IS an entry, and phorj's entry lives at the
/// source root as `package Main;` — a dotted package must sit in a matching subdirectory (`E-PKG-PATH`), so
/// an entry left in its namespace package makes the whole project fail to LOAD, not merely fail to run.
/// [Verified: the same tree is `E-PKG-PATH` with the entry as `package Acme.Blog;` and reports
/// "whole project type-checks clean" with it as `package Main;` at `src/main.phg`.]
fn entry_section(entry: Option<&str>, conflicts: &[String]) -> String {
    let mut out = String::new();
    match entry {
        Some(rel) => out.push_str(&format!(
            "## Entry\n\n`{rel}` had top-level code, so it became the project entry: `src/main.phg`, \
             `package Main;`. Check or run the project from there.\n\n"
        )),
        None => out.push_str(
            "## Entry\n\nNo file had top-level code, so the lift produced a LIBRARY project with no \
             entry — there is nothing to `phg run`. Add a file with an `#[Entry]` function to make it \
             runnable; `phg check` on any file still validates the whole project.\n\n",
        ),
    }
    if !conflicts.is_empty() {
        out.push_str(&format!(
            "### {} further script(s) with top-level code\n\nPHP allows any number of scripts; a phorj \
             project has ONE entry per role, so these were left in their own packages and still carry an \
             `#[Entry]` — which means the project will not check until you pick one. That choice is \
             yours, not the lifter's.\n\n",
            conflicts.len()
        ));
        for c in conflicts {
            out.push_str(&format!("- `{c}`\n"));
        }
        out.push('\n');
    }
    out
}

/// Every PHP file in the tree the lift did NOT examine, because it sits outside composer's `autoload`
/// map — a Symfony/Laravel app is full of them (`public/index.php`, `bin/console`, `migrations/`,
/// `config/*.php`, `artisan`).
///
/// This section exists because its absence made the report LIE: it counted "files I looked at" and called
/// it "files that exist". [Verified on a Symfony-shaped fixture: 8 PHP files present, 4 examined, 4
/// invisible.] Listing them is the floor, not the fix — what should HAPPEN to each class of file is a
/// ruling in flight (DEC-439 follow-up), and until it lands the honest behaviour is to name them.
///
/// Note what the detection had to do to see `bin/console` at all: PHP-ness is decided by CONTENT, since
/// that file and Laravel's `artisan` have no extension for a filter to match.
fn unexamined_section(unexamined: &[String]) -> String {
    if unexamined.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "## {} PHP file(s) NOT examined — outside composer's autoload map\n\n\
         These exist in the tree and were not lifted, not attempted, and not counted above. They are \
         listed so the count is honest, NOT because listing them is the right long-term answer.\n\n\
         Most of them are one of two things, and neither is a lift:\n\n\
         - **framework bootstrap** (`public/index.php`, `bin/console`, `artisan`, `bootstrap/app.php`) — \
           these construct a Symfony `Kernel` or Laravel `Application`. There is nothing to port: phorj \
           has no Kernel. They are REPLACED by a phorj entry — `#[Entry(kind: Web)]` for the front \
           controller, `#[Entry(kind: Cli)]` for the console;\n\
         - **framework configuration written in PHP** (`config/*.php`, `routes/web.php`) — re-expressed \
           with phorj's own `#[Config]` (DEC-318) and `#[Route]`, not translated statement by statement.\n\n\
         The exception is code that is genuinely YOURS but happens to live outside the autoload map — \
         Doctrine `migrations/` above all (`AbstractMigration` subclasses). Those should be lifted, and \
         how to find them without hardcoding a framework's directory names is the open question.\n\n",
        unexamined.len()
    );
    for u in unexamined {
        out.push_str(&format!("- `{u}`\n"));
    }
    out.push('\n');
    out
}

/// Files whose draft had to be RENAMED because two sources mapped to the same package and stem.
///
/// This section is the fix for a measured silent data loss: `src/A/Helper.php` and `src/B/Helper.php`, both
/// `namespace App`, both wanted `src/App/Helper.phg` — the second overwrote the first and the summary still
/// reported "lifted 2/2". Legacy PHP hits it constantly, since every namespace-LESS file lands in
/// `package Main` and collides on its bare stem.
///
/// Renaming loses nothing (a phorj package directory may hold any number of files under any names), but a
/// developer looking for a particular file's draft has to be told where it went.
fn rename_section(renames: &[(String, String)]) -> String {
    if renames.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "## {} draft(s) renamed to avoid a collision\n\nTwo or more source files mapped to the same \
         package AND the same file name. Nothing was lost — a phorj package directory may hold any number \
         of files, and a file's own name carries no meaning — but the draft is not where you would look \
         for it, so each is listed here.\n\n| Source | Written to |\n|---|---|\n",
        renames.len()
    );
    for (src, dest) in renames {
        out.push_str(&format!("| `{src}` | `{dest}` |\n"));
    }
    out.push('\n');
    out
}

/// `VENDOR-REPORT.md` — every composer dependency symbol the app references, ranked by reference count.
pub(super) fn vendor_report(
    root: &Path,
    composer: &super::discover::Composer,
    refs: &[VendorRef],
) -> String {
    let mut out = String::new();
    out.push_str("# Vendor report\n\n");
    out.push_str(&format!("Source: `{}`\n\n", root.display()));
    if refs.is_empty() {
        out.push_str(
            "The app references no symbols outside its own PSR-4 namespaces. Nothing here needs porting \
             or stubbing.\n",
        );
        return out;
    }
    out.push_str(&format!(
        "The app references **{} symbol(s)** it does not declare, ordered by how many files use each — \
         so the top of this table is where porting or stubbing pays off most.\n\n\
         | References | Symbol | Composer package |\n|---|---|---|\n",
        refs.len()
    ));
    for v in refs {
        out.push_str(&format!(
            "| {} | `{}` | {} |\n",
            v.refs,
            v.fqn,
            v.package
                .as_deref()
                .map_or_else(|| "*unattributed*".to_string(), |p| format!("`{p}`"))
        ));
    }
    if composer.installed_psr4.is_empty() {
        out.push_str(
            "\n> Symbols are **unattributed** because no `vendor/composer/installed.json` was found. Run \
             `composer install` in the source tree and re-run the lift to get exact package attribution — \
             the symbol list itself is already complete.\n",
        );
    }
    out.push_str(
        "\n## What happens to these\n\n\
         Nothing, by default — they are reported, never synthesized. Each one is a choice:\n\n\
         1. **port it** — lift or rewrite the dependency as a phorj package;\n\
         2. **stub it** — declare its shape as a foreign PHP symbol (`declare class` / `declare function`, \
            M8.5). Generating these is the ruled `--vendor=stub` option, not yet implemented (DEC-439).\n\n\
         > **Stubs have a price, and it is not obvious.** A program carrying foreign declarations cannot \
         > run on either phorj engine — `phg run` reports `E-FOREIGN-RUNTIME` — so it becomes \
         > TRANSPILE-ONLY: no VM, no JIT, and no byte-identity spine, since there is then only one leg to \
         > compare. That is a deliberate trade to make on purpose, which is why stubs are opt-in rather \
         > than the default.\n",
    );
    out
}

/// The one-screen stdout summary. Names the reports rather than dumping them: a directory lift can produce
/// hundreds of rows, and the useful stdout is "what happened and where to look".
/// What one directory lift did, in numbers. A struct rather than eight positional arguments: the counts
/// are all `usize` and a transposed pair would be invisible at the call site.
pub(super) struct Counts {
    pub(super) total: usize,
    pub(super) lifted: usize,
    pub(super) failed: usize,
    pub(super) vendor: usize,
    /// The source file that became the entry, if any.
    pub(super) entry: Option<String>,
    pub(super) entry_conflicts: usize,
    pub(super) unexamined: usize,
}

pub(super) fn summary(out_dir: &Path, c: &Counts) -> String {
    let Counts {
        total,
        lifted,
        failed,
        vendor,
        entry,
        entry_conflicts,
        unexamined,
    } = c;
    let (failed, vendor, entry_conflicts, unexamined) =
        (*failed, *vendor, *entry_conflicts, *unexamined);
    let entry = entry.as_deref();
    let mut s = format!(
        "lifted {lifted}/{total} PHP file(s) into `{}`\n",
        out_dir.display()
    );
    match entry {
        Some(rel) => s.push_str(&format!(
            "  entry: `{rel}` -> `src/main.phg` (package Main)\n"
        )),
        None => s.push_str("  no entry — a LIBRARY project (no file had top-level code)\n"),
    }
    if entry_conflicts > 0 {
        s.push_str(&format!(
            "  {entry_conflicts} further script(s) with top-level code — only one can be the entry, \
             see `LIFT-REPORT.md`\n"
        ));
    }
    if failed > 0 {
        s.push_str(&format!(
            "  {failed} file(s) NOT lifted — each with its reason in `LIFT-REPORT.md`\n"
        ));
    }
    if unexamined > 0 {
        s.push_str(&format!(
            "  {unexamined} PHP file(s) NOT examined — outside composer's autoload map \
             (`public/`, `bin/`, `migrations/`, `config/`…), listed in `LIFT-REPORT.md`\n"
        ));
    }
    s.push_str(&format!(
        "  {vendor} vendor symbol(s) referenced — ranked in `VENDOR-REPORT.md` (nothing was stubbed)\n"
    ));
    s.push_str(&format!(
        "\nThe drafts are `// lifted (verify)`. Check the WHOLE project with `phg check {}` — an \
         unresolved vendor symbol is expected until you port or stub it.\n",
        if entry.is_some() {
            "<out>/src/main.phg"
        } else {
            "<out>/src/<any-file>.phg"
        }
    ));
    s
}
