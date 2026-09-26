//! Row 5v — `foreach ($xs as [$a, $b])`, DEC-510's positional destructure in a loop header. It lifts
//! to phorj's own lowering of `for ((a, b) in xs)` (DEC-288): a synthetic `__fortup_N` binder whose
//! body opens with `var (a, b) = __fortup_N;`. `cmd_lift` formats the draft, and the formatter
//! re-collapses that shape, so a shipped draft reads `for ((a, b) in xs)`. The checker judges that
//! the elements are tuples; a keyed, nested or skipped-slot pattern is refused by name.

use super::lifter::lift_source;
use super::lifter_tests::lift;

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

const PAIRS: &str = "/**
 * @param list<array{string, int}> $ps
 */";

#[test]
fn a_destructuring_foreach_lowers_to_the_tuple_loop_shape() {
    let out = lift(&format!(
        "<?php {PAIRS}
function f(array $ps): int {{
    $sum = 0;
    foreach ($ps as [$w, $n]) {{ $sum += $n; }}
    return $sum;
}}"
    ));
    assert!(out.contains("var (w, n) = __fortup_"), "{out}");
    checks_clean(&out);
}

#[test]
fn the_formatted_draft_reads_as_the_sugared_loop() {
    let out = crate::cli::cmd_lift(&format!(
        "<?php {PAIRS}
function f(array $ps): int {{
    $sum = 0;
    foreach ($ps as list($w, $n)) {{ $sum += $n; }}
    return $sum;
}}"
    ))
    .unwrap();
    assert!(out.contains("for ((w, n) in ps) {"), "{out}");
    assert!(!out.contains("__fortup"), "{out}");
}

#[test]
fn sequential_and_nested_loops_get_their_own_binders() {
    let out = lift(&format!(
        "<?php {PAIRS}
function f(array $ps): int {{
    $sum = 0;
    foreach ($ps as [$w, $n]) {{ $sum += $n; }}
    foreach ($ps as [$w, $n]) {{
        foreach ($ps as [$v, $m]) {{ $sum += $n * $m; }}
    }}
    return $sum;
}}"
    ));
    checks_clean(&out);
}

#[test]
fn a_reserved_word_binder_is_renamed_like_any_local() {
    let out = lift(&format!(
        "<?php {PAIRS}
function f(array $ps): int {{
    $sum = 0;
    foreach ($ps as [$match, $type]) {{ $sum += $type; }}
    return $sum;
}}"
    ));
    assert!(out.contains("var (matchValue, typeValue) = "), "{out}");
    checks_clean(&out);
}

#[test]
fn a_binder_name_reused_after_the_loop_is_a_fresh_declaration() {
    // PHP's binders leak out of the loop; phorj's are loop-scoped, so a later `$w = …` must DECLARE.
    let out = lift(&format!(
        "<?php {PAIRS}
function f(array $ps): string {{
    foreach ($ps as [$w, $n]) {{ echo $w; }}
    $w = 'after';
    return $w;
}}"
    ));
    assert!(out.contains("var w = \"after\";"), "{out}");
    checks_clean(&out);
}

#[test]
fn keyed_nested_skipped_and_single_patterns_are_refused_by_name() {
    let keyed = refused(&format!(
        "<?php {PAIRS} function f(array $ps): void {{ foreach ($ps as ['k' => $v, 'j' => $w]) {{}} }}"
    ));
    assert!(keyed.contains("KEYED destructure"), "{keyed}");
    let nested = refused(&format!(
        "<?php {PAIRS} function f(array $ps): void {{ foreach ($ps as [[$a, $b], $c]) {{}} }}"
    ));
    assert!(nested.contains("foreach"), "{nested}");
    let skipped = refused(&format!(
        "<?php {PAIRS} function f(array $ps): void {{ foreach ($ps as [, $n]) {{}} }}"
    ));
    assert!(skipped.contains("skipped slot"), "{skipped}");
    let inner_skip = refused(&format!(
        "<?php {PAIRS} function f(array $ps): void {{ foreach ($ps as [$a, , $n]) {{}} }}"
    ));
    assert!(inner_skip.contains("skipped slot"), "{inner_skip}");
    let single = refused(&format!(
        "<?php {PAIRS} function f(array $ps): void {{ foreach ($ps as [$w]) {{}} }}"
    ));
    assert!(single.contains("two or more"), "{single}");
    let keyed_loop = refused(&format!(
        "<?php {PAIRS} function f(array $ps): void {{ foreach ($ps as $i => [$w, $n]) {{}} }}"
    ));
    assert!(keyed_loop.contains("key"), "{keyed_loop}");
}

#[test]
fn a_binder_written_in_the_body_is_not_a_hoist_candidate() {
    // The binders are declared BY the loop, so the DEC-397 pre-scan must not sight them: a body
    // write would otherwise count as the variable's first assignment, inside a CONDITIONAL block,
    // and the post-loop `$w = …` would stop being a declaration.
    let out = lift(&format!(
        "<?php {PAIRS}
function f(array $ps): string {{
    foreach ($ps as [$w, $n]) {{ $w = 'in'; }}
    $w = 'after';
    return $w;
}}"
    ));
    assert!(out.contains("var w = \"after\";"), "{out}");
    // Sighted, the body write would be `w`'s first assignment and the planner would BLOCK it.
    assert!(!out.contains("CANNOT LIFT"), "{out}");
}
