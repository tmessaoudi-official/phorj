//! Row 4e — a docblock callable SIGNATURE (`callable(A): R`, `\Closure(A): R`, PHPStan/Psalm) types
//! a native `callable` / `\Closure`, which lifts to the phorj function type `(A) => R`. A native
//! with no signature keeps its named refusal: phorj function types always carry one, and the lifter
//! never invents it. `\Closure::fromCallable($f)` lifts to `f` — the argument is already a function
//! value once its parameter is typed from the signature.

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
fn a_callable_param_signature_lifts_to_a_function_type() {
    // Unparenthesized, with a description tail: the type runs across the space after `):`.
    let out = lifted(
        "<?php\n/** @param callable(string): ?int $f resolves a key */\n\
         function ap(callable $f, string $k): int { return $f($k) ?? -1; }",
    );
    assert!(out.contains("(string) => int? f"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_named_signature_param_and_a_closure_return_lift() {
    let out = lifted(
        "<?php\n/** @return \\Closure(int $n): string */\n\
         function mk(): \\Closure { return fn (int $n): string => 'n'; }\n\
         /** @param callable(): void $f */\nfunction run(callable $f): void { $f(); }",
    );
    assert!(out.contains("function mk(): (int) => string"), "{out}");
    assert!(out.contains("() => void f"), "{out}");
    assert!(
        !out.contains("import Closure"),
        "no phantom import for `\\Closure`:\n{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_parenthesized_nullable_callable_lifts_to_an_optional_function() {
    let out = lifted(
        "<?php\n/** @param (callable(string): ?int)|null $f */\n\
         function ap(?callable $f = null): int { return $f === null ? -2 : ($f('a') ?? -1); }",
    );
    assert!(out.contains("((string) => int?)? f = null"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_closure_property_takes_its_var_signature_and_from_callable_is_identity() {
    let out = lifted(
        "<?php\nfinal class C {\n\
             /** @var \\Closure(string): ?int */\n    private \\Closure $g;\n\
             /** @param callable(string): ?int $g */\n\
             public function __construct(callable $g) { $this->g = \\Closure::fromCallable($g); }\n\
             /** @return \\Closure(string): ?int */\n\
             public function get(): \\Closure { return $this->g; }\n}",
    );
    assert!(out.contains("(string) => int? g;"), "{out}");
    assert!(
        !out.contains("fromCallable"),
        "fromCallable is the identity:\n{out}"
    );
    assert!(out.contains("function get(): (string) => int?"), "{out}");
    assert!(!out.contains("Closure"), "{out}");
    checks_clean(&out);
}

#[test]
fn self_in_a_callable_signature_resolves_to_the_class() {
    let out = lifted(
        "<?php\nfinal class C {\n\
             /** @param callable(self): self $f */\n\
             public function ap(callable $f): C { return $f($this); }\n}",
    );
    assert!(out.contains("(C) => C f"), "{out}");
}

#[test]
fn what_a_phorj_function_type_cannot_say_is_refused_by_name() {
    for (doc, what) in [
        ("callable(int=): int", "an optional parameter"),
        ("callable(int...): int", "a variadic parameter"),
        ("callable(int &$x): int", "a by-reference parameter"),
        ("callable(int $x = 1): int", "an optional parameter"),
        ("callable(int)", "return type"),
    ] {
        let e = refused(&format!(
            "<?php\n/** @param {doc} $f */\nfunction ap(callable $f): int {{ return 1; }}"
        ));
        assert!(e.contains(what), "{doc}: {e}");
    }
}

#[test]
fn a_signature_less_callable_keeps_its_named_refusal() {
    let e =
        refused("<?php\n/** @param callable $f */\nfunction ap(callable $f): int { return 1; }");
    assert!(e.contains("needs a signature"), "{e}");
    let e = refused("<?php\nfunction ap(\\Closure $f): int { return 1; }");
    assert!(e.contains("needs a signature"), "{e}");
}

#[test]
fn a_docblock_of_the_wrong_shape_is_refused_not_swapped_in() {
    // A grouped non-function type on a `callable` would otherwise become `int?` for a closure.
    let e =
        refused("<?php\n/** @param (int)|null $f */\nfunction ap(?callable $f): int { return 1; }");
    assert!(
        e.contains("does not describe the declared `callable`"),
        "{e}"
    );
    // And a signature on an `array` would otherwise become a function for a collection.
    let e = refused(
        "<?php\n/** @param callable(int): int $xs */\nfunction ap(array $xs): int { return 1; }",
    );
    assert!(e.contains("does not describe the declared `array`"), "{e}");
}
