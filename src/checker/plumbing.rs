//! Checker plumbing — construction, diagnostics emission, scopes, bindings, lookups.

use super::*;

impl Checker {
    pub(super) fn new() -> Self {
        // Built-in `Error` marker interface (M-faults 2b): a thrown type is `class X implements
        // Error`. It declares **no** methods (a pure marker) — the `message` field is conventional and
        // special-cased only by the transpiler (`extends \Exception` + `parent::__construct`). The name
        // is reserved (see `is_builtin_type_name`), so user code cannot redefine it. Class `extends`
        // (inheritance) is a future slice (S6), so an interface is the only available shape today.
        let mut interfaces = HashMap::new();
        interfaces.insert(
            "Error".to_string(),
            InterfaceInfo {
                methods: HashMap::new(),
                extends: Vec::new(),
            },
        );
        Checker {
            funcs: HashMap::new(),
            enums: HashMap::new(),
            classes: HashMap::new(),
            interfaces,
            sealed_types: std::collections::BTreeSet::new(),
            traits: std::collections::HashSet::new(),
            prebound: std::collections::HashSet::new(),
            class_implements: std::collections::BTreeMap::new(),
            class_supertypes: std::collections::BTreeMap::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            cur_ret: Ty::Void,
            cur_throws: Vec::new(),
            cur_is_main: false,
            try_catch_stack: Vec::new(),
            skip_throws_discharge: false,
            under_new: false,
            in_field_init: false,
            in_static_init: false,
            test_mode: false,
            in_static_method: false,
            in_constructor: false,
            parent_ctor_ok: false,
            cur_class: None,
            depth: 0,
            loop_depth: 0,
            aliases: HashMap::new(),
            alias_stack: Vec::new(),
            alias_cycle_reported: std::collections::HashSet::new(),
            imports: HashMap::new(),
            fn_imports: HashMap::new(),
            html_resolutions: HashMap::new(),
            ufcs_resolutions: HashMap::new(),
            default_fills: HashMap::new(),
            cast_resolutions: HashMap::new(),
            pending_fill: None,
            reflect_resolutions: HashMap::new(),
            active_type_params: Vec::new(),
            cur_class_type_params: Vec::new(),
            return_overload_sets: HashMap::new(),
            free_fn_decls: Vec::new(),
            overload_def_renames: HashMap::new(),
            overload_resolutions: HashMap::new(),
            return_overload_methods: HashMap::new(),
            method_fn_decls: Vec::new(),
            reified_operands: HashMap::new(),
            parent_parents: std::collections::BTreeMap::new(),
            parent_mro: std::collections::BTreeMap::new(),
            parent_origins: std::collections::BTreeMap::new(),
        }
    }

    /// Record an error and return the poison type so callers can keep going.
    pub(super) fn err(&mut self, span: Span, msg: impl Into<String>) -> Ty {
        self.errors
            .push(Diagnostic::new(Stage::Type, msg, span.line, span.col));
        Ty::Error
    }

    /// Like [`Self::err`] but attaches a stable diagnostic `code` (for `phg explain`) and an
    /// optional hint.
    pub(super) fn err_coded(
        &mut self,
        span: Span,
        msg: impl Into<String>,
        code: &'static str,
        hint: Option<String>,
    ) -> Ty {
        let mut d = Diagnostic::new(Stage::Type, msg, span.line, span.col).with_code(code);
        d.hint = hint;
        self.errors.push(d);
        Ty::Error
    }

    /// Record a non-fatal lint (the warning channel — M3 S2.5). Unlike [`err_coded`] this does not
    /// poison a type; it is collected separately and surfaced to stderr without failing the build.
    pub(super) fn warn_coded(
        &mut self,
        span: Span,
        msg: impl Into<String>,
        code: &'static str,
        hint: Option<String>,
    ) {
        let mut d = Diagnostic::new(Stage::Type, msg, span.line, span.col).with_code(code);
        d.hint = hint;
        self.warnings.push(d);
    }

    /// Assignment-failure diagnostic. Recognizes the optional-misuse case (a `T?` used where a
    /// non-optional `T` is required) and attaches `E-OPT-ASSIGN` + an unwrap hint; otherwise the
    /// generic type-mismatch message.
    pub(super) fn err_assign(&mut self, span: Span, actual: &Ty, declared: &Ty) {
        let optional_misuse =
            matches!(actual, Ty::Optional(_) | Ty::Null) && !matches!(declared, Ty::Optional(_));
        if optional_misuse {
            self.err_coded(
                span,
                format!("cannot use `{actual}` where non-optional `{declared}` is required"),
                "E-OPT-ASSIGN",
                Some("unwrap it first with `??`, `?.`, `if (var …)`, or `!`".into()),
            );
        } else {
            self.err(span, format!("expected `{declared}`, found `{actual}`"));
        }
    }

    /// Every name currently visible — block-scope locals + top-level functions + (inside a method)
    /// the current class's fields — used to suggest the nearest match on an unknown identifier.
    pub(super) fn in_scope_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for scope in &self.scopes {
            names.extend(scope.keys().cloned());
        }
        names.extend(self.funcs.keys().cloned());
        if let Some(cls) = &self.cur_class {
            if let Some(info) = self.classes.get(cls) {
                names.extend(info.fields.keys().cloned());
            }
        }
        names
    }

    /// The closest candidate to `name` within a small edit distance (≤ 2), if any — the
    /// "did you mean `…`?" suggestion.
    pub(super) fn nearest_name(&self, name: &str, candidates: &[String]) -> Option<String> {
        candidates
            .iter()
            .map(|c| (levenshtein(name, c), c))
            .filter(|(d, _)| *d > 0 && *d <= 2)
            .min_by_key(|(d, _)| *d)
            .map(|(_, c)| c.clone())
    }

    // ---- M-faults 2b: checked-exception discharge ----

    // ---- scopes ----
    pub(super) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub(super) fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    pub(super) fn declare(&mut self, name: &str, ty: Ty, span: Span) {
        self.declare_binding(name, ty, false, span);
    }
    /// Declare a binding with an explicit mutability (M-mut.1). `declare` is the immutable-default
    /// shorthand used by params, patterns, and if-let bindings (none of which are reassignable).
    pub(super) fn declare_binding(&mut self, name: &str, ty: Ty, mutable: bool, span: Span) {
        // A value binding may not shadow an imported module qualifier: were `console` both a local
        // and `import core.console;`, the run backends (locals-first) would treat `console.x()` as a
        // method call while the transpiler (import-map-driven) would emit the native — a silent
        // divergence. Forbidding the overlap keeps all four backends consistent (M3 Wave 1).
        if self.imports.contains_key(name) {
            self.err_coded(
                span,
                format!("`{name}` shadows the imported module qualifier `{name}`"),
                "E-SHADOW-IMPORT",
                Some(format!(
                    "rename the binding, or remove the matching `import …{name};`"
                )),
            );
        }
        // Likewise, a value binding may not shadow a top-level function name. A bare `f(…)` call
        // dispatches functions-first in the run backends but locals-first in the transpiler, and a
        // bare `f` value reference resolves to the function in the backends but the local in PHP —
        // so an overlap is a silent four-backend divergence (the function-name analogue of
        // E-SHADOW-IMPORT, made reachable once functions became first-class values in M3 S3).
        if self.funcs.contains_key(name) {
            self.err_coded(
                span,
                format!("`{name}` shadows the top-level function `{name}`"),
                "E-SHADOW-FN",
                Some(format!(
                    "rename the binding; a local may not share a name with a function (`{name}` is callable as a value)"
                )),
            );
        }
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name.to_string(), (ty, mutable));
        }
    }
    /// A local binding's `(type, mutable)` — locals only (does not fall through to class fields).
    /// Used by the reassignment check (M-mut.1): a non-local target is `E-ASSIGN-UNKNOWN`.
    pub(super) fn lookup_binding(&self, name: &str) -> Option<(Ty, bool)> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.get(name) {
                return Some(b.clone());
            }
        }
        None
    }
    /// Resolve a name against the lexical scope stack only (params + locals + captures). Bare *field*
    /// access is intentionally NOT resolved here (2026-06-27): Phorj requires `this.field` everywhere,
    /// matching PHP's `$this->field` (no bare field access) — a bare field reference is reported as
    /// `E-BARE-FIELD` at the `Ident` site, not silently resolved.
    pub(super) fn lookup(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some((t, _)) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }
    /// Is `name` an instance field of the class currently being checked? Used by the `Ident` site to
    /// turn a bare field reference into a targeted `E-BARE-FIELD` ("write `this.{name}`").
    pub(super) fn is_cur_field(&self, name: &str) -> bool {
        self.cur_class
            .as_ref()
            .and_then(|c| self.classes.get(c))
            .is_some_and(|info| info.fields.contains_key(name))
    }

    // ---- totality: structural termination analysis (M-RT totality cluster) ----
    // Conservatively answers "does control definitely never fall through this block / statement?".
    // Drives return-on-all-paths (`E-MISSING-RETURN`/`E-NEVER-RETURN`) and the `W-UNREACHABLE` lint.
    // Soundness direction: returns `true` only for shapes that *provably* diverge — a false `true`
    // would silently suppress a real missing-return error, so it never over-claims.

    // ---- statements ----
}
