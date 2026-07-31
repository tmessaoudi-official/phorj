//! Transpiler driver: the public [`emit`] entry point + the `decomposed_classes` pre-pass it seeds
//! the `Transpiler` with. Split out of `transpile/mod.rs` (M-Decomp) to keep the root under the
//! file-size cap; the `Transpiler` struct + `Transpiler::new` and the emit methods live in sibling
//! modules and are reached here via `use super::*`. Pure code movement — no emit-logic change.

use super::*;

/// Transpile a parsed program to PHP source, or a `transpile error: …` for an unsupported construct.
///
/// Carries no doc comments — see [`emit_with_source`] for the doc-bearing form. Kept so callers that
/// hold only a `Program` (the benchmark path) are unchanged.
pub fn emit(program: &Program) -> Result<String, String> {
    emit_with_source(program, None)
}

/// [`emit`] plus the ORIGINAL phorj source, which lets `/** … */` doc comments be re-emitted as PHP
/// docblocks (DEC-419). Comments are not AST nodes, so the source text is the only channel; passing
/// `None` is exactly [`emit`].
pub fn emit_with_source(program: &Program, src: Option<&str>) -> Result<String, String> {
    collisions::check_variant_collisions(program)?;
    let mut t = Transpiler::new();
    t.src = src.map(str::to_string);
    t.class_implements = crate::ast::class_implements(program);
    t.class_tables = crate::native::ClassTables::from_program(program);
    t.consts = crate::ast::class_consts(program).into_keys().collect();
    t.decomposed = decomposed_classes(program);
    t.collect(program);
    t.emit_program(program)?;
    Ok(t.out)
}

/// The set of classes that must lower to the interface+trait decomposition (M-RT S6b): every
/// transitive ancestor of any multi-parent (`extends A, B`) class. A multi-parent class itself is
/// emitted as a class that `implements`+`use`s (see [`Transpiler::emit_multi_class`]) and is *not*
/// in this set, unless it is also an ancestor of another multi-parent class.
pub(super) fn decomposed_classes(program: &Program) -> BTreeSet<String> {
    let parents: HashMap<&str, &[String]> = program
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Class(c) => Some((c.name.as_str(), c.extends.as_slice())),
            _ => None,
        })
        .collect();
    let mut out: BTreeSet<String> = BTreeSet::new();
    // Seed: the direct parents of every multi-parent class; then close upward over `extends`.
    let mut queue: Vec<String> = Vec::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            if c.extends.len() >= 2 {
                queue.extend(c.extends.iter().cloned());
            }
        }
    }
    while let Some(name) = queue.pop() {
        if !out.insert(name.clone()) {
            continue;
        }
        if let Some(ps) = parents.get(name.as_str()) {
            queue.extend(ps.iter().cloned());
        }
    }
    out
}
