//! Row 5d — DEC-515's `@var`-local half. A local whose `@var` docblock names a keyed shape keeps that
//! type on its declaration, its reads become field reads (`$rows[0]['name']` → `rows[0].name`, and a
//! `foreach` binder over it inherits the element shape), and a keyed literal assigned to it — or listed
//! in a declared `list<array{…}>` — becomes a tuple literal. Nothing is inferred: a local with no
//! `@var` stays `var` (DEC-166), and a literal whose keys are not the declared fields in declared
//! order stays a map for the checker to report.

use super::lifter::lift_source;
use super::lifter_tests::lift;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

const LOAD: &str = "/** @return list<array{id: int, name: string}> */
function load(): array { return [['id' => 1, 'name' => 'a'], ['id' => 2, 'name' => 'b']]; }
";

#[test]
fn a_list_of_keyed_literals_returned_as_a_declared_list_becomes_tuples() {
    let out = lift(&format!("<?php {LOAD}"));
    assert!(out.contains("(id: 1, name: \"a\")"), "{out}");
    checks_clean(&out);
}

#[test]
fn r2_a_binder_over_an_empty_declared_local_reads_fields() {
    let out = lift(&format!(
        "<?php {LOAD}
function total(): int {{
    /** @var list<array{{id: int, name: string}}> $kept */
    $kept = [];
    foreach (load() as $r) {{ $kept[] = $r; }}
    $sum = 0;
    foreach ($kept as $k) {{ $sum += $k['id']; }}
    return $sum;
}}"
    ));
    assert!(out.contains("k.id"), "{out}");
    checks_clean(&out);
}

#[test]
fn r3_a_call_initialized_local_keeps_its_declared_type() {
    let out = lift(&format!(
        "<?php {LOAD}
function first(): string {{
    /** @var list<array{{id: int, name: string}}> $rows */
    $rows = load();
    return $rows[0]['name'];
}}"
    ));
    assert!(
        out.contains("mutable List<(id: int, name: string)> rows = load();"),
        "{out}"
    );
    assert!(out.contains("rows[0].name"), "{out}");
    checks_clean(&out);
}

#[test]
fn r4_a_nullable_local_takes_a_tuple_and_reads_it_after_a_null_test() {
    // Scout `Rent/Cli/Pipeline.php:1129-1167`'s shape: declared null, replaced by a keyed literal under
    // a `$seen === null || …` condition (DEC-535 narrows the right operand), read behind `!== null`.
    // The loop iterates a DECLARED parameter: a binder over a call (`load()`) takes no shape (DEC-166).
    let out = lift(&format!(
        "<?php {LOAD}
/** @param list<array{{id: int, name: string}}> $rows */
function pick(array $rows): string {{
    /** @var array{{id: int, name: string}}|null $seen */
    $seen = null;
    foreach ($rows as $r) {{
        if ($seen === null || $r['id'] > $seen['id']) {{
            $seen = ['id' => $r['id'], 'name' => $r['name']];
        }}
    }}
    if ($seen !== null) {{ return $seen['name']; }}
    return \"\";
}}"
    ));
    assert!(
        out.contains("mutable (id: int, name: string)? seen = null;"),
        "{out}"
    );
    assert!(out.contains("seen.id"), "{out}");
    assert!(out.contains("seen = (id: r.id, name: r.name);"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_literal_in_another_key_order_stays_a_map() {
    // Reordering would move the value expressions' evaluation order — never done silently.
    let out = lift_source(
        "<?php function f(): string {
            /** @var array{id: int, name: string} $x */
            $x = ['name' => 'a', 'id' => 1];
            return $x['name'];
        }",
    )
    .unwrap();
    assert!(out.contains("[\"name\" => \"a\", \"id\" => 1]"), "{out}");
}

#[test]
fn no_var_docblock_or_one_naming_another_variable_changes_nothing() {
    let out = lift(&format!(
        "<?php {LOAD}
function first(): int {{
    /** @var list<array{{id: int}}> $other */
    $rows = load();
    $plain = load();
    return count($rows) + count($plain);
}}"
    ));
    assert!(out.contains("mutable var rows = load();"), "{out}");
    assert!(out.contains("mutable var plain = load();"), "{out}");
}

#[test]
fn a_docblock_tag_for_rows_does_not_answer_for_row() {
    // `doc_tag` matched the variable name as a PREFIX, so `$rows`'s `@param` also typed `$row`.
    let out = lift_source(
        "<?php
/**
 * @param list<array{id: int}> $rows
 * @param array{name: string} $row
 */
function f(array $rows, array $row): string { return $row['name']; }",
    )
    .unwrap();
    assert!(out.contains("(name: string) row"), "{out}");
    assert!(out.contains("row.name"), "{out}");
}

#[test]
fn a_keyed_literal_appended_to_a_declared_list_becomes_a_tuple() {
    let out = lift(
        "<?php
function f(): int {
    /** @var list<array{id: int, name: string}> $xs */
    $xs = [];
    $xs[] = ['id' => 4, 'name' => 'd'];
    return $xs[0]['id'];
}",
    );
    assert!(out.contains("(id: 4, name: \"d\")"), "{out}");
    assert!(out.contains("xs[0].id"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_var_typed_literal_still_hoists_out_of_an_always_executing_block() {
    // 6C finding (row 5d): the `Declared` wrapper hid the literal from DEC-397's hoist, so a `@var int`
    // above the first assignment turned a working hoist into a declaration trapped inside the block.
    let out = lift(
        "<?php function pick(): int {
            if (true) {
                /** @var int $b */
                $b = 5;
            }
            $b = 7;
            return $b;
        }",
    );
    assert!(out.contains("mutable var b = 5;\n    if (true) {"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_shape_declared_inside_a_closure_does_not_leak_into_the_enclosing_function() {
    // 6C finding (row 5d): a closure body is its own PHP scope, so a `@var` registered there must not
    // rewrite the enclosing function's same-named variable — here a genuine map.
    let out = lift_source(
        "<?php function f(): int {
            $g = function (): int {
                /** @var array{a: int} $row */
                $row = ['a' => 1];
                return $row['a'];
            };
            $row = ['a' => 2, 'b' => 3];
            return $g() + $row['a'];
        }",
    )
    .unwrap();
    assert!(
        out.contains("return row.a;"),
        "the closure's own read: {out}"
    );
    assert!(
        out.contains("g() + row[\"a\"]"),
        "leaked out of the closure: {out}"
    );
}

#[test]
fn a_captured_shape_is_still_visible_inside_a_closure() {
    // The closure snapshot must not CLEAR the maps: a by-value `use` capture is the same shape inside.
    let out = lift(
        "<?php
/** @param list<array{id: int}> $rows */
function f(array $rows): int {
    $g = function () use ($rows): int { return $rows[0]['id']; };
    return $g();
}",
    );
    assert!(out.contains("rows[0].id"), "{out}");
    checks_clean(&out);
}
