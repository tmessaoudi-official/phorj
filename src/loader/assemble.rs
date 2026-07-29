//! The shared two-pass assembly (parse + validate + mangle + rewrite + flat-merge). Split out of
//! `loader/mod.rs` (M-Decomp) to keep the root under the file-size cap; all shared types
//! (`Unit`, `Source`, `DefInfo`, `ResolveCtx`, `LoadStats`) and the `mangle` helper stay in the
//! root and are reached here via `use super::*`. Called by the pipeline in `entry.rs` (a sibling),
//! so it is `pub(super)`.

use super::*;

/// The shared two-pass assembly (DEC-282 factored it out of `load_project` so the unified loader
/// reuses it verbatim): parse + validate every source, mangle non-`Main` definitions to globally
/// unique names, rewrite call/type sites per file, merge into one flat [`Program`].
/// `decl_files` is the pre-collected ambient `*.d.phg` set (the CALLER owns the sweep scope —
/// the unified loader deliberately keeps the entry-local sweep NON-recursive so a directory of
/// unrelated scripts never inhales a distant project's foreign declares).
pub(super) fn assemble(
    entry: &Path,
    sources: Vec<Source>,
    decl_files: &[PathBuf],
    buffer: Option<(&Path, &str)>,
) -> Result<Unit, String> {
    // Pass 1 — parse, validate, and index every top-level definition by (package, name) ⇒ mangled
    // global name. Functions and types live in separate symbol tables (PHP namespaces functions and
    // classes separately), so a `compute` function and a `Compute` type never collide. Library
    // packages may now declare types (the old `E-PKG-TYPE` gate is retired — cross-package types).
    let mut parsed: Vec<(PathBuf, Program)> = Vec::with_capacity(sources.len());
    let mut defined: HashMap<(String, String), String> = HashMap::new();
    let mut types: HashMap<(String, String), String> = HashMap::new();
    // Declaration-visibility provenance (visibility modifiers): where each definition lives + its
    // visibility, keyed by (package, name) like the rename tables. Consumed by the lattice in Pass 2.
    let mut prov_fns: HashMap<(String, String), DefInfo> = HashMap::new();
    let mut prov_types: HashMap<(String, String), DefInfo> = HashMap::new();
    // Whole-project scope counters for `phg check`'s success summary.
    let mut pkgset: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut defs: usize = 0;
    // Trace-attribution maps (error-handling slice 1): per-file source + function → file.
    let mut src_map: HashMap<PathBuf, String> = HashMap::new();
    let mut fn_files: HashMap<String, PathBuf> = HashMap::new();
    let mut item_files: HashMap<String, PathBuf> = HashMap::new();
    for src_entry in &sources {
        let file = &src_entry.file;
        // The LSP buffer override (DEC-252): the entry's text may be the editor's unsaved buffer.
        let src = match buffer {
            Some((p, b)) if same_file(p, file) => b.to_string(),
            _ => read_file(file)?,
        };
        src_map.insert(file.clone(), src.clone());
        let prog = parse_at(file, &src)?;
        validate_folder_path(&prog, file, &src_entry.root)?;
        validate_package_decl(&prog, file)?;
        validate_public_surface(&prog, file)?;
        if src_entry.vendored && (prog.package.is_empty() || prog.package == ["Main"]) {
            return Err(format!(
                "{}: a vendored dependency is a library and cannot declare `package Main` \
                 (it would collide with the consumer's entry) [E-VENDOR-MAIN]",
                file.display()
            ));
        }
        let pkg = prog.package.join(".");
        pkgset.insert(if pkg.is_empty() {
            "main".to_string()
        } else {
            pkg.clone()
        });
        for item in &prog.items {
            let (name, is_type, vis) = match item {
                Item::Function(f) => (&f.name, false, f.vis),
                Item::Class(c) => (&c.name, true, c.vis),
                Item::Enum(e) => (&e.name, true, e.vis),
                Item::Interface(i) => (&i.name, true, i.vis),
                // A trait is a public named symbol in the type namespace (it carries no visibility
                // modifier — always public reuse). Register it so a cross-package type import +
                // `use T;` can resolve and mangle it to its FQN, exactly like a class/interface.
                Item::Trait(t) => (&t.name, true, crate::ast::Visibility::Public),
                _ => continue,
            };
            let table = if is_type { &mut types } else { &mut defined };
            if table
                .insert((pkg.clone(), name.clone()), mangle(&prog.package, name))
                .is_some()
            {
                return Err(format!(
                    "{}: duplicate definition of `{}` in package `{}` \
                     (a name must be unique within its package) [E-DUP-DEF]",
                    file.display(),
                    name,
                    if pkg.is_empty() { "main" } else { &pkg }
                ));
            }
            let prov = if is_type {
                &mut prov_types
            } else {
                &mut prov_fns
            };
            prov.insert(
                (pkg.clone(), name.clone()),
                DefInfo {
                    file: file.clone(),
                    package: pkg.clone(),
                    vis,
                },
            );
            // A free function's trace frame is keyed by its compiled (mangled) name — map it to its
            // file so a runtime trace can show `file:line` (methods/ctors are synthesized elsewhere).
            if !is_type {
                fn_files.insert(mangle(&prog.package, name), file.clone());
            }
            // DEC-320: every top-level definition (types too) → its declaring file, for the
            // `phg build --php` sibling emit's per-item routing.
            item_files.insert(mangle(&prog.package, name), file.clone());
            defs += 1;
        }
        parsed.push((file.clone(), prog));
    }

    // Q-A — expand wildcard imports (`import X.*;`) to per-member imports NOW, using the Pass-1 index
    // (user/vendored, gated by `vis_violation`) or the native registry (`Core.*`). Compile-time sugar
    // (Inv 5): Pass-2 and every backend see only plain per-symbol `Item::Import`s.
    expand_wildcard_imports(&mut parsed, &prov_fns, &prov_types)?;

    // M8.5 S3b — ambient `*.d.phg` declaration files: a file of foreign `declare`s carrying no package,
    // loaded into the project (the `.d.ts` analog). Parsed + validated (no package, all foreign) but
    // NOT folder=path-validated and NOT indexed as package definitions; their foreign items merge
    // ambiently into the unit (the checker's prebind makes merge order irrelevant) and are emitted by
    // the transpiler as global `\Name` symbols. First-party only — vendored decl bundling is deferred.
    // Excluded from `collect_phg`, so a decl file is never compiled as a package source.
    let mut decl_items: Vec<Item> = Vec::new();
    let mut decl_count = 0usize;
    let mut decl_seen: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for f in decl_files {
        if !decl_seen.insert(f.clone()) {
            continue;
        }
        let src = read_file(f)?;
        let prog = parse_at(f, &src)?;
        validate_decl_file(&prog, f)?;
        src_map.insert(f.clone(), src);
        decl_items.extend(prog.items);
        decl_count += 1;
    }

    let stats = LoadStats {
        files: sources.len() + decl_count,
        packages: pkgset.len(),
        defs,
    };

    // Pass 2 — resolve call sites per file, then flat-merge.
    let mut merged_items: Vec<Item> = Vec::new();
    // The merged unit runs as the entry's package (normally `main`); its span anchors any
    // program-level diagnostic.
    let mut unit_package: Vec<String> = vec!["Main".to_string()];
    let mut unit_span = Span {
        start: 0,
        len: 0,
        line: 0,
        col: 0,
    };

    for (file, prog) in parsed {
        if same_file(&file, entry) {
            unit_package = prog.package.clone();
            unit_span = prog.span;
        }
        // Q-A step 3 (G6): reject member imports naming a non-existent member (E-IMPORT-UNKNOWN).
        validate_member_imports(&prog, &defined, &types, &pkgset, &file)?;
        let user_imports = user_import_map(&prog.items, &types, &defined);
        let type_imports = build_type_imports(&prog, &types, &prov_types, &user_imports, &file)?;
        let function_imports =
            build_function_imports(&prog, &defined, &prov_fns, &user_imports, &file)?;
        let ctx = ResolveCtx {
            package: prog.package.clone(),
            user_imports,
            defined: &defined,
            types: &types,
            type_imports,
            function_imports,
            file: &file,
            prov_types: &prov_types,
            prov_fns: &prov_fns,
            violations: RefCell::new(Vec::new()),
        };
        for item in prog.items {
            merged_items.push(resolve_item(item, &ctx));
        }
        // Surface the first visibility violation collected while resolving this file (the
        // infallible `resolve_*` chain buffers them).
        if let Some(first) = ctx.violations.into_inner().into_iter().next() {
            return Err(first);
        }
    }

    // Ambient foreign declarations merge unmangled (they are global PHP symbols — never namespaced).
    merged_items.extend(decl_items);

    Ok(Unit {
        program: Program {
            package: unit_package,
            items: merged_items,
            span: unit_span,
        },
        diag_src: String::new(),
        stats: Some(stats),
        sources: src_map,
        fn_files,
        item_files,
    })
}
