//! DEC-512's THIRD clause — a positional array literal in an ORDERING operand position lifts to a
//! tuple (`[$a, $b] <=> [$c, $d]` → `(a, b) <=> (c, d)`).
//!
//! The rule exists because PHP orders arrays and phorj does not: `List<T>` has no static arity, so
//! PHP's count-first rule and lexicographic order disagree (`[2] <=> [1, 1]` is `-1` in PHP, `+1`
//! lexicographically), and phorj refuses lists with `E-ORDER-LIST`. A LITERAL is the one case where
//! the arity is syntactically present, so converting it reads no intent and DEC-166 holds.
//!
//! That makes the rule's NARROWNESS the thing worth pinning, not its happy path. Every case below
//! that asserts a NON-conversion is guarding the DEC-166 boundary: the moment the lifter converts a
//! variable, a mismatched arity, a keyed array or a non-ordering operator, it is guessing an arity
//! the source does not state. A single over-broad `matches!` would turn all of those green and only
//! these tests would notice.

use super::lifter_tests::{assert_reparses, lift};

/// scout's exact shape — `Rent/Core/Classification.php:48`, the comparator this clause was built
/// for. Note what is and is not claimed: the OPERATOR on that line lifts. The enclosing `usort` has
/// no lift mapping at all, so the call still refuses.
#[test]
fn the_scout_comparator_lifts_both_sides_to_tuples() {
    let out = lift(
        "<?php class Signal { public int $tier = 0; public int $position = 0; }\n\
         function cmp(Signal $a, Signal $b): int {\n\
           return [$a->tier, $a->position] <=> [$b->tier, $b->position];\n\
         }",
    );
    assert!(
        out.contains("(a.tier, a.position) <=> (b.tier, b.position)"),
        "{out}"
    );
    assert_reparses(&out);
}

/// The rule is keyed on the OPERATOR CLASS, not on `<=>` alone: `< > <= >=` order tuples too, so a
/// literal in any of their operand positions converts. If they did not, `[$a] < [$b]` would lift to
/// a list comparison that `phg check` then refuses — a draft that lifts and does not check, the
/// two-halves-disagree class.
#[test]
fn a_relational_operator_converts_the_same_way() {
    for op in ["<", ">", "<=", ">="] {
        let out = lift(&format!(
            "<?php function cmp(int $a, int $b): bool {{ return [$a, 1] {op} [$b, 2]; }}"
        ));
        assert!(out.contains(&format!("(a, 1) {op} (b, 2)")), "{op}: {out}");
        assert_reparses(&out);
    }
}

/// `==` is NOT an ordering operator, and list equality is a real phorj operation — converting here
/// would silently change what the program compares.
#[test]
fn equality_does_not_convert_its_operands() {
    let out = lift("<?php function eq(int $a, int $b): bool { return [$a, 1] == [$b, 2]; }");
    assert!(out.contains("[a, 1] == [b, 2]"), "{out}");
    assert!(!out.contains("(a, 1) =="), "must stay a list: {out}");
    assert_reparses(&out);
}

/// A VARIABLE operand keeps lifting to whatever it was. The lifter cannot see a runtime array's
/// arity, and assuming one is exactly the guess DEC-166 forbids — so this correctly produces a
/// program the checker refuses with `E-ORDER-LIST`, which is the honest outcome rather than a
/// quietly-invented tuple.
#[test]
fn a_variable_operand_is_never_converted() {
    let out = lift(
        "<?php\n/**\n * @param int[] $xs\n */\n\
         function cmp(array $xs, int $b): int { return $xs <=> [$b, 1]; }",
    );
    assert!(out.contains("xs <=> [b, 1]"), "{out}");
    assert!(!out.contains("(b, 1)"), "one-sided conversion: {out}");
    assert_reparses(&out);
}

/// Unequal arity cannot be a tuple comparison at all — the two sides would not even have the same
/// type. This is also the exact case where PHP's count-first rule DIVERGES from lexicographic, so
/// converting it would manufacture the divergence DEC-512 exists to avoid.
#[test]
fn mismatched_arity_stays_a_list_on_both_sides() {
    let out = lift("<?php function cmp(int $a, int $b): int { return [$a] <=> [$b, $a]; }");
    assert!(out.contains("[a] <=> [b, a]"), "{out}");
    assert_reparses(&out);
}

/// A KEYED array is a map, not a positional shape. `['t' => 1] <=> …` has no tuple reading.
///
/// What this pins is the OUTCOME, not the `positional` predicate in the clause: `lift_array` only
/// ever produces an `Expr::List` for a fully positional literal (all-keyed becomes a `Map`, mixed
/// refuses as Tier-2), so the clause's `Expr::List` destructure is what rejects this input and
/// defeating `positional` leaves the suite green [Verified by mutation, 2026-09-08]. Said plainly
/// because a reader would otherwise assume this test covers that guard.
#[test]
fn a_keyed_array_is_not_converted() {
    let out =
        lift("<?php function cmp(int $a, int $b): int { return ['t' => $a] <=> ['t' => $b]; }");
    assert!(!out.contains("(\"t\""), "keyed array became a tuple: {out}");
    assert!(!out.contains("<=> (t"), "keyed array became a tuple: {out}");
    assert_reparses(&out);
}

/// The empty literal is excluded by the `!li.is_empty()` guard: a zero-arity tuple is not a phorj
/// type, so converting would emit `()` — a draft that lifts and then fails to parse.
#[test]
fn an_empty_literal_is_not_converted() {
    let out = lift("<?php function cmp(): int { return [] <=> []; }");
    assert!(out.contains("[] <=> []"), "{out}");
    assert_reparses(&out);
}

/// Nesting: the clause runs inside `lift_expr`, so a converted operand's own elements are lifted
/// normally — and an INNER positional literal is not an ordering operand, so it stays a list. Pinned
/// because the tempting "convert positional literals under ordering" reading would recurse and
/// change the element type of the tuple.
#[test]
fn only_the_immediate_operands_convert_not_nested_literals() {
    let out =
        lift("<?php function cmp(int $a, int $b): int { return [[$a, 1], 2] <=> [[$b, 3], 4]; }");
    assert!(out.contains("([a, 1], 2) <=> ([b, 3], 4)"), "{out}");
    assert_reparses(&out);
}
