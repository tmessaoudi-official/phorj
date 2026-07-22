//! `impl Checker` — matches cluster (M-Decomp W2). See checker/mod.rs for the struct + entry points.

use super::*;

impl Checker {
    pub(super) fn check_match(
        &mut self,
        scrutinee: &crate::ast::Expr,
        arms: &[crate::ast::MatchArm],
        span: Span,
    ) -> Ty {
        use crate::ast::Pattern;
        let scrut = self.check_expr(scrutinee);

        let mut result: Option<Ty> = None;
        let mut covered: Vec<String> = Vec::new();
        // Type-pattern coverage for match-over-union exhaustiveness (M-RT S4): the class/interface
        // names matched by `Circle c =>` arms.
        let mut covered_types: Vec<String> = Vec::new();
        let mut has_catch_all = false;
        // After a `null` arm, a later catch-all binding over a `T?` scrutinee sees the non-null
        // inner (`match opt { null => …, v => … }` binds `v: T`, M3 S2.6). Tracks null coverage.
        let mut null_seen = false;
        // Reachability lints (W-MATCH-UNREACHABLE): an arm after a catch-all, or a duplicate
        // literal/variant/type arm, can never match. `catch_all_seen`/`seen_keys` reflect *prior*
        // arms only (updated at the end of each iteration), so the offending arm — not the first
        // good one — is flagged.
        let mut catch_all_seen = false;
        let mut seen_keys: Vec<String> = Vec::new();
        // Shapes (variant/type names) that appear on a guarded arm but never on an unguarded one —
        // used to attach the `E-MATCH-GUARD-EXHAUST` hint when a guard leaves a shape undischarged.
        let mut guarded_shapes: Vec<String> = Vec::new();

        for arm in arms {
            // A guarded arm (`pat when cond =>`) may fall through, so it does NOT discharge its
            // shape for exhaustiveness, become a catch-all, or count as a duplicate/unreachable.
            let unguarded = arm.guard.is_none();
            if catch_all_seen {
                self.warn_coded(
                    arm.span,
                    "unreachable match arm: a previous arm already matches everything",
                    "W-MATCH-UNREACHABLE",
                    Some(
                        "a `default` or bare lowercase-identifier arm is a catch-all; arms after it never run"
                            .into(),
                    ),
                );
            } else if unguarded {
                if let Some(key) = match_arm_key(&arm.pattern) {
                    if seen_keys.contains(&key) {
                        self.warn_coded(
                            arm.span,
                            "unreachable match arm: a previous arm already covers this pattern",
                            "W-MATCH-UNREACHABLE",
                            Some("remove the duplicate arm".into()),
                        );
                    } else {
                        seen_keys.push(key);
                    }
                }
            }
            if unguarded && matches!(arm.pattern, Pattern::Wildcard(_) | Pattern::Binding { .. }) {
                has_catch_all = true;
                catch_all_seen = true;
            }
            if let Pattern::Variant { name, fields, .. } = &arm.pattern {
                // A variant arm discharges its variant's coverage only when its payload is
                // *irrefutable* (every field is a wildcard/binding). A refutable payload — a nested
                // type/struct/literal pattern (S5.2-T2) that can fail to match a value of the payload
                // type — may fall through, so `Wrapper(Circle c)` alone does NOT cover `Wrapper`; an
                // unguarded `Wrapper(_)`/`_` fallback is still required (mirrors the guard rule, and
                // closes the pre-existing literal-payload soundness gap, e.g. `Some(0)` alone).
                if unguarded && fields.iter().all(is_irrefutable) {
                    covered.push(name.clone());
                } else if !unguarded {
                    guarded_shapes.push(name.clone());
                }
                // else: unguarded but a refutable payload — neither covered nor a guarded shape; a
                // plain "non-exhaustive: missing <variant>" fires unless a fallback also covers it.
            }
            // A type pattern (`Circle c`) and a struct pattern (`Circle { r }`) are both `instanceof`
            // tests over a union member, so both discharge that member's coverage (S5.2).
            if let Pattern::Type { type_name, .. } | Pattern::Struct { type_name, .. } =
                &arm.pattern
            {
                if unguarded {
                    covered_types.push(type_name.clone());
                } else {
                    guarded_shapes.push(type_name.clone());
                }
            }
            // The type a catch-all binding sees: narrowed to the inner `T` when a preceding `null`
            // arm already handled absence; otherwise the scrutinee type unchanged.
            let arm_scrut = match (&scrut, null_seen) {
                (Ty::Optional(inner), true) => (**inner).clone(),
                _ => scrut.clone(),
            };
            // each arm gets its own scope for pattern bindings
            self.push_scope();
            self.check_pattern(&arm.pattern, &arm_scrut);
            // An arm guard is typed in the arm's scope (its pattern bindings are visible) and must
            // be boolean. A guard never changes the arm's result type.
            if let Some(g) = &arm.guard {
                let gt = self.check_expr(g);
                if !matches!(gt, Ty::Bool | Ty::Error) {
                    self.err_coded(
                        Self::expr_span(g),
                        format!("match guard must be `bool`, found `{gt}`"),
                        "E-GUARD-TYPE",
                        Some("a `when` guard is a boolean condition".into()),
                    );
                }
            }
            let body_ty = self.check_expr(&arm.body);
            self.pop_scope();
            if unguarded && matches!(arm.pattern, Pattern::Null(_)) {
                null_seen = true;
            }

            match &result {
                None => result = Some(body_ty),
                Some(first) => {
                    if !self.ty_assignable(&body_ty, first) && !self.ty_assignable(first, &body_ty)
                    {
                        self.err(
                            span,
                            format!(
                                "match arms must share one type; found `{first}` and `{body_ty}`"
                            ),
                        );
                    }
                }
            }
        }

        // exhaustiveness
        if !has_catch_all {
            match &scrut {
                Ty::Named(enum_name, _) if self.enums.contains_key(enum_name) => {
                    let all: Vec<String> = self.enums[enum_name].variants.keys().cloned().collect();
                    let mut missing: Vec<String> =
                        all.into_iter().filter(|v| !covered.contains(v)).collect();
                    // `variants` is a HashMap, so `keys()` order is nondeterministic — sort the
                    // missing list so the error message is stable across runs (otherwise it's an
                    // intermittent test/diff hazard).
                    missing.sort();
                    if !missing.is_empty() {
                        if missing.iter().any(|m| guarded_shapes.contains(m)) {
                            self.err_coded(
                                span,
                                format!(
                                    "non-exhaustive match: missing {} (covered only by guarded arms)",
                                    missing.join(", ")
                                ),
                                "E-MATCH-GUARD-EXHAUST",
                                Some(
                                    "a `when`-guarded arm may fall through; add an unguarded arm for that shape"
                                        .into(),
                                ),
                            );
                        } else {
                            self.err(
                                span,
                                format!("non-exhaustive match: missing {}", missing.join(", ")),
                            );
                        }
                    }
                }
                // W5-3 sealed hierarchies (Wave A): a scrutinee of a `sealed` class/interface is a
                // CLOSED set — its permitted subtypes are exactly the whole-program concrete classes
                // that are subtypes of it. So `match` over the sealed base is exhaustive without a `_`,
                // reusing the union coverage rule (each subtype covered by a type-pattern naming it or a
                // covering supertype). `sealed` erases before any backend — the arms lower to the same
                // `instanceof` chain a written-out union match does, so this is purely a compile-time
                // exhaustiveness gate (byte-identity by construction).
                Ty::Named(base, _) if self.sealed_types.contains(base) => {
                    let members = self.sealed_permitted_subtypes(base);
                    self.report_union_nonexhaustive(
                        &members,
                        &covered_types,
                        &guarded_shapes,
                        span,
                    );
                }
                // Match-over-union exhaustiveness (M-RT S4 + Wave A): every nominal member must be
                // covered by a type pattern naming it OR a covering supertype/interface; a
                // DISCRIMINABLE primitive member (int/float/string/bool/null) is covered by a primitive
                // type pattern naming it. A non-discriminable member (decimal/bytes/html/attr — erases
                // to a PHP string) still can't be type-matched, so it always needs a `_`.
                Ty::Union(members) => {
                    self.report_union_nonexhaustive(members, &covered_types, &guarded_shapes, span);
                }
                // Wave A slice 2b (DEC-183): a flat exhaustive `match` over `T?`. `Optional<T>` is
                // `T | null`, so the member arms plus a `null` arm discharge it — no `_` needed
                // (`int?`, `Circle?`, `(A | B)?`). Reuses the union coverage over `T`'s members plus a
                // synthetic `null` member (covered by a `null` arm — tracked in `null_seen` — or a
                // `null` type-pattern in `covered_types`). Byte-identity holds: the arms lower to
                // `is_int`/`is_string`/`=== null`, pattern-driven and scrutinee-type-agnostic on every
                // backend. Caveat (ruled): an `Optional<enum>` (`Color?`) still needs a `_` — enum
                // variant coverage isn't threaded through `?` yet (separate follow-up).
                Ty::Optional(inner) => {
                    // DEC-250: `Optional<enum>` (`Color?`) threads VARIANT coverage through the
                    // `?` — every variant of the inner enum plus a `null` arm discharge it, no `_`
                    // needed (the DEC-183 caveat closed: "exhaustive matching is a flagship").
                    if let Ty::Named(enum_name, _) = &**inner {
                        if self.enums.contains_key(enum_name) {
                            let all: Vec<String> =
                                self.enums[enum_name].variants.keys().cloned().collect();
                            let mut missing: Vec<String> =
                                all.into_iter().filter(|v| !covered.contains(v)).collect();
                            if !null_seen {
                                missing.push("`null`".to_string());
                            }
                            missing.sort();
                            if !missing.is_empty() {
                                self.err(
                                    span,
                                    format!("non-exhaustive match: missing {}", missing.join(", ")),
                                );
                            }
                            return result.unwrap_or(Ty::Error);
                        }
                    }
                    let mut members: Vec<Ty> = match &**inner {
                        Ty::Union(ms) => ms.clone(),
                        other => vec![other.clone()],
                    };
                    members.push(Ty::Null);
                    let mut ct = covered_types.clone();
                    if null_seen {
                        ct.push("null".to_string());
                    }
                    self.report_union_nonexhaustive(&members, &ct, &guarded_shapes, span);
                }
                Ty::Error => {}
                _ => {
                    self.err(
                        span,
                        "non-exhaustive match: add a `_` wildcard arm for non-enum scrutinees",
                    );
                }
            }
        }

        result.unwrap_or(Ty::Error)
    }

    /// Emit the non-exhaustiveness diagnostic for a union-shaped scrutinee: which discriminable
    /// members (a primitive type, or a class/interface reached by a covering supertype) are left
    /// uncovered by the arms. A member reachable only through a `when`-guarded arm upgrades the
    /// message to `E-MATCH-GUARD-EXHAUST`. Shared by the `Ty::Union` and `Ty::Optional` scrutinee
    /// arms (Wave A) — the `Optional` caller appends a synthetic `null` member.
    fn report_union_nonexhaustive(
        &mut self,
        members: &[Ty],
        covered_types: &[String],
        guarded_shapes: &[String],
        span: Span,
    ) {
        let mut missing: Vec<String> = members
            .iter()
            .filter(|m| match m {
                Ty::Named(n, _) => !covered_types
                    .iter()
                    .any(|t| t == n || self.is_subtype(n, t)),
                _ => match prim_ty_name(m) {
                    Some(name) => !covered_types.iter().any(|t| t == name),
                    None => true,
                },
            })
            .map(std::string::ToString::to_string)
            .collect();
        missing.sort();
        if missing.is_empty() {
            return;
        }
        if missing.iter().any(|m| guarded_shapes.contains(m)) {
            self.err_coded(
                span,
                format!(
                    "non-exhaustive match: missing {} (covered only by guarded arms)",
                    missing.join(", ")
                ),
                "E-MATCH-GUARD-EXHAUST",
                Some(
                    "a `when`-guarded arm may fall through; add an unguarded arm for that shape"
                        .into(),
                ),
            );
        } else {
            self.err(
                span,
                format!("non-exhaustive match: missing {}", missing.join(", ")),
            );
        }
    }

    /// The permitted concrete subtypes of a `sealed` base (W5-3), each as a `Ty::Named` "member" a
    /// `match` must cover — every non-abstract class that is a subtype of `base`, plus `base` itself
    /// when it is a concrete (instantiable) class (a value can then be exactly the base). Whole-program:
    /// the compilation IS the closed set. Coverage reuses the union rule (a type-pattern naming a
    /// subtype, or a covering supertype, discharges it), so an intermediate `open` subtype covers its
    /// own descendants and Java-style `permits`/`non-sealed` transitivity is unnecessary.
    fn sealed_permitted_subtypes(&self, base: &str) -> Vec<Ty> {
        let mut out: Vec<Ty> = self
            .classes
            .iter()
            .filter(|(name, info)| {
                !info.is_abstract && name.as_str() != base && self.is_subtype(name, base)
            })
            .map(|(name, _)| Ty::Named(name.clone(), Vec::new()))
            .collect();
        if self.classes.get(base).is_some_and(|info| !info.is_abstract) {
            out.push(Ty::Named(base.to_string(), Vec::new()));
        }
        out
    }

    /// Check a pattern against the scrutinee type, declaring its bindings into the
    /// current scope.
    pub(super) fn check_pattern(&mut self, pat: &crate::ast::Pattern, scrut: &Ty) {
        use crate::ast::Pattern;
        match pat {
            Pattern::Wildcard(_) => {}
            Pattern::Binding { name, span } => self.declare(name, scrut.clone(), *span),
            Pattern::Int(_, span) => self.expect_prim(scrut, &Ty::Int, *span),
            Pattern::Float(_, span) => self.expect_prim(scrut, &Ty::Float, *span),
            Pattern::Decimal { span, .. } => self.expect_prim(scrut, &Ty::Decimal, *span),
            Pattern::Str(_, span) => self.expect_prim(scrut, &Ty::String, *span),
            Pattern::Bool(_, span) => self.expect_prim(scrut, &Ty::Bool, *span),
            Pattern::Null(span) => {
                // A `null` pattern is only meaningful against an optional scrutinee (M3 S2.6).
                if !matches!(scrut, Ty::Optional(_) | Ty::Null | Ty::Error) {
                    self.err(
                        *span,
                        format!(
                            "`null` pattern requires an optional `T?` scrutinee, found `{scrut}`"
                        ),
                    );
                }
            }
            Pattern::Variant {
                name,
                fields,
                enum_qualifier,
                span,
            } => {
                let (enum_name, eargs) = match scrut {
                    Ty::Named(n, eargs) if self.enums.contains_key(n) => (n.clone(), eargs.clone()),
                    // DEC-250: an `Optional<enum>` scrutinee admits variant patterns — the pattern
                    // checks against the INNER enum (a null value matches no variant and falls to
                    // the required `null`/catch-all arm; the exhaustiveness pass enforces it).
                    Ty::Optional(inner) => match &**inner {
                        Ty::Named(n, eargs) if self.enums.contains_key(n) => {
                            (n.clone(), eargs.clone())
                        }
                        Ty::Error => return,
                        other => {
                            self.err(*span, format!("variant pattern `{name}` requires an enum scrutinee, found `{other}?`"));
                            return;
                        }
                    },
                    Ty::Error => return,
                    other => {
                        self.err(*span, format!("variant pattern `{name}` requires an enum scrutinee, found `{other}`"));
                        return;
                    }
                };
                // A qualified pattern `Enum.Variant(..)` (A2): the qualifier must name the scrutinee's
                // enum. (The variant-belongs check is the `None` arm below, shared with the bare form.)
                match enum_qualifier {
                    Some(q) if q != &enum_name => {
                        self.err_coded(
                            *span,
                            format!("pattern qualifier `{q}` does not match the scrutinee's enum `{enum_name}`"),
                            "E-VARIANT-QUALIFIER",
                            Some(format!("use `{enum_name}.{name}(…)` (or the bare `{name}(…)`)")),
                        );
                    }
                    // Qualification B: a bare injected-enum pattern is "in the wind" — qualify it.
                    None if self.enums.get(&enum_name).is_some_and(|i| i.injected) => {
                        self.err_coded(
                            *span,
                            format!("`{name}` is a variant of the injected enum `{enum_name}` — match it qualified"),
                            "E-INJECTED-VARIANT-BARE",
                            Some(format!("write `{enum_name}.{name}(…)`")),
                        );
                    }
                    _ => {}
                }
                // DEC-329.3: record the scrutinee's enum (feeds `qualify_variants`).
                let k = span.start;
                self.variant_resolutions.insert(k, enum_name.clone());
                let field_tys: Vec<Ty> = match self.enums[&enum_name].variants.get(name) {
                    // Substitute enum type params with the scrutinee's args (`Option<int>` ⇒
                    // `{T → int}`) — generic payloads bind concrete; identity for non-generic.
                    Some(f) => {
                        let theta = self.enum_subst(&enum_name, &eargs);
                        f.iter().map(|t| apply_subst(t, &theta)).collect()
                    }
                    None => {
                        self.err(*span, format!("enum `{enum_name}` has no variant `{name}`"));
                        return;
                    }
                };
                if field_tys.len() != fields.len() {
                    self.err(
                        *span,
                        format!(
                            "variant `{name}` expects {} field(s), found {}",
                            field_tys.len(),
                            fields.len()
                        ),
                    );
                    return;
                }
                for (fp, ft) in fields.iter().zip(field_tys) {
                    // A nested pattern in a variant field (`Wrapper(Circle c)`, S5.2-T2) is checked
                    // recursively against the field's type — type patterns, struct patterns and
                    // literals are all allowed here now (every backend recurses variant fields). The
                    // exhaustiveness consequence — a refutable payload no longer discharges the
                    // variant's coverage — is handled by `is_irrefutable` in `check_match`.
                    self.check_pattern(fp, &ft);
                }
            }
            Pattern::Type {
                type_name,
                binding,
                span,
            } => {
                if let Some(prim) = prim_pat_ty(type_name) {
                    // Wave A: a PRIMITIVE type pattern (`int i`, `string s`) over a union member.
                    // Discriminated at runtime by `Value` variant (interpreter + VM `Op::IsInstance`)
                    // and transpiled to `is_int()`/`is_float()`/`is_string()`/`is_bool()`/`is_null()`.
                    // Byte-identity guard: `string` is ambiguous in the PHP leg if the union also holds
                    // a type that erases to a PHP `string` (decimal/bytes/html/attr), so reject it —
                    // `run`/`runvm` distinguish them by `Value` variant but the transpiled PHP cannot.
                    if matches!(prim, Ty::String) {
                        // The union may sit behind an `Optional` (`(string | decimal)?`, e.g. the
                        // `T?` a `List.first`/`Map.get` returns) — `union_members_of` unwraps it so
                        // the guard isn't bypassed on that path (byte-identity hole, G-1).
                        if let Some(members) = union_members_of(scrut) {
                            if members
                                .iter()
                                .any(|m| !matches!(m, Ty::String) && erases_to_php_string(m))
                            {
                                self.err_coded(
                                    *span,
                                    "type pattern `string` is ambiguous here — the union also holds a type that erases to a PHP string (decimal/bytes/html/attr), so the transpiled PHP can't distinguish them".to_string(),
                                    "E-MATCH-ERASED-AMBIG",
                                    Some("split the union or add a `_` arm — run/runvm could tell them apart, but the PHP leg cannot".into()),
                                );
                            }
                        }
                    }
                    if let Some(name) = binding {
                        self.declare(name, prim, *span);
                    }
                } else if matches!(type_name.as_str(), "decimal" | "bytes" | "html" | "attr") {
                    // These erase to a PHP `string`, so a runtime type-test can't be byte-identical in
                    // the transpiled leg (LADDER-forced: reject rather than silently diverge).
                    self.err_coded(
                        *span,
                        format!("type pattern `{type_name}` can't be runtime-discriminated — it erases to a PHP string; only int/float/string/bool/null and classes/interfaces can be type-tested"),
                        "E-MATCH-TYPE-ERASED",
                        Some("match its wrapping form, or use a class/interface".into()),
                    );
                } else {
                    // M-RT S4 type pattern: the type must be a class or interface (the runtime test is
                    // `instanceof`, which is class/interface-only — an enum value is never an instance).
                    let known = self.classes.contains_key(type_name)
                        || self.interfaces.contains_key(type_name);
                    if !known && !matches!(scrut, Ty::Error) {
                        let hint = if self.enums.contains_key(type_name) {
                            "an enum is a closed sum — match its variants directly, not via a type pattern"
                        } else {
                            "a type pattern matches a class or interface (as in a union scrutinee)"
                        };
                        self.err_coded(
                            *span,
                            format!("type pattern `{type_name}` must name a class or interface"),
                            "E-MATCH-TYPE",
                            Some(hint.into()),
                        );
                    }
                    // Bind the narrowed value as the named type. A generic class carries erased (poison)
                    // arguments — `instanceof` keeps no type arguments at runtime, so its generic members
                    // read as `mixed`, mirroring the if/instanceof smart-cast (M-RT generics-all).
                    if let Some(name) = binding {
                        let arity = self
                            .classes
                            .get(type_name)
                            .map_or(0, |c| c.type_params.len());
                        let args = vec![Ty::Error; arity];
                        self.declare(name, Ty::Named(type_name.clone(), args), *span);
                    }
                }
            }
            Pattern::Struct {
                type_name,
                fields,
                span,
            } => {
                // The head must name a class — interfaces and enums carry no fields to destructure.
                let is_class = self.classes.contains_key(type_name);
                if !is_class && !matches!(scrut, Ty::Error) {
                    let hint = if self.interfaces.contains_key(type_name) {
                        "an interface has no fields — use a type pattern `Iface x` to bind it"
                    } else {
                        "a struct pattern destructures the fields of a class instance"
                    };
                    self.err_coded(
                        *span,
                        format!("struct pattern `{type_name}` must name a class"),
                        "E-STRUCT-PAT-TYPE",
                        Some(hint.into()),
                    );
                }
                // Duplicate bind names anywhere in this pattern (incl. nested) — one would silently
                // shadow the other in the arm body.
                let mut binds: Vec<String> = Vec::new();
                collect_binds(pat, &mut binds);
                let mut seen = std::collections::HashSet::new();
                for b in &binds {
                    if !seen.insert(b.clone()) {
                        self.err_coded(
                            *span,
                            format!("duplicate binding `{b}` in struct pattern"),
                            "E-PATTERN-DUP-BIND",
                            Some("each destructured binding needs a distinct name".into()),
                        );
                    }
                }
                // Each named field must exist on the class; its sub-pattern checks against the
                // field's declared type (so a nested struct / literal / rename binds at the right
                // type — the operand-type then flows to the compiler's `CTy` via class_field_ctys).
                for fp in fields {
                    let fty = self
                        .classes
                        .get(type_name)
                        .and_then(|ci| ci.fields.get(&fp.field).cloned());
                    match fty {
                        Some(t) => {
                            // Wave 1.1: a struct pattern reads the field (→ PHP `$obj->field`), so an
                            // out-of-scope `private`/`protected` field is rejected here too.
                            let v = self
                                .classes
                                .get(type_name)
                                .and_then(|ci| ci.field_vis.get(&fp.field).cloned());
                            self.enforce_member_vis(v, &fp.field, *span, true);
                            self.check_pattern(&fp.pat, &t);
                        }
                        None if is_class => {
                            self.err_coded(
                                *span,
                                format!("class `{type_name}` has no field `{}`", fp.field),
                                "E-STRUCT-FIELD-UNKNOWN",
                                Some("destructure only the class's declared fields".into()),
                            );
                        }
                        None => {}
                    }
                }
            }
        }
    }

    pub(super) fn expect_prim(&mut self, scrut: &Ty, want: &Ty, span: Span) {
        // A literal pattern matches when the scrutinee *is* that primitive, or is a union that
        // *contains* it (M-RT S4): `match code { 0 => …, "ok" => … }` over `int | string` is well
        // typed (the runtime/transpiler match by value, so a non-member literal simply never fires).
        let ok = *scrut == Ty::Error
            || scrut == want
            || matches!(scrut, Ty::Union(members) if members.contains(want));
        if !ok {
            self.err(
                span,
                format!("pattern of type `{want}` cannot match scrutinee of type `{scrut}`"),
            );
        }
    }
}

/// Collect every variable name a pattern binds, in source order (with repeats), for the struct
/// pattern's duplicate-binding check (`E-PATTERN-DUP-BIND`). Mirrors `ast::collect_pattern_bindings`
/// but keeps a `Vec` so duplicates are visible rather than collapsed into a set.
fn collect_binds(pat: &crate::ast::Pattern, acc: &mut Vec<String>) {
    use crate::ast::Pattern;
    match pat {
        Pattern::Binding { name, .. } => acc.push(name.clone()),
        Pattern::Type {
            binding: Some(n), ..
        } => acc.push(n.clone()),
        Pattern::Variant { fields, .. } => {
            for f in fields {
                collect_binds(f, acc);
            }
        }
        Pattern::Struct { fields, .. } => {
            for f in fields {
                collect_binds(&f.pat, acc);
            }
        }
        _ => {}
    }
}

/// The discriminable primitive's name, for exhaustiveness coverage of a union member (inverse of
/// [`prim_pat_ty`]). A non-discriminable `Ty` returns `None` (still needs a `_`).
fn prim_ty_name(ty: &Ty) -> Option<&'static str> {
    match ty {
        Ty::Int => Some("int"),
        Ty::Float => Some("float"),
        Ty::String => Some("string"),
        Ty::Bool => Some("bool"),
        Ty::Null => Some("null"),
        _ => None,
    }
}
