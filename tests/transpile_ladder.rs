//! The transpiler's Invariant-14 LADDER refusals, driven through the shipped binary so the code a
//! user sees on stderr is the one asserted. Both codes are emitted as message prefixes / brackets
//! rather than as `err_coded` arguments, which is why the surface ratchet needed its second and
//! third emit forms to see them at all.
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_phg");

fn transpile_stderr(name: &str, src: &str) -> String {
    let dir = std::env::temp_dir().join(format!("phorj-ladder-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.phg");
    std::fs::write(&path, src).unwrap();
    let out = Command::new(BIN)
        .args(["transpile", path.to_str().unwrap()])
        .output()
        .expect("spawn phg");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !out.status.success(),
        "transpile must refuse: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_native_only_unicode_function_is_refused_on_transpile() {
    let err = transpile_stderr(
        "unicode",
        "package Main;\nimport Core.String;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void { string s = String.unicodeUpper(\"straße\"); }\n",
    );
    assert!(err.contains("E-TRANSPILE-UNICODE"), "{err}");
}

/// `E-TRANSPILE-VARIANT-COLLISION` is a DEFENSIVE guard: a variant `Shape.Circle` emits the PHP
/// class `Shape_Circle`, and the guard refuses a user class of that spelling. But every colliding
/// spelling needs an underscore, and `is_pascal` (E-TYPE-CASE) rejects underscores in type names
/// before the transpiler ever runs — so through `phg transpile` the guard cannot fire. It is asserted
/// here the way the pass's own inline tests do, on a parsed-but-unchecked program, because the
/// surface ratchet cannot see an inline `#[cfg(test)]` module, and a guard nobody can observe is
/// exactly what the 100% rule refuses to count.
#[test]
fn a_variant_that_would_collide_with_a_class_is_refused_by_the_emitter() {
    let src = "package Main;\nenum Shape { Circle(float r) }\nclass Shape_Circle { }\nfunction main() -> void { }";
    let toks = phorj::tokenizer::lex(src).expect("lex");
    let prog = phorj::parser::Parser::new(toks)
        .parse_program()
        .expect("parse");
    let err = phorj::transpile::emit(&prog).expect_err("the emitter must refuse the collision");
    assert!(err.contains("E-TRANSPILE-VARIANT-COLLISION"), "{err}");
}
