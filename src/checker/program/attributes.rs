//! Program pass — attribute validation (built-in + user `#[Tag]` attributes).

use super::*;

impl Checker {
    /// DEC-194 2b-3: if `attr` names a user-defined attribute — a class carrying `#[Attribute]` — validate
    /// the use and return `true` (handled). The attribute name is a class leaf (bare `Tag`, or qualified
    /// `Pkg.Tag` — resolved by its last segment). Arguments are checked against the attribute class's
    /// constructor ARITY; full argument TYPE-checking is a tracked follow-up (2b-3b). A user attribute is
    /// legal on EVERY target this slice — target restriction rides the `targets:` marker arguments, which
    /// need named arguments (a later slice). Returns `false` if `attr` is not a user attribute (the caller
    /// then falls through to the built-in handling / unknown-attribute error).
    pub(in crate::checker) fn check_user_attribute_use(
        &mut self,
        attr: &crate::ast::Attribute,
    ) -> bool {
        let leaf = attr.name.rsplit('.').next().unwrap_or(attr.name.as_str());
        // Clone the constructor param types out (small) so the `self.classes` borrow ends before the
        // `&mut self` type-checking calls below.
        let params: Vec<Ty> = match self.classes.get(leaf) {
            Some(info) if info.is_user_attribute => info.ctor.clone(),
            _ => return false,
        };
        if attr.args.len() != params.len() {
            self.err_coded(
                attr.span,
                format!(
                    "attribute `#[{}]` takes {} argument(s), got {}",
                    attr.name,
                    params.len(),
                    attr.args.len()
                ),
                "E-ATTRIBUTE-ARITY",
                Some(
                    "an attribute's arguments must match its attribute class's constructor parameters"
                        .into(),
                ),
            );
        }
        // 2b-3b: type-check each provided argument against the corresponding constructor parameter —
        // the COMPILE-TIME typed-argument guarantee (PHP only fails when the attribute is reflected).
        // `check_arg` types the argument (with the same literal-threading a `new Tag(…)` call gets); the
        // assignability check + error mirror `check_args_defaulted`. Surplus args (already flagged by the
        // arity error) are dropped by `zip`; a missing arg is covered by the arity error.
        for (i, (arg, pty)) in attr.args.iter().zip(params.iter()).enumerate() {
            let at = self.check_arg(arg, pty);
            if !self.ty_assignable(&at, pty) {
                self.err_coded(
                    attr.span,
                    format!(
                        "attribute `#[{}]` argument {} expects `{pty}`, found `{at}`",
                        attr.name,
                        i + 1
                    ),
                    "E-ATTRIBUTE-ARG-TYPE",
                    Some(
                        "an attribute argument must match its attribute class's constructor parameter type"
                            .into(),
                    ),
                );
            }
        }
        true
    }

    /// Validate a class declaration's `#[…]` attributes (DEC-194 slice 2a). Attributes now PARSE on a
    /// class, but no attribute currently *targets* a class — the built-ins `#[Route]` (route handler) and
    /// `#[UncheckedOverflow]` (free function) are not class-valid, and user-declared attributes (which
    /// will be able to target a class) arrive in a later DEC-194 slice. So every class attribute is a
    /// clean `E-ATTR-TARGET` for now: a class attribute is always *validated*, never silently accepted,
    /// keeping the surface strict as the target set grows.
    pub(in crate::checker) fn check_class_attributes(&mut self, c: &crate::ast::ClassDecl) {
        for attr in &c.attrs {
            // `#[Attribute]` (DEC-194 2b) DECLARES this class as a user-defined attribute type — the one
            // attribute that legally targets a class. Import-gated by the injected-type discipline
            // (`import Core.Runtime.Attribute;` bare, or `import Core.Runtime;` → `#[Runtime.Attribute]`).
            if attr.is_attribute_marker() {
                // 2b-1 accepts the BARE marker (the class is an attribute, valid on all targets,
                // non-repeatable). The `targets: […]` / `repeatable` arguments land in 2b-2; until then a
                // marker with arguments is a clean, explicit "not yet" rather than silent tolerance.
                if !attr.args.is_empty() {
                    self.err_coded(
                        attr.span,
                        "`#[Attribute]` arguments (`targets: […]`, `repeatable`) are not supported yet"
                            .to_string(),
                        "E-ATTRIBUTE-ARGS",
                        Some(
                            "use the bare marker `#[Attribute]` for now — it declares an attribute \
                             valid on all targets; target restriction + `repeatable` arrive next slice"
                                .into(),
                        ),
                    );
                }
                continue;
            }
            // DEC-194 2b-3: a user-defined attribute (`#[Tag]`, where `Tag` carries `#[Attribute]`) is a
            // legal use on a class (valid on all targets this slice); validated against its constructor.
            if self.check_user_attribute_use(attr) {
                continue;
            }
            // DI v1: `#[Injectable]` (slice 1) and `#[Transient]` (slice 4b) are built-in CLASS attributes
            // consumed by `desugar_di` before any backend (then inert). `#[Transient]` opts the class out
            // of the default-shared lifetime. Accept them so they are not `E-UNKNOWN-ATTRIBUTE`.
            if attr.is_di_builtin() || attr.is_di_transient() {
                if attr.is_di_transient() && !attr.args.is_empty() {
                    self.err_coded(
                        attr.span,
                        "`#[Transient]` takes no arguments".to_string(),
                        "E-TRANSIENT-ARGS",
                        Some("write it bare: `#[Transient]` on the class".into()),
                    );
                }
                continue;
            }
            self.err_coded(
                attr.span,
                format!(
                    "attribute `#[{}]` is not valid on a class — mark a class as an attribute with \
                     `#[Attribute]`, or use a declared user attribute",
                    attr.name
                ),
                "E-ATTR-TARGET",
                Some(
                    "mark a class as an attribute with `#[Attribute]` (import Core.Runtime.Attribute); \
                     the built-ins `#[Route]`/`#[UncheckedOverflow]` target functions/methods"
                        .into(),
                ),
            );
        }
    }

    /// Validate a free function's `#[…]` attributes (M6 W2). Only `#[Route("METHOD", "/path")]` is
    /// recognized; every other name is a hard `E-UNKNOWN-ATTRIBUTE`. A `Route` must carry exactly two
    /// string-literal args (`E-ROUTE-ARGS`), a non-empty method + a `/`-leading path (`E-ROUTE-SPEC`),
    /// and the handler must take one parameter and return a value (`E-ROUTE-HANDLER` — the structural
    /// shape; the precise `(Request) -> Response` typing is enforced where `Http.autoRouter()` lowers
    /// the route into a `.route(…)` registration). Front-end-only — attributes never reach a backend.
    pub(in crate::checker) fn check_attributes(&mut self, f: &crate::ast::FunctionDecl) {
        for attr in &f.attrs {
            // `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow, perf-wave): marks the
            // whole free function's int `+`/`-`/`*`/unary-`-` as WRAPPING (no overflow fault) — the opt-in
            // perf escape hatch. Takes no arguments. Recognition is single-sourced in
            // `Attribute::is_unchecked_overflow` (checker/compiler/interp/transpile agree); import-gated by
            // the injected-type discipline (`enforce_injected`); a using function is `E-TRANSPILE-UNCHECKED`
            // (no PHP analog, §14 LADDER).
            if attr.is_unchecked_overflow() {
                if !attr.args.is_empty() {
                    self.err_coded(
                        attr.span,
                        "`#[UncheckedOverflow]` takes no arguments".to_string(),
                        "E-UNCHECKED-ARGS",
                        Some("write it bare: `#[UncheckedOverflow]`".into()),
                    );
                }
                continue;
            }
            // DI v1 slice 4: `#[Provides]` marks a `static` method whose return type is a provided type;
            // the DI graph (`desugar_di`) constructs that type via this method. Valid only on a `static`
            // method with a declared return type — otherwise a clean error (never silently ignored).
            // Import-gated by the injected-type discipline (`enforce_injected`). Consumed pre-check, inert
            // for backends.
            if attr.is_di_provides() {
                if !attr.args.is_empty() {
                    self.err_coded(
                        attr.span,
                        "`#[Provides]` takes no arguments".to_string(),
                        "E-PROVIDES-ARGS",
                        Some("write it bare: `#[Provides]` on a `static` factory method".into()),
                    );
                }
                if !f
                    .modifiers
                    .iter()
                    .any(|m| matches!(m, crate::ast::Modifier::Static))
                {
                    self.err_coded(
                        attr.span,
                        "`#[Provides]` must annotate a `static` method".to_string(),
                        "E-PROVIDES-TARGET",
                        Some(
                            "make the factory method `static` — a provider is resolved without an instance"
                                .into(),
                        ),
                    );
                } else if f.ret.is_none() {
                    self.err_coded(
                        attr.span,
                        "a `#[Provides]` method must declare a return type — it names the provided type"
                            .to_string(),
                        "E-PROVIDES-TARGET",
                        Some("annotate the return type: `static function make(): Db { … }`".into()),
                    );
                }
                continue;
            }
            // DEC-191: `#[Entry]` — the program-entry marker (roles inferred from the signature).
            // Fully validated by the program-level pass in `walk.rs` (bare args, static-only on
            // methods, role signature, one-per-role); here it just needs to be KNOWN.
            if crate::ast::is_entry_attr(attr) {
                continue;
            }
            // DEC-318: `#[Config]` — the typed-config provider marker. Fully validated (zero-arg,
            // concrete return, one per type, top-level only) by the pre-check `desugar_config`
            // pass; here it just needs to be KNOWN, exactly like `#[Entry]` above.
            if attr.is_config() {
                continue;
            }
            // DEC-194 2b-3: a user-defined attribute (`#[Tag]`, where `Tag` carries `#[Attribute]`) is a
            // legal use on a function/method (valid on all targets this slice); validated against its ctor.
            if self.check_user_attribute_use(attr) {
                continue;
            }
            if !attr.is_route() {
                self.err_coded(
                    attr.span,
                    format!(
                        "unknown attribute `#[{}]` — use a declared user attribute, `#[Route(...)]`, or `#[UncheckedOverflow]`",
                        attr.name
                    ),
                    "E-UNKNOWN-ATTRIBUTE",
                    Some("remove it, or use `#[Route(\"GET\", \"/path\")]` / `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow)".into()),
                );
                continue;
            }
            let lits: Vec<Option<String>> =
                attr.args.iter().map(Self::string_literal_value).collect();
            if attr.args.len() != 2 || lits.iter().any(Option::is_none) {
                self.err_coded(
                    attr.span,
                    "`#[Route]` takes exactly two string-literal arguments: an HTTP method and a path"
                        .to_string(),
                    "E-ROUTE-ARGS",
                    Some("e.g. `#[Route(\"GET\", r\"/users/{id}\")]`".into()),
                );
                continue;
            }
            let method = lits[0].clone().unwrap_or_default();
            let path = lits[1].clone().unwrap_or_default();
            if method.is_empty() || !path.starts_with('/') {
                self.err_coded(
                    attr.span,
                    "`#[Route]` method must be non-empty and the path must start with `/`"
                        .to_string(),
                    "E-ROUTE-SPEC",
                    Some("e.g. `#[Route(\"GET\", \"/health\")]`".into()),
                );
            }
            if f.params.len() != 1 || f.ret.is_none() {
                self.err_coded(
                    f.span,
                    format!(
                        "a `#[Route]` handler must take exactly one `Request` parameter and return a `Response` (got {} parameter(s))",
                        f.params.len()
                    ),
                    "E-ROUTE-HANDLER",
                    Some("declare `function name(Request req) -> Response { … }`".into()),
                );
            }
            // A `#[Route]` on a *method* (checked inside a class — `cur_class` is set) must be `static`:
            // `Http.autoRouter()` lowers it to `function(req) => ClassName.method(req)`, a static call. An
            // instance method has no routable receiver this slice (M6 W2-ext slice 3).
            if self.cur_class.is_some() && !f.modifiers.contains(&crate::ast::Modifier::Static) {
                self.err_coded(
                    f.span,
                    "a `#[Route]` method must be `static`".to_string(),
                    "E-ROUTE-METHOD-STATIC",
                    Some(
                        "mark it `static function …`; an instance controller has no routable receiver yet"
                            .into(),
                    ),
                );
            }
        }
    }

    /// The static string value of an expression iff it is a string literal with no interpolation
    /// (a plain `"…"` or a raw `r"…"`); `None` for any interpolated or non-string expression. Used to
    /// read `#[Route]`'s arguments at check time.
    pub(in crate::checker) fn string_literal_value(e: &crate::ast::Expr) -> Option<String> {
        match e {
            crate::ast::Expr::Str(parts, _) => {
                let mut s = String::new();
                for p in parts {
                    match p {
                        crate::ast::StrPart::Literal(lit) => s.push_str(lit.as_str()),
                        crate::ast::StrPart::Expr(_) => return None,
                    }
                }
                Some(s)
            }
            _ => None,
        }
    }
}
