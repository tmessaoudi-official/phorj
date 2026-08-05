//! PHP transpiler — **attribute re-emission** (developer-ruled 2026-08-05, DEC-437).
//!
//! phorj attributes used to be erased entirely on the PHP leg. They are now re-emitted, so a
//! transpiled program's metadata is visible to PHP-side reflection and a `phorj → PHP → phorj` round
//! trip keeps its attributes instead of dropping them silently.
//!
//! # What is emitted, and what is NOT — this is where byte-identity is either kept or lost
//!
//! **USER attributes** (a use of a class declared `#[Attribute]`) and the `#[Attribute]` MARKER itself.
//! The marker is not optional decoration: without it PHP's `ReflectionAttribute::newInstance()` refuses
//! with *"Attempting to use non-attribute class"*, so emitting the uses without it would produce
//! metadata PHP cannot actually read.
//!
//! **BUILT-IN attributes are never emitted** — `#[Entry]`, `#[Route]`, `#[Config]`, `#[Injectable]`,
//! `#[Provides]`, `#[Transient]`, `#[Invoke]`, `#[ToString]`, `#[UncheckedOverflow]`, `#[Deprecated]`.
//! They are phorj COMPILE-TIME machinery: consumed by a desugar (`#[Entry]` → the entry call,
//! `#[Route]` → an `autoRouter` registration, `#[Invoke]`/`#[ToString]` → PHP's `__invoke`/`__toString`)
//! or refused outright (`#[UncheckedOverflow]` is `E-TRANSPILE-UNCHECKED`). They describe how phorj
//! compiles the program, not what the program IS, so erasing them is correct rather than lossy.
//!
//! `#[Deprecated]` is in that list for a HARDER reason, and it is measured, not assumed: PHP 8.4's own
//! `#[\Deprecated]` has RUNTIME behaviour — calling the function prints
//! `Deprecated: Function greet() is deprecated, …`. phorj's `#[Deprecated]` is compile-time only
//! (DEC-417: use-site warnings come from the reference pass, at CHECK time). Mapping the two would make
//! the PHP leg print a line the VM and interpreter do not — a direct Invariant 1 break.
//! [Verified: `#[\Deprecated(message: "use shout")] function greet()` under php-8.5.8 prints the notice
//! to output; the same program on either phorj engine prints nothing.]
//!
//! # The argument gate
//!
//! PHP parses an attribute's arguments as a **constant expression at compile time**, and a function
//! call is not one: `#[SomeAttr(f())]` is an outright
//! *"Fatal error: Constant expression contains invalid operations"* — the whole FILE dies, before any
//! output. [Verified under php-8.5.8.] That is not hypothetical for phorj, because arithmetic lowers to
//! a runtime helper: `#[Tag(1 + 2)]` would emit `#[Tag(__phorj_checked_add(1, 2))]` and kill the file.
//! phorj accepts a computed attribute argument today [Verified: `#[Tag(1 + 2)]` type-checks clean], so
//! the shape is reachable.
//!
//! So only arguments with a faithful PHP CONSTANT form are emitted — literals, an enum member, and
//! literal lists/maps of those. An attribute with any other argument is **not** emitted, and the PHP
//! output carries a comment naming it, because a dropped attribute must never be invisible (DEC-166 /
//! Invariant 14: disclosed, never silent). Constant-FOLDING such arguments (`1 + 2` → `3`) would emit
//! them faithfully and is the obvious next step — phorj has no constant folder yet, so it is recorded as
//! a follow-up rather than half-built here.

use super::*;

impl Transpiler {
    /// The `#[…]` lines for a declaration, already indented, or an empty string.
    pub(super) fn attr_lines(&mut self, attrs: &[Attribute]) -> String {
        let mut out = String::new();
        for attr in attrs {
            // The MARKER: this declaration IS an attribute class. PHP's own `#[\Attribute]` means
            // exactly that, and the root `\` keeps it resolving to PHP's built-in from inside a
            // `namespace` block.
            if attr.is_attribute_marker() {
                out.push_str(&self.attr_line("\\Attribute", &[]));
                continue;
            }
            // Every other built-in is compile-time machinery with nothing to say in PHP.
            if is_builtin_attribute(attr) {
                continue;
            }
            let Some(name) = self.user_attribute_php_name(attr) else {
                // Not a resolvable user attribute. The checker has already reported this
                // (`E-UNKNOWN-ATTRIBUTE`), so transpile stays silent rather than inventing a name.
                continue;
            };
            match self.php_const_args(attr) {
                Some(args) => out.push_str(&self.attr_line(&name, &args)),
                None => out.push_str(&self.attr_comment(&name)),
            }
        }
        out
    }

    /// One `#[Name(args)]` line at the current indentation.
    fn attr_line(&self, name: &str, args: &[String]) -> String {
        let pad = "    ".repeat(self.indent);
        if args.is_empty() {
            format!("{pad}#[{name}]\n")
        } else {
            format!("{pad}#[{name}({})]\n", args.join(", "))
        }
    }

    /// The PHP class name of the user attribute this use names, resolved by CANONICAL PATH — the same
    /// rule `check_user_attribute_use` applies (DEC-435), so the transpiler cannot bind a name the
    /// checker validated against a different class.
    ///
    /// More than one hit is `E-AMBIGUOUS-ATTRIBUTE`, already reported by the checker, so `None` is
    /// returned rather than a coin flip.
    fn user_attribute_php_name(&self, attr: &Attribute) -> Option<String> {
        let mut hit = None;
        for (canonical, php) in &self.attr_classes {
            if crate::ast::attr_path_matches(&attr.name, canonical) {
                if hit.is_some() {
                    return None;
                }
                hit = Some(php.clone());
            }
        }
        hit
    }

    /// The disclosure for an attribute whose arguments have no PHP constant form.
    fn attr_comment(&self, name: &str) -> String {
        let pad = "    ".repeat(self.indent);
        format!(
            "{pad}// phorj: `#[{name}(…)]` not re-emitted — an argument has no PHP constant form \
             (PHP evaluates attribute arguments as constant expressions).\n"
        )
    }
}

/// Is this one of phorj's built-in attributes? Defined against the SINGLE enumeration
/// [`crate::ast::BUILTIN_ATTRIBUTE_PATHS`] rather than a hand-written list, so a new built-in is
/// automatically excluded from PHP emission instead of leaking out the first time someone forgets.
fn is_builtin_attribute(attr: &Attribute) -> bool {
    crate::ast::BUILTIN_ATTRIBUTE_PATHS
        .iter()
        .any(|(canonical, _)| crate::ast::attr_path_matches(&attr.name, canonical))
}

/// A PHP single-quoted string body: only `\` and `'` are special inside one, so nothing else is
/// touched — which is what keeps an attribute argument byte-faithful to the phorj literal.
fn php_single_quoted(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

impl Transpiler {
    /// Every argument rendered as a PHP constant expression, or `None` if any argument has no such form.
    ///
    /// All-or-nothing on purpose: emitting an attribute with SOME of its arguments would be a silent
    /// semantic change to the metadata, which is worse than not emitting it and saying so.
    fn php_const_args(&self, attr: &Attribute) -> Option<Vec<String>> {
        attr.args.iter().map(|a| self.php_const_arg(a)).collect()
    }

    /// One attribute argument as a PHP constant expression.
    ///
    /// Deliberately narrow — a shape is admitted only when its PHP form is exactly as constant as the
    /// phorj one. An arithmetic expression is NOT, because it lowers to a helper CALL and PHP fatals the
    /// whole file on a call in an attribute argument.
    fn php_const_arg(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Int(n, _) => Some(n.to_string()),
            Expr::Bool(b, _) => Some(if *b { "true".into() } else { "false".into() }),
            Expr::Null(_) => Some("null".into()),
            // The same `{x:?}` spelling the expression emitter uses (`12.0` stays `12.0`), so an
            // attribute argument and an ordinary literal cannot render differently.
            Expr::Float(f, _) => Some(format!("{f:?}")),
            // A single-part string only: an INTERPOLATED string lowers to concatenation, which is not a
            // constant expression in PHP.
            Expr::Str(parts, _) => match parts.as_slice() {
                [StrPart::Literal(s)] => Some(format!("'{}'", php_single_quoted(s))),
                [] => Some("''".into()),
                _ => None,
            },
            Expr::List(items, _) => {
                let rendered: Option<Vec<String>> =
                    items.iter().map(|i| self.php_const_arg(i)).collect();
                Some(format!("[{}]", rendered?.join(", ")))
            }
            Expr::Map(pairs, _) => {
                let mut out = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    out.push(format!(
                        "{} => {}",
                        self.php_const_arg(k)?,
                        self.php_const_arg(v)?
                    ));
                }
                Some(format!("[{}]", out.join(", ")))
            }
            // A named argument keeps its name — PHP 8.0 spells it identically.
            Expr::NamedArg { name, value, .. } => {
                Some(format!("{name}: {}", self.php_const_arg(value)?))
            }
            // A CONSTRUCTION — `new Colour.Red()`, `new Inner("x")`. `Expr::New` is unwrapped by the
            // checker before any backend, so what arrives here is the inner `Call`, and only a
            // construction is admissible: PHP 8.1 allows `new` in an attribute argument (evaluated on
            // REFLECTION, not at parse time) [Verified under php-8.5.8], while an ordinary function call
            // is *"Constant expression contains invalid operations"* and kills the whole file.
            //
            // The first version of this gate matched `Expr::Member` instead — the spelling `Colour.Red`
            // WITHOUT `new`, which Invariant 12 makes invalid phorj everywhere (`E-NEW-REQUIRED`). So it
            // matched a shape that can never arrive, and every enum-valued attribute silently fell
            // through to "no PHP constant form". Found by writing the reproducer properly.
            Expr::Call { callee, args, .. } => self.php_const_construction(callee, args),
            // `Expr::New` REACHES the transpiler inside an attribute argument, contradicting both
            // Invariant 5's "expanded out before any backend" discipline and `Expr::New`'s own doc
            // comment ("the interpreter/compiler/transpiler never see it"). [Verified: neither
            // `unwrap_new` (`checker/rewrite_new.rs:50`) nor `qualify_variants`
            // (`checker/qualify_variants.rs:46`) walks `attrs` — both visit function bodies and class
            // members only.] So the wrapper is unwrapped HERE.
            //
            // Note the latent hazard this exposes: `emit_expr` carries
            // `unreachable!("Expr::New is unwrapped before transpilation")`, so anything that ever routes
            // an attribute argument through the ordinary expression emitter would PANIC on valid user
            // code. Nothing does today (this gate is the only reader), and teaching the desugars to walk
            // `attrs` is the root fix — recorded as its own item, because every attribute-consuming
            // desugar reads attributes STRUCTURALLY and would have to be re-checked against the change.
            Expr::New(inner, _) => self.php_const_arg(inner),
            _ => None,
        }
    }

    /// A construction call rendered as `new <PhpName>(<const args>)`, or `None` when the callee is not a
    /// class/enum-variant construction. The three callee shapes and their guards mirror
    /// `emit_call`'s own dispatch exactly, so this cannot admit a call the emitter would treat
    /// differently.
    fn php_const_construction(&self, callee: &Expr, args: &[Expr]) -> Option<String> {
        let rendered: Option<Vec<String>> = args.iter().map(|a| self.php_const_arg(a)).collect();
        let argv = rendered?.join(", ");
        match callee {
            // Bare variant (`Red()` after a variant import) or a declared class (`Inner("x")`).
            Expr::Ident(name, _) => {
                if let Some(en) = self.variant_owner.get(name) {
                    return Some(format!("new {}({argv})", self.variant_ref(en, name)));
                }
                if self.classes.contains(name) {
                    return Some(format!("new {}({argv})", super::php_type_ref(name)));
                }
                None
            }
            // Qualified variant (`Colour.Red()`) — the canonical form `qualify_variants` produces.
            Expr::Member {
                object,
                name,
                safe: false,
                ..
            } => match object.as_ref() {
                Expr::Ident(en, _)
                    if !self.is_local(en)
                        && self.enums.contains(en)
                        && self.variants.contains(name) =>
                {
                    Some(format!("new {}({argv})", self.variant_ref(en, name)))
                }
                _ => None,
            },
            _ => None,
        }
    }
}
