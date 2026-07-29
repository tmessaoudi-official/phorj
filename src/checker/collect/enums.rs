//! Collection pass — ENUM declarations (variants, generic params, backed-enum scalar backing).
//! Split out of `types_decls.rs` by cohesion (Invariant 13, M-Decomp): enum collection and CLASS
//! collection share nothing but the `Checker` receiver — `types_decls.rs` keeps `collect_class` and
//! its class-only `validate_set_vis` helper. Re-attached to the same `impl Checker`, so every call
//! site is unchanged.

use super::*;

impl Checker {
    pub(in crate::checker) fn collect_enum(&mut self, e: &crate::ast::EnumDecl) {
        if is_builtin_type_name(&e.name) {
            self.err(
                e.span,
                format!("cannot redefine built-in type `{}`", e.name),
            );
            return;
        }
        if !self.prebound.contains(&e.name)
            && (self.enums.contains_key(&e.name) || self.classes.contains_key(&e.name))
        {
            self.err_coded(
                e.span,
                format!("type `{}` is already defined", e.name),
                "E-DUP-TYPE",
                Some("rename one declaration — a class/enum/interface/trait/type name must be unique".into()),
            );
            return;
        }
        // DEC-302 backed enum: resolve + validate the scalar backing type (`enum Suit: string { … }`).
        // Must be `int` or `string` (PHP 8.1's two backing types); generics + backing together are
        // rejected (the erasure/backing interaction is out of scope). `None` for a plain enum.
        let backing: Option<Ty> = match &e.backing_type {
            Some(bt) => {
                let ty = self.resolve_type(bt);
                if !matches!(ty, Ty::Int | Ty::String | Ty::Error) {
                    self.err_coded(
                        e.span,
                        format!("enum backing type must be `int` or `string`, found `{ty}`"),
                        "E-ENUM-BACKING-TYPE",
                        Some(
                            "a backed enum backs each variant with an `int` or `string` literal"
                                .into(),
                        ),
                    );
                    None
                } else if !e.type_params.is_empty() {
                    self.err_coded(
                        e.span,
                        format!(
                            "generic enum `{}` cannot also declare a scalar backing type",
                            e.name
                        ),
                        "E-ENUM-BACKING-GENERIC",
                        Some(
                            "drop the type parameters, or drop the `: int`/`: string` backing"
                                .into(),
                        ),
                    );
                    None
                } else if matches!(ty, Ty::Error) {
                    None
                } else {
                    Some(ty)
                }
            }
            None => None,
        };
        // Register the name + type parameters first so variant field types can reference the enum
        // itself (including a self-referential `Tree<T>` payload) with correct arity (M-RT generic
        // enums).
        self.validate_type_params(&e.type_params, e.span);
        self.enums.insert(
            e.name.clone(),
            EnumInfo {
                variants: HashMap::new(),
                type_params: e.type_params.clone(),
                injected: e.injected,
                backing: backing.clone(),
            },
        );
        // The enum's type parameters are in scope while resolving every variant field type, so a bare
        // `T` resolves to `Ty::Param("T")` (M-RT generic enums); cleared after, like a generic class.
        self.active_type_params = e.type_params.clone();
        let mut variants = HashMap::new();
        // DEC-302: distinct-value check for a backed enum — a duplicate backing value
        // (`enum E: int { A = 1, B = 1 }`) is rejected so `from`/`tryFrom` stay unambiguous.
        let mut seen_values: Vec<crate::value::Value> = Vec::new();
        for v in &e.variants {
            // DEC-302: `cases`/`from`/`tryFrom` are the enum's static-method surface (any enum for
            // `cases`, a backed enum for `from`/`tryFrom`); a variant of one of those names would
            // collide with the interception, so reserve them across every enum.
            if matches!(v.name.as_str(), "cases" | "from" | "tryFrom") {
                self.err_coded(
                    v.span,
                    format!(
                        "`{}` is a reserved enum method name and cannot be a variant",
                        v.name
                    ),
                    "E-ENUM-RESERVED-VARIANT",
                    Some(
                        "rename the variant — `cases`/`from`/`tryFrom` are enum static methods"
                            .into(),
                    ),
                );
            }
            let fields: Vec<Ty> = v.fields.iter().map(|p| self.resolve_type(&p.ty)).collect();
            // DEC-302 backed-enum per-variant validation.
            match (&backing, &v.backing_value) {
                (Some(bt), Some(val)) => {
                    if !fields.is_empty() {
                        self.err_coded(
                            v.span,
                            format!("backed-enum variant `{}` cannot carry a payload", v.name),
                            "E-ENUM-BACKED-PAYLOAD",
                            Some("a backed enum's variants are scalar-valued; drop the payload fields".into()),
                        );
                    }
                    if crate::value::const_literal(val).is_none() {
                        self.err_coded(
                            Self::expr_span(val),
                            format!(
                                "backed-enum variant `{}`'s value must be a literal constant",
                                v.name
                            ),
                            "E-ENUM-VALUE-NOT-LITERAL",
                            Some("use an int/string literal matching the backing type".into()),
                        );
                    } else if super::entry::literal_ty(val).as_ref() != Some(bt) {
                        self.err_coded(
                            Self::expr_span(val),
                            format!("backed-enum variant `{}`'s value does not match backing type `{bt}`", v.name),
                            "E-ENUM-VALUE-TYPE",
                            Some(format!("give `{}` a `{bt}` literal", v.name)),
                        );
                    } else if let Some(cv) = crate::value::const_literal(val) {
                        if seen_values.iter().any(|s| s.eq_val(&cv)) {
                            self.err_coded(
                                Self::expr_span(val),
                                format!("duplicate backing value for variant `{}`", v.name),
                                "E-ENUM-DUP-VALUE",
                                Some("each backed variant must have a distinct value".into()),
                            );
                        }
                        seen_values.push(cv);
                    }
                }
                (Some(_), None) => {
                    self.err_coded(
                        v.span,
                        format!("backed-enum variant `{}` needs a value (`{} = …`)", v.name, v.name),
                        "E-ENUM-VARIANT-NO-VALUE",
                        Some("every variant of a backed enum assigns a literal, e.g. `Hearts = \"H\"`".into()),
                    );
                }
                (None, Some(val)) => {
                    self.err_coded(
                        Self::expr_span(val),
                        format!(
                            "variant `{}` has a value but enum `{}` declares no backing type",
                            v.name, e.name
                        ),
                        "E-ENUM-VALUE-UNBACKED",
                        Some(
                            "add a backing type (`enum E: int { … }`), or drop the `= value`"
                                .into(),
                        ),
                    );
                }
                (None, None) => {}
            }
            // M-DX S1 (soundness hole C): a repeated variant name used to silently overwrite the
            // first in this `HashMap` — a duplicate `enum E { A, A }` type-checked clean. Reject it.
            if variants.insert(v.name.clone(), fields).is_some() {
                self.err_coded(
                    v.span,
                    format!("duplicate enum variant `{}`", v.name),
                    "E-DUP-VARIANT",
                    Some("each variant of an enum must have a distinct name".into()),
                );
            }
        }
        self.active_type_params.clear();
        self.enums.get_mut(&e.name).unwrap().variants = variants;
    }
}
