//! Row 5l — DEC-532: PHP 8's throw-as-expression lifts 1:1 in the four positions phorj allows it
//! (right of `??`, a ternary branch → an `if`-expression arm, a `match` arm, an arrow-fn body) and is
//! refused by name anywhere else. The thrown class is mapped exactly as a `throw` STATEMENT's is: an
//! expression-position throw is still an exception site, so `\RuntimeException` must become
//! `RuntimeError` and bring its import.

use super::lifter::lift_source;
use super::lifter_tests::lift;

#[test]
fn a_coalesce_throw_lifts_and_maps_its_class() {
    let out = lift(
        "<?php function k(?string $s): string { return $s ?? throw new \\RuntimeException('no key'); }",
    );
    assert!(
        out.contains("return s ?? throw new RuntimeError(\"no key\");"),
        "{out}"
    );
    assert!(
        out.contains("import Core.ErrorModule.RuntimeError;"),
        "{out}"
    );
}

#[test]
fn a_static_factory_operand_lifts() {
    let out = lift(
        "<?php final class Bad extends \\RuntimeException { public static function of(string $w): self { return new self($w); } }\n\
         function k(?string $s): string { return $s ?? throw Bad::of('x'); }",
    );
    assert!(out.contains("return s ?? throw Bad::of(\"x\");"), "{out}");
}

#[test]
fn a_match_default_throw_lifts() {
    let out = lift(
        "<?php function r(string $a): int { return match ($a) { 'x' => 1, default => throw new \\InvalidArgumentException('type ' . $a) }; }",
    );
    assert!(
        out.contains("default => throw new InvalidValueError(\"type {a}\")"),
        "{out}"
    );
    assert!(
        out.contains("import Core.ErrorModule.InvalidValueError;"),
        "{out}"
    );
}

#[test]
fn a_ternary_branch_and_an_arrow_body_lift() {
    let out = lift(
        "<?php function t(bool $c): int { return $c ? throw new \\LogicException('t') : 2; }\n\
         function a(): int { $f = fn(int $x): int => throw new \\LogicException('l'); return 0; }",
    );
    assert!(
        out.contains("if (c) { throw new LogicError(\"t\") } else { 2 }"),
        "{out}"
    );
    assert!(out.contains("=> throw new LogicError(\"l\")"), "{out}");
}

#[test]
fn a_throw_in_any_other_position_is_refused_by_name() {
    for src in [
        "<?php function f(): int { return g(throw new \\LogicException('x')); }",
        "<?php function f(bool $c): bool { return $c || throw new \\LogicException('x'); }",
        "<?php function f(): void { $x = throw new \\LogicException('x'); }",
    ] {
        let err = lift_source(src).unwrap_err();
        assert!(err.contains("throw"), "{src}\n{err}");
        assert!(err.contains("DEC-532"), "{src}\n{err}");
    }
}

#[test]
fn a_caught_coalesce_throw_checks_clean() {
    let out = lift(
        "<?php function k(?string $s): string {\n\
           try { return $s ?? throw new \\RuntimeException('none'); }\n\
           catch (\\RuntimeException $e) { return 'caught'; }\n\
         }",
    );
    let prog =
        crate::cli::parse_program(&out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, &out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}
