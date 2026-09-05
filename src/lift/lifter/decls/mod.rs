//! PHP lifter — program assembly + the Lifter walker (declarations/statements in submodules).

use super::*;

mod declarations;
pub(in crate::lift) mod hoist;
mod interfaces;
mod seed;
pub(in crate::lift) mod statements;

pub fn lift_source(php_src: &str) -> Result<String, String> {
    // DEC-419: lex WITH the PHPDoc side channel and print WITH the recovered docs, so documentation
    // survives PHP → phorj instead of being dropped on the floor.
    let (toks, docs) = crate::lift::lexer::lex_php_with_docs(php_src)?;
    let prog = crate::lift::parser::parse_php_with_docs(toks, docs)?;
    let phorj = lift(&prog)?;
    let out = crate::lift::printer::print_program_with_docs(&phorj, &prog.docs)?;
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
    if notes.is_empty() {
        return Ok(out);
    }
    Ok(format!("{notes}{out}"))
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
            _ => {}
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

/// Lift a parsed PHP program into a Phorj program (`package Main; import Core.Runtime.Entry;`).
pub fn lift(prog: &php::PhpProgram) -> Result<Program, String> {
    // DEC-312: reset the per-lift native-module recorder (never leak across runs on this thread).
    let _ = super::drain_native_modules();
    let mut l = Lifter {
        needs_console: false,
    };
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
            php::PhpItem::Enum(e) => items.push(Item::Enum(lift_enum(e)?)),
            php::PhpItem::Interface(i) => {
                items.push(Item::Interface(interfaces::lift_interface(i)?));
            }
            php::PhpItem::Stmt(s) => {
                top_stmts.extend(l.lift_stmt(s, &mut top_declared)?);
            }
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
            span: SP,
        }));
    }

    // Prepend `import Core.Output;` if any `echo` was lifted.
    let mut final_items = Vec::new();
    // DEC-191 addendum: the lifted draft's #[Entry] needs its import (wind rule). DEC-337: the
    // `kind: EntryKind.Cli` variant is import-gated too — emit its import alongside `Entry`.
    let emitted_entry = items
        .iter()
        .any(|i| matches!(i, Item::Function(f) if f.attrs.iter().any(crate::ast::is_entry_attr)));
    if emitted_entry {
        final_items.push(Item::Import {
            path: vec!["Core".into(), "Runtime".into(), "Entry".into()],
            alias: None,
            wildcard: false,
            except: Vec::new(),
            span: SP,
        });
        final_items.push(Item::Import {
            path: vec![
                "Core".into(),
                "Runtime".into(),
                crate::ast::ENTRY_KIND_ENUM.into(),
            ],
            alias: None,
            wildcard: false,
            except: Vec::new(),
            span: SP,
        });
    }
    if l.needs_console {
        final_items.push(Item::Import {
            path: vec!["Core".into(), "Output".into()],
            alias: None,
            wildcard: false,
            except: Vec::new(),
            span: SP,
        });
    }
    // DEC-421: a catch clause that mapped onto phorj's standard taxonomy needs `Core.ErrorModule`
    // imported, plus a MEMBER import per type used — an injected type referenced bare without its
    // import is `E-INJECTED-TYPE-BARE`, so emitting the mapping without these would produce a draft
    // that still does not check, defeating the point of mapping at all.
    let used = super::exceptions::mapped_error_types(prog);
    if !used.is_empty() {
        final_items.push(Item::Import {
            path: vec!["Core".into(), "ErrorModule".into()],
            alias: None,
            wildcard: false,
            except: Vec::new(),
            span: SP,
        });
        for t in used {
            final_items.push(Item::Import {
                path: vec!["Core".into(), "ErrorModule".into(), t],
                alias: None,
                wildcard: false,
                except: Vec::new(),
                span: SP,
            });
        }
    }
    // LIFT-NS: one phorj `import` per PHP `use`, in source order. These land AFTER the imports the lifter
    // synthesized above (`Core.Runtime.Entry`, `Core.Output`, the `Core.ErrorModule` members), which is
    // simply the order they are pushed in — nothing depends on it and no test pins it. Phorj supports import aliases natively, so `use X as Y;`
    // lifts to `import X as Y;` rather than being expanded away — which keeps the alias the developer
    // wrote instead of inlining a fully-qualified name at every use site.
    //
    // Path segments are PascalCase-ized for the same reason `lift_package` does it: a namespace segment
    // is a package segment on the phorj side. The LAST segment is a TYPE name and is left alone — it is
    // already the class's own name, and PHP class names are PascalCase by universal convention.
    //
    // An import is emitted ONLY when the lifted output actually references its local name.
    // `E-UNUSED-IMPORT` is a HARD error in phorj while an unused `use` is legal and extremely common
    // in PHP, so emitting every `use` verbatim produces "a lift that fails the very check it should
    // pass" — the exact rule `exceptions.rs` already follows for error-type imports. Dropping an
    // unreferenced one is semantically LOSSLESS: a PHP `use` only creates a local alias, so an unused
    // one carries no behaviour to lose.
    //
    // Usage is judged against the LIFTED text, not the PHP source, and LIFT-ATTR is why that matters
    // most: an attribute name is RESOLVED during the lift, so a Doctrine-style `use … as ORM;` whose
    // only referent was `#[ORM\Column]` has no referent left once the attribute is emitted as the
    // expanded `#[Doctrine.ORM.Mapping.Column]`. A PHP-source scan would keep that import; scanning the
    // lifted text drops it, which is what phorj's hard `E-UNUSED-IMPORT` requires.
    // The probe MUST propagate a printer error rather than defaulting to `""`: an empty probe makes
    // `references_ident` false for every name, silently dropping EVERY import. Swallowing it with
    // `unwrap_or_default()` was a bandaid with no evidenced failure mode (CLAUDE.md's anti-bandaid gate
    // rates that P0), and the two prints are not interchangeable — this one passes `items` with a
    // placeholder package and no docs, the final one passes `final_items` with the real package.
    let lifted_decls = crate::lift::printer::print_program(&Program {
        package: vec!["Main".into()],
        items: items.clone(),
        span: SP,
    })?;
    // Local names already bound by the imports the LIFTER synthesized above (`Core.Output`,
    // `Core.Runtime.Entry`, `Core.Runtime.EntryKind`, the `Core.ErrorModule` members …). A phorj import
    // binds its LAST segment, so a PHP `use App\Output;` would bind `Output` a second time and shadow
    // `Core.Output` — and the lifter's own `Output.print(…)` call would then resolve to the user's
    // class. That produced SILENT WRONG OUTPUT: `phg check` clean, all three legs agreeing with each
    // other and disagreeing with the PHP the file was lifted from, so the differential harness could
    // never see it. Refused loudly instead (DEC-166) — and PHP itself errors on the same shape
    // (`use A\Helper; use B\Helper;` → "Cannot use B\Helper as Helper"), so refusing is also faithful.
    let mut bound: Vec<String> = final_items
        .iter()
        .filter_map(|i| match i {
            Item::Import { path, alias, .. } => {
                Some(alias.clone().or_else(|| path.last().cloned())?)
            }
            _ => None,
        })
        .collect();
    for u in &prog.uses {
        let Some(local) = u.alias.clone().or_else(|| u.path.last().cloned()) else {
            continue;
        };
        if bound.contains(&local) {
            return Err(format!(
                "lift: `use {}` binds the name `{local}`, which this file already imports — phorj has \
                 no way to express two imports under one name, and silently letting one win would make \
                 the lifted program disagree with the PHP it came from. Alias it (`use … as Other;`) \
                 and re-run.",
                u.path.join("\\")
            ));
        }
        if !references_ident(&lifted_decls, &local) {
            continue;
        }
        let mut path: Vec<String> = Vec::with_capacity(u.path.len());
        for (i, seg) in u.path.iter().enumerate() {
            if i + 1 == u.path.len() {
                // The last segment is the class's own NAME — never re-cased (that would stop it
                // matching the class), but still checked for the two ways a legal PHP class name is not
                // a legal phorj identifier. `use App\Café;` used to emit an import the draft could not
                // even LEX, and a lex error suppresses every other diagnostic in the file.
                path.push(type_segment(seg)?);
            } else {
                path.push(package_segment(seg)?);
            }
        }
        bound.push(local);
        final_items.push(Item::Import {
            path,
            alias: u.alias.clone(),
            wildcard: false,
            except: Vec::new(),
            span: SP,
        });
    }
    // DEC-312: one `import <module>;` per Core module a builtin→native resolution referenced.
    for module in super::drain_native_modules() {
        final_items.push(Item::Import {
            path: module.split('.').map(str::to_string).collect(),
            alias: None,
            wildcard: false,
            except: Vec::new(),
            span: SP,
        });
    }
    final_items.extend(items);

    Ok(Program {
        package: lift_package(&prog.namespace)?,
        items: final_items,
        span: SP,
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

pub(in crate::lift::lifter) struct Lifter {
    /// Set when an `echo` is lifted to `Output.print`, so the import is prepended.
    needs_console: bool,
}
