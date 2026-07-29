//! The loader's three disjoint import maps (M-Decomp, Inv 13): module call-qualifiers,
//! cross-package type imports, and cross-package function imports — plus the shared path
//! classifiers and the `E-IMPORT-UNKNOWN` member-existence check. Split out of `loader/mod.rs`
//! to keep it under the file-size cap; the visibility lattice (`DefInfo`/`vis_violation`/
//! `vis_word`) stays in the parent and is reached via `super`.

use super::*;

/// A file's **user** import map: bound qualifier ⇒ target package segments, for non-`Core` imports
/// only. Native (`Core.*`) imports are excluded — their member calls stay native and are resolved by
/// the backends (and the transpiler) as before. An alias (`import a.b as c;`) binds `c`, else the
/// path's last segment.
pub(super) fn user_import_map(
    items: &[Item],
    types: &HashMap<(String, String), String>,
    defined: &HashMap<(String, String), String>,
) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for item in items {
        if let Item::Import { path, alias, .. } = item {
            if path.first().map(String::as_str) == Some("Core") {
                continue;
            }
            // Unified-import classification (2026-07-03 spec + DEC-197): a path whose last segment is a
            // type the package exports is a *type* import (bound bare by `build_type_imports`), and one
            // whose leaf is an exported *function* is a *function* import (bound bare by
            // `build_function_imports`) — neither is a module qualifier, so skip both here to keep the
            // three import maps disjoint.
            if is_type_import_path(path, types) || is_function_import_path(path, defined) {
                continue;
            }
            let qualifier = alias.clone().or_else(|| path.last().cloned());
            if let Some(q) = qualifier {
                map.insert(q, path.clone());
            }
        }
    }
    map
}

/// A `import Pkg.Path.TypeName` path resolves to a known type iff its last segment is a type exported
/// by the package formed from the preceding segments. Such an import binds a bare type name; every
/// other import binds a module call-qualifier. The single classifier shared by both import maps.
pub(super) fn is_type_import_path(
    path: &[String],
    types: &HashMap<(String, String), String>,
) -> bool {
    match path.split_last() {
        Some((leaf, pkg)) if !pkg.is_empty() => types.contains_key(&(pkg.join("."), leaf.clone())),
        _ => false,
    }
}

/// DEC-197: a `import Pkg.Path.fn` path resolves to a known FUNCTION iff its last segment is a
/// function exported by the package formed from the preceding segments. Such an import binds a bare
/// function name (like a member variant/type import binds a bare name); every other import binds a
/// module call-qualifier. Disjoint from [`is_type_import_path`] (a name is a type XOR a function in a
/// package, `E-TYPE-IMPORT-SHADOW`), so the three import maps never overlap.
pub(super) fn is_function_import_path(
    path: &[String],
    defined: &HashMap<(String, String), String>,
) -> bool {
    match path.split_last() {
        Some((leaf, pkg)) if !pkg.is_empty() => {
            defined.contains_key(&(pkg.join("."), leaf.clone()))
        }
        _ => false,
    }
}

/// DEC-197: build a file's **function-import map** — bare name (or `as` alias) ⇒ the mangled FQN of a
/// cross-package FUNCTION, from each `import a.b.fn [as g];` whose leaf is a function package `a.b`
/// exports. The function analog of [`build_type_imports`]: it consults the `defined` function table
/// (not `types`) and `prov_fns` for visibility. A bare imported function call is resolved to this FQN
/// by `resolve_call` AFTER a same-package function of the same name — the `local > user fn > imported`
/// order means a same-name same-package definition deterministically wins, so it is NOT a conflict
/// here. Errors:
/// - a visibility violation — a cross-package import may only reach a `public`/`internal`-visible fn;
/// - `E-IMPORT-SHADOW` — the bound name collides with an imported module qualifier (the import kinds
///   stay disjoint; function imports are already excluded from `user_import_map`, so this only fires on
///   a genuine module-qualifier clash);
/// - `E-IMPORT-CONFLICT` — two function imports bind the same bare name (alias one with `as`).
pub(super) fn build_function_imports(
    prog: &Program,
    defined: &HashMap<(String, String), String>,
    prov_fns: &HashMap<(String, String), DefInfo>,
    user_imports: &HashMap<String, Vec<String>>,
    file: &Path,
) -> Result<HashMap<String, String>, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for item in &prog.items {
        let Item::Import { path, alias, .. } = item else {
            continue;
        };
        // Core natives are member-imported at the checker layer (`fn_imports`), not here.
        if path.first().map(String::as_str) == Some("Core") {
            continue;
        }
        let (leaf, pkg_segs) = match path.split_last() {
            Some((leaf, pkg)) if !pkg.is_empty() => (leaf, pkg),
            _ => continue, // single-segment ⇒ module import
        };
        let pkg = pkg_segs.join(".");
        let Some(mangled) = defined.get(&(pkg.clone(), leaf.clone())) else {
            // Leaf isn't a function this package exports — a type import (handled by
            // `build_type_imports`) or a module import (handled by `user_import_map`). Skip.
            continue;
        };
        // Visibility: a cross-package function import may only reach a visible function.
        if let Some(info) = prov_fns.get(&(pkg.clone(), leaf.clone())) {
            if let Some(code) = vis_violation(info, file, &prog.package.join(".")) {
                return Err(format!(
                    "{}: function `{leaf}` is not visible from package `{}` — it is `{}` in package \
                     `{pkg}`; mark it `public` to export it [{code}]",
                    file.display(),
                    prog.package.join("."),
                    vis_word(info.vis),
                ));
            }
        }
        let bound = alias.clone().unwrap_or_else(|| leaf.clone());
        if user_imports.contains_key(&bound) {
            return Err(format!(
                "{}: imported function `{bound}` shadows an imported module qualifier — alias it \
                 with `as` [E-IMPORT-SHADOW]",
                file.display()
            ));
        }
        if map.insert(bound.clone(), mangled.clone()).is_some() {
            return Err(format!(
                "{}: two imports bind the function name `{bound}` — alias one with `as` \
                 [E-IMPORT-CONFLICT]",
                file.display()
            ));
        }
    }
    Ok(map)
}

/// Q-A step 3 (G6) — `E-IMPORT-UNKNOWN` for a member import naming nothing real. A member import
/// `import a.b.C;` is valid iff `a.b.C` is itself a (sub-)package/module (in `pkgset`), OR `C` is a
/// function (`defined`) or type (`types`) that package `a.b` exports. When `a.b` IS loaded but exports
/// no such member, it's an error AT THE IMPORT LINE (used or not) — closing the silent-accept gap the
/// two `build_*_imports` builders leave for lowercase / function-only-package members. A non-existent
/// PACKAGE is already `E-MODULE-NOT-FOUND` (earlier); `Core.*` is native (skipped). `build_type_imports`
/// still catches the uppercase-type-lookalike case with the same code.
pub(super) fn validate_member_imports(
    prog: &Program,
    defined: &HashMap<(String, String), String>,
    types: &HashMap<(String, String), String>,
    pkgset: &std::collections::HashSet<String>,
    file: &Path,
) -> Result<(), String> {
    for item in &prog.items {
        let Item::Import { path, .. } = item else {
            continue;
        };
        if path.first().map(String::as_str) == Some("Core") {
            continue; // native — resolved at the checker layer
        }
        let Some((leaf, pkg_segs)) = path.split_last().filter(|(_, p)| !p.is_empty()) else {
            continue; // single-segment ⇒ a module import (E-MODULE-NOT-FOUND covers non-existence)
        };
        if is_builtin_type_leaf(leaf) {
            continue; // a built-in type leaf — `build_type_imports` owns it (E-IMPORT-BUILTIN)
        }
        if pkgset.contains(&path.join(".")) {
            continue; // the whole path is a (sub-)package — a module import
        }
        let pkg = pkg_segs.join(".");
        if defined.contains_key(&(pkg.clone(), leaf.clone()))
            || types.contains_key(&(pkg.clone(), leaf.clone()))
        {
            continue; // a real function or type member
        }
        if pkgset.contains(&pkg) {
            return Err(format!(
                "{}: package `{pkg}` exports no member `{leaf}` — no such function, type, or \
                 sub-module (check the spelling, or that it is `public`) [E-IMPORT-UNKNOWN]",
                file.display()
            ));
        }
        // else: `pkg` itself never loaded — E-MODULE-NOT-FOUND already handled that; stay silent.
    }
    Ok(())
}

/// Build a file's **type-import map**: bare name (or `as` alias) ⇒ the mangled FQN of a cross-package
/// type, from each type-classified unified `import a.b.C [as D];`. Validates against the global `types` table and the
/// file's own definitions / module imports (cross-package types, M-RT generics-all):
/// - `E-IMPORT-BUILTIN` — the leaf is a built-in type (`List`/`Map`/`Set`/scalars); built-ins
///   are import-free, like `int`.
/// - `E-IMPORT-UNKNOWN` — a known type-bearing package exports no such type (a mistyped type import).
/// - `E-IMPORT-CONFLICT` — two terminal imports bind the same bare name (alias one with `as`).
/// - `E-IMPORT-SHADOW` — the bound name collides with a local type in this file or a module-import
///   qualifier (the two import kinds stay disjoint, the `E-SHADOW-IMPORT` discipline).
pub(super) fn build_type_imports(
    prog: &Program,
    types: &HashMap<(String, String), String>,
    prov_types: &HashMap<(String, String), DefInfo>,
    user_imports: &HashMap<String, Vec<String>>,
    file: &Path,
) -> Result<HashMap<String, String>, String> {
    // The file's own type names (collide → SHADOW). A `package Main` file's types are its locals.
    let local_types: std::collections::HashSet<&str> = prog
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Class(c) => Some(c.name.as_str()),
            Item::Enum(e) => Some(e.name.as_str()),
            Item::Interface(i) => Some(i.name.as_str()),
            Item::Trait(t) => Some(t.name.as_str()),
            _ => None,
        })
        .collect();
    let mut map: HashMap<String, String> = HashMap::new();
    for item in &prog.items {
        if let Item::Import { path, alias, .. } = item {
            // Unified-import classification (2026-07-03 spec): a type-import is a multi-segment path
            // whose last segment is a type the package exports. Everything else — single-segment
            // paths and paths whose leaf is not a known type — is a module import (handled by
            // `user_import_map`); skip it here so the two maps stay disjoint.
            // `Core.*` imports are module/native imports (their injected types get discipline in a
            // later slice); never classified as user type-imports — skip, like `user_import_map`.
            if path.first().map(String::as_str) == Some("Core") {
                continue;
            }
            let (leaf, pkg_segs) = match path.split_last() {
                Some((leaf, pkg)) if !pkg.is_empty() => (leaf, pkg),
                _ => continue, // single-segment ⇒ module import
            };
            if is_builtin_type_leaf(leaf) {
                return Err(format!(
                    "{}: `{leaf}` is a built-in type and needs no import (built-ins are \
                     import-free, like `int`) [E-IMPORT-BUILTIN]",
                    file.display()
                ));
            }
            let pkg = pkg_segs.join(".");
            let Some(mangled) = types.get(&(pkg.clone(), leaf.clone())) else {
                // Leaf isn't a type this package exports. If `pkg` is a known (type-bearing) package
                // and the leaf looks like a type name, the user meant a type import that does not
                // exist → diagnose (preserves the old `import type` UNKNOWN check under the unified
                // surface). Otherwise this is a module import (handled by `user_import_map`) — skip.
                // (S0 limitation: a 3-level *module* import under a type-bearing package would
                // false-positive here; refined when module existence is modelled in S2.)
                let pkg_is_known = types.keys().any(|(p, _)| p == &pkg);
                let looks_like_type = leaf.chars().next().is_some_and(char::is_uppercase);
                if pkg_is_known && looks_like_type {
                    return Err(format!(
                        "{}: package `{pkg}` exports no type `{leaf}` [E-IMPORT-UNKNOWN]",
                        file.display()
                    ));
                }
                continue;
            };
            // Visibility: a cross-package type import may only reach a `public` type.
            if let Some(info) = prov_types.get(&(pkg.clone(), leaf.clone())) {
                if let Some(code) = vis_violation(info, file, &prog.package.join(".")) {
                    return Err(format!(
                        "{}: type `{leaf}` is not visible from package `{}` — it is `{}` in package \
                         `{pkg}`; mark it `public` to export it [{code}]",
                        file.display(),
                        prog.package.join("."),
                        vis_word(info.vis),
                    ));
                }
            }
            let bound = alias.clone().unwrap_or_else(|| leaf.clone());
            if local_types.contains(bound.as_str()) || user_imports.contains_key(&bound) {
                return Err(format!(
                    "{}: imported type `{bound}` shadows a local type or an imported module \
                     qualifier — alias it with `as` [E-IMPORT-SHADOW]",
                    file.display()
                ));
            }
            if map.insert(bound.clone(), mangled.clone()).is_some() {
                return Err(format!(
                    "{}: two imports bind the type name `{bound}` — alias one with `as` \
                     [E-IMPORT-CONFLICT]",
                    file.display()
                ));
            }
        }
    }
    Ok(map)
}

/// Built-in type names that are import-free (resolved by the checker/compiler, not a package member).
/// A type import naming one of these is `E-TYPE-IMPORT-BUILTIN`.
pub(super) fn is_builtin_type_leaf(name: &str) -> bool {
    matches!(
        name,
        "int" | "float" | "bool" | "string" | "bytes" | "void" | "empty" | "List" | "Map" | "Set"
    )
}
