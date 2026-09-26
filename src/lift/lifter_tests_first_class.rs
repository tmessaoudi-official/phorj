//! Row 4f — PHP 8.1 first-class callables. `name(...)` of a NAMED function lifts to a phorj
//! function reference (`len`), which phorj already has: `xs.map(len)` runs on all three legs and
//! transpiles back to `len(...)`. A BUILTIN's `strlen(...)` is still refused — which native it
//! means can depend on the arity it is called with (`min($xs)` vs `min($a, $b)`), and that data is
//! row 4g's. `$f(...)`, `$o->m(...)` and `C::m(...)` are refused too. Every refusal names a
//! rewrite that lifts TODAY: a closure with typed parameters (an untyped `fn ($x)` does not lift).

use super::lifter::lift_source;

fn lifted(src: &str) -> String {
    lift_source(src).unwrap_or_else(|e| panic!("expected a lift, refused: {e}\n{src}"))
}

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

fn refused(src: &str) -> String {
    match lift_source(src) {
        Err(e) => e,
        Ok(out) => panic!("expected a refusal, lifted:\n{out}"),
    }
}

#[test]
fn a_named_functions_first_class_callable_lifts_to_a_reference() {
    let out = lifted(
        "<?php\nfunction len(string $s): int { return strlen($s); }\n\
         /** @param list<string> $xs\n * @return list<int> */\n\
         function lens(array $xs): array { return array_map(len(...), $xs); }\n\
         function one(): int { $f = len(...); return $f('x'); }",
    );
    assert!(out.contains("xs.map(len)"), "{out}");
    assert!(out.contains("var f = len;"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_builtins_first_class_callable_is_refused_by_name() {
    let e = refused("<?php function f(): int { $g = strlen(...); return $g('x'); }");
    assert!(
        e.contains("first-class callable") && e.contains("`strlen`"),
        "{e}"
    );
    assert!(
        e.contains("typed parameter"),
        "the hint must name a rewrite that lifts: {e}"
    );
}

#[test]
fn a_first_class_callable_of_a_method_or_value_is_refused_by_name() {
    for src in [
        "<?php function f(\\Closure $g): int { $h = $g(...); return 1; }",
        "<?php final class C { public function m(): int { $h = $this->m(...); return 1; } }",
        "<?php final class C { public static function m(): int { $h = self::m(...); return 1; } }",
    ] {
        let e = refused(src);
        assert!(e.contains("first-class callable"), "{src}\n{e}");
        assert!(e.contains("typed parameter"), "{src}\n{e}");
    }
}
