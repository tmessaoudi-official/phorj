//! PHP transpiler — **pass 1: `collect`**, the top-level name index.
//!
//! Split out of `program_emit.rs` by cohesion (Invariant 13): DEC-437 added the attribute-class index
//! here and pushed that file to 507, past the 500 hard cap. Indexing and emission are two passes over
//! the same program that share only the `Transpiler` — `collect` answers "what names exist", the emit
//! functions answer "what PHP do they become".

use super::*;

impl Transpiler {
    /// Pass 1 — index top-level names so call dispatch and match binding can resolve them.
    pub(super) fn collect(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Function(f) if f.foreign => {
                    // M8.5: a foreign `declare function` — index it as foreign (emitted nowhere; a call
                    // resolves to the `\name(…)` global form). Not added to `funcs`/`fn_ret_kinds`.
                    self.foreign_fns.insert(f.name.clone());
                }
                Item::Function(f) => {
                    self.funcs.insert(f.name.clone());
                    // T6c: a free function's return kind — overloads with differing kinds collapse
                    // to `Other` (the safe fallback), since the call site can't pick statically.
                    let rk = f.ret.as_ref().map_or(OpKind::Other, kind_of_type);
                    match self.fn_ret_kinds.get(&f.name) {
                        Some(existing) if *existing != rk => {
                            self.fn_ret_kinds.insert(f.name.clone(), OpKind::Other);
                        }
                        None => {
                            self.fn_ret_kinds.insert(f.name.clone(), rk);
                        }
                        _ => {}
                    }
                }
                Item::Class(c) => {
                    self.classes.insert(c.name.clone());
                    // DEC-437: an attribute CLASS, indexed so a use site can be resolved to it by the
                    // same canonical-path rule the checker applies (DEC-435).
                    if c.attrs
                        .iter()
                        .any(crate::ast::Attribute::is_attribute_marker)
                    {
                        self.attr_classes.push((
                            c.name.replace('\\', "."),
                            super::php_class_name(super::last_segment(&c.name)),
                        ));
                    }
                    // M8.5: a foreign class is also indexed as foreign — its definition is suppressed and
                    // construction/static calls take the `\Name` global form. Its members' return kinds
                    // are still recorded below so a foreign method result is a typed operand.
                    if c.foreign {
                        self.foreign_classes.insert(c.name.clone());
                    }
                    // T6b: record this class's own field/hook/promoted-ctor-param operand kinds and
                    // its parents, so field reads (`p.x`, `this.x`) resolve to a native operand.
                    self.class_parents.insert(c.name.clone(), c.extends.clone());
                    let mut fields: HashMap<String, OpKind> = HashMap::new();
                    for m in &c.members {
                        match m {
                            ClassMember::Field { ty, name, .. }
                            | ClassMember::Hook { ty, name, .. } => {
                                fields.insert(name.clone(), kind_of_type(ty));
                            }
                            ClassMember::Constructor { params, .. } => {
                                // Promoted params (those with a visibility modifier) become fields;
                                // a non-promoted param is ctor-local and never read as `o.x`, so
                                // recording it is harmless.
                                for p in params {
                                    fields.insert(p.name.clone(), kind_of_type(&p.ty));
                                }
                            }
                            // T6c: method return kinds — differing overloads collapse to `Other`.
                            ClassMember::Method(f) => {
                                let key = (c.name.clone(), f.name.clone());
                                let rk = f.ret.as_ref().map_or(OpKind::Other, kind_of_type);
                                match self.method_ret_kinds.get(&key) {
                                    Some(existing) if *existing != rk => {
                                        self.method_ret_kinds.insert(key, OpKind::Other);
                                    }
                                    None => {
                                        self.method_ret_kinds.insert(key, rk);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    self.class_field_kinds.insert(c.name.clone(), fields);
                }
                // Interfaces are not callable/constructible, so they need no resolution index;
                // they are emitted as PHP `interface` blocks in pass 2.
                Item::Interface(_) => {}
                Item::Enum(e) => {
                    self.enums.insert(e.name.clone()); // DEC-302: route Enum.cases()/from/tryFrom
                    for v in &e.variants {
                        self.variants.insert(v.name.clone());
                        self.variant_owner.insert(v.name.clone(), e.name.clone());
                        self.variant_fields.insert(
                            (e.name.clone(), v.name.clone()),
                            v.fields.iter().map(|p| p.name.clone()).collect(),
                        );
                        // T6b: payload kinds (positional) for variant-payload match bindings.
                        self.variant_field_kinds.insert(
                            (e.name.clone(), v.name.clone()),
                            v.fields.iter().map(|p| kind_of_type(&p.ty)).collect(),
                        );
                    }
                }
                Item::Import { path, alias, .. } => {
                    // The bound qualifier is the alias when present (`import a.b as c;` ⇒ `c`),
                    // else the path's last segment — the same rule as `native::import_map`.
                    // Honoring the alias matters since DEC-277: the friendly preludes import
                    // their raw natives as `import Core.Native.Debug as NativeDebug;`.
                    if let Some(q) = alias.clone().or_else(|| path.last().cloned()) {
                        self.imports.insert(q, path.join("."));
                    }
                    // DEC-197: a member import of a module FUNCTION (`import Core.Output.printLine;`)
                    // also binds the MODULE qualifier (`Output` → `Core.Output`), so the checker's
                    // bare→qualified rewrite (`Output.printLine(x)`) resolves here through the import
                    // map — mirroring `native::import_map`'s Http/Time/Decimal member-type binding. The
                    // checker rejects an un-imported qualified sibling upstream, so this never resolves
                    // a call the checker did not bless. `entry` keeps a whole-module import's binding.
                    if path.len() >= 3 {
                        let module = path[..path.len() - 1].join(".");
                        if crate::native::index_of(&module, &path[path.len() - 1]).is_some() {
                            self.imports
                                .entry(path[path.len() - 2].clone())
                                .or_insert(module);
                        }
                    }
                }
                // M-RT S8: a trait is emitted as a native PHP `trait` in pass 2; it needs no call/
                // construction resolution index (it is never called or constructed by name).
                Item::Trait(_) => {}
                // Aliases are expanded out of the AST before transpiling; arm only for exhaustiveness.
                Item::TypeAlias { .. } => {}
                // M-Test: `test` items are checker-gated out of any transpiled build.
                Item::Test { .. } => {}
            }
        }
    }
}
