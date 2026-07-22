//! DEC-325 P1 / DEC-329.3 — pre-emission collision check (split from `mod.rs` per Invariant 13).

use crate::ast::{Item, Program};

/// DEC-329.3: variant classes are enum-SCOPED (`Shape_Circle`), so two enums sharing a variant
/// name emit distinct classes and the old flat-name refusal is LIFTED. What remains is the
/// pathological composed-name collision — a scoped name equal to another top-level PHP class in
/// the same namespace (`class Shape_Circle` beside `enum Shape { Circle }`, or `enum A_B { C }`
/// beside `enum A { B_C }`). Still refused LOUDLY (THE LADDER RULE: refusing beats a
/// silently-fatal `Cannot redeclare class`); the program still runs on the Rust legs.
pub(super) fn check_variant_collisions(program: &Program) -> Result<(), String> {
    // Every top-level PHP class name the emission will declare, keyed (namespace, class name).
    let mut seen: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    let claim = |seen: &mut std::collections::HashMap<(String, String), String>,
                 ns: String,
                 class: String,
                 what: String|
     -> Option<(String, String)> {
        seen.insert((ns, class), what.clone()).map(|w| (w, what))
    };
    for it in &program.items {
        let (name, what) = match it {
            Item::Class(c) if !c.foreign => (&c.name, "class"),
            Item::Interface(i) => (&i.name, "interface"),
            Item::Trait(t) => (&t.name, "trait"),
            Item::Enum(e) => {
                let ns = super::namespace_of(&e.name);
                let base = super::php_class_name(super::last_segment(&e.name));
                claim(&mut seen, ns.clone(), base, format!("enum `{}`", e.name));
                for v in &e.variants {
                    let scoped = super::php_scoped_variant_name(&e.name, &v.name);
                    if let Some((first, second)) = claim(
                        &mut seen,
                        ns.clone(),
                        scoped.clone(),
                        format!("variant `{}.{}`", e.name, v.name),
                    ) {
                        return Err(format!(
                            "transpile error: {second} and {first} would both emit the PHP class \
                             `{scoped}` — rename one (the program still runs with `phg run`) \
                             [E-TRANSPILE-VARIANT-COLLISION]"
                        ));
                    }
                }
                continue;
            }
            _ => continue,
        };
        let ns = super::namespace_of(name);
        let leaf = super::php_class_name(super::last_segment(name));
        // A duplicate among plain classes/interfaces is a checker-level name error, not this
        // guard's concern — only record them so a variant's SCOPED name colliding with one trips
        // the check above (item order puts enums wherever the source declares them, so also test
        // here when the class comes second).
        if let Some((first, second)) =
            claim(&mut seen, ns, leaf.clone(), format!("{what} `{name}`"))
        {
            if first.starts_with("variant ") || second.starts_with("variant ") {
                return Err(format!(
                    "transpile error: {second} and {first} would both emit the PHP class \
                     `{leaf}` — rename one (the program still runs with `phg run`) \
                     [E-TRANSPILE-VARIANT-COLLISION]"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    fn transpiled(src: &str) -> Result<String, String> {
        let toks = crate::tokenizer::lex(src).unwrap();
        let prog = crate::parser::Parser::new(toks).parse_program().unwrap();
        crate::transpile::emit(&prog)
    }

    #[test]
    fn shared_variant_names_now_emit_distinct_scoped_classes() {
        // Pre-DEC-329.3 this was the E-TRANSPILE-VARIANT-COLLISION refusal; scoping lifts it.
        let src = "package Main;\nenum A { Dup(int x) }\nenum B { Dup(string y) }\nfunction main() -> void { }";
        let php = transpiled(src).expect("shared variant names transpile now");
        assert!(php.contains("final class A_Dup extends A"), "{php}");
        assert!(php.contains("final class B_Dup extends B"), "{php}");
    }

    #[test]
    fn pathological_scoped_name_collision_is_a_clean_transpile_error() {
        // `A_B.C` and `A.B_C` both compose to the PHP class `A_B_C`.
        let src = "package Main;\nenum A_B { C(int x) }\nenum A { B_C(string y) }\nfunction main() -> void { }";
        let err = transpiled(src).expect_err("composed-name collision must refuse");
        assert!(err.contains("E-TRANSPILE-VARIANT-COLLISION"), "{err}");
        assert!(err.contains("A_B_C"), "{err}");
    }

    #[test]
    fn class_named_like_a_scoped_variant_is_a_clean_transpile_error() {
        let src = "package Main;\nenum Shape { Circle(float r) }\nclass Shape_Circle { }\nfunction main() -> void { }";
        let err = transpiled(src).expect_err("class-vs-scoped collision must refuse");
        assert!(err.contains("E-TRANSPILE-VARIANT-COLLISION"), "{err}");
    }
}
