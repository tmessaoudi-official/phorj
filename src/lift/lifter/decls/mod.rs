//! PHP lifter — program assembly + the Lifter walker (declarations/statements in submodules).

use super::*;

mod declarations;
mod statements;

pub fn lift_source(php_src: &str) -> Result<String, String> {
    // DEC-419: lex WITH the PHPDoc side channel and print WITH the recovered docs, so documentation
    // survives PHP → phorj instead of being dropped on the floor.
    let (toks, docs) = crate::lift::lexer::lex_php_with_docs(php_src)?;
    let prog = crate::lift::parser::parse_php_with_docs(toks, docs)?;
    let phorj = lift(&prog)?;
    crate::lift::printer::print_program_with_docs(&phorj, &prog.docs)
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
        package: vec!["Main".into()],
        items: final_items,
        span: SP,
    })
}

pub(in crate::lift::lifter) struct Lifter {
    /// Set when an `echo` is lifted to `Output.print`, so the import is prepended.
    needs_console: bool,
}
