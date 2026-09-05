//! PHP transpiler — program orchestration: whole-program emission (flat + namespaced) and `main`
//! bootstrap shaping. Pass-1 name collection lives in `collect.rs`.

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
        self.out.push_str(PHP_PROLOGUE);
        let mut emitted_overloads: HashSet<String> = HashSet::new();
        for item in &program.items {
            // DEC-320 split emission: a per-file/runtime pass emits only its routed items.
            if !self.keeps(item) {
                continue;
            }
            // DEC-419: the declaration's `/** … */` doc comment, re-emitted as a PHP docblock. One site
            // for every top-level kind, so a new kind cannot silently lose its docs.
            if let Some(span) = crate::ast::item_decl_span(item) {
                self.emit_doc_block(span);
            }
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
        // DEC-320: a split build embeds phorj code in a host PHP app — no bootstrap; the runtime
        // pass still initializes statics AT INCLUDE TIME (composer `files` loads it before any
        // user code, the same before-main point the classic emit guarantees).
        if self.split == split::SplitPass::Off
            && crate::ast::entry_for(program, crate::ast::EntryRole::Cli).is_some()
        {
            if !rt_statics.is_empty() {
                self.line("__phorj_init_statics();");
            }
            let stmt = main_bootstrap_stmt(program, "");
            self.line(&stmt);
        }
        if self.split == split::SplitPass::Runtime && !rt_statics.is_empty() {
            self.line("__phorj_init_statics();");
        }
        if self.split != split::SplitPass::File && !rt_statics.is_empty() {
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
        // DEC-320: a per-file pass emits NO helpers — they all live in the shared runtime.
        if self.split != split::SplitPass::File {
            self.emit_runtime_helpers();
            self.emit_log_helpers();
            self.emit_fs_helpers();
            self.emit_db_helpers();
        }
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
        self.out.push_str(PHP_PROLOGUE);
        let mut buckets: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
        for item in &program.items {
            // DEC-320 split emission: a per-file/runtime pass buckets only its routed items.
            if !self.keeps(item) {
                continue;
            }
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
        // Derived from the FULL program (not the possibly-filtered buckets): a DEC-320 per-file
        // pass still needs every Main-bucket name aliased — the injected preludes it references
        // are emitted by the RUNTIME pass, not this one. Byte-identical for the classic emit.
        let main_names: Vec<(bool, String)> = {
            let items: Vec<&Item> = program
                .items
                .iter()
                .filter(|it| match it {
                    Item::Function(f) if f.foreign => false,
                    Item::Class(c) if c.foreign => false,
                    Item::Function(f) => namespace_of(&f.name) == "Main",
                    Item::Enum(e) => namespace_of(&e.name) == "Main",
                    Item::Class(c) => namespace_of(&c.name) == "Main",
                    Item::Interface(i) => namespace_of(&i.name) == "Main",
                    Item::Trait(t) => namespace_of(&t.name) == "Main",
                    _ => false,
                })
                .collect();
            Some(items)
                .filter(|v| !v.is_empty())
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
                                    ns_names
                                        .push((false, php_scoped_variant_name(&e.name, &v.name)));
                                }
                            }
                            Item::Function(f) => ns_names.push((true, f.name.clone())),
                            _ => {}
                        }
                    }
                    ns_names
                })
                .unwrap_or_default()
        };
        let mut emitted_overloads: HashSet<String> = HashSet::new();
        for (ns, items) in &buckets {
            self.line(&format!("namespace {ns} {{"));
            self.current_ns = Some(ns.clone());
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
                // DEC-419: same docblock re-emission as the flat path — the namespaced form must not
                // silently drop documentation just because the program spans packages.
                if let Some(span) = crate::ast::item_decl_span(item) {
                    self.emit_doc_block(span);
                }
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
        // DEC-455.11: the `__phorj_*` runtime helpers below live in the GLOBAL namespace, but the
        // injected preludes they instantiate (`new RequestBody(…)`, `new RegexMatch(…)`, …) are
        // `package Main` classes emitted into `namespace Main {}`. A bare name here resolves to
        // `\RequestBody` and PHP fatals with `Class "RequestBody" not found` — for every project
        // touching Http, Regex, Json, Decimal or Session. Alias the Main-bucket names in, exactly as
        // the DEC-325 loop does for each non-Main package block.
        //
        // Why centrally and not per helper family: `emit_json_helpers` already qualifies ITS class
        // references with a `\Main\` prefix (`runtime_tables.rs`), and that per-family fix was never
        // carried to the other four preludes — which is precisely how this defect survived. One
        // alias block covers every prelude that exists and every one added later.
        //
        // FUNCTIONS are deliberately NOT aliased. The helper bodies call PHP builtins bare
        // (`count`, `strlen`, `implode`), and inside the global block a bare call already resolves
        // to the builtin; a `use function \Main\count;` would hijack it. Class aliases carry no such
        // hazard — the helpers spell every PHP builtin CLASS fully qualified (`\RuntimeException`,
        // `\OutOfRangeException`, `\Closure`), and the global block declares no classes of its own.
        //
        // Emitted BEFORE the bootstrap statement: a `use` only binds names that follow it.
        if self.split != split::SplitPass::File {
            for (is_fn, name) in &main_names {
                if !*is_fn {
                    self.line(&format!("use \\Main\\{name};"));
                }
            }
        }
        if self.split == split::SplitPass::Off
            && crate::ast::entry_for(program, crate::ast::EntryRole::Cli).is_some()
        {
            let stmt = main_bootstrap_stmt(program, "\\Main\\");
            self.line(&stmt);
        }
        // DEC-320: helpers live in the shared runtime for a split build (see the flat path).
        if self.split != split::SplitPass::File {
            self.emit_runtime_helpers();
            self.emit_log_helpers();
            self.emit_fs_helpers();
            self.emit_db_helpers();
        }
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
