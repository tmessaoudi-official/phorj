//! Row 4g — builtins whose phorj form depends on the call's arity (`lifter/arity_fns.rs`). The
//! registry keeps every arity it can place; these pin the ones it cannot, and that the hook never
//! steals a registry lift.

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
fn one_argument_min_and_max_take_a_list() {
    let out = lifted(
        "<?php\n/** @param list<int> $xs */\n\
         function spread(array $xs): int { return max($xs) - min($xs); }",
    );
    assert!(out.contains("xs.max()! - xs.min()!"), "{out}");
    checks_clean(&out);
}

#[test]
fn three_argument_min_folds_and_two_argument_min_keeps_its_registry_lift() {
    let out = lifted(
        "<?php\nfunction clamp(int $a, int $b, int $c): int { return min($a, $b, $c) + max($a, $b); }",
    );
    assert!(out.contains("a.min(b).min(c) + a.max(b)"), "{out}");
    checks_clean(&out);
}

#[test]
fn two_argument_substr_runs_to_the_end() {
    let out = lifted("<?php\nfunction tail(string $s): string { return substr($s, 1); }");
    assert!(out.contains("s.substring(1, 9223372036854775807)"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_spread_or_named_argument_is_not_the_one_array_form() {
    // A spread is refused by `lift_expr` on any path (the guard's spread half is defense in depth);
    // a NAMED argument lifts, and must reach the generic call, not the one-array form.
    let e = refused(
        "<?php\n/** @param list<int> $xs */\nfunction f(array $xs): int { return min(...$xs); }",
    );
    assert!(!e.contains("min()!"), "{e}");
    let out = lifted(
        "<?php\n/** @param list<int> $xs */\nfunction f(array $xs): int { return min(value: $xs); }",
    );
    assert!(out.contains("min(value: xs)"), "{out}");
}

#[test]
fn a_callback_less_filter_and_a_pairs_strtr_are_refused_by_name() {
    let e = refused("<?php\n/** @param list<int> $xs\n * @return list<int> */\nfunction f(array $xs): array { return array_filter($xs); }");
    assert!(
        e.contains("PHP-TRUTHY") && e.contains("write the callback"),
        "{e}"
    );
    // scout's `$signals` is a map of LISTS: the callback would need an untyped `array` param.
    assert!(
        e.contains("list of ARRAYS") && e.contains("`foreach`"),
        "{e}"
    );
    let e = refused("<?php\n/** @param array<string, string> $m */\nfunction f(string $s, array $m): string { return strtr($s, $m); }");
    assert!(
        e.contains("longest-first") && e.contains("String.translate"),
        "{e}"
    );
}
