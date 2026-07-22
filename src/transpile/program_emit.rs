//! PHP transpiler — program orchestration: name collection, whole-program emission
//! (flat + namespaced), `main` bootstrap shaping.

use super::*;

/// B2 — trait-alias lookup for MI/decomposed `parent.m(…)` calls: each call's `(ancestor-as-written,
/// method)` → the `private` trait alias it lowers to. See [`Transpiler::mi_parent_aliases`].
pub(super) type ParentAliasMap = std::collections::BTreeMap<(Option<String>, String), String>;

/// Feature B-static: the program's **non-literal** static-field initializers, as `(class, field,
/// init_expr)` in declaration order. These can't be PHP property defaults (PHP requires a constant
/// expression), so they are set once by a generated `__phorj_init_statics()` called before `main()`.
/// A literal static stays a plain PHP `static $x = <lit>;` default and is absent here.
fn runtime_static_inits(program: &Program) -> Vec<(&str, &str, &Expr)> {
    let mut out = Vec::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            for m in &c.members {
                if let ClassMember::Field {
                    modifiers,
                    name,
                    init: Some(e),
                    ..
                } = m
                {
                    if modifiers.contains(&Modifier::Static)
                        && !modifiers.contains(&Modifier::Const)
                        && crate::value::const_literal(e).is_none()
                    {
                        out.push((c.name.as_str(), name.as_str(), e));
                    }
                }
            }
        }
    }
    out
}

/// The entry-point bootstrap shape (Batch-1 B): `(main takes an argv param, main returns int)`. Drives
/// the PHP call site — argv is passed as `array_slice($argv ?? [], 1)` (matching `Core.Process.args()`)
/// and an `int`-returning `main` is wrapped in `exit(…)` so the return value becomes the process exit
/// status. A `void` `main()` keeps the bare `main();` call (byte-identical to pre-Batch-1 B output).
fn main_entry_shape(program: &Program) -> (bool, bool) {
    match crate::ast::entry_for(program, crate::ast::EntryRole::Cli) {
        Some((_, f)) => {
            let has_argv = !f.params.is_empty();
            let returns_int = matches!(&f.ret, Some(Type::Named { name, .. }) if name == "int");
            (has_argv, returns_int)
        }
        None => (false, false),
    }
}

/// The PHP statement that invokes the entry point (Batch-1 B/D), given the namespace prefix (`""` in
/// flat mode, `"\Main\"` namespaced). A top-level entry is `{prefix}main(...)`; a class-static entry
/// (Batch-1 D) is `{prefix}App::main(...)`. Empty string when the program has no entry (a library/web
/// file) — the caller guards on that too. Composes [`main_entry_shape`]'s argv + exit-code decisions.
fn main_bootstrap_stmt(program: &Program, ns_prefix: &str) -> String {
    let Some((entry_class, entry_decl)) =
        crate::ast::entry_for(program, crate::ast::EntryRole::Cli)
    else {
        return String::new();
    };
    let (has_argv, returns_int) = main_entry_shape(program);
    // DEC-191: the entry's NAME is whatever the program chose — key on the resolved decl.
    let callee = match entry_class {
        Some(c) => format!("{ns_prefix}{c}::{}", entry_decl.name),
        None => format!("{ns_prefix}{}", entry_decl.name),
    };
    let call = if has_argv {
        format!("{callee}(array_slice($argv ?? [], 1))")
    } else {
        format!("{callee}()")
    };
    if returns_int {
        format!("exit({call});")
    } else {
        format!("{call};")
    }
}

/// Whether class `cls` declares its own `private`/`protected` constructor (Batch A). A static-field
/// initializer of such a class (the singleton pattern — `static C inst = new C(...)`) must run in the
/// class's own scope in PHP, else PHP rejects the construction from the global `__phorj_init_statics`
/// while the Phorj backends (which treat a static init as in-class) accept it — a byte-identity break.
fn class_has_restricted_ctor(program: &Program, cls: &str) -> bool {
    program.items.iter().any(|it| {
        matches!(it, Item::Class(c) if c.name == cls
            && c.members.iter().any(|m| matches!(m,
                ClassMember::Constructor { modifiers, .. }
                    if modifiers.iter().any(|md| matches!(md, Modifier::Private | Modifier::Protected)))))
    })
}

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
                    let ns = namespace_of(&e.name);
                    self.enums.insert(e.name.clone()); // DEC-302: route Enum.cases()/from/tryFrom
                    for v in &e.variants {
                        self.variants.insert(v.name.clone());
                        self.variant_ns.insert(v.name.clone(), ns.clone());
                        self.variant_fields.insert(
                            v.name.clone(),
                            v.fields.iter().map(|p| p.name.clone()).collect(),
                        );
                        // T6b: payload kinds (positional) for variant-payload match bindings.
                        self.variant_field_kinds.insert(
                            v.name.clone(),
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

    pub(super) fn emit_program(&mut self, program: &Program) -> Result<(), String> {
        // A mangled (`\`-bearing) top-level name means a multi-package project (M5 S2c): switch to
        // the brace-namespace form. A single-package program (every existing example) has no `\`
        // names and stays on the flat path — byte-identical to today's output.
        self.namespaced = program.items.iter().any(|it| match it {
            Item::Function(f) => f.name.contains('\\'),
            // A cross-package *type* (class/enum/interface) is mangled too — a project may export
            // only types and no functions (M-RT cross-package types), so check type names as well.
            Item::Class(c) => c.name.contains('\\'),
            Item::Enum(e) => e.name.contains('\\'),
            Item::Interface(i) => i.name.contains('\\'),
            // A cross-package *trait* is mangled too (a class composes it via `use \FQN`), so a
            // project may carry only a library trait + a `package Main` consumer — switch on it.
            Item::Trait(t) => t.name.contains('\\'),
            _ => false,
        });
        if self.namespaced {
            return self.emit_program_namespaced(program);
        }
        self.out.push_str("<?php\n");
        let mut emitted_overloads: HashSet<String> = HashSet::new();
        for item in &program.items {
            match item {
                Item::Import { .. } => {}
                // M8.5: a foreign `declare function` produces no PHP definition (PHP already has it).
                Item::Function(f) if f.foreign => {}
                Item::Function(f) => {
                    self.emit_free_fn(&program.items, f, &mut emitted_overloads)?
                }
                Item::Enum(e) => self.emit_enum(e)?,
                // M8.5: a foreign `declare class` produces no PHP definition (PHP already has it).
                Item::Class(c) if c.foreign => {}
                Item::Class(c) => {
                    // M-RT S6b: multiple inheritance lowers to traits/interfaces (PHP has no MI).
                    if c.extends.len() >= 2 {
                        self.emit_multi_class(c, program)?;
                    } else if self.decomposed.contains(&c.name) {
                        self.emit_decomposed_class(c, program)?;
                    } else {
                        self.emit_class(c, program)?;
                    }
                }
                Item::Interface(i) => self.emit_interface(i)?,
                // M-RT S8: a native PHP `trait` (composed by classes via `use`).
                Item::Trait(t) => self.emit_trait(t)?,
                // Aliases are expanded out of the AST before transpiling; arm only for exhaustiveness.
                Item::TypeAlias { .. } => {}
                // M-Test: `test` items are checker-gated out of any transpiled build.
                Item::Test { .. } => {}
            }
        }
        // Feature B-static: runtime static initializers run once, before `main` (matching the Rust
        // backends' eager-at-startup eval). PHP hoists the function, so emitting its body after the
        // call is fine.
        let rt_statics = runtime_static_inits(program);
        // The interpreter auto-invokes `main`; PHP does not. Emit the call so the output
        // is a runnable program, not just definitions.
        // Batch-1 D: the entry may be a top-level `main` OR a class-static `main` (so the guard is
        // `entry_point`, not `funcs.contains("main")` — a static entry isn't a free function).
        if crate::ast::entry_for(program, crate::ast::EntryRole::Cli).is_some() {
            if !rt_statics.is_empty() {
                self.line("__phorj_init_statics();");
            }
            let stmt = main_bootstrap_stmt(program, "");
            self.line(&stmt);
        }
        if !rt_statics.is_empty() {
            self.line("function __phorj_init_statics() {");
            self.indent += 1;
            for (cls, field, e) in &rt_statics {
                let v = self.emit_expr(e)?;
                if class_has_restricted_ctor(program, cls) {
                    // Run the initializer in the class's own scope so a `private`/`protected` ctor is
                    // callable here (the singleton pattern), matching the Phorj backends (Batch A).
                    self.line(&format!(
                        "{cls}::${field} = (\\Closure::bind(static fn() => {v}, null, {cls}::class))();"
                    ));
                } else {
                    self.line(&format!("{cls}::${field} = {v};"));
                }
            }
            self.indent -= 1;
            self.line("}");
        }
        // The runtime helpers, each defined once when used. PHP hoists top-level function
        // declarations, so emitting them after `main();` is still callable from any body.
        self.emit_runtime_helpers();
        self.emit_log_helpers();
        self.emit_fs_helpers();
        Ok(())
    }

    /// Multi-package emission (M5 S2c, M5-7): one `namespace …{}` brace-block per package, then a
    /// nameless `namespace {}` block that bootstraps `\Main\main()` and holds the global `opt!`
    /// helper. A definition's namespace is its mangled prefix (`Acme\Util\compute` ⇒ `Acme\Util`,
    /// `Acme\Geometry\Point` ⇒ `Acme\Geometry`); bare names (the `main` package) land in `Main`. A
    /// cross-package type's definition (class/enum/interface) is bucketed into its own namespace
    /// (M-RT cross-package types). The bootstrap block is emitted last so every package's functions
    /// and types are already declared when it runs.
    pub(super) fn emit_program_namespaced(&mut self, program: &Program) -> Result<(), String> {
        use std::collections::BTreeMap;
        self.out.push_str("<?php\n");
        let mut buckets: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
        for item in &program.items {
            let ns = match item {
                // M8.5: a foreign `declare` (function or class) produces no PHP definition — PHP already
                // has it. References emit the global `\Name` form; never bucket it into a namespace.
                Item::Function(f) if f.foreign => continue,
                Item::Class(c) if c.foreign => continue,
                Item::Function(f) => namespace_of(&f.name),
                Item::Enum(e) => namespace_of(&e.name),
                Item::Class(c) => namespace_of(&c.name),
                Item::Interface(i) => namespace_of(&i.name),
                // A `use`d trait is bucketed into its own package namespace, exactly like a class
                // (its FQN is the mangled prefix); the using class emits `use \Acme\Mix\Greet`.
                Item::Trait(t) => namespace_of(&t.name),
                _ => continue,
            };
            buckets.entry(ns).or_default().push(item);
        }
        // DEC-325 P1 (recorded KNOWN_ISSUES): injected prelude classes/enums land in `namespace
        // Main`, so a bare reference from any OTHER package fatals (`Class "Acme\\X\\FileSystem"
        // not found`). Alias every Main-bucket top-level name into each non-Main block (`use
        // \\Main\\X;` — inert when unused; skipped when the block declares the same name itself).
        let main_names: Vec<(bool, String)> = buckets
            .get("Main")
            .map(|items| {
                let mut ns_names = Vec::new();
                for it in items {
                    match it {
                        Item::Class(c) => ns_names.push((false, c.name.clone())),
                        Item::Interface(i) => ns_names.push((false, i.name.clone())),
                        Item::Trait(t) => ns_names.push((false, t.name.clone())),
                        Item::Enum(e) => {
                            ns_names.push((false, e.name.clone()));
                            for v in &e.variants {
                                ns_names.push((false, php_variant_name(&v.name)));
                            }
                        }
                        Item::Function(f) => ns_names.push((true, f.name.clone())),
                        _ => {}
                    }
                }
                ns_names
            })
            .unwrap_or_default();
        let mut emitted_overloads: HashSet<String> = HashSet::new();
        for (ns, items) in &buckets {
            self.line(&format!("namespace {ns} {{"));
            self.indent += 1;
            if ns != "Main" {
                let declared: HashSet<String> = items
                    .iter()
                    .filter_map(|it| match it {
                        Item::Class(c) => Some(leaf_name(&c.name)),
                        Item::Interface(i) => Some(leaf_name(&i.name)),
                        Item::Trait(t) => Some(leaf_name(&t.name)),
                        Item::Enum(e) => Some(leaf_name(&e.name)),
                        Item::Function(f) => Some(leaf_name(&f.name)),
                        _ => None,
                    })
                    .collect();
                for (is_fn, name) in &main_names {
                    if declared.contains(name) {
                        continue;
                    }
                    if *is_fn {
                        self.line(&format!("use function \\Main\\{name};"));
                    } else {
                        self.line(&format!("use \\Main\\{name};"));
                    }
                }
            }
            for item in items {
                match item {
                    Item::Function(f) => {
                        // Group M-RT overloads within this package's bucket (same full name).
                        let group: Vec<&FunctionDecl> = items
                            .iter()
                            .filter_map(|it| match &**it {
                                Item::Function(g) if g.name == f.name => Some(g),
                                _ => None,
                            })
                            .collect();
                        if group.len() > 1 {
                            if emitted_overloads.insert(f.name.clone()) {
                                self.emit_overload_set(&f.name, &group, false)?;
                            }
                        } else {
                            self.emit_function(f, false)?;
                        }
                    }
                    Item::Enum(e) => self.emit_enum(e)?,
                    Item::Class(c) => self.emit_class(c, program)?,
                    Item::Interface(i) => self.emit_interface(i)?,
                    // M-RT S8 cross-package: a native PHP `trait` declared in its package's block.
                    Item::Trait(t) => self.emit_trait(t)?,
                    _ => {}
                }
            }
            self.indent -= 1;
            self.line("}");
        }
        self.line("namespace {");
        self.indent += 1;
        if crate::ast::entry_for(program, crate::ast::EntryRole::Cli).is_some() {
            let stmt = main_bootstrap_stmt(program, "\\Main\\");
            self.line(&stmt);
        }
        self.emit_runtime_helpers();
        self.emit_log_helpers();
        self.emit_fs_helpers();
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
}

/// The declared LEAF of a (possibly package-mangled) top-level name — `Acme\\Fs\\Probe` → `Probe`,
/// bare names unchanged. Used by the DEC-325 `use \\Main\\…` aliasing to skip names a namespace
/// declares itself.
fn leaf_name(name: &str) -> String {
    name.rsplit('\\').next().unwrap_or(name).to_string()
}
