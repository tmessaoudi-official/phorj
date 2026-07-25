//! Program pass — import-collision / wildcard-import validation.

use super::*;

impl Checker {
    /// Q-A guard: a wildcard import (`import X.*`) is compile-time sugar the LOADER expands into
    /// per-member imports before the checker ever runs (Inv 5). In project mode none survive here. It
    /// survives ONLY in loose mode (`-e` / stdin / dap read raw source and skip loader assembly),
    /// where there is no package graph to expand against — the reserved single `Main` package has
    /// nothing to wildcard-import. Rather than silently ignore it (an unexpanded `*` would bind
    /// nothing), reject it loudly so the loose-mode user learns the feature needs a project.
    pub(in crate::checker) fn check_no_surviving_wildcard_imports(
        &mut self,
        program: &crate::ast::Program,
    ) {
        use crate::ast::Item;
        for item in &program.items {
            if let Item::Import {
                path,
                wildcard: true,
                span,
                ..
            } = item
            {
                let pkg = path.join(".");
                self.err_coded(
                    *span,
                    format!(
                        "wildcard import `import {pkg}.*;` is only available inside a project (a \
                         package graph the loader can expand it against); it cannot be used in \
                         single-file or `-e` mode"
                    ),
                    "E-WILDCARD-NO-PROJECT",
                    Some(format!(
                        "import the members explicitly (e.g. `import {pkg}.Member;`), or run this \
                         inside a project so `{pkg}` resolves"
                    )),
                );
            }
        }
    }

    /// Validate variant imports (Wave B B-2c, DEC-186): `import Core.<Enum>.<Variant> [as A];`. The
    /// pre-check rewrite (`resolve_variant_imports`) has already qualified the *resolvable* ones; here we
    /// report the cases it deliberately left alone so nothing is mis-resolved silently:
    /// - `E-IMPORT-UNKNOWN` — the enum owns no such variant (a mistyped variant import);
    /// - `E-IMPORT-CONFLICT` — the bound name (alias, else the variant leaf) already names a type in this
    ///   file, or two variant imports bind the same name (the rewrite skips both, so bare use would be
    ///   ambiguous / wrongly shadow the local type — reject it, `as`-alias to disambiguate).
    pub(in crate::checker) fn check_variant_import_collisions(
        &mut self,
        program: &crate::ast::Program,
    ) {
        use crate::ast::Item;
        let mut bound_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for item in &program.items {
            let Item::Import {
                path, alias, span, ..
            } = item
            else {
                continue;
            };
            if path.len() != 3 || path[0] != "Core" {
                continue;
            }
            let (enum_name, variant) = (&path[1], &path[2]);
            // Only a Core path whose middle segment is an enum this program declares/injects is a variant
            // import; anything else (`Core.Http.Router`, `Core.Output.printLine`) is a different import kind.
            let Some(info) = self.enums.get(enum_name) else {
                continue;
            };
            if !info.variants.contains_key(variant.as_str()) {
                self.err_coded(
                    *span,
                    format!("`Core.{enum_name}` has no variant `{variant}`"),
                    "E-IMPORT-UNKNOWN",
                    Some(format!(
                        "check the spelling — import a variant `{enum_name}` actually declares"
                    )),
                );
                continue;
            }
            let bound = alias.clone().unwrap_or_else(|| variant.clone());
            if self.classes.contains_key(&bound)
                || self.enums.contains_key(&bound)
                || self.interfaces.contains_key(&bound)
            {
                self.err_coded(
                    *span,
                    format!("imported variant binds `{bound}`, which already names a type in this file"),
                    "E-IMPORT-CONFLICT",
                    Some(format!(
                        "alias the import to a free name — `import Core.{enum_name}.{variant} as My{variant};`"
                    )),
                );
                continue;
            }
            // A bound name that shadows a USER enum's variant would silently hijack that enum's bare
            // construction/pattern (`import Core.Result.Success;` + a local `enum Local { Success(..) }`),
            // producing a baffling type mismatch — reject it. Injected enums are exempt (their variants
            // are exactly what a variant import binds).
            if self
                .enums
                .iter()
                .any(|(_, info)| !info.injected && info.variants.contains_key(&bound))
            {
                self.err_coded(
                    *span,
                    format!(
                        "imported variant binds `{bound}`, which already names a variant of an enum in this file"
                    ),
                    "E-IMPORT-CONFLICT",
                    Some(format!(
                        "alias the import — `import Core.{enum_name}.{variant} as My{variant};`"
                    )),
                );
                continue;
            }
            if !bound_seen.insert(bound.clone()) {
                self.err_coded(
                    *span,
                    format!("`{bound}` is imported more than once"),
                    "E-IMPORT-CONFLICT",
                    Some(
                        "alias one of the imports with `as` so each bound name is unique"
                            .to_string(),
                    ),
                );
            }
        }
    }

    /// DEC-197 collision guard: two member imports binding the same bare function name (`import
    /// Core.List.map;` + another module's `map`) are ambiguous — reject with `E-IMPORT-CONFLICT` and
    /// point at `as`-aliasing (the ruled resolution for collisions). A bare import that shadows a
    /// user function or a local wins deterministically by the resolution order (`local > user fn >
    /// imported native`, enforced in `check_named_call`), so it is NOT a conflict here. Runs alongside
    /// `check_variant_import_collisions`; the underlying binding set is the single-source
    /// [`function_imports::function_import_bindings`], so it never diverges from what `fn_imports` maps.
    pub(in crate::checker) fn check_function_import_collisions(
        &mut self,
        program: &crate::ast::Program,
    ) {
        // DEC-277: a `Core.Native.*` raw-native module is whole-module-import only — a MEMBER
        // fn-import is excluded from the binding set (see `function_import_bindings`), so reject
        // it here with guidance instead of letting the bare call fail as an unknown function.
        for it in &program.items {
            let crate::ast::Item::Import { path, span, .. } = it else {
                continue;
            };
            if path.len() >= 4 && path[0] == "Core" && path[1] == "Native" {
                let module = path[..path.len() - 1].join(".");
                let leaf = &path[path.len() - 1];
                if crate::native::index_of(&module, leaf).is_some() {
                    self.err_coded(
                        *span,
                        format!(
                            "`{module}.{leaf}` cannot be member-imported — raw `Core.Native.*` \
                             modules are whole-module imports only"
                        ),
                        "E-IMPORT-NATIVE-MEMBER",
                        Some(format!(
                            "write `import {module};` and call `{}.{leaf}(...)` qualified — or use \
                             the friendly prelude module instead",
                            path[path.len() - 2]
                        )),
                    );
                }
            }
        }
        let bindings = super::function_imports::function_import_bindings(&program.items);
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (bound, _, _, span) in &bindings {
            if !seen.insert(bound.clone()) {
                self.err_coded(
                    *span,
                    format!("`{bound}` is imported as a function more than once"),
                    "E-IMPORT-CONFLICT",
                    Some(format!(
                        "two modules export `{bound}` — alias one with `as`, e.g. \
                         `import <Module>.{bound} as {bound}2;`"
                    )),
                );
            }
        }
    }
}
