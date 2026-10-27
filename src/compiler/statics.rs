//! The static-field tables the VM program carries (M-mut.7): one slot per `(class, static)` with its
//! literal seed (`static_inits`, a `Unit` placeholder for a computed one), the `(class, field)` index
//! — inherited statics aliasing their owner's slot — and the `main` prelude that evaluates the computed
//! initializers. Split from `program.rs` (Invariant 13; plan row 4c items 1 and 2).

use super::*;

/// Each `static` field gets a program-wide slot (declaration order across classes, then each using
/// class's trait copies) and its literal initializer const-folded into `static_inits`, which the VM
/// seeds its `statics` vector from at startup; the interpreter seeds its map from the same
/// `const_literal` kernel (F3). A computed initializer seeds `Unit` and runs in the `main` prelude.
///
/// `(statics_index, static_inits, static_runtime_inits)` for `program`. `class_decls` holds the classes
/// and the trait synthetics, in that order.
#[allow(clippy::type_complexity)]
pub(super) fn build<'p>(
    program: &'p Program,
    class_decls: &[&ClassDecl],
) -> (
    HashMap<(String, String), (usize, CTy)>,
    Vec<Value>,
    Vec<(usize, &'p Expr)>,
) {
    let mut statics_index: HashMap<(String, String), (usize, CTy)> = HashMap::new();
    let mut static_inits: Vec<Value> = Vec::new();
    // Feature B-static: a static whose initializer is NOT a compile-time literal gets a `Unit`
    // placeholder in `static_inits` and a `(slot, init expr)` entry here; a `SetStatic` prelude is
    // emitted at the start of `main` (declaration order) to evaluate it once before any user code —
    // matching the interpreter's `eval_static_inits` and the transpiler's `__phorj_init_statics`.
    for c in class_decls {
        for m in &c.members {
            if let ClassMember::Field {
                modifiers,
                name,
                ty,
                init,
                ..
            } = m
            {
                if modifiers.contains(&Modifier::Static) {
                    let slot = static_inits.len();
                    statics_index.insert((c.name.clone(), name.clone()), (slot, resolve_cty(ty)));
                    // A non-literal gets a `Unit` placeholder, set by the main prelude; which
                    // initializers run, and in what order, is `ast::static_runtime_inits` (below).
                    static_inits.push(
                        init.as_ref()
                            .and_then(crate::value::const_literal)
                            .unwrap_or(Value::Unit),
                    );
                }
            }
        }
    }
    // M-RT S8: a `use`d trait's `static` field becomes a PER-USING-CLASS copy (PHP `use` semantics) —
    // each using class gets its own slot keyed `(class, field)`. The trait synthetic's own
    // `(trait, field)` entry from the loop above is inert (a trait is never a static holder at runtime).
    for it in &program.items {
        let Item::Class(c) = it else { continue };
        for u in &c.uses {
            for t in &program.items {
                let Item::Trait(td) = t else { continue };
                if td.name != u.name {
                    continue;
                }
                for m in &td.members {
                    if let ClassMember::Field {
                        modifiers,
                        name,
                        ty,
                        init,
                        ..
                    } = m
                    {
                        if modifiers.contains(&Modifier::Static) {
                            statics_index.insert(
                                (c.name.clone(), name.clone()),
                                (static_inits.len(), resolve_cty(ty)),
                            );
                            let v = init
                                .as_ref()
                                .and_then(crate::value::const_literal)
                                .unwrap_or(Value::Unit);
                            static_inits.push(v);
                        }
                    }
                }
            }
        }
    }
    // Plan row 4c item 2: a static named through an inheriting class is its owner's ONE slot — an
    // alias entry, never a second slot (`ast::static_field_owners`, shared with both other backends).
    for ((class, field), owner) in crate::ast::static_field_owners(program) {
        if owner != class {
            if let Some(entry) = statics_index.get(&(owner, field.clone())).cloned() {
                statics_index.insert((class, field), entry);
            }
        }
    }
    // Plan row 4c item 1: the prelude evaluates the shared list — each class's own non-literal statics,
    // then its copy of each trait's — into the owning slot; a trait synthetic's inert slot never runs.
    let static_runtime_inits: Vec<(usize, &'p Expr)> = crate::ast::static_runtime_inits(program)
        .into_iter()
        .filter_map(|(class, field, e)| {
            statics_index
                .get(&(class, field))
                .map(|&(slot, _)| (slot, e))
        })
        .collect();
    (statics_index, static_inits, static_runtime_inits)
}
