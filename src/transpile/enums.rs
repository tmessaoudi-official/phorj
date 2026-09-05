//! PHP transpiler — ENUM declarations (DEC-302 backed enums, DEC-329.3 scoped variant classes).
//!
//! Split out of `classes.rs` by cohesion (Invariant 13): that file was a grandfathered 543-line breach
//! that the size gate forbids GROWING, and attribute emission needed one line in `emit_class`. The enum
//! emitter is the largest self-contained unit in it and shares nothing with the class/trait emitters
//! beyond the `Transpiler` itself — so the split burns the debt down instead of deferring it again.

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
        // DEC-302 backed enum (repr B): the base carries a `value` property + static `cases()`/
        // `from()`/`tryFrom()`; each variant sets `$this->value` in its ctor. `cases()` is also
        // emitted for a plain payload-less enum (it's valid on any). A payload enum is unchanged.
        let all_payload_less = e.variants.iter().all(|v| v.fields.is_empty());
        let backed = e.backing_type.is_some();
        if backed || all_payload_less {
            self.line(&format!("abstract class {base} {{"));
            self.indent += 1;
            if let Some(bt) = &e.backing_type {
                self.line(&format!("public {} $value;", self.emit_type(bt)));
            }
            // cases() → a PHP array of one fresh instance per variant, declaration order.
            let cases: Vec<String> = e
                .variants
                .iter()
                .map(|v| format!("new {}()", super::php_scoped_variant_name(&e.name, &v.name)))
                .collect();
            self.line("public static function cases(): array {");
            self.indent += 1;
            self.line(&format!("return [{}];", cases.join(", ")));
            self.indent -= 1;
            self.line("}");
            if backed {
                // from(x): first variant whose backing equals x (=== ), else throw; tryFrom: null.
                for (method, miss) in [
                    ("from", "throw new \\ValueError(\"no matching case\")"),
                    ("tryFrom", "return null"),
                ] {
                    let ret = if method == "from" {
                        base.clone()
                    } else {
                        format!("?{base}")
                    };
                    let bt = self.emit_type(e.backing_type.as_ref().unwrap());
                    self.line(&format!(
                        "public static function {method}({bt} $value): {ret} {{"
                    ));
                    self.indent += 1;
                    self.line("foreach (self::cases() as $c) {");
                    self.indent += 1;
                    self.line("if ($c->value === $value) { return $c; }");
                    self.indent -= 1;
                    self.line("}");
                    self.line(&format!("{miss};"));
                    self.indent -= 1;
                    self.line("}");
                }
            }
            self.indent -= 1;
            self.line("}");
        } else {
            self.line(&format!("abstract class {base} {{}}"));
        }
        for v in &e.variants {
            // DEC-329.3: variant classes are enum-SCOPED (`Shape_Circle`) — collision-proof and
            // never a bare reserved word; construction/`instanceof` match via `variant_ref`.
            let vname = super::php_scoped_variant_name(&e.name, &v.name);
            // DEC-238: record `php-class → (enum, variant)` so `__phorj_debug_render` can render a
            // transpiled enum value as `Ty.Variant(...)` (never the mangled class shape).
            // TRANSPILE-NS-REFLECT-TABLES (measured 2026-09-05): under namespaced emission the
            // class is declared inside `namespace Acme {`, so `get_class` returns `Acme\Color_Green`
            // — the row is keyed by that FQN (never stripped at lookup: `Acme\Color_Green` and
            // `Other\Color_Green` share a leaf). The enum NAME is `e.name` as the checker holds it —
            // bare for the entry package (`Local`), mangled for a library package (`Acme\Color`) —
            // which is exactly what the interpreter's dump prints, so the three legs agree.
            let key = match &self.current_ns {
                Some(ns) => format!("{ns}\\{vname}"),
                None => vname.clone(),
            };
            self.debug_enum_rows
                .push((key, e.name.clone(), v.name.clone()));
            self.line(&format!("final class {} extends {} {{", vname, base));
            self.indent += 1;
            if let Some(bv) = &v.backing_value {
                // DEC-302: a backed variant's ctor sets the scalar `value` (the base declares it).
                let lit = self.emit_expr(bv)?;
                self.line(&format!(
                    "public function __construct() {{ $this->value = {lit}; }}"
                ));
            } else if !v.fields.is_empty() {
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
}
