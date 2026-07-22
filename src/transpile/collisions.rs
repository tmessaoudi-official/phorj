//! DEC-325 P1 — pre-emission collision checks (split from `mod.rs` per Invariant 13).

use crate::ast::{Item, Program};

/// Two enums sharing a variant name would emit colliding flat PHP classes (`final class <Variant>`)
/// — refuse LOUDLY instead (THE LADDER RULE: refusing beats a silently-fatal program). The program
/// still runs on the Rust legs; enum-scoped variant classes are the recorded follow-up.
pub(super) fn check_variant_collisions(program: &Program) -> Result<(), String> {
    let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for it in &program.items {
        if let Item::Enum(e) = it {
            for v in &e.variants {
                if let Some(first) = seen.insert(v.name.as_str(), e.name.as_str()) {
                    if first != e.name {
                        return Err(format!(
                            "transpile error: variant `{}` is declared by both `{first}` and `{}` — \
                             the PHP class model emits flat variant classes, so transpiling would \
                             redeclare it [E-TRANSPILE-VARIANT-COLLISION]. Rename one variant (the \
                             program still runs with `phg run`); enum-scoped variant classes are a \
                             recorded follow-up.",
                            v.name, e.name
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn colliding_variant_names_are_a_clean_transpile_error() {
        let src = "package Main;\nenum A { Dup(int x) }\nenum B { Dup(string y) }\nfunction main() -> void { }";
        let toks = crate::tokenizer::lex(src).unwrap();
        let prog = crate::parser::Parser::new(toks).parse_program().unwrap();
        let err = crate::transpile::emit(&prog).expect_err("collision must refuse");
        assert!(err.contains("E-TRANSPILE-VARIANT-COLLISION"), "{err}");
        assert!(err.contains('A') && err.contains('B'), "{err}");
    }
}
