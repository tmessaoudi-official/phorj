//! `impl Checker` — resolve cluster (M-Decomp W2). See checker/mod.rs for the struct + entry points.

use super::*;

impl Checker {
    /// Resolve an AST type annotation to an internal `Ty`. Records and poisons on
    /// unknown / deferred types.
    pub(super) fn resolve_type(&mut self, ty: &crate::ast::Type) -> Ty {
        use crate::ast::Type;
        match ty {
            Type::Optional { inner, .. } => Ty::Optional(Box::new(self.resolve_type(inner))),
            Type::Union(members, span) => {
                // DEC-253: `A | B | null` — the PHP-familiar nullable-union spelling. Strip the
                // `null` member(s) and wrap the remaining union in `Optional`, so both spellings
                // resolve to the same `Ty::Optional(Ty::Union(..))` as the canonical `(A | B)?`
                // (assignability, narrowing, match — all inherited for free). A single non-null
                // remainder is just `A?`.
                let (nulls, members): (Vec<&Type>, Vec<&Type>) = members
                    .iter()
                    .partition(|m| matches!(m, Type::Named { name, args, .. } if name == "null" && args.is_empty()));
                if !nulls.is_empty() {
                    if members.is_empty() {
                        return self.err_coded(
                            *span,
                            "a union needs at least one non-`null` member".to_string(),
                            "E-UNION-ARITY",
                            Some(
                                "`null` alone is not a type — write `T?` around a real type".into(),
                            ),
                        );
                    }
                    let inner = if members.len() == 1 {
                        self.resolve_type(members[0])
                    } else {
                        let rebuilt = Type::Union(members.into_iter().cloned().collect(), *span);
                        self.resolve_type(&rebuilt)
                    };
                    return Ty::Optional(Box::new(inner));
                }
                let members: Vec<Type> = members.into_iter().cloned().collect();
                // M-RT S4: resolve each member, validate its kind (classes/interfaces/primitives
                // only — enums/optionals/functions are rejected so the PHP `A|B` emission and the
                // instanceof-based match stay sound), then normalize. A degenerate union that
                // collapses to one member after dedupe is `E-UNION-ARITY`.
                let resolved: Vec<Ty> = members.iter().map(|m| self.resolve_type(m)).collect();
                for ty in &resolved {
                    // `void` is the uncapturable nothing: a union containing it is uninhabited at the
                    // value level (you can never hold a `void`), so it is rejected with a dedicated
                    // code. `empty` — the *holdable* nothing — is inhabited and IS allowed (`int|empty`).
                    if matches!(ty, Ty::Void) {
                        self.err_coded(
                            *span,
                            "`void` cannot be a union member — it is the uncapturable nothing, so the union would be uninhabited".to_string(),
                            "E-VOID-IN-UNION",
                            Some("use `empty` for the holdable nothing (`int|string|empty` is allowed); `void` must stand alone".into()),
                        );
                        continue;
                    }
                    let ok = match ty {
                        Ty::Int
                        | Ty::Float
                        | Ty::Decimal
                        | Ty::Bool
                        | Ty::String
                        | Ty::Bytes
                        | Ty::Html
                        | Ty::Attr
                        | Ty::Empty
                        | Ty::Error => true,
                        Ty::Named(n, _) => {
                            self.classes.contains_key(n) || self.interfaces.contains_key(n)
                        }
                        _ => false,
                    };
                    if !ok {
                        self.err_coded(
                            *span,
                            format!(
                                "union member `{ty}` is not allowed — members must be classes, interfaces, or primitives"
                            ),
                            "E-UNION-MEMBER",
                            Some(
                                "enum, optional `T?`, and function members are not supported in a union this slice".into(),
                            ),
                        );
                    }
                }
                let norm = Ty::union_of(resolved);
                if !matches!(norm, Ty::Union(_) | Ty::Error) {
                    // ≥2 source members collapsed to one (`A | A`): a union needs ≥2 distinct types.
                    self.err_coded(
                        *span,
                        "a union needs two or more distinct types".to_string(),
                        "E-UNION-ARITY",
                        None,
                    );
                }
                norm
            }
            Type::Intersection(members, span) => {
                // M-RT S5: resolve each member, validate kinds (D1: interfaces, plus *at most one*
                // concrete class — a value has exactly one class, so two distinct classes are the
                // bottom type), then enforce shared-method signature agreement (D2: no overloading
                // yet, so two members whose shared method differs is uninhabited) and normalize.
                let resolved: Vec<Ty> = members.iter().map(|m| self.resolve_type(m)).collect();
                let mut class_count = 0;
                for ty in &resolved {
                    match ty {
                        Ty::Error => {}
                        Ty::Named(n, _) if self.interfaces.contains_key(n) => {}
                        Ty::Named(n, _) if self.classes.contains_key(n) => class_count += 1,
                        _ => {
                            self.err_coded(
                                *span,
                                format!(
                                    "intersection member `{ty}` is not allowed — members must be interfaces, with at most one concrete class"
                                ),
                                "E-INTERSECT-MEMBER",
                                Some("primitives, enums, optionals, and function types cannot be intersection members".into()),
                            );
                        }
                    }
                }
                if class_count >= 2 {
                    self.err_coded(
                        *span,
                        "an intersection may name at most one concrete class — no value can be two distinct classes at once".to_string(),
                        "E-INTERSECT-MULTI-CLASS",
                        Some("compose with interfaces instead; a second class becomes possible only when class `extends` lands (S6)".into()),
                    );
                }
                // DEC-245 (executes DEC-057's revisit clause): a method declared by several
                // members forms an OVERLOAD SET at the access site — identical signatures merge,
                // different parameter lists coexist (a class can legally implement both
                // interfaces; the DEC-058 overload machinery dispatches). The ONE genuinely
                // uninhabitable combo stays rejected: SAME parameters with DIFFERENT returns — no
                // class can implement both, and no call-site selector could pick
                // (`E-INTERSECT-SIG`, narrowed).
                let mut method_sigs: HashMap<String, Vec<(Vec<Ty>, Ty)>> = HashMap::new();
                let mut sig_conflict: Option<String> = None;
                for ty in &resolved {
                    if let Ty::Named(n, _) = ty {
                        let methods: Vec<(String, (Vec<Ty>, Ty))> =
                            if self.interfaces.contains_key(n) {
                                self.iface_flat_methods(n)
                            } else if let Some(info) = self.classes.get(n) {
                                // Every overload of a member class participates in the merged set.
                                info.methods
                                    .iter()
                                    .flat_map(|(m, sigs)| {
                                        sigs.iter()
                                            .map(|s| (m.clone(), (s.params.clone(), s.ret.clone())))
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            };
                        for (m, sig) in methods {
                            let set = method_sigs.entry(m.clone()).or_default();
                            let same_params_diff_ret =
                                set.iter().any(|(ps, r)| *ps == sig.0 && *r != sig.1);
                            if same_params_diff_ret && sig_conflict.is_none() {
                                sig_conflict = Some(m);
                            } else if !set.contains(&sig) {
                                set.push(sig);
                            }
                        }
                    }
                }
                if let Some(m) = sig_conflict {
                    self.err_coded(
                        *span,
                        format!(
                            "intersection members declare method `{m}` with the same parameters but different return types — no class could implement both"
                        ),
                        "E-INTERSECT-SIG",
                        Some("distinct PARAMETER lists coexist as an overload set (DEC-245); only a same-params/different-return clash is uninhabitable".into()),
                    );
                }
                let norm = Ty::intersection_of(resolved);
                if !matches!(norm, Ty::Intersection(_) | Ty::Error) {
                    // ≥2 source members collapsed to one (`A & A`): an intersection needs ≥2 distinct.
                    self.err_coded(
                        *span,
                        "an intersection needs two or more distinct types".to_string(),
                        "E-INTERSECT-ARITY",
                        None,
                    );
                }
                norm
            }
            Type::Function {
                params,
                ret,
                throws,
                ..
            } => {
                // DEC-222: resolve the declared throws set, flatten any union members, and canonical-
                // sort (by `Display`) so member order never affects the type's identity or `Display` —
                // matching how `cur_throws`/union members are normalized. NOT Error-validated here (a
                // bare function-type annotation is structural; validation happens at the lambda
                // definition site, `check_lambda`).
                let resolved: Vec<Ty> = throws.iter().map(|t| self.resolve_type(t)).collect();
                let mut es = Self::flatten_throws(resolved);
                es.sort_by_key(std::string::ToString::to_string);
                es.dedup();
                Ty::Function(
                    params.iter().map(|p| self.resolve_type(p)).collect(),
                    Box::new(self.resolve_type(ret)),
                    es,
                )
            }
            // `[T; N]` (Phase 1 types slice): a fixed-length list. The element resolves like a
            // `List<T>` element; the length rides along for static bounds + length-checked init.
            Type::FixedList { elem, len, .. } => {
                Ty::FixedList(Box::new(self.resolve_type(elem)), *len)
            }
            // `var` is intercepted in `check_stmt`; reaching here means it was written somewhere it
            // is not allowed (a parameter, field, or return type).
            Type::Infer(span) => self.err(
                *span,
                "`var` type inference is only valid for a local variable declaration",
            ),
            // Defensive: `Type::Erased` is produced by `erase_generics` *after* a successful check,
            // so a normal pipeline never resolves it. Treat it as poison so a stray re-check of an
            // already-erased program can't cascade (M-RT S7).
            Type::Erased(_) => Ty::Error,
            Type::Named { name, args, span } => match name.as_str() {
                // DEC-253: `null` is a union-member marker only (`A | B | null` — the parser mints
                // it; `null` is a keyword so no user type collides). Standalone it is not a type.
                "null" => self.err_coded(
                    *span,
                    "`null` by itself is not a type".to_string(),
                    "E-NULL-TYPE",
                    Some(
                        "write an optional `T?`, or a nullable union `A | B | null` / `(A | B)?`"
                            .into(),
                    ),
                ),
                "int" => self.no_args(name, args, *span, Ty::Int),
                "float" => self.no_args(name, args, *span, Ty::Float),
                "decimal" => self.no_args(name, args, *span, Ty::Decimal),
                "bool" => self.no_args(name, args, *span, Ty::Bool),
                "string" => self.no_args(name, args, *span, Ty::String),
                "bytes" => self.no_args(name, args, *span, Ty::Bytes),
                // The bottom type (M-RT totality cluster): a `-> never` function never returns. Only
                // meaningful in return position, but resolvable anywhere a type name appears.
                "never" => self.no_args(name, args, *span, Ty::Never),
                // The two-type "nothing" model (S0a). `void` = uncapturable (the implicit return
                // type); `empty` = the holdable nothing. Both resolve here; the *position* rules
                // (void rejected as a param/field type, void value uncapturable) are enforced at the
                // collection / var-decl sites, not here.
                "void" => self.no_args(name, args, *span, Ty::Void),
                "empty" => self.no_args(name, args, *span, Ty::Empty),
                "Html" => self.no_args(name, args, *span, Ty::Html),
                "Attr" => self.no_args(name, args, *span, Ty::Attr),
                "List" => Ty::List(Box::new(self.one_arg(name, args, *span))),
                "Set" => Ty::Set(Box::new(self.one_arg(name, args, *span))),
                "Map" => {
                    if args.len() != 2 {
                        return self.err_coded(
                            *span,
                            format!("Map expects 2 type arguments, got {}", args.len()),
                            "E-TYPE-ARG-COUNT",
                            Some("a `Map<K, V>` needs exactly two type arguments".into()),
                        );
                    }
                    let k = self.resolve_type(&args[0]);
                    let v = self.resolve_type(&args[1]);
                    Ty::Map(Box::new(k), Box::new(v))
                }
                // Green-thread handle types (M6 W4): `Channel<T>` / `Task<T>`. The element type is the
                // single type argument; kept as a `Ty::Named` (no dedicated `Ty` variant — channels /
                // tasks never participate in arithmetic/compare, so the single-sourced value kernels and
                // the type machinery treat them as any other one-arg nominal). `Channel.new()` /
                // `.send` / `.recv` / `.join` are typed by dedicated checker arms (see `check_spawn`,
                // `check_method_call`, `check_static_method_call`).
                "Channel" => Ty::Named("Channel".into(), vec![self.one_arg(name, args, *span)]),
                "Task" => Ty::Named("Task".into(), vec![self.one_arg(name, args, *span)]),
                // `DbHandle` (DEC-208): the opaque native connection/statement/row handle the `Core.Db`
                // prelude classes store in a field and thread to the `Core.DbSys` natives. Reserved +
                // IMPORT-GATED (never ambient — the developer's "nothing in the wind" rule): it resolves
                // only when `Core.DbSys` is in scope (the injected `Core.Db` prelude imports it), so a
                // user cannot name `DbHandle` without importing `Core.Db`. Opaque: it never participates
                // in arithmetic/compare/display (like `Channel`/`Task`), so the value kernels are
                // untouched; the natives downcast the underlying `Value::Db`/`Value::Map` at runtime.
                "DbHandle" if self.imports.values().any(|m| m == "Core.DbSys") => {
                    Ty::Named("DbHandle".into(), vec![])
                }
                // `MailHandle` (DEC-223) — the `Core.Mail` twin of `DbHandle`: the opaque native
                // mailer/draft handle, import-gated on `Core.MailSys` (nothing in the wind).
                "MailHandle" if self.imports.values().any(|m| m == "Core.MailSys") => {
                    Ty::Named("MailHandle".into(), vec![])
                }
                // `HcHandle` (W3-2) — the `Core.HttpClient` opaque handle, gated on `Core.HttpClientSys`.
                "HcHandle" if self.imports.values().any(|m| m == "Core.HttpClientSys") => {
                    Ty::Named("HcHandle".into(), vec![])
                }
                "double" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => self.err(
                    *span,
                    format!("the numeric type `{name}` is not yet supported in M1"),
                ),
                other => {
                    if self.active_type_params.iter().any(|p| p == other) {
                        // A generic type parameter in scope (`T` in `function id<T>(T x)`) is an
                        // opaque `Ty::Param`, unified away at call sites and erased before backends.
                        // A type arg on it (`T<int>`) is meaningless — reject it.
                        if args.is_empty() {
                            Ty::Param(other.to_string())
                        } else {
                            self.err_coded(
                                *span,
                                format!("type parameter `{other}` takes no type arguments"),
                                "E-TYPE-ARG-COUNT",
                                Some(format!(
                                    "`{other}` is an opaque type parameter — drop the `<…>`"
                                )),
                            )
                        }
                    } else if self.aliases.contains_key(other) {
                        if self.alias_stack.iter().any(|n| n == other) {
                            // W0-4: the cycle is `other` plus every alias currently on the stack from
                            // the point `other` first appears. Report it *coded* (E-ALIAS-CYCLE) and
                            // deduped, so a cycle already caught by the collect-time walk (or an
                            // earlier use) is not re-reported.
                            let start = self
                                .alias_stack
                                .iter()
                                .position(|n| n == other)
                                .unwrap_or(0);
                            let mut cycle: Vec<String> = self.alias_stack[start..].to_vec();
                            cycle.push(other.to_string());
                            self.report_alias_cycle(&cycle, *span);
                            return Ty::Error;
                        }
                        let aliased = self.aliases.get(other).cloned().expect("alias present");
                        self.alias_stack.push(other.to_string());
                        let ty = self.resolve_type(&aliased);
                        self.alias_stack.pop();
                        ty
                    } else if let Some(info) = self.interfaces.get(other) {
                        // DEC-257 generic interfaces: `Iterator<int>` resolves with arity-checked
                        // arguments; a non-generic interface still takes none.
                        let arity = info.type_params.len();
                        if args.len() != arity {
                            let name = other.to_string();
                            self.err_coded(
                                *span,
                                format!(
                                    "interface `{name}` takes {arity} type argument{}, found {}",
                                    if arity == 1 { "" } else { "s" },
                                    args.len()
                                ),
                                "E-TYPE-ARG-COUNT",
                                None,
                            )
                        } else {
                            let resolved: Vec<Ty> =
                                args.iter().map(|a| self.resolve_type(a)).collect();
                            Ty::Named(other.to_string(), resolved)
                        }
                    } else if self.traits.contains(other) {
                        // M-RT S8: a trait is collected into `classes` for member lookup, but it is
                        // **not a type** — a value can never be typed as a trait. Reject it here before
                        // the class branch would accept it.
                        self.err_coded(
                            *span,
                            format!("`{other}` is a trait, not a type"),
                            "E-USE-AS-TYPE",
                            Some(
                                "a trait is reuse, not a type — `use` it in a class; you cannot type a value as a trait"
                                    .into(),
                            ),
                        )
                    } else if let Some(arity) = self
                        .classes
                        .get(other)
                        .map(|c| c.type_params.len())
                        .or_else(|| self.enums.get(other).map(|e| e.type_params.len()))
                    {
                        // A class or enum. A generic one requires exactly its declared number of type
                        // arguments (`Box<int>`, `Option<int>`); a non-generic one takes none (M-RT
                        // generics-all / generic enums).
                        if args.len() != arity {
                            let plural = if arity == 1 { "" } else { "s" };
                            self.err_coded(
                                *span,
                                format!(
                                    "type `{other}` expects {arity} type argument{plural}, got {}",
                                    args.len()
                                ),
                                "E-TYPE-ARG-COUNT",
                                Some(format!(
                                    "give `{other}` exactly {arity} type argument{plural}"
                                )),
                            )
                        } else {
                            let resolved = args.iter().map(|a| self.resolve_type(a)).collect();
                            Ty::Named(other.to_string(), resolved)
                        }
                    } else {
                        self.err_coded(
                            *span,
                            format!("unknown type `{other}`"),
                            "E-UNKNOWN-TYPE",
                            None,
                        )
                    }
                }
            },
        }
    }

    pub(super) fn no_args(
        &mut self,
        name: &str,
        args: &[crate::ast::Type],
        span: Span,
        ty: Ty,
    ) -> Ty {
        if args.is_empty() {
            ty
        } else {
            self.err_coded(
                span,
                format!("type `{name}` takes no type arguments"),
                "E-TYPE-ARG-COUNT",
                Some(format!("`{name}` is not generic — drop the `<…>`")),
            )
        }
    }

    pub(super) fn one_arg(&mut self, name: &str, args: &[crate::ast::Type], span: Span) -> Ty {
        if args.len() != 1 {
            self.err_coded(
                span,
                format!("{name} expects 1 type argument, got {}", args.len()),
                "E-TYPE-ARG-COUNT",
                Some(format!("`{name}<T>` needs exactly one type argument")),
            );
            Ty::Error
        } else {
            self.resolve_type(&args[0])
        }
    }
}
