//! PHP lifter — program assembly + the Lifter walker (declarations/statements in submodules).

use super::*;

mod declarations;
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
    if unmapped.is_empty() {
        return Ok(out);
    }
    let notes: String = unmapped
        .iter()
        .map(|c| {
            format!(
                "// CANNOT LIFT: `{c}` has no phorj counterpart — declare it, or catch one of \
                 `Core.ErrorModule`'s types instead.\n"
            )
        })
        .collect();
    Ok(format!("{notes}{out}"))
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
    let mut has_main = false;

    for item in &prog.items {
        match item {
            php::PhpItem::Function(f) => {
                let mut lifted = l.lift_function(f)?;
                if f.name == "main" {
                    has_main = true;
                    // DEC-191: a PHP `main` is the entry INTENT — the lifted draft attributes it
                    // so it actually runs (entries are attribute-declared, never name-magic).
                    lifted.attrs.push(entry_cli_attr());
                }
                items.push(Item::Function(lifted));
            }
            php::PhpItem::Class(c) => items.push(Item::Class(l.lift_class(c)?)),
            php::PhpItem::Enum(e) => items.push(Item::Enum(lift_enum(e)?)),
            php::PhpItem::Stmt(s) => {
                let mut declared = HashSet::new();
                top_stmts.extend(l.lift_stmt(s, &mut declared)?);
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
    // LIFT-NS: one phorj `import` per PHP `use`, in source order, BEFORE the DEC-312 native imports so
    // the file's own dependencies read first. Phorj supports import aliases natively, so `use X as Y;`
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
    // Usage is judged against the LIFTED text, not the PHP source: a Doctrine-style
    // `use … as ORM;` is referenced only from `#[ORM\Column]`, and attributes are not lifted yet
    // (LIFT-ATTR), so a PHP-source scan would keep an import whose only referent was dropped.
    let lifted_decls = crate::lift::printer::print_program(&Program {
        package: vec!["Main".into()],
        items: items.clone(),
        span: SP,
    })
    .unwrap_or_default();
    for u in &prog.uses {
        let local = u
            .alias
            .clone()
            .or_else(|| u.path.last().cloned())
            .unwrap_or_default();
        if !references_ident(&lifted_decls, &local) {
            continue;
        }
        let mut path: Vec<String> = Vec::with_capacity(u.path.len());
        for (i, seg) in u.path.iter().enumerate() {
            if i + 1 == u.path.len() {
                path.push(seg.clone());
            } else {
                path.push(pascalize(seg));
            }
        }
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
        package: lift_package(&prog.namespace),
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
fn lift_package(namespace: &[String]) -> Vec<String> {
    if namespace.is_empty() {
        return vec!["Main".into()];
    }
    namespace.iter().map(|s| pascalize(s)).collect()
}

/// Upper-camel a single identifier: split on `_`/`-`, capitalize each part, join. A part already
/// PascalCase is preserved rather than lower-cased (`ORM` stays `ORM`, not `Orm`) — the segment is a
/// name the developer chose, and only its FIRST character is what `E-PKG-CASE` constrains.
/// Does `text` reference the identifier `name` on a word boundary?
///
/// A plain substring test would keep an import for `Money` because the output mentions `MoneyBag`, and
/// re-parsing the printed text just to answer this would be circular (the whole point is that the draft
/// may not yet check). Word-boundary matching on the printed declarations is the honest middle: it can
/// still be fooled by the name appearing inside a STRING literal or comment, which errs toward keeping
/// an import — the safe direction, since a spurious `E-UNUSED-IMPORT` is visible and trivially fixed
/// whereas a wrongly-dropped import would be a silent loss.
fn references_ident(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    text.match_indices(name).any(|(i, _)| {
        let before_ok = i == 0 || !is_word(bytes[i - 1]);
        let j = i + name.len();
        let after_ok = j >= bytes.len() || !is_word(bytes[j]);
        before_ok && after_ok
    })
}

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
