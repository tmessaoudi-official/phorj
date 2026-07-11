//! Program pass — entry walk: whole-program check, tests, import collisions, static inits.

use super::*;

/// Walk a field initializer (Feature B) for a read of a **not-yet-initialized** field — returns the
/// first forbidden name reached via `this.X` or a bare `X`. Lambda bodies are skipped: a lambda that
/// touches `this` is independently rejected (`E-LAMBDA-THIS`), so a closure default cannot smuggle in
/// a forward reference. The set is the fields that are *not* available when this initializer runs.
pub(in crate::checker) fn field_init_forbidden_ref(
    e: &crate::ast::Expr,
    forbidden: &std::collections::HashSet<String>,
) -> Option<String> {
    use crate::ast::{Expr, StrPart};
    pub(in crate::checker) fn walk(
        e: &Expr,
        f: &std::collections::HashSet<String>,
        out: &mut Option<String>,
    ) {
        if out.is_some() {
            return;
        }
        match e {
            Expr::Ident(n, _) if f.contains(n) => *out = Some(n.clone()),
            Expr::Member { object, name, .. } => {
                if matches!(&**object, Expr::This(_)) && f.contains(name) {
                    *out = Some(name.clone());
                } else {
                    walk(object, f, out);
                }
            }
            Expr::Str(parts, _) | Expr::Html(parts, _) => {
                for p in parts {
                    if let StrPart::Expr(x) = p {
                        walk(x, f, out);
                    }
                }
            }
            Expr::List(xs, _) => xs.iter().for_each(|x| walk(x, f, out)),
            Expr::Map(ps, _) => ps.iter().for_each(|(k, v)| {
                walk(k, f, out);
                walk(v, f, out);
            }),
            Expr::Unary { expr, .. } => walk(expr, f, out),
            Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => walk(inner, f, out),
            Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, f, out);
                walk(rhs, f, out);
            }
            Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => walk(value, f, out),
            Expr::Call { callee, args, .. } => {
                walk(callee, f, out);
                args.iter().for_each(|a| walk(a, f, out));
            }
            Expr::Index { object, index, .. } => {
                walk(object, f, out);
                walk(index, f, out);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                walk(scrutinee, f, out);
                arms.iter().for_each(|a| walk(&a.body, f, out));
            }
            Expr::Range { start, end, .. } => {
                walk(start, f, out);
                walk(end, f, out);
            }
            Expr::If {
                cond,
                then_expr,
                else_expr,
                ..
            } => {
                walk(cond, f, out);
                walk(then_expr, f, out);
                walk(else_expr, f, out);
            }
            Expr::CloneWith { object, fields, .. } => {
                walk(object, f, out);
                fields.iter().for_each(|(_, v)| walk(v, f, out));
            }
            // Literals / `this` / `Lambda` (its `this`-use is `E-LAMBDA-THIS`) read no forbidden field.
            _ => {}
        }
    }
    let mut out = None;
    walk(e, forbidden, &mut out);
    out
}

impl Checker {
    /// Phase 2 — check every function/method body.
    pub(in crate::checker) fn check_program(&mut self, program: &Program) {
        use crate::ast::Item;
        // Reshape slice 2a: identifier casing is a hard, front-end-only rule. Run it first so its
        // diagnostics surface regardless of body-level errors (it is purely declaration-shaped).
        self.check_casing(program);
        // M5 S1: every file is packaged, never inferred. empty ⇒ no declaration; a `core` root is
        // reserved for the standard library. (Strict folder=path and loose-mode `main`-only land
        // with the project model in S2 — `docs/specs/2026-06-18-m5-project-model-design.md`.)
        if program.package.is_empty() {
            self.err_coded(
                program.span,
                "every file must declare a package (e.g. `package Main;`) as its first line",
                "E-NO-PACKAGE",
                Some("add `package Main;` at the top of the file".into()),
            );
        } else if program.package[0] == "Core" {
            self.err_coded(
                program.span,
                "`Core` is a reserved package root (the standard library)",
                "E-RESERVED-PACKAGE",
                Some("use a different root, e.g. `package App;`".into()),
            );
        }
        // Reshape slice 2b: package + import path segments are PascalCase (`E-PKG-CASE`) — a 1:1
        // mapping to PHP namespaces with no casing transform. Front-end-only, so it cannot affect
        // byte-identity (every backend sees the same AST; the rule only gates which programs reach
        // them). The reserved `Main`/`Core` roots are already PascalCase. An empty package is left to
        // `E-NO-PACKAGE` above (the loop is empty), so the two never double-report.
        for seg in &program.package {
            if !is_pascal(seg) {
                self.err_coded(
                    program.span,
                    format!("package segment `{seg}` must be PascalCase"),
                    "E-PKG-CASE",
                    Some(format!("did you mean `package {}`?", to_pascal(seg))),
                );
            }
        }
        for item in &program.items {
            if let Item::Import {
                path, alias, span, ..
            } = item
            {
                // Carve-out for member imports naming a VALUE (a function or a fault intrinsic):
                // `import Core.Output.printLine;` / `import Core.Abort.panic;` deliberately end in a
                // camelCase leaf — the value's name — so the LEAF is exempt from the PascalCase segment
                // rule (DEC-196 intrinsics + DEC-197 module functions). Prefix segments are still checked
                // (`Core`/`Output` are PascalCase). Leaf validity (is it a real function/intrinsic of that
                // module?) is enforced by `resolve_function_imports`/`resolve_intrinsic_imports`, not here.
                // A type/variant member import keeps a PascalCase leaf and is checked as usual.
                let member_value_leaf = path.len() >= 3 && path.last().is_some_and(|l| is_camel(l));
                let last = path.len().saturating_sub(1);
                for (i, seg) in path.iter().enumerate() {
                    if member_value_leaf && i == last {
                        continue;
                    }
                    if !is_pascal(seg) {
                        self.err_coded(
                            *span,
                            format!("import segment `{seg}` must be PascalCase"),
                            "E-PKG-CASE",
                            Some(format!("did you mean `{}`?", to_pascal(seg))),
                        );
                    }
                }
                // An alias renames the call-site name (`import A.B as C;`). For a value-leaf import the
                // alias is a value identifier (camelCase, like the function it renames — DEC-197
                // `import Core.List.map as listMap;`); otherwise it occupies a package-qualifier position
                // and follows the same PascalCase rule as the segments.
                if let Some(a) = alias {
                    if member_value_leaf {
                        if !is_camel(a) {
                            self.err_coded(
                                *span,
                                format!(
                                    "import alias `{a}` must be camelCase (it renames a function)"
                                ),
                                "E-NAME-CASE",
                                Some(format!("did you mean `as {}`?", to_camel(a))),
                            );
                        }
                    } else if !is_pascal(a) {
                        self.err_coded(
                            *span,
                            format!("import alias `{a}` must be PascalCase"),
                            "E-PKG-CASE",
                            Some(format!("did you mean `as {}`?", to_pascal(a))),
                        );
                    }
                }
            }
        }
        self.check_variant_import_collisions(program);
        self.check_function_import_collisions(program);
        // Feature B-static: type-check every static field's (now arbitrary) initializer, after all
        // classes + functions are collected, with no `this` — so an initializer may call a function or
        // read another static.
        self.check_static_inits(program);
        // Batch-1 D: an entry point may be a top-level `function main` OR a class-static `main` method,
        // but never more than one — an ambiguous entry is an error, never a silent pick.
        if crate::ast::entry_point_count(program, "main") > 1 {
            // Report at every entry after the first, so each duplicate is flagged.
            let mut seen = false;
            for item in &program.items {
                let dup_span = match item {
                    Item::Function(f) if f.name == "main" => Some(f.span),
                    Item::Class(c) => c.members.iter().find_map(|m| match m {
                        crate::ast::ClassMember::Method(f)
                            if f.name == "main"
                                && f.modifiers.contains(&crate::ast::Modifier::Static) =>
                        {
                            Some(f.span)
                        }
                        _ => None,
                    }),
                    _ => None,
                };
                if let Some(span) = dup_span {
                    if seen {
                        self.err_coded(
                            span,
                            "multiple program entry points named `main`",
                            "E-MULTIPLE-MAIN",
                            Some(
                                "a program has at most one entry: either a top-level `function main` \
                                 or a single class `static function main` — remove the extras"
                                    .into(),
                            ),
                        );
                    }
                    seen = true;
                }
            }
        }
        for item in &program.items {
            match item {
                Item::Function(f) => self.check_function(f),
                // M8.5: a foreign `declare class` has only bodyless member signatures (its bodies live
                // in PHP) — skip body/definite-assignment/totality validation. It is still registered for
                // member-call resolution by the collect pass, so `new Name(…)` / `o.m(…)` type-check.
                Item::Class(c) if c.foreign => {}
                Item::Class(c) => {
                    self.check_class_attributes(c);
                    self.check_type_body(&c.name, &c.type_params, &c.members);
                }
                // M-RT S8: a trait's method/ctor/hook bodies are checked once, in trait context
                // (correct spans, no double-reporting), with the trait's own collected members as
                // `this`. A trait has no type parameters this slice.
                Item::Trait(t) => self.check_type_body(&t.name, &[], &t.members),
                // M-Test: a `test "name" { … }` block is checked like a `-> void` body with no `this`,
                // but only under `phg test`; in a normal build it is rejected (production code cannot
                // smuggle test blocks).
                Item::Test { name, body, span } => self.check_test(name, body, *span),
                // Interface method signatures have no body to check (the conformance/graph
                // validation ran in `collect`); enums/imports/aliases have nothing here.
                Item::Enum(_)
                | Item::Interface(_)
                | Item::Import { .. }
                | Item::TypeAlias { .. } => {}
            }
        }
    }

    /// Type-check one `test "name" { … }` item (M-Test). Outside test mode it is an error so test
    /// blocks cannot appear in production code. In test mode the body is checked like a `-> void`
    /// function body — fresh scope, no parameters, no `this`, no return value expected.
    pub(in crate::checker) fn check_test(
        &mut self,
        _name: &str,
        body: &[crate::ast::Stmt],
        span: crate::token::Span,
    ) {
        if !self.test_mode {
            self.err_coded(
                span,
                "a `test` block is only allowed in a test file run by `phg test`",
                "E-TEST-OUTSIDE-TESTS",
                Some("move this into a `*.phg` under `tests/` and run `phg test`".into()),
            );
            return;
        }
        let prev_ret = std::mem::replace(&mut self.cur_ret, Ty::Void);
        let prev_class = self.cur_class.take();
        self.check_block(body);
        self.cur_ret = prev_ret;
        self.cur_class = prev_class;
    }

    /// Feature B-static: type-check each class's static-field initializers (now arbitrary expressions,
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
            let Item::Import { path, alias, span } = item else {
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

    /// not just literals), evaluated once at program start. Checked with **no `this`** (statics are
    /// class-level — referencing `this` errors) and after full collection, so an initializer may call a
    /// function or read another static. A type mismatch is `E-STATIC-INIT-TYPE`.
    pub(in crate::checker) fn check_static_inits(&mut self, program: &crate::ast::Program) {
        use crate::ast::{ClassMember, Item, Modifier};
        let prev = self.cur_class.take();
        // A static initializer runs in its owning class's scope (so it may call that class's
        // `private`/`protected` constructor — the singleton pattern), but there is no instance, so
        // `this` is forbidden via `in_static_init` (Batch A).
        self.in_static_init = true;
        for item in &program.items {
            let Item::Class(c) = item else { continue };
            self.cur_class = Some(c.name.clone());
            for m in &c.members {
                if let ClassMember::Field {
                    modifiers,
                    ty,
                    name,
                    init: Some(e),
                    ..
                } = m
                {
                    if modifiers.contains(&Modifier::Static)
                        && !modifiers.contains(&Modifier::Const)
                    {
                        let fty = self.resolve_type(ty);
                        let ity = self.check_expr(e);
                        if !self.ty_assignable(&ity, &fty) {
                            self.err_coded(
                                Self::expr_span(e),
                                format!("static field `{name}: {fty}` initialized with `{ity}`"),
                                "E-STATIC-INIT-TYPE",
                                None,
                            );
                        }
                    }
                }
            }
        }
        self.in_static_init = false;
        self.cur_class = prev;
    }
}
