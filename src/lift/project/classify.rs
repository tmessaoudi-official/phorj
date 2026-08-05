//! Directory lift — the ROLE of a PHP file, decided from its CONTENT (DEC-439 part 2).
//!
//! This is the answer to "is there an automatic way without hardcoding their paths". A Symfony app keeps
//! `public/index.php`, `bin/console`, `migrations/`, `config/*.php` outside `autoload.psr-4`, and a Laravel
//! app keeps `artisan`, `routes/web.php`, `bootstrap/app.php` outside it — but a rule that matched those
//! NAMES would be a list of frameworks the lifter happens to know, wrong for the next one. What the files
//! actually differ in is their content, and that generalizes:
//!
//! | Role | Shape | What it means |
//! |---|---|---|
//! | [`Role::Code`] | declares a class / interface / trait / enum / function | the app's own code — LIFT it |
//! | [`Role::Config`] | a top-level `return` of DATA, and no declarations | configuration expressed as PHP — RE-EXPRESS it |
//! | [`Role::Bootstrap`] | anything else with no declarations | a script that wires a framework up — REPLACE it |
//!
//! `migrations/Version*.php` lands in `Code` because it declares a class, with no mention of Doctrine
//! anywhere in this file. `public/index.php` lands in `Bootstrap`, whether the skeleton returns a closure
//! (Symfony's runtime component) or calls the kernel directly.
//!
//! The "of DATA" qualifier on [`Role::Config`] is load-bearing, not a refinement: Symfony's front controller
//! and a `config/*.php` file are BOTH a top-level `return`, so a rule that stopped there told the developer
//! to re-express their front controller as typed configuration — wrong advice, confidently given. A returned
//! closure or `new` is a FACTORY; only returned data is configuration.
//!
//! The classification is TOKEN-level on purpose: these files are exactly the ones most likely to be outside
//! the Tier-1 subset, so anything requiring a successful parse would fail to classify precisely where the
//! answer matters.

use crate::lift::lexer::PTok;

/// What kind of PHP file this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// Declares types or functions — the app's own code. Lift it.
    Code,
    /// A top-level `return` of DATA, with nothing declared: framework configuration.
    Config,
    /// Top-level statements, or a returned factory: a front controller, a console entry, a wiring script.
    Bootstrap,
}

impl Role {
    /// The phorj counterpart to re-express this file with. `None` for [`Role::Code`], which is lifted
    /// rather than replaced.
    ///
    /// Naming the replacement is the difference between a list of files and a migration plan: phorj HAS the
    /// equivalents (`#[Entry(kind: …)]` for an entry, `#[Config]` from DEC-318 for typed configuration,
    /// `#[Route]` for routing), so the report can say what to write instead of only what was skipped.
    pub(super) fn phorj_counterpart(self) -> Option<&'static str> {
        match self {
            Role::Code => None,
            Role::Config => Some(
                "this is DATA: re-express it as a `#[Config]` class (DEC-318 typed configuration) read at \
                 the entry — not as logic to translate statement by statement",
            ),
            Role::Bootstrap => Some(
                "replace with a phorj entry — `#[Entry(kind: EntryKind.Web)]` for a front controller, \
                 `#[Entry(kind: EntryKind.Cli)]` for a console — plus `#[Route]` handlers for whatever it \
                 registers. There is no Kernel to port",
            ),
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Role::Code => "code",
            Role::Config => "configuration",
            Role::Bootstrap => "bootstrap",
        }
    }
}

/// Classify a PHP file from its source.
///
/// A file that will not even LEX is treated as [`Role::Code`]: the lift attempt then produces a real,
/// specific error in `LIFT-REPORT.md`, which is far more useful than this function guessing at a role it
/// cannot see.
pub(super) fn classify(src: &str) -> Role {
    let Ok(toks) = crate::lift::lexer::lex_php(src) else {
        return Role::Code;
    };
    let mut depth: i32 = 0;
    let mut returns_data = false;
    for (i, t) in toks.iter().enumerate() {
        match &t.tok {
            PTok::LBrace => depth += 1,
            PTok::RBrace => depth -= 1,
            // A declaration ANYWHERE at top level makes this the app's own code. Checked at depth 0 only,
            // so a `function` used as a closure inside a body cannot make a script look like a library.
            PTok::Ident(k) if depth == 0 => match k.as_str() {
                "class" | "interface" | "trait" | "enum" => return Role::Code,
                // `function` at top level is a declaration UNLESS it is an anonymous closure — `function (`
                // — which is what Symfony's `public/index.php` returns.
                "function" if !is_closure_head(&toks, i) => return Role::Code,
                "return" if !returns_factory(&toks, i) => returns_data = true,
                _ => {}
            },
            _ => {}
        }
    }
    if returns_data {
        Role::Config
    } else {
        Role::Bootstrap
    }
}

/// Is the `function` at `i` an anonymous closure rather than a declaration?
///
/// A closure's name position holds `(`; a declaration holds an identifier. The optional `&` between them is
/// the by-reference marker, legal on both (`function &foo()`, `function &() {}`), so it is skipped before the
/// test rather than treated as either answer.
fn is_closure_head(toks: &[crate::lift::lexer::PTokenSpanned], i: usize) -> bool {
    let mut j = i + 1;
    if matches!(toks.get(j).map(|t| &t.tok), Some(PTok::Amp)) {
        j += 1;
    }
    matches!(toks.get(j).map(|t| &t.tok), Some(PTok::LParen))
}

/// Does the `return` at `i` return a FACTORY (a closure or a `new`) rather than data?
///
/// This is the whole difference between Symfony's front controller and a `config/*.php` file, which are
/// otherwise the same shape. `static` may precede a closure (`return static function () {…}`), so it is
/// skipped.
fn returns_factory(toks: &[crate::lift::lexer::PTokenSpanned], i: usize) -> bool {
    let mut j = i + 1;
    if matches!(toks.get(j).map(|t| &t.tok), Some(PTok::Ident(k)) if k == "static") {
        j += 1;
    }
    matches!(toks.get(j).map(|t| &t.tok), Some(PTok::Ident(k)) if k == "function" || k == "fn" || k == "new")
}
