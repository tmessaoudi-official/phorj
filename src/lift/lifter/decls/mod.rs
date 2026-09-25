//! PHP lifter — program assembly + the Lifter walker (declarations/statements in submodules).

use super::*;

mod declarations;
pub(in crate::lift) mod hoist;
mod imports;
mod interfaces;
mod seed;
pub(in crate::lift) mod statements;

pub fn lift_source(php_src: &str) -> Result<String, String> {
    Ok(lift_source_split(php_src, false)?.0)
}

/// [`lift_source`] for the directory lift: the primary draft, plus one `(file stem, draft)` per
/// `<Enum>Functions` companion DEC-526 split out (scout row 5i). The `// CANNOT LIFT:` notes stay on
/// the primary — they describe the PHP file, which is the primary's source.
pub fn lift_source_files(php_src: &str) -> Result<(String, Vec<(String, String)>), String> {
    lift_source_split(php_src, true)
}

fn lift_source_split(
    php_src: &str,
    split: bool,
) -> Result<(String, Vec<(String, String)>), String> {
    // DEC-419: lex WITH the PHPDoc side channel and print WITH the recovered docs, so documentation
    // survives PHP → phorj instead of being dropped on the floor.
    let (toks, docs) = crate::lift::lexer::lex_php_with_docs(php_src)?;
    let prog = crate::lift::parser::parse_php_with_docs(toks, docs)?;
    let files = lift_files(&prog, split)?;
    let out = crate::lift::printer::print_program_with_docs(&files.primary, &prog.docs)?;
    let mut companions = Vec::new();
    for (stem, p) in &files.companions {
        companions.push((
            stem.clone(),
            crate::lift::printer::print_program_with_docs(p, &prog.docs)?,
        ));
    }
    // DEC-421: an exception class with no mapping into phorj's standard taxonomy keeps its own name,
    // which will NOT type-check. Saying so beats leaving the reader to discover it from `phg check`:
    // the lifter's contract is that anything it cannot do is refused LOUDLY, never guessed.
    let unmapped = super::exceptions::unmapped_exception_classes(&prog);
    let mut notes: String = unmapped
        .iter()
        .map(|c| {
            format!(
                "// CANNOT LIFT: `{c}` has no phorj counterpart — declare it, or catch one of \
                 `Core.ErrorModule`'s types instead.\n"
            )
        })
        .collect();
    // DEC-397: a variable whose first assignment sits in a CONDITIONAL block, and which is read outside
    // it, cannot be hoisted soundly — PHP reads an unassigned variable as null (plus a warning) and
    // phorj has no way to express that without a type the lifter cannot infer. Hoisting a literal anyway
    // would make the draft COMPILE and be WRONG, so the name is reported instead. The draft still fails
    // `phg check`, which is in-contract for a `// lifted (verify)` draft — what is not acceptable is
    // failing it silently.
    notes.push_str(&hoist_notes(&prog));
    // LIFT-ATTR: an attribute whose class is not in this file (every framework attribute) is emitted with
    // its identity intact and named here, so the draft says why `phg check` will flag it.
    notes.push_str(&super::attrs::unresolved_attribute_notes(&prog));
    Ok((format!("{notes}{out}"), companions))
}

/// The `// CANNOT LIFT:` notes for every function-scoped variable the DEC-397 hoist had to refuse,
/// deduped and in first-seen order so the header is deterministic (Invariant 10).
fn hoist_notes(prog: &php::PhpProgram) -> String {
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut push = |label: &str, params: &[php::PhpParam], body: &[php::PhpStmt]| {
        let names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        for name in hoist::plan(body, &names).blocked {
            if !seen.iter().any(|(f, n)| f == label && n == &name) {
                seen.push((label.to_string(), name));
            }
        }
    };
    for item in &prog.items {
        match item {
            php::PhpItem::Function(f) => push(&f.name, &f.params, &f.body),
            php::PhpItem::Class(c) => {
                for m in &c.members {
                    if let php::PhpMember::Method(me) = m {
                        if let Some(body) = &me.body {
                            push(&format!("{}.{}", c.name, me.name), &me.params, body);
                        }
                    }
                }
            }
            // A lowered enum method (DEC-509) is a FREE function in the draft, so it is named bare —
            // the name the reader will find. Skipped until scout row 5i: its refused hoists went
            // unreported, leaving an `E-UNKNOWN-IDENT` with no note saying why.
            php::PhpItem::Enum(e) => {
                for m in &e.methods {
                    if let Some(body) = &m.body {
                        push(&m.name, &m.params, body);
                    }
                }
            }
            php::PhpItem::Interface(_) | php::PhpItem::Stmt(_) => {}
        }
    }
    seen.iter()
        .map(|(func, name)| {
            format!(
                "// CANNOT LIFT: `${name}` in `{func}()` is first assigned inside a CONDITIONAL block \
                 and read outside it. PHP would read it as null there (function scope); phorj has \
                 block scope and no inferable nullable to stand in, so declare `{name}` before the \
                 block by hand.\n"
            )
        })
        .collect()
}

/// DEC-331 D1: the lifted entry is always a CLI script (PHP has no entry-role concept), so it
/// carries `#[Entry(kind: EntryKind.Cli)]` — the role is declared, never inferred.
fn entry_cli_attr() -> crate::ast::Attribute {
    crate::ast::entry_attr("Cli", SP)
}

/// One PHP file lifted to phorj FILES: the primary program, plus one `<Enum>Functions` companion per
/// enum whose methods DEC-509 lowered, when DEC-526 splits them out (scout row 5i).
pub struct LiftedFiles {
    pub primary: Program,
    /// `(file stem, program)`, in declaration order.
    pub companions: Vec<(String, Program)>,
}

/// Lift a parsed PHP program into ONE Phorj program (`package Main; import Core.Runtime.Entry;`) —
/// lowered enum methods stay beside their enum. The single-file surface (`phg lift foo.php`).
pub fn lift(prog: &php::PhpProgram) -> Result<Program, String> {
    Ok(lift_files(prog, false)?.primary)
}

/// Lift a parsed PHP program; with `split`, lowered enum methods outside `package Main` go to
/// companion files (DEC-526) — the directory lift's surface, since only it writes more than one file.
pub fn lift_files(prog: &php::PhpProgram, split: bool) -> Result<LiftedFiles, String> {
    // DEC-531: rename reserved-word locals and parameters first, so no stage below sees one.
    let renamed = super::keyword_locals::rename(prog)?;
    let prog = &renamed;
    // DEC-312: reset the per-lift native-module recorder (never leak across runs on this thread).
    let _ = super::drain_native_modules();
    super::reset_console();
    let mut l = Lifter {};
    let mut items: Vec<Item> = Vec::new();
    let mut top_stmts: Vec<Stmt> = Vec::new();
    // Every top-level statement lands in ONE synthesized `main` body, so they share one
    // already-declared set. A fresh set per statement (the shape until 2026-09-05) made the second
    // assignment to a variable re-DECLARE it — `$x = 1; $x = $x + 2;` lifted to two `mutable var x`
    // and the draft failed to check with `E-SHADOW-LOCAL`. A function body always threaded one set,
    // which is why the bug was invisible anywhere but at file scope — where PHP scripts do most of
    // their assigning.
    let mut top_declared = HashSet::new();
    let mut has_main = false;
    // LIFT-ATTR: attribute names resolve against the file's `namespace` + `use` map, so the context is
    // built once here rather than threaded through the Lifter walker — attribute lifting needs no
    // per-declaration state.
    let actx = AttrCtx::new(prog);
    // Lane L1: which names are enums decides whether `Foo::BAR` is a case or a class constant. A
    // single-file lift knows only this file's enums; `lift_directory` has already seeded every enum
    // in the tree, so the union is what a cross-file `Tenure::PLAI` needs.
    super::enums::begin_file(super::enums::enum_names_of(prog));
    // DEC-534: this file's Map-typed class constants, for PHP `+` as a map union.
    super::map_consts::begin_file(prog);
    // DEC-509: free functions lowered from enum methods, and the names already taken by them. The
    // name set spans the whole PHP file even when the functions are split across companions: the
    // companions share ONE package, where two functions of one name still collide.
    let mut lowered: Vec<(String, Vec<FunctionDecl>, imports::Recorded)> = Vec::new();
    let mut lowered_names: HashSet<String> = HashSet::new();
    // Scout row 5i: what the recorders saw for the PRIMARY file's items. Drained at every item
    // boundary around a lowering, so an `echo` only inside an enum method imports `Core.Output` into
    // the companion and not here.
    let mut recorded = imports::Recorded::default();

    for item in &prog.items {
        match item {
            php::PhpItem::Function(f) => {
                let mut lifted = l.lift_function(f)?;
                lifted.attrs = actx.lift_attributes(&f.attrs)?;
                if f.name == "main" {
                    has_main = true;
                    // DEC-191: a PHP `main` is the entry INTENT — the lifted draft attributes it
                    // so it actually runs (entries are attribute-declared, never name-magic).
                    lifted.attrs.push(entry_cli_attr());
                }
                items.push(Item::Function(lifted));
            }
            php::PhpItem::Class(c) => {
                let mut lifted = l.lift_class(c)?;
                lifted.attrs = actx.lift_attributes(&c.attrs)?;
                items.push(Item::Class(lifted));
            }
            php::PhpItem::Enum(e) => {
                items.push(Item::Enum(lift_enum(e)?));
                recorded.take();
                let fns = super::enums::lower_methods(&mut l, e, &mut lowered_names)?;
                let mut rec = imports::Recorded::default();
                rec.take();
                if !fns.is_empty() {
                    lowered.push((e.name.clone(), fns, rec));
                }
            }
            php::PhpItem::Interface(i) => {
                items.push(Item::Interface(interfaces::lift_interface(i)?));
            }
            php::PhpItem::Stmt(s) => {
                top_stmts.extend(l.lift_stmt(s, &mut top_declared)?);
            }
        }
    }

    recorded.take();

    // DEC-526 (scout row 5i): outside `package Main` a public enum and public functions cannot share
    // a file (`E-FILE-MIXED-PUBLIC`), so each enum's lowered methods become a sibling
    // `<Enum>Functions` file in the same package. `Main` may mix, and so keeps the single-file shape.
    // An ENTRY file keeps it too: the directory lift re-packages the entry as `Main` (a dotted
    // package would have to sit in a matching directory), which would leave a companion stranded in
    // a package its enum no longer belongs to.
    let is_entry = has_main || !top_stmts.is_empty();
    let split = split && !prog.namespace.is_empty() && !is_entry;
    let mut companions = Vec::new();
    for (enum_name, fns, rec) in lowered {
        if split {
            let fns = fns.into_iter().map(Item::Function).collect();
            let package = lift_package(&prog.namespace)?;
            companions.push((
                format!("{enum_name}Functions"),
                imports::assemble(prog, package, fns, &rec)?,
            ));
        } else {
            // The lowered enum methods land AFTER every enum declaration, so a reader sees the type
            // before the functions over it, and `main` (synthesized below) stays last.
            items.extend(fns.into_iter().map(Item::Function));
            recorded.absorb(rec);
        }
    }

    // Top-level PHP code becomes the runnable entry `function main()` (M5 model).
    if !top_stmts.is_empty() {
        if has_main {
            return Err(
                "lift: file has both a main() function and top-level code (ambiguous entry)".into(),
            );
        }
        items.push(Item::Function(FunctionDecl {
            modifiers: Vec::new(),
            // DEC-191: the synthesized entry carries #[Entry(kind: EntryKind.Cli)] (attribute-declared).
            attrs: vec![entry_cli_attr()],
            vis: crate::ast::Visibility::Public,
            name: "main".into(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            params: Vec::new(),
            ret: Some(named("void")),
            throws: Vec::new(),
            body: top_stmts,
            foreign: false,
            generic_ret_from_param: None,
            private_window: None,
            span: SP,
        }));
    }

    let primary = imports::assemble(prog, lift_package(&prog.namespace)?, items, &recorded)?;
    Ok(LiftedFiles {
        primary,
        companions,
    })
}

/// A PHP `namespace A\B` → the phorj `package` segments, or `["Main"]` when the file declares none
/// (the historical default, so an un-namespaced file lifts exactly as before).
///
/// Each segment is PascalCase-ized because `E-PKG-CASE` is ENFORCED — `package app.entity;` is rejected
/// with *"package segment `app` must be PascalCase"* [Verified] — and PHP does not guarantee PascalCase
/// namespaces. `snake_case` and `kebab` separators become word boundaries (`my_pkg` → `MyPkg`), so the
/// result is a legal phorj package for every input that PHP itself accepts as a namespace.
fn lift_package(namespace: &[String]) -> Result<Vec<String>, String> {
    if namespace.is_empty() {
        return Ok(vec!["Main".into()]);
    }
    // `Core.` is phorj's RESERVED package root (Invariant 12 — the standard library). A PHP project with
    // a `Core\` namespace is entirely ordinary, and passing it through emits a draft that dies on
    // `E-RESERVED-PACKAGE`, so it is refused here with the reason instead.
    if namespace.first().map(String::as_str) == Some("Core") {
        return Err(
            "lift: `namespace Core\\…` maps onto phorj's reserved `Core.` package root (the standard \
             library). Rename the namespace, or lift into a different package root."
                .into(),
        );
    }
    namespace.iter().map(|s| package_segment(s)).collect()
}

/// A PHP namespace segment → a legal phorj package segment, or a loud refusal.
///
/// `pascalize` alone is not enough, and the two cases below are why this returns a `Result` — both are
/// legal PHP that produced a draft the toolchain then rejected, which is the exact failure mode DEC-166
/// exists to prevent (refuse loudly; never emit a guess):
///   * a segment made only of separators (`___`) pascalizes to `""` → `package ;`, a parse error;
///   * a non-ASCII segment (`café`) is a legal PHP namespace but phorj's own lexer rejects `é`, so the
///     draft does not even LEX — and a lex error suppresses every other diagnostic in the file.
pub(in crate::lift::lifter) fn package_segment(seg: &str) -> Result<String, String> {
    let out = pascalize(seg);
    if out.is_empty() {
        return Err(format!(
            "lift: namespace segment `{seg}` has no letters or digits, so it cannot become a phorj \
             package segment. Rename it."
        ));
    }
    if !out.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "lift: namespace segment `{seg}` is not ASCII. PHP allows it, but a phorj identifier must \
             be ASCII, so the lifted draft would not lex. Rename it."
        ));
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(format!(
            "lift: namespace segment `{seg}` starts with a digit, which cannot begin a phorj \
             identifier. Rename it."
        ));
    }
    Ok(out)
}

/// A PHP class name → the same name, checked for the two ways a legal PHP class name is NOT a legal
/// phorj identifier. Unlike [`package_segment`] this does NOT pascalize: the segment is a TYPE name,
/// already the class's own name, and re-casing it would stop it matching the class the lift emits.
///
/// The check earns its keep on the non-ASCII case: `class Café` is legal PHP, phorj's lexer rejects
/// `é`, and a LEX error suppresses every other diagnostic in the file — so emitting it hides the
/// draft's real problems behind one unrelated failure.
pub(in crate::lift::lifter) fn type_segment(seg: &str) -> Result<String, String> {
    if seg.is_empty() {
        return Err("lift: an empty class-name segment".to_string());
    }
    if !seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "lift: the class name `{seg}` is not ASCII. PHP allows it, but a phorj identifier must be \
             ASCII, so the lifted draft would not lex. Rename it."
        ));
    }
    if seg.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(format!(
            "lift: the class name `{seg}` starts with a digit, which cannot begin a phorj identifier. \
             Rename it."
        ));
    }
    Ok(seg.to_string())
}

/// Does `text` reference the identifier `name` on a word boundary?
///
/// A plain substring test would keep an import for `Money` because the output mentions `MoneyBag`, and
/// re-parsing the printed text just to answer this would be circular (the whole point is that the draft
/// may not yet check). Word-boundary matching on the printed declarations is the honest middle: it can
/// still be fooled by the name appearing inside a STRING literal or comment, which errs toward keeping
/// an import — the safe direction, since a spurious `E-UNUSED-IMPORT` is visible and trivially fixed
/// whereas a wrongly-dropped import would be a silent loss.
///
/// An occurrence preceded by `.` does NOT count: in phorj an imported name is referenced at the HEAD of
/// a dotted chain (`Router.new()`, `Money m`) and never after a dot, where it is a member or an interior
/// package segment. LIFT-ATTR made this load-bearing — `use Attribute;` maps onto the canonical
/// `Core.Runtime.Attribute`, and the plain word-boundary test saw the `Attribute` inside that path and
/// kept an `import Attribute;` for a name the output no longer references.
fn references_ident(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    text.match_indices(name).any(|(i, _)| {
        let before_ok = i == 0 || (!is_word(bytes[i - 1]) && bytes[i - 1] != b'.');
        let j = i + name.len();
        let after_ok = j >= bytes.len() || !is_word(bytes[j]);
        before_ok && after_ok
    })
}

/// Upper-camel a single identifier: split on `_`/`-`, capitalize each part, join. A part already
/// PascalCase is preserved rather than lower-cased (`ORM` stays `ORM`, not `Orm`) — the segment is a
/// name the developer chose, and only its FIRST character is what `E-PKG-CASE` constrains.
///
/// Callers must go through [`package_segment`], which rejects the results that are not legal phorj
/// identifiers (empty, non-ASCII, digit-leading) rather than emitting them.
fn pascalize(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    for part in seg.split(['_', '-']).filter(|p| !p.is_empty()) {
        let mut cs = part.chars();
        if let Some(c0) = cs.next() {
            out.extend(c0.to_uppercase());
            out.push_str(cs.as_str());
        }
    }
    out
}

/// The lifter's walker. Stateless since 2026-09-07 — the one flag it carried (`echo` was lifted, so
/// prepend `import Core.Output`) moved to a thread-local, because a block-closure body is lifted
/// through the FREE `lift_expr`, which has no `Lifter` to set.
pub(in crate::lift::lifter) struct Lifter {}
