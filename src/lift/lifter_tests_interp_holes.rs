//! Row 4b — a PHP ternary (or `match`) inside a `.` chain lifts to an if-/match-expression INSIDE an
//! interpolation hole (DEC-511). The lexer closes a hole at the first unescaped `}`, so an unescaped
//! block brace ended the hole early and the draft did not LEX — 6 of 91 scout drafts. The formatter
//! already escapes a hole's printed form (`escape_interp`); the lift printer now shares it, so a
//! draft's holes are byte-for-byte what `phg format` would print.

use super::lifter::lift_source;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

fn lifts_and_checks(src: &str) -> String {
    let out = lift_source(src).unwrap_or_else(|e| panic!("lift failed: {e}\n{src}"));
    checks_clean(&out);
    out
}

#[test]
fn a_ternary_in_a_concatenation_lifts_to_a_draft_that_lexes() {
    lifts_and_checks(
        "<?php function f(string $t, bool $b): string { return $t . ($b ? 'x' : 'y'); }",
    );
}

#[test]
fn a_ternary_with_non_string_arms_is_stringified_by_the_hole() {
    lifts_and_checks("<?php function f(bool $b): string { return 'n=' . ($b ? 1 : 2); }");
}

#[test]
fn a_ternary_inside_a_call_inside_a_hole_lifts_to_a_draft_that_lexes() {
    // scout `NtfyChannel.php`: the conditional is a part of an INNER concatenation that is itself a
    // call argument inside the OUTER interpolation, so escaping must compose across both levels.
    lifts_and_checks(
        "<?php function h(string $s): string { return '[' . $s . ']'; }
         function f(string $t): string { return 'Tags: ' . h(($t === '' ? '' : $t . ',') . 'k'); }",
    );
}

#[test]
fn a_match_in_a_concatenation_lifts_to_a_draft_that_lexes() {
    lifts_and_checks(
        "<?php function f(int $n): string { return 'v=' . match ($n) { 1 => 'one', default => 'many' }; }",
    );
}

#[test]
fn the_lifted_holes_are_what_phg_format_prints() {
    // The drift this row closes: two printers, one escaping a hole and one not. A formatted draft
    // must keep every lifted line unchanged (the format pass `cmd_lift` runs is then a no-op here).
    let out = lifts_and_checks(
        "<?php function f(string $t, bool $b): string { return $t . ($b ? 'x' : 'y'); }",
    );
    let formatted = crate::format::format(&out).unwrap_or_else(|e| panic!("formats: {e:?}\n{out}"));
    let hole = |s: &str| {
        s.lines()
            .find(|l| l.contains("return"))
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        hole(&out),
        hole(&formatted),
        "\n--- lift ---\n{out}\n--- format ---\n{formatted}"
    );
}
