//! Row 5k — DEC-531: PHP names that are phorj reserved words.
//!
//! MEMBERS keep their name (the parser accepts a reserved word in member position), so a lifted
//! `match()` method or promoted `$type` stays the API PHP callers know. LOCALS and plain PARAMETERS
//! are private to their function, so the lifter renames them with a `Value` suffix (`E-NAME-CASE`
//! forbids `_`, so `type_` never checked) — and refuses by name when `typeValue` is taken, because a
//! named argument renamed the same way could then silently bind a different parameter. Every lift
//! here ends in `checks_clean`: asserting on the text alone is how the `_` shape got past the tests.

use super::lifter::lift_source;
use super::lifter_tests::lift;

fn checks_clean(phg: &str) {
    let prog =
        crate::cli::parse_program(phg).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{phg}"));
    crate::cli::check_and_expand(&prog, phg)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{phg}"));
}

#[test]
fn keyword_members_keep_their_names() {
    let out = lift(
        "<?php final class Rule {\n\
           public function __construct(public string $type) {}\n\
           public static function match(int $x): int { return $x; }\n\
           public function open(): string { return $this->type . self::match(1); }\n\
         }",
    );
    assert!(
        out.contains("constructor(public mutable string type)"),
        "{out}"
    );
    assert!(
        out.contains("public static function match(int x): int"),
        "{out}"
    );
    assert!(out.contains("this.type"), "{out}");
    checks_clean(&out);
}

#[test]
fn keyword_locals_binders_and_parameters_get_a_value_suffix() {
    let out = lift(
        "<?php /** @param array<string, string> $xs */\n\
         function f(int $match, array $xs): string {\n\
           $new = '';\n\
           foreach ($xs as $class => $type) { $new = $new . \"$type\"; }\n\
           try { $new = $new . $match; } catch (Exception $throw) { $new = ''; }\n\
           return $new;\n\
         }",
    );
    for (want, why) in [
        ("int matchValue", "a parameter"),
        ("var newValue", "a local"),
        ("foreach (xs as classValue => typeValue)", "foreach binders"),
        ("{typeValue}", "an interpolated variable"),
        ("catch (RuntimeError throwValue)", "a catch variable"),
    ] {
        assert!(out.contains(want), "{why}: `{want}` missing in\n{out}");
    }
    assert!(!out.contains(" type "), "{out}");
    checks_clean(&out);
}

#[test]
fn this_is_never_renamed() {
    let out = lift(
        "<?php final class C { public int $n = 1; public function get(): int { return $this->n; } }",
    );
    assert!(out.contains("this.n"), "{out}");
    assert!(!out.contains("thisValue"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_closure_sees_the_same_renamed_name() {
    let out = lift(
        "<?php function f(int $type): int { $g = fn(int $x): int => $x + $type; return $g(1); }",
    );
    assert!(out.contains("x + typeValue"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_taken_underscore_name_is_refused_by_name() {
    let err = lift_source(
        "<?php function f(int $type, int $typeValue): int { return $type + $typeValue; }",
    )
    .unwrap_err();
    assert!(
        err.contains("`$type`") && err.contains("`$typeValue`"),
        "{err}"
    );
}

#[test]
fn a_named_argument_follows_its_parameter() {
    let out = lift(
        "<?php final class S { public function __construct(public string $type) {} }\n\
         function g(int $match): int { return $match; }\n\
         function h(): int { $s = new S(type: 'a'); return g(match: 2); }",
    );
    assert!(
        out.contains("new S(type: \"a\")"),
        "a promoted field keeps its name: {out}"
    );
    assert!(
        out.contains("g(matchValue: 2)"),
        "a plain parameter's argument is renamed: {out}"
    );
    checks_clean(&out);
}

#[test]
fn a_promoted_keyword_parameter_read_in_its_constructor_is_refused() {
    let err = lift_source(
        "<?php final class S { public int $n;\n\
         public function __construct(public string $type) { $this->n = strlen($type); } }",
    )
    .unwrap_err();
    assert!(
        err.contains("`$type`") && err.contains("constructor"),
        "{err}"
    );
}
