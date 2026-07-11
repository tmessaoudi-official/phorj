//! Collection pass — entry: name prebinding, top-level collect walk, alias-cycle detection,
//! reserved-symbol screening.

use super::*;

/// PHP-erasure key of a type: types PHP cannot distinguish at runtime share a key. `string`/`bytes`
/// both erase to PHP `string`; `List`/`Map`/`Set` all to PHP `array`; an `Optional<T>` keys by its
/// inner type. Everything else keys by its own `Display` (distinct), so only genuinely PHP-ambiguous
/// pairs collide. Used by `validate_new_overload` to reject overloads the transpiler can't dispatch.
pub(in crate::checker) fn php_erasure_key(t: &Ty) -> String {
    match t {
        Ty::String | Ty::Bytes => "php:string".to_string(),
        Ty::List(_) | Ty::Map(..) | Ty::Set(_) => "php:array".to_string(),
        Ty::Optional(inner) => format!("php:?{}", php_erasure_key(inner)),
        other => format!("ty:{other}"),
    }
}

/// Two overload parameter lists are *PHP-erasure-alike* when they have the same arity and every
/// position shares a [`php_erasure_key`] — transpiled PHP cannot tell them apart — yet they are not
/// literally identical (that case is `E-OVERLOAD-DUPLICATE`). Such a pair is `E-OVERLOAD-ERASE`.
pub(in crate::checker) fn overloads_erase_alike(a: &[Ty], b: &[Ty]) -> bool {
    a.len() == b.len()
        && a != b
        && a.iter()
            .zip(b)
            .all(|(x, y)| php_erasure_key(x) == php_erasure_key(y))
}

impl Checker {
    /// Phase 1 — hoist all top-level declarations and the active import map. There is no longer a
    /// builtin prelude: every callable is namespaced ("nothing in the wind"), so even `println` must
    /// be reached as `console.println` after `import core.console;` (M3 Wave 1). A bare `println(…)`
    /// now resolves as an unknown function.
    /// Name-binding pre-pass: register every top-level type's name (+ generic arity) as a placeholder
    /// before any member type is resolved, so a type reference is **order-independent** — a forward
    /// reference within a file and a cross-file reference (a sibling merged earlier by the loader's
    /// alphabetical sort) both resolve. Duplicate detection lives here (order-independent); the per-item
    /// collectors then treat a prebound name as "fill my placeholder", not a duplicate. Built-in-named
    /// types are skipped (the per-item collector emits `cannot redefine built-in type`).
    pub(in crate::checker) fn prebind_types(&mut self, program: &Program) {
        use crate::ast::Item;
        for item in &program.items {
            let (name, type_params, span): (&str, &[String], crate::token::Span) = match item {
                Item::Class(c) => (&c.name, &c.type_params, c.span),
                Item::Enum(e) => (&e.name, &e.type_params, e.span),
                Item::Interface(i) => (&i.name, &[][..], i.span),
                Item::Trait(t) => (&t.name, &[][..], t.span),
                _ => continue,
            };
            if is_builtin_type_name(name) {
                continue; // the per-item collector reports `cannot redefine built-in type`
            }
            if !self.prebound.insert(name.to_string()) {
                self.err_coded(
                    span,
                    format!("type `{name}` is already defined"),
                    "E-DUP-TYPE",
                    Some("rename one declaration — a class/enum/interface/trait/type name must be unique".into()),
                );
                continue;
            }
            match item {
                Item::Enum(_) => {
                    self.enums.insert(
                        name.to_string(),
                        EnumInfo::placeholder(type_params.to_vec()),
                    );
                }
                Item::Interface(_) => {
                    self.interfaces
                        .insert(name.to_string(), InterfaceInfo::placeholder());
                }
                // A class or a trait (a trait reuses the class machinery, keyed by its name).
                _ => {
                    self.classes.insert(
                        name.to_string(),
                        ClassInfo::placeholder(type_params.to_vec()),
                    );
                    if matches!(item, Item::Trait(_)) {
                        self.traits.insert(name.to_string());
                    }
                }
            }
        }
    }

    pub(in crate::checker) fn collect(&mut self, program: &Program) {
        use crate::ast::Item;
        self.imports = crate::native::import_map(&program.items);
        // DEC-197: bare-name → (module, native) map for member-imported module functions. Last-write
        // wins on a duplicate bound name; that duplicate is separately reported as `E-IMPORT-CONFLICT`
        // (`check_function_import_collisions`), which stops compilation — so the arbitrary pick is never
        // observed. Empty unless the program member-imports a stdlib function.
        self.fn_imports = super::function_imports::function_import_bindings(&program.items)
            .into_iter()
            .map(|(bound, module, real, _)| (bound, (module, real)))
            .collect();
        // Pre-bind all type names so member/annotation resolution is order-independent (forward +
        // cross-file references). Must run before the per-item collect loop resolves any type.
        self.prebind_types(program);
        // M-RT super/parent: cache the inheritance tables once for `parent`-call resolution (the same
        // tables `ast::resolve_parent_method` threads to both backends).
        self.parent_parents = crate::ast::class_parents(program);
        self.parent_mro = crate::ast::class_mro(program);
        self.parent_origins = crate::ast::class_method_origins(program).0;
        for item in &program.items {
            // PHP reserves a set of words in symbol positions (a free function / class / enum /
            // interface / trait / type-alias). Several are usable Phorj value identifiers (param /
            // field / local / property / method — e.g. `var`, `list`, `print`, `int`) but naming a
            // *symbol* with one would transpile to invalid PHP, so reject it here with a clear
            // diagnostic (kind-aware — `int` is a legal PHP function name but not a class name) rather
            // than letting the PHP oracle fail. Methods are collected inside `collect_class` (a class
            // body), so they are exempt (legal as `->list()` / `->var()`).
            if let Some((name, span, kind)) = reserved_symbol_decl(item) {
                if is_php_reserved_symbol_name(name, kind) {
                    self.err_coded(
                        span,
                        format!("`{name}` is a reserved word in PHP and cannot name a {kind}"),
                        "E-RESERVED-NAME",
                        Some(format!(
                            "`{name}` is fine as a variable, parameter, field, or method name — rename this {kind}"
                        )),
                    );
                    continue;
                }
            }
            match item {
                Item::Function(f) => self.collect_function(f),
                Item::Enum(e) => self.collect_enum(e),
                Item::Class(c) => self.collect_class(c),
                Item::Interface(i) => self.collect_interface(i),
                Item::Trait(t) => self.collect_trait(t),
                Item::Import { .. } => {} // import map already built above; nothing per-item to hoist
                Item::TypeAlias { name, ty, span } => {
                    if is_builtin_type_name(name) {
                        // Aliasing a built-in would make the checker (primitive wins) and the
                        // backend expansion (alias wins) disagree — reject it outright.
                        self.err(*span, format!("cannot redefine built-in type `{name}`"));
                    } else if self.aliases.contains_key(name) {
                        self.err(*span, format!("duplicate type name `{name}`"));
                    } else {
                        self.aliases.insert(name.clone(), ty.clone());
                    }
                }
                // M-Test: a `test` item declares no top-level symbol; nothing to hoist here. Its body
                // is checked (test mode) / rejected (normal build) in `check_program`.
                Item::Test { .. } => {}
            }
        }
        // Interfaces are fully registered now: validate the extends graph + every class's
        // `implements` (cycles, unknown names, method conformance) and build the shared
        // class→interface table the backends consume verbatim (M-RT S2).
        self.check_interface_graph(program);
        // W0-4: every alias is registered now — walk the alias→alias graph eagerly so a cycle that
        // is never *used* (never reaches `resolve_type`) is still rejected. Coded + deduped with the
        // resolve-time detection via `alias_cycle_reported`.
        self.check_alias_cycles(program);
    }

    /// W0-4: detect `type` alias cycles by a graph walk over alias→alias edges, independent of whether
    /// the alias is ever used. An unused cycle (`type A = B; type B = A;`) never reaches `resolve_type`,
    /// so this collect-time pass is the only thing that catches it. One `E-ALIAS-CYCLE` per cycle,
    /// deduped against the resolve-time use-site path through `alias_cycle_reported`.
    pub(in crate::checker) fn check_alias_cycles(&mut self, program: &Program) {
        use crate::ast::Item;
        for item in &program.items {
            let Item::TypeAlias { name, span, .. } = item else {
                continue;
            };
            if self.alias_cycle_reported.contains(name) {
                continue;
            }
            // DFS from `name` over alias→alias edges; a return to `name` is a cycle. `path` is the
            // route back to `name` when found, so the message can name the whole loop.
            let mut path: Vec<String> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            if self.alias_reaches_self(name, name, &mut path, &mut seen) {
                let mut cycle = path;
                cycle.push(name.clone());
                self.report_alias_cycle(&cycle, *span);
            }
        }
    }

    /// DFS helper for [`Self::check_alias_cycles`]: does following alias edges from `current` lead back
    /// to `target`? `path` accumulates the alias names walked (for the diagnostic); `seen` bounds the
    /// walk on any (possibly unrelated) sub-cycle so it always terminates.
    pub(in crate::checker) fn alias_reaches_self(
        &self,
        target: &str,
        current: &str,
        path: &mut Vec<String>,
        seen: &mut std::collections::HashSet<String>,
    ) -> bool {
        let Some(ty) = self.aliases.get(current) else {
            return false;
        };
        let mut refs: Vec<String> = Vec::new();
        collect_alias_refs(ty, &self.aliases, &mut refs);
        for r in refs {
            if r == target {
                return true;
            }
            if seen.insert(r.clone()) {
                path.push(r.clone());
                if self.alias_reaches_self(target, &r, path, seen) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    /// Emit one `E-ALIAS-CYCLE` for a detected cycle, deduped: if any name in the cycle was already
    /// reported (by the other detection path), stay silent. Marks every member reported (W0-4).
    pub(in crate::checker) fn report_alias_cycle(&mut self, cycle: &[String], span: Span) {
        if cycle.iter().any(|n| self.alias_cycle_reported.contains(n)) {
            return;
        }
        for n in cycle {
            self.alias_cycle_reported.insert(n.clone());
        }
        self.err_coded(
            span,
            format!("type alias cycle: {}", cycle.join(" → ")),
            "E-ALIAS-CYCLE",
            Some(
                "break the cycle so every alias bottoms out at a built-in, class, or enum type"
                    .into(),
            ),
        );
    }
}

/// Collect the alias-name references inside a type annotation (W0-4). Walks the `Type` structure and
/// pushes every `Named` head that is itself a registered alias — the edges of the alias→alias graph
/// used by `check_alias_cycles`. Non-alias names (classes, primitives, enums) and generic args that
/// aren't aliases are ignored; nested composite types (optional, union, intersection, function,
/// fixed-list) are recursed so `type A = List<B>?` still records the edge to `B`.
pub(in crate::checker) fn collect_alias_refs(
    ty: &crate::ast::Type,
    aliases: &HashMap<String, crate::ast::Type>,
    out: &mut Vec<String>,
) {
    use crate::ast::Type;
    match ty {
        Type::Named { name, args, .. } => {
            if aliases.contains_key(name) {
                out.push(name.clone());
            }
            for a in args {
                collect_alias_refs(a, aliases, out);
            }
        }
        Type::Optional { inner, .. } => collect_alias_refs(inner, aliases, out),
        Type::Union(members, _) | Type::Intersection(members, _) => {
            for m in members {
                collect_alias_refs(m, aliases, out);
            }
        }
        Type::Function { params, ret, .. } => {
            for p in params {
                collect_alias_refs(p, aliases, out);
            }
            collect_alias_refs(ret, aliases, out);
        }
        Type::FixedList { elem, .. } => collect_alias_refs(elem, aliases, out),
        Type::Infer(_) | Type::Erased(_) => {}
    }
}

/// The name, span, and human label of a top-level item that defines a *symbol* in PHP's namespace
/// (free function / class / enum / interface / trait / type-alias) — the positions where a
/// PHP-reserved name (e.g. `var`) would transpile to invalid PHP. `None` for imports.
pub(in crate::checker) fn reserved_symbol_decl(
    item: &crate::ast::Item,
) -> Option<(&str, Span, &'static str)> {
    use crate::ast::Item;
    match item {
        Item::Function(f) => Some((&f.name, f.span, "function")),
        Item::Class(c) => Some((&c.name, c.span, "class")),
        Item::Enum(e) => Some((&e.name, e.span, "enum")),
        Item::Interface(i) => Some((&i.name, i.span, "interface")),
        Item::Trait(t) => Some((&t.name, t.span, "trait")),
        Item::TypeAlias { name, span, .. } => Some((name, *span, "type alias")),
        Item::Import { .. } => None,
        // A `test` name is a string label, not a PHP symbol — no reserved-name concern.
        Item::Test { .. } => None,
    }
}

/// The type of a *literal constant* expression, or `None` if `e` is not a literal (M4 default
/// parameters: a default value must be a literal, so its type is determined without invoking the full
/// checker). A string literal qualifies only when it has no interpolation holes (all `Literal` parts).
pub(in crate::checker) fn literal_ty(e: &crate::ast::Expr) -> Option<Ty> {
    use crate::ast::{Expr, StrPart};
    match e {
        Expr::Int(..) => Some(Ty::Int),
        Expr::Float(..) => Some(Ty::Float),
        Expr::Decimal { .. } => Some(Ty::Decimal),
        Expr::Bool(..) => Some(Ty::Bool),
        Expr::Bytes(..) => Some(Ty::Bytes),
        Expr::Null(_) => Some(Ty::Null),
        Expr::Str(parts, _) if parts.iter().all(|p| matches!(p, StrPart::Literal(_))) => {
            Some(Ty::String)
        }
        _ => None,
    }
}
