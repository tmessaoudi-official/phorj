//! DEC-320 v1 — `phg build --php`: per-file SIBLING emission (the TS→JS playbook).
//!
//! One whole-program transpile (the checker/collision context needs it), routed per item: every
//! class/enum/interface/trait lands in the `.php` SIBLING of the `.phg` that declared it (PSR-4
//! addressing unchanged — the folder=package law + the public-surface file rule make the sibling
//! path exactly the autoloader's expectation), while everything the siblings share lands in ONE
//! `_phorj/runtime.php`:
//!   * every `__phorj_*` helper any sibling uses (flags accumulate across the per-file passes,
//!     so the runtime carries exactly the project's helper set — no force-list to drift);
//!   * the injected prelude items (`Result`, `Option`, `FileSystem`, … — anything the loader has
//!     no origin file for);
//!   * every FREE FUNCTION (PHP autoloads classes, never functions — composer's `files` entry
//!     loads the runtime eagerly, so cross-file function calls always resolve);
//!   * the `__phorj_init_statics()` runtime-static initializer, CALLED at include time (composer
//!     loads `files` entries at autoload init — the same before-any-user-code point the
//!     whole-program transpile guarantees).
//!
//! The `#[Entry]` bootstrap is NOT emitted — a split build embeds phorj code in a host PHP app,
//! which owns the request lifecycle; `\Main\main()` stays a plain callable.

use super::*;
use std::path::PathBuf;

/// What a split build emits: one PHP source per originating `.phg` file (sorted by path, so the
/// caller's writes are deterministic) + the shared runtime.
pub struct PhpSplit {
    /// `(declaring .phg path, generated PHP source)` — the caller writes each to the `.php` sibling.
    pub files: Vec<(PathBuf, String)>,
    /// The shared `_phorj/runtime.php` body.
    pub runtime: String,
    /// Whether the program emitted in brace-namespace form (a trailing top-level statement — the
    /// classmap autoloader — must then live inside a `namespace {}` block).
    pub namespaced: bool,
    /// Every sibling-declared PHP class FQN → its declaring `.phg` (sorted). Drives the generated
    /// classmap autoloader in the runtime: an enum emits SEVERAL classes (base + the DEC-329.3
    /// scoped variant classes) from ONE file, which plain PSR-4 cannot address — so the runtime
    /// registers its own `spl_autoload_register` map and the host needs NOTHING beyond the one
    /// composer `files` entry.
    pub classmap: Vec<(String, PathBuf)>,
}

/// Which slice of the program an emission pass produces (Off = the classic whole-program emit).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum SplitPass {
    /// The classic single-output emit: bootstrap + statics + gated helpers, no item filter.
    Off,
    /// One originating file's items only: no bootstrap, no statics fn, no helpers (they all live
    /// in the runtime), item filter active.
    File,
    /// The shared runtime: injected items + free functions + statics + the accumulated helpers;
    /// still no bootstrap.
    Runtime,
}

/// Split-transpile `program` (already loaded + expanded through the Invariant-6 chokepoint),
/// routing each item to its declaring file per `item_files` (the loader's DEC-320 attribution;
/// mangled top-level name → `.phg` path). Items with no attribution (injected preludes) and all
/// free functions go to the shared runtime.
pub fn emit_split(
    program: &Program,
    item_files: &HashMap<String, PathBuf>,
) -> Result<PhpSplit, String> {
    collisions::check_variant_collisions(program)?;
    let mut t = Transpiler::new();
    t.class_implements = crate::ast::class_implements(program);
    t.class_tables = crate::native::ClassTables::from_program(program);
    t.consts = crate::ast::class_consts(program).into_keys().collect();
    t.decomposed = decomposed_classes(program);
    t.collect(program);

    // Route: TYPE items with an origin file → that file's bucket; everything else (free functions,
    // injected prelude items) → the runtime. BTreeMap: deterministic file order (Invariant 10).
    let mut buckets: std::collections::BTreeMap<PathBuf, HashSet<String>> =
        std::collections::BTreeMap::new();
    let mut runtime_keep: HashSet<String> = HashSet::new();
    for item in &program.items {
        let (name, is_type) = match item {
            Item::Class(c) if !c.foreign => (&c.name, true),
            Item::Enum(e) => (&e.name, true),
            Item::Interface(i) => (&i.name, true),
            Item::Trait(tr) => (&tr.name, true),
            Item::Function(f) if !f.foreign => (&f.name, false),
            _ => continue,
        };
        match item_files.get(name) {
            Some(file) if is_type => {
                buckets
                    .entry(file.clone())
                    .or_default()
                    .insert(name.clone());
            }
            _ => {
                runtime_keep.insert(name.clone());
            }
        }
    }

    // Per-file passes FIRST (helper flags accumulate on the shared instance), runtime LAST — so
    // the runtime's gated helpers cover exactly what the siblings (and the runtime items
    // themselves, emitted before the trailing helpers) actually use.
    let mut files = Vec::with_capacity(buckets.len());
    t.split = SplitPass::File;
    for (file, keep) in &buckets {
        t.keep = Some(keep.clone());
        t.out = String::new();
        t.emit_program(program)?;
        files.push((file.clone(), std::mem::take(&mut t.out)));
    }
    t.split = SplitPass::Runtime;
    t.keep = Some(runtime_keep);
    t.out = String::new();
    t.emit_program(program)?;
    let runtime = std::mem::take(&mut t.out);

    // The classmap: every PHP class each SIBLING declares (runtime items load eagerly with the
    // runtime itself). FQNs mirror the emission: namespaced mode puts every item in its package
    // namespace (`Main` for bare names); flat mode declares globals.
    let namespaced = program.items.iter().any(|it| match it {
        Item::Function(f) => f.name.contains('\\'),
        Item::Class(c) => c.name.contains('\\'),
        Item::Enum(e) => e.name.contains('\\'),
        Item::Interface(i) => i.name.contains('\\'),
        Item::Trait(t) => t.name.contains('\\'),
        _ => false,
    });
    let fqn = |name: &str, class: String| -> String {
        if !namespaced {
            return class;
        }
        match super::namespace_of(name).as_str() {
            "Main" => format!("Main\\{class}"),
            ns => format!("{ns}\\{class}"),
        }
    };
    let mut classmap: Vec<(String, PathBuf)> = Vec::new();
    for item in &program.items {
        let name = match item {
            Item::Class(c) if !c.foreign => &c.name,
            Item::Interface(i) => &i.name,
            Item::Trait(tr) => &tr.name,
            Item::Enum(e) => {
                if let Some(file) = item_files.get(&e.name) {
                    let base = super::php_class_name(super::last_segment(&e.name));
                    classmap.push((fqn(&e.name, base), file.clone()));
                    for v in &e.variants {
                        let scoped = super::php_scoped_variant_name(&e.name, &v.name);
                        classmap.push((fqn(&e.name, scoped), file.clone()));
                    }
                }
                continue;
            }
            _ => continue,
        };
        if let Some(file) = item_files.get(name) {
            classmap.push((
                fqn(name, super::php_class_name(super::last_segment(name))),
                file.clone(),
            ));
        }
    }
    classmap.sort();
    Ok(PhpSplit {
        files,
        runtime,
        namespaced,
        classmap,
    })
}

impl Transpiler {
    /// Whether the current pass emits this top-level item (always true in the classic emit).
    pub(super) fn keeps(&self, item: &Item) -> bool {
        let Some(keep) = &self.keep else { return true };
        let name = match item {
            Item::Class(c) => &c.name,
            Item::Enum(e) => &e.name,
            Item::Interface(i) => &i.name,
            Item::Trait(t) => &t.name,
            Item::Function(f) => &f.name,
            _ => return false,
        };
        keep.contains(name)
    }
}
