//! Import hygiene for the loader (M-Decomp, Inv 13): the whole-word unused-import lint
//! (`E-UNUSED-IMPORT`) + the per-file `E-DUP-IMPORT`/`E-IMPORT-MAIN` gates, and the Q-A
//! wildcard expansion (`import X.*` → per-member imports, before any backend). Split out of
//! `loader/mod.rs` to keep it under the file-size cap; `DefInfo`/`vis_violation` stay in the
//! parent and are reached via `super`.

use super::*;

/// Q-A — the member names a `import <pkg>.*` binds: native registry leaves for a `Core.*` submodule,
/// else the Pass-1 index entries declared in `pkg` that `vis_violation` would let THIS file import
/// individually (public cross-package; public+internal same-package — the spec's unifying principle,
/// "every member you'd be allowed to import individually"). Sorted + deduped (Inv 10 determinism).
pub(super) fn wildcard_members(
    pkg: &str,
    referrer_file: &Path,
    referrer_pkg: &str,
    prov_fns: &HashMap<(String, String), DefInfo>,
    prov_types: &HashMap<(String, String), DefInfo>,
) -> Vec<String> {
    if pkg == "Core" || pkg.starts_with("Core.") {
        return crate::native::module_members(pkg);
    }
    let mut names: Vec<String> = Vec::new();
    for prov in [prov_fns, prov_types] {
        for ((p, name), info) in prov {
            if p == pkg && vis_violation(info, referrer_file, referrer_pkg).is_none() {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Q-A — expand every wildcard import (`import X.Y.*;`) in each parsed program into per-member
/// `Item::Import`s BEFORE Pass-2 (compile-time sugar, Inv 5, so backends/PHP never see `*`). An
/// EXPLICIT import of a name wins over a wildcard (D2 escape hatch — the wildcard drops that leaf).
/// Diagnostics: `E-WILDCARD-STDLIB-ROOT` (bare `Core.*`), `E-EXCEPT-UNKNOWN` (an `except` name the
/// package lacks), `E-WILDCARD-EMPTY` (binds nothing), `E-IMPORT-AMBIGUOUS` (two wildcards → same leaf).
pub(super) fn expand_wildcard_imports(
    parsed: &mut [(PathBuf, Program)],
    prov_fns: &HashMap<(String, String), DefInfo>,
    prov_types: &HashMap<(String, String), DefInfo>,
) -> Result<(), String> {
    for (file, prog) in parsed.iter_mut() {
        // Names bound by EXPLICIT (non-wildcard) imports — these win over any wildcard (D2).
        let explicit: std::collections::HashSet<String> = prog
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Import {
                    path,
                    alias,
                    wildcard: false,
                    ..
                } => Some(
                    alias
                        .clone()
                        .unwrap_or_else(|| path.last().cloned().unwrap_or_default()),
                ),
                _ => None,
            })
            .collect();
        let referrer_pkg = prog.package.join(".");
        let mut out: Vec<Item> = Vec::with_capacity(prog.items.len());
        // leaf → the wildcard package that bound it, for cross-wildcard ambiguity detection.
        let mut wildcard_bound: HashMap<String, String> = HashMap::new();
        for item in std::mem::take(&mut prog.items) {
            let Item::Import {
                path,
                wildcard: true,
                except,
                span,
                ..
            } = &item
            else {
                out.push(item);
                continue;
            };
            let (path, except, span) = (path.clone(), except.clone(), *span);
            if path.len() == 1 && path[0] == "Core" {
                return Err(format!(
                    "{}: `import Core.*;` is not allowed — it would bind the entire standard library; \
                     import a specific submodule (e.g. `import Core.Http.*;`) or a member \
                     [E-WILDCARD-STDLIB-ROOT]",
                    file.display()
                ));
            }
            let pkg = path.join(".");
            let members = wildcard_members(&pkg, file, &referrer_pkg, prov_fns, prov_types);
            for ex in &except {
                if !members.contains(ex) {
                    return Err(format!(
                        "{}: `import {pkg}.* except {{ … }}` excludes `{ex}`, but `{pkg}` has no such \
                         member [E-EXCEPT-UNKNOWN]",
                        file.display()
                    ));
                }
            }
            let mut bound = 0usize;
            for name in members {
                if except.contains(&name) || explicit.contains(&name) {
                    continue; // excepted, or an explicit import already binds it (D2)
                }
                if let Some(prev) = wildcard_bound.insert(name.clone(), pkg.clone()) {
                    return Err(format!(
                        "{}: `{name}` is brought by both `import {prev}.*` and `import {pkg}.*` — \
                         wildcard imports may not bind the same name; import it explicitly \
                         (`import {pkg}.{name};`) or exclude it (`except {{ {name} }}`) \
                         [E-IMPORT-AMBIGUOUS]",
                        file.display()
                    ));
                }
                let mut mpath = path.clone();
                mpath.push(name);
                out.push(Item::Import {
                    path: mpath,
                    alias: None,
                    wildcard: false,
                    except: Vec::new(),
                    span,
                });
                bound += 1;
            }
            if bound == 0 {
                return Err(format!(
                    "{}: `import {pkg}.*` binds no names — `{pkg}` exports nothing importable here \
                     (or `except`/an explicit import removed them all) [E-WILDCARD-EMPTY]",
                    file.display()
                ));
            }
        }
        prog.items = out;
    }
    Ok(())
}

/// DEC-282 Go-maximal import hygiene — an import whose bound name(s) never appear in the file is
/// dead text and a HARD error. The bound names of `import A.B.C [as D];` are `D` (aliased) or `C`;
/// a whole-module `import Core.X;` additionally binds every injected bare type of that module
/// (`Core.IteratorModule` binds `Iterator`, `Core.Runtime` binds `Entry`, …). "Appears" is a
/// WHOLE-WORD source scan off the import lines themselves — deliberately over-approximate (a
/// mention inside a comment or string counts as a use), so the hard error can under-report but
/// never mis-flag: interpolation holes, attributes, type positions, and qualified calls are all
/// plain source words.
pub(super) fn check_unused_imports(prog: &Program, src: &str, file: &Path) -> Result<(), String> {
    let mut imports: Vec<(&Vec<String>, Vec<String>)> = Vec::new();
    for item in &prog.items {
        // Q-A: wildcard imports (`import X.*;`) bind many names, not one — the hard whole-word
        // unused-scan doesn't apply (a softer W-UNUSED-IMPORT is the wildcard/group story, step 4).
        // Their expanded per-member imports are created AFTER this check, so they never reach here.
        if let Item::Import {
            path,
            alias,
            wildcard: false,
            ..
        } = item
        {
            let names = match alias {
                Some(a) => vec![a.clone()],
                None => {
                    let leaf = vec![path.last().cloned().unwrap_or_default()];
                    if path.first().map(String::as_str) == Some("Core") {
                        crate::cli::preludes::core_module_bound_names(path).unwrap_or(leaf)
                    } else {
                        leaf
                    }
                }
            };
            imports.push((path, names));
        }
    }
    if imports.is_empty() {
        return Ok(());
    }
    // Blank out each `import …;` STATEMENT (not its whole line — one-liner programs put real
    // code after the import) so the scan below never counts an import's own path as a use.
    let mut scan = src.as_bytes().to_vec();
    {
        let bytes = src.as_bytes();
        let mut i = 0;
        while let Some(rel) = src[i..].find("import") {
            let at = i + rel;
            // Statement position only: the previous non-space/tab char must be a line break, a
            // `;`, or the start of file — so the word "import" inside a comment or string (e.g.
            // "unused-import") never triggers a blank-to-semicolon sweep.
            let before_ok = {
                let mut j = at;
                while j > 0 && (bytes[j - 1] == b' ' || bytes[j - 1] == b'\t') {
                    j -= 1;
                }
                j == 0 || bytes[j - 1] == b'\n' || bytes[j - 1] == b';'
            };
            let end_kw = at + "import".len();
            let after_ok =
                end_kw < bytes.len() && (bytes[end_kw] == b' ' || bytes[end_kw] == b'\t');
            if before_ok && after_ok {
                if let Some(semi) = src[end_kw..].find(';') {
                    for b in &mut scan[at..=end_kw + semi] {
                        if *b != b'\n' {
                            *b = b' ';
                        }
                    }
                    i = end_kw + semi + 1;
                    continue;
                }
            }
            i = at + "import".len();
        }
    }
    let scan = String::from_utf8_lossy(&scan).into_owned();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    // Whole-word containment of `name` in `line`.
    let word_in = |line: &str, name: &str| -> bool {
        let bytes = line.as_bytes();
        let mut from = 0;
        while let Some(i) = line[from..].find(name) {
            let at = from + i;
            let before_ok = at == 0 || !is_word(bytes[at - 1]);
            let end = at + name.len();
            let after_ok = end >= bytes.len() || !is_word(bytes[end]);
            if before_ok && after_ok {
                return true;
            }
            from = at + 1;
        }
        false
    };
    let body_lines: Vec<&str> = scan.lines().collect();
    for (path, names) in &imports {
        let mut used = names
            .iter()
            .any(|n| !n.is_empty() && body_lines.iter().any(|l| word_in(l, n)));
        // DEC-326 (UFCS canonical style): a Core module used ONLY through the receiver form
        // (`s.upperCase()`) never mentions its qualifier — count a `.nativeName(` call of any of
        // the module's natives as a use. Textual and deliberately generous: a false positive just
        // silences a hygiene lint; the checker still validates the real resolution.
        if !used && path.first().map(String::as_str) == Some("Core") {
            let module = path.join(".");
            used = crate::native::registry()
                .iter()
                .filter(|n| n.module == module)
                .any(|n| {
                    let needle = format!(".{}(", n.name);
                    body_lines.iter().any(|l| l.contains(&needle))
                });
        }
        if !used {
            return Err(format!(
                "{}: unused import `{}` — nothing in this file references `{}` \
                 (remove the import, or use it) [E-UNUSED-IMPORT]",
                file.display(),
                path.join("."),
                names.join("`/`")
            ));
        }
    }
    Ok(())
}

/// The entry-relevant (non-`Core`) import paths of one file, with the DEC-282 hygiene gates that
/// need no cross-file knowledge: `import Main;` (or any `Main.…`) is never legal — `Main` is the
/// entry's own package (E-IMPORT-MAIN); the same import written twice is dead text (E-DUP-IMPORT).
pub(super) fn user_imports(prog: &Program, file: &Path) -> Result<Vec<Vec<String>>, String> {
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for item in &prog.items {
        if let Item::Import { path, .. } = item {
            let joined = path.join(".");
            if !seen.insert(joined.clone()) {
                return Err(format!(
                    "{}: duplicate import `{}` — remove the repeated line [E-DUP-IMPORT]",
                    file.display(),
                    joined
                ));
            }
            if path.first().map(String::as_str) == Some("Main") {
                return Err(format!(
                    "{}: `Main` is the entry package — it is never importable (every file's own \
                     package is already in scope) [E-IMPORT-MAIN]",
                    file.display()
                ));
            }
            if path.first().map(String::as_str) != Some("Core") {
                out.push(path.clone());
            }
        }
    }
    Ok(out)
}
