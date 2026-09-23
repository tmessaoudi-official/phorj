//! PHP lifter — the IMPORT block of one emitted phorj file. Split out of `mod.rs` for scout row 5i
//! (DEC-526), which made it per-file: one PHP file can now become SEVERAL phorj files (the enum, and
//! a sibling `<Enum>Functions.phg` holding its lowered methods), and `E-UNUSED-IMPORT` is a HARD error
//! — so each file must import exactly what its own items use, never a list shared across them.

use super::*;
use std::collections::BTreeSet;

/// What the lifter's per-lift recorders saw while ONE output file's items were being lifted.
///
/// The recorders are flags, not text scans, and that is load-bearing: a builtin that lifts to a
/// receiver-form call (`xs.map(f)`, row 5f) needs `import Core.List;` while the text never names
/// `List`, so no scan of the printed file could decide it. Draining them at every item boundary is
/// what attributes each import to the file whose items asked for it.
#[derive(Default)]
pub(super) struct Recorded {
    console: bool,
    natives: BTreeSet<&'static str>,
}

impl Recorded {
    /// Move whatever the recorders hold now into `self`, leaving them empty for the next items.
    pub(super) fn take(&mut self) {
        self.console |= super::super::took_console();
        super::super::reset_console();
        self.natives.extend(super::super::drain_native_modules());
    }

    /// Fold another file's recordings in — used when a split is NOT applied and the lowered enum
    /// methods stay in the primary file (`package Main`, where mixing is legal).
    pub(super) fn absorb(&mut self, other: Recorded) {
        self.console |= other.console;
        self.natives.extend(other.natives);
    }
}

/// `items` → a whole phorj file: the synthesized imports, the PHP `use` imports this file's items
/// reference, then the items themselves.
pub(super) fn assemble(
    prog: &php::PhpProgram,
    package: Vec<String>,
    items: Vec<Item>,
    rec: &Recorded,
) -> Result<Program, String> {
    // The probe every "is this import used?" decision below reads — see the comment at its first use.
    let lifted_decls = crate::lift::printer::print_program(&Program {
        package: vec!["Main".into()],
        items: items.clone(),
        span: SP,
    })?;
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
    if rec.console {
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
    // Filtered by THIS file's text since scout row 5i: a `catch` inside a lowered enum method lands in the
    // `<Enum>Functions` companion, and the enum's own file importing its error type would be
    // `E-UNUSED-IMPORT`.
    let used: Vec<String> = super::super::exceptions::mapped_error_types(prog)
        .into_iter()
        .filter(|t| references_ident(&lifted_decls, t))
        .collect();
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
    for module in &rec.natives {
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
        package,
        items: final_items,
        span: SP,
    })
}
