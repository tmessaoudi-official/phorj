//! PHP transpiler — type declarations: enums, classes, traits, members.

use super::*;

impl Transpiler {
    /// An enum with payload variants becomes an abstract base class plus one `final`
    /// subclass per variant, with promoted public props for the payload fields.
    pub(super) fn emit_enum(&mut self, e: &EnumDecl) -> Result<(), String> {
        // The base + its variant subclasses are declared inside the enum's own `namespace` block, so
        // both use the bare trailing segment (`Acme\Geometry\Color` ⇒ `Color`); a single-package enum
        // is unchanged. Variant subclass names are never mangled (they aren't types).
        // Mangle a reserved enum-class name (`RoundingMode` → `RoundingMode_`) so it can't collide
        // with a PHP built-in enum (M-NUM S2); a non-reserved name is unchanged.
        let base = super::php_class_name(last_segment(&e.name));
        self.line(&format!("abstract class {} {{}}", base));
        for v in &e.variants {
            // A variant whose name is a PHP-reserved class word (`Int`/`Bool`/`Null`/…) is mangled
            // (`Int_`); the construction + `instanceof` sites mangle identically via `variant_ref`.
            let vname = super::php_variant_name(&v.name);
            // DEC-238: record `php-class → (enum, variant)` so `__phorj_debug_render` can render a
            // transpiled enum value as `Ty.Variant(...)` (never the mangled class shape).
            self.debug_enum_rows.push((
                vname.clone(),
                last_segment(&e.name).to_string(),
                v.name.clone(),
            ));
            self.line(&format!("final class {} extends {} {{", vname, base));
            self.indent += 1;
            if !v.fields.is_empty() {
                let props: Vec<String> = v
                    .fields
                    .iter()
                    .map(|p| format!("public {} ${}", self.emit_type(&p.ty), p.name))
                    .collect();
                self.line(&format!(
                    "public function __construct({}) {{}}",
                    props.join(", ")
                ));
            }
            self.indent -= 1;
            self.line("}");
        }
        Ok(())
    }

    pub(super) fn emit_class(&mut self, c: &ClassDecl, program: &Program) -> Result<(), String> {
        // Names of ctor params that PHP will promote to properties.
        let mut promoted_names: HashSet<String> = HashSet::new();
        for m in &c.members {
            if let ClassMember::Constructor { params, .. } = m {
                for p in params {
                    if is_promoted(&p.modifiers) {
                        promoted_names.insert(p.name.clone());
                    }
                }
            }
        }
        // Field set for `$this->` resolution = explicit decls + promoted ctor params
        // (mirrors the checker's `collect_class`).
        let mut fields: HashSet<String> = promoted_names.clone();
        for m in &c.members {
            if let ClassMember::Field { name, .. } = m {
                fields.insert(name.clone());
            }
        }
        // M-faults 2b: a class `implements Error` becomes a real PHP exception — `extends \Exception`
        // (so `throw` targets a `\Throwable`, and native `getMessage()` works). The built-in `Error`
        // marker has no PHP declaration, so it is dropped from the `implements` list; any *other*
        // interfaces stay. A promoted/declared field whose name collides with one of `\Exception`'s
        // own properties (`message`/`code`/`file`/`line`) must be emitted **untyped** — PHP rejects a
        // typed redeclaration of an inherited untyped property.
        let is_error = c.implements.iter().any(|i| last_segment(i) == "Error");
        let other_ifaces: Vec<String> = c
            .implements
            .iter()
            .filter(|i| last_segment(i) != "Error")
            .map(|i| php_type_ref(i))
            .collect();
        let extends_clause = if is_error {
            " extends \\Exception".to_string()
        } else if let Some(parent) = c.extends.first() {
            // M-RT S6: single inheritance → PHP `extends Parent`. (Multiple parents lower via trait
            // decomposition in S6b.)
            format!(" extends {}", php_type_ref(parent))
        } else {
            String::new()
        };
        let implements = if other_ifaces.is_empty() {
            String::new()
        } else {
            format!(" implements {}", other_ifaces.join(", "))
        };
        // Declared inside its `namespace` block in multi-package mode ⇒ bare trailing segment.
        let disp = if self.namespaced {
            last_segment(&c.name)
        } else {
            &c.name
        };
        // M-RT S6: final-by-default — a non-`open` class emits as a PHP `final class` (it can never be
        // a parent, since the checker rejects `extends` of a non-`open` class via E-EXTEND-FINAL). An
        // `open` class emits as a plain `class` so a subclass may `extends` it.
        let final_kw = if c.open { "" } else { "final " };
        self.line(&format!(
            "{final_kw}class {disp}{extends_clause}{implements} {{"
        ));
        self.indent += 1;
        // M-RT S8 + Wave 1.3: compose each `use`d trait. A collision-free composition emits a plain
        // `use Trait;` per trait. When two composed traits supply the same method name (resolved on the
        // Phorj side by `use P.m`/`rename`/`exclude`), emit a single combined `use P, Q { … }` block
        // with the PHP `insteadof`/`as` clauses — otherwise PHP rejects the composition with a trait
        // method collision. Mirrors `build_trait_clauses` (the MI-decomposition analogue) for the
        // explicit trait-composition path. Trait names are used directly (no `T` prefix, unlike MI).
        if !c.uses.is_empty() {
            let clauses = self.build_use_trait_clauses(c, program);
            if clauses.is_empty() {
                for u in &c.uses {
                    self.line(&format!("use {};", self.type_pos_ref(&u.name)));
                }
            } else {
                let names: Vec<String> =
                    c.uses.iter().map(|u| self.type_pos_ref(&u.name)).collect();
                self.line(&format!("use {} {{", names.join(", ")));
                self.indent += 1;
                for cl in &clauses {
                    self.line(cl);
                }
                self.indent -= 1;
                self.line("}");
            }
        }
        let prev = self.cur_class_fields.replace(fields);
        self.emit_class_members(c, &promoted_names, is_error, false)?;
        self.cur_class_fields = prev;
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// M-RT S8: emit a native PHP `trait` from a [`crate::ast::TraitDecl`]. Members are emitted in
    /// trait mode (`as_trait = true`) — promoted ctor params become plain properties — reusing the
    /// shared `emit_class_members`. A trait is `package Main`-only this slice, so its name is bare.
    pub(super) fn emit_trait(&mut self, t: &crate::ast::TraitDecl) -> Result<(), String> {
        let mut promoted_names: HashSet<String> = HashSet::new();
        let mut fields: HashSet<String> = HashSet::new();
        for m in &t.members {
            match m {
                ClassMember::Constructor { params, .. } => {
                    for p in params {
                        if is_promoted(&p.modifiers) {
                            promoted_names.insert(p.name.clone());
                            fields.insert(p.name.clone());
                        }
                    }
                }
                ClassMember::Field { name, .. } => {
                    fields.insert(name.clone());
                }
                _ => {}
            }
        }
        let synthetic = ClassDecl {
            vis: crate::ast::Visibility::Public,
            attrs: Vec::new(), // synthetic trait→class carries no attributes
            name: t.name.clone(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            extends: Vec::new(),
            implements: Vec::new(),
            implements_args: Vec::new(),
            open: true,
            is_abstract: false,
            sealed: false,
            resolutions: Vec::new(),
            uses: Vec::new(),
            members: t.members.clone(),
            foreign: false,
            span: t.span,
        };
        let disp = if self.namespaced {
            last_segment(&t.name)
        } else {
            &t.name
        };
        self.line(&format!("trait {disp} {{"));
        self.indent += 1;
        let prev = self.cur_class_fields.replace(fields);
        // `as_trait = false`: a USER trait emits like a normal class body — including a real
        // `__construct` with promotion (M-RT S8 T3). PHP makes that `__construct` the using class's
        // constructor automatically (a class composes at most one trait ctor — the checker rejects two
        // via `E-TRAIT-CTOR-COLLISION`). This differs from the S6 MI decomposition, which uses
        // `as_trait = true` precisely to suppress colliding multi-parent trait ctors.
        self.emit_class_members(&synthetic, &promoted_names, false, false)?;
        self.cur_class_fields = prev;
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    /// Emit a class's members (fields, constructor, methods, hooks) — the shared body used by a plain
    /// `class` (`emit_class`) and a multi-parent class (`emit_multi_class`, M-RT S6b). The caller has
    /// already emitted the class header + opening `{`, raised the indent, and set `cur_class_fields`;
    /// it restores them after.
    ///
    /// `as_trait` (M-RT S6c.2b): when emitting a decomposed class's *trait* body, a constructor cannot
    /// be a `__construct` (two trait constructors collide fatally in PHP), so its promoted params are
    /// emitted as PLAIN `public` fields and its body is dropped — the construction logic moves to an
    /// explicit-assignment `__construct` on the concrete class / multi-parent subclass
    /// (`emit_synth_construct`).
    pub(super) fn emit_class_members(
        &mut self,
        c: &ClassDecl,
        promoted_names: &HashSet<String>,
        is_error: bool,
        as_trait: bool,
    ) -> Result<(), String> {
        // T6b: `this` inside these method bodies resolves to `c`'s class for field-read kinds.
        let prev_class = self.cur_class.replace(c.name.clone());
        let result = self.emit_class_members_inner(c, promoted_names, is_error, as_trait);
        self.cur_class = prev_class;
        result
    }

    fn emit_class_members_inner(
        &mut self,
        c: &ClassDecl,
        promoted_names: &HashSet<String>,
        is_error: bool,
        as_trait: bool,
    ) -> Result<(), String> {
        let mut emitted_method_overloads: HashSet<String> = HashSet::new();
        for m in &c.members {
            match m {
                ClassMember::Field {
                    modifiers,
                    ty,
                    name,
                    init,
                    ..
                } => {
                    // A field that is ALSO a promoted ctor param is declared by the
                    // promotion — emitting it again is a PHP "redeclare" fatal.
                    if promoted_names.contains(name) {
                        continue;
                    }
                    // A typed PHP property requires a visibility keyword (`int $x;` is a syntax
                    // error). Phorj fields are immutable-by-default and visibility is not enforced
                    // at runtime by the backends, so a field with no explicit visibility (e.g.
                    // `mutable int x;`) emits as `public` — the spine-safe choice (M-mut.6).
                    let v = vis(modifiers);
                    let v = if v.is_empty() {
                        "public".to_string()
                    } else {
                        v
                    };
                    if modifiers.contains(&Modifier::Const) {
                        // A `const` class constant (Feature A) → a PHP **typed class constant**
                        // `[vis] const TYPE NAME = <literal>;` (PHP 8.3+; floor 8.5 ✓). Accessed
                        // `Class::NAME` (no `$`), distinct from a static field's `Class::$name`. The
                        // initializer is a checker-validated literal, so it round-trips byte-identically.
                        let init_php = match init {
                            Some(e) => self.emit_expr(e)?,
                            None => "null".to_string(),
                        };
                        self.line(&format!(
                            "{v} const {} {name} = {init_php};",
                            self.emit_type(ty)
                        ));
                    } else if modifiers.contains(&Modifier::Static) {
                        // A `static` field → PHP `public static <type> $name`. A **literal** initializer
                        // round-trips as a PHP default (`= 0;`). A **non-literal** initializer (Feature
                        // B-static) can't be a PHP property default (PHP requires a constant expression),
                        // so the property is declared *without* a default and set once by
                        // `__phorj_init_statics()` before `main()`.
                        match init
                            .as_ref()
                            .filter(|e| crate::value::const_literal(e).is_some())
                        {
                            Some(e) => {
                                let init_php = self.emit_expr(e)?;
                                self.line(&format!(
                                    "{v} static {} ${name} = {init_php};",
                                    self.emit_type(ty)
                                ));
                            }
                            None => {
                                self.line(&format!("{v} static {} ${name};", self.emit_type(ty)));
                            }
                        }
                    } else if is_error && exception_reserved(name) {
                        // Collides with an inherited \Exception property → emit untyped.
                        self.line(&format!("{v} ${name};"));
                    } else {
                        self.line(&format!("{v} {} ${name};", self.emit_type(ty)));
                    }
                }
                ClassMember::Constructor {
                    modifiers,
                    params,
                    body,
                    ..
                } => {
                    // Batch A: a `private`/`protected constructor` emits the PHP visibility keyword on
                    // `__construct` (so PHP enforces it natively, matching the checker). A public/default
                    // ctor stays bare (`function __construct`) for byte-identity with prior output.
                    let cvis = match vis(modifiers).as_str() {
                        "private" => "private ",
                        "protected" => "protected ",
                        _ => "",
                    };
                    // M-RT S6c.2b: in a decomposed class's trait, a constructor can't be `__construct`
                    // (two trait `__construct`s are a PHP fatal). Emit its promoted params as plain
                    // `public` fields (the trait owns the storage); the construction logic moves to the
                    // concrete class / multi-parent subclass via `emit_synth_construct`.
                    if as_trait {
                        for p in params {
                            if is_promoted(&p.modifiers) {
                                self.line(&format!(
                                    "public {} ${};",
                                    self.emit_type(&p.ty),
                                    p.name
                                ));
                            }
                        }
                        continue;
                    }
                    // M-faults 2c: a promoted `cause` param of marker-`Error` type on an Error subtype
                    // feeds PHP's native exception chain (`$previous`) — recognized by name + type so a
                    // mis-typed `cause` stays a plain field. Emitted as `?\Throwable` (the `$previous`
                    // type), not the engine `Error` class.
                    let is_cause = |p: &CtorParam| {
                        is_error
                            && !vis(&p.modifiers).is_empty()
                            && p.name == "cause"
                            && is_error_marker_type(&p.ty)
                    };
                    // Fork B: the injected `Secret` class's promoted value param is marked
                    // `#[\SensitiveParameter]` so PHP redacts it in stack traces (the `K-secrets-type`
                    // intent). Keyed by class name — every other class is byte-identical to before.
                    let is_secret = c.name == "Secret";
                    let ps: Vec<String> = params
                        .iter()
                        .map(|p| {
                            let v = vis(&p.modifiers);
                            // A promoted param whose name collides with an \Exception property is
                            // emitted untyped (PHP rejects a typed redeclaration); a plain param keeps
                            // its type (it is not a property).
                            let untyped = is_error && !v.is_empty() && exception_reserved(&p.name);
                            let attr = if is_secret && !v.is_empty() {
                                "#[\\SensitiveParameter] "
                            } else {
                                ""
                            };
                            if is_cause(p) {
                                format!("{attr}{v} ?\\Throwable ${}", p.name)
                            } else if v.is_empty() {
                                format!("{attr}{} ${}", self.emit_type(&p.ty), p.name)
                            } else if untyped {
                                format!("{attr}{} ${}", v, p.name)
                            } else {
                                format!("{attr}{} {} ${}", v, self.emit_type(&p.ty), p.name)
                            }
                        })
                        .collect();
                    // For an Error subtype, feed \Exception's own stores via `parent::__construct`:
                    // `$message` (so native `getMessage()` works) and, when a conventional `cause` is
                    // promoted, `$cause` as the 3rd `$previous` arg (so `getPrevious()` reports the
                    // cause chain idiomatically — interop + the 2c bridge). `$code` is 0 (Phorj has no
                    // exception-code surface). Either, both, or neither may be present.
                    let has_message = is_error
                        && params
                            .iter()
                            .any(|p| !vis(&p.modifiers).is_empty() && p.name == "message");
                    let has_cause = params.iter().any(is_cause);
                    let parent_args = match (has_message, has_cause) {
                        (true, true) => Some("$message, 0, $cause"),
                        (false, true) => Some("\"\", 0, $cause"),
                        (true, false) => Some("$message"),
                        (false, false) => None,
                    };
                    // Feature B: this class's own expression field initializers lower into the ctor
                    // prelude (after promotion + any `parent::__construct`, before the body), so an
                    // initializer reads `this` and an earlier sibling — matching the Rust backends.
                    let field_inits = crate::ast::own_field_initializers(c);
                    if body.is_empty() && parent_args.is_none() && field_inits.is_empty() {
                        self.line(&format!(
                            "{cvis}function __construct({}) {{}}",
                            ps.join(", ")
                        ));
                    } else {
                        self.line(&format!("{cvis}function __construct({}) {{", ps.join(", ")));
                        self.indent += 1;
                        self.push_scope();
                        for p in params {
                            self.declare(&p.name);
                        }
                        if let Some(args) = parent_args {
                            self.line(&format!("parent::__construct({args});"));
                        }
                        for (fname, init) in &field_inits {
                            let e = self.emit_expr(init)?;
                            self.line(&format!("$this->{fname} = {e};"));
                        }
                        for s in body {
                            self.emit_stmt(s)?;
                        }
                        self.pop_scope();
                        self.indent -= 1;
                        self.line("}");
                    }
                }
                ClassMember::Method(f) => {
                    // Group M-RT method overloads (methods of one name on this class).
                    let group: Vec<&FunctionDecl> = c
                        .members
                        .iter()
                        .filter_map(|mm| match mm {
                            ClassMember::Method(g) if g.name == f.name => Some(g),
                            _ => None,
                        })
                        .collect();
                    if group.len() > 1 {
                        if emitted_method_overloads.insert(f.name.clone()) {
                            self.emit_overload_set(&f.name, &group, true)?;
                        }
                    } else {
                        self.emit_function(f, true)?;
                    }
                }
                // A property hook (M-mut.7b) → a PHP 8.4 property hook. The hook is virtual (no
                // backing store), so it emits no default; the get expression and set block reference
                // *other* (real) fields. `public` because Phorj does not enforce field visibility.
                ClassMember::Hook {
                    ty, name, get, set, ..
                } => {
                    let pty = self.emit_type(ty);
                    self.line(&format!("public {pty} ${name} {{"));
                    self.indent += 1;
                    if let Some(g) = get {
                        let e = self.emit_expr(g)?;
                        self.line(&format!("get => {e};"));
                    }
                    if let Some((p, body)) = set {
                        self.line(&format!("set({pty} ${}) {{", p.name));
                        self.indent += 1;
                        self.push_scope();
                        self.declare(&p.name);
                        for s in body {
                            self.emit_stmt(s)?;
                        }
                        self.pop_scope();
                        self.indent -= 1;
                        self.line("}");
                    }
                    self.indent -= 1;
                    self.line("}");
                }
            }
        }
        // Feature B: a class with expression field initializers but NO constructor needs a synthesized
        // zero-arg `__construct` to run them (PHP property defaults can't be arbitrary expressions). Not
        // for a decomposed trait body (`as_trait`) — its construction is emitted via `emit_synth_construct`.
        if !as_trait
            && !c
                .members
                .iter()
                .any(|m| matches!(m, ClassMember::Constructor { .. }))
        {
            let field_inits = crate::ast::own_field_initializers(c);
            if !field_inits.is_empty() {
                self.line("function __construct() {");
                self.indent += 1;
                self.push_scope();
                for (fname, init) in &field_inits {
                    let e = self.emit_expr(init)?;
                    self.line(&format!("$this->{fname} = {e};"));
                }
                self.pop_scope();
                self.indent -= 1;
                self.line("}");
            }
        }
        Ok(())
    }
}
