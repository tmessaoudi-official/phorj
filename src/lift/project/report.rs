//! Directory lift — the two REPORTS and the stdout summary (DEC-439).
//!
//! Both reports exist for the same reason the lifter refuses loudly elsewhere: a migration tool that
//! quietly does 80% of the job leaves the developer with no way to see the other 20%. `LIFT-REPORT.md`
//! answers "what could the lifter not do", `VENDOR-REPORT.md` answers "what does this app depend on that
//! is not its own code" — and both are ranked worklists rather than prose.

use super::{Failure, VendorRef};
use std::path::Path;

/// Everything one directory lift has to say about the FILES it saw — the input to `LIFT-REPORT.md`. A
/// struct rather than eight positional parameters: four of them are lists of strings, so a transposed
/// pair would be invisible at the call site.
pub(super) struct Outcome<'a> {
    pub(super) total: usize,
    pub(super) lifted: usize,
    pub(super) failures: &'a [Failure],
    /// The source file that became the entry, if any.
    pub(super) entry: Option<&'a str>,
    /// Further scripts with top-level code — only one file can be the entry.
    pub(super) entry_conflicts: &'a [String],
    /// Files that are NOT the app's code — framework bootstrap and PHP configuration, each with the phorj
    /// replacement its role implies.
    pub(super) glue: &'a [super::Glue],
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
    out.push_str(&glue_section(o.glue));
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

/// The files REPORTED rather than lifted, grouped by ROLE with the phorj replacement each one implies
/// (DEC-439 parts 2 and 3).
///
/// This section is the difference between a list of skipped files and a migration PLAN. The classification
/// is by CONTENT, never by path — a rule matching `public/index.php` or `artisan` by name would be a list of
/// the frameworks the lifter happens to know, and wrong for the next one. What these files actually differ
/// in is their shape: declarations mean the app's own code (lifted, not listed here), a top-level `return`
/// of data means configuration, and anything else means a bootstrap script. The one exception is TEST code,
/// which declares classes like any other and so is recognized from composer's own `autoload-dev` — still a
/// machine-readable declaration rather than a guess at a directory name.
///
/// Note what these files are NOT: candidates for a better lifter. `public/index.php` constructs a Symfony
/// `Kernel`; there is nothing to port, because phorj has no Kernel and will not. It is REPLACED by an
/// `#[Entry]`, and saying so is more useful than lifting it into something that looks like code and means
/// nothing.
fn glue_section(glue: &[super::Glue]) -> String {
    if glue.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "## {} file(s) to RE-EXPRESS, not lift\n\nNone of these has a phorj TRANSLATION; each has a phorj \
         REPLACEMENT, which is the right outcome rather than a limitation. No framework paths are hardcoded \
         anywhere in the lifter: bootstrap and configuration are told apart by CONTENT, and test code by \
         composer's own `autoload-dev` declaration.\n\n| File | Role | phorj counterpart |\n|---|---|---|\n",
        glue.len()
    );
    for g in glue {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            g.rel,
            g.role.label(),
            g.role.phorj_counterpart().unwrap_or("—")
        ));
    }
    out.push_str(
        "\n> A file here with role `bootstrap` or `configuration` that you believe IS your own code is worth \
         reporting: it means it declares nothing the classifier could see, which is unusual for application \
         code.\n\n",
    );
    out
}

/// The RENAME section: drafts written somewhere other than where their package would put them.
///
/// Disclosed rather than silent because the draft is not where a reader would look for it. Before this
/// existed, two sources mapping to the same package AND stem overwrote each other while the summary still
/// reported "lifted 2/2" — a silent data loss.
fn rename_section(renames: &[(String, String)]) -> String {
    if renames.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "## {} draft(s) renamed to avoid a collision\n\nTwo or more sources mapped to the same package and \
         file name, so the later ones were written under a disambiguated name instead of overwriting the \
         earlier one. Nothing was lost — but the file name no longer matches the original.\n\n\
         | Source | Written to |\n|---|---|\n",
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

/// What one directory lift did, in numbers — the input to the stdout summary. A struct rather than seven
/// positional arguments: the counts are all `usize`, so a transposed pair would be invisible.
pub(super) struct Counts {
    pub(super) total: usize,
    pub(super) lifted: usize,
    pub(super) failed: usize,
    pub(super) vendor: usize,
    /// The source file that became the entry, if any.
    pub(super) entry: Option<String>,
    pub(super) entry_conflicts: usize,
    pub(super) glue: usize,
}

/// The one-screen stdout summary. Names the reports rather than dumping them: a directory lift can produce
/// hundreds of rows, and the useful stdout is "what happened and where to look".
pub(super) fn summary(out_dir: &Path, c: &Counts) -> String {
    let Counts {
        total,
        lifted,
        failed,
        vendor,
        entry,
        entry_conflicts,
        glue,
    } = c;
    let (failed, vendor, entry_conflicts, glue) = (*failed, *vendor, *entry_conflicts, *glue);
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
    if glue > 0 {
        s.push_str(&format!(
            "  {glue} file(s) to RE-EXPRESS, not lift (bootstrap / PHP config / test code) — each paired \
             with its phorj counterpart in `LIFT-REPORT.md`\n"
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
