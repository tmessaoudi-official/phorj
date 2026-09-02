//! Program pass — entry walk: whole-program check, tests, static field-init forward-ref guard.

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
                Some(
                    "add `package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;` at the top of the file".into(),
                ),
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
        self.check_no_surviving_wildcard_imports(program);
        self.check_variant_import_collisions(program);
        self.check_function_import_collisions(program);
        // Feature B-static: type-check every static field's (now arbitrary) initializer, after all
        // classes + functions are collected, with no `this` — so an initializer may call a function or
        // read another static.
        self.check_static_inits(program);
        // DEC-331 D1 / DEC-337: entries are declared by `#[Entry(kind: EntryKind.Cli|Web)]` — validate
        // every attributed candidate (target/kind/signature/duplicate). See `check_entry_points`.
        self.check_entry_points(program);
        for item in &program.items {
            match item {
                Item::Function(f) => self.check_function(f),
                // M8.5: a foreign `declare class` has only bodyless member signatures (its bodies live
                // in PHP) — skip body/definite-assignment/totality validation. It is still registered for
                // member-call resolution by the collect pass, so `new Name(…)` / `o.m(…)` type-check.
                Item::Class(c) if c.foreign => {}
                Item::Class(c) => {
                    self.check_class_attributes(c);
                    self.check_invoke_tostring_class(c); // DEC-331 D9 uniqueness
                    self.check_type_body(&c.name, &c.type_params, &c.type_param_bounds, &c.members);
                }
                // M-RT S8: a trait's method/ctor/hook bodies are checked once, in trait context
                // (correct spans, no double-reporting), with the trait's own collected members as
                // `this`. A trait has no type parameters this slice.
                Item::Trait(t) => {
                    // DEC-331 D9: enforce uniqueness on the trait too (it flattens into using classes).
                    self.check_invoke_tostring_members(&t.name, &t.members, t.span);
                    self.check_type_body(&t.name, &[], &[], &t.members);
                }
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
                "a `test` block cannot be run, transpiled or built — it is executed by `phg test`",
                "E-TEST-OUTSIDE-TESTS",
                Some(
                    "run it with `phg test <file>` (`phg check` and the editors accept `test` items)"
                        .into(),
                ),
            );
            return;
        }
        let prev_ret = std::mem::replace(&mut self.cur_ret, Ty::Void);
        let prev_class = self.cur_class.take();
        self.check_block(body);
        self.cur_ret = prev_ret;
        self.cur_class = prev_class;
    }
}
