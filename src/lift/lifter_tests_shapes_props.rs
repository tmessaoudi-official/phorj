//! Row 4p (2026-09-27) — keyed shapes that reach a variable through a DECLARATION other than a
//! parameter. DEC-515 seeded the field registry from parameters and `@var` locals only, so two
//! declared shapes still read as string indexes in the scout census:
//!
//! * a `foreach` binder over a PROPERTY — `foreach ($this->bands as $band)` over a promoted
//!   `@param array<string, array{max: int, tenure: Tenure}> $bands` (`Core/PlafondBands.php`);
//! * the builder local of a declared return — `$hits = []; $hits[$at] = [...]; return $hits;` under
//!   `@return array<int, array{tenure: Tenure, literal: string}>` (`Core/TenureClassifier.php`),
//!   whose local is already TYPED from the return (Lane R-6) but whose writes stayed keyed arrays.
//!
//! Nothing here is inferred from a call or a literal (DEC-166): an unannotated `$x = []` that is
//! not returned stays loud, and a binder over a CALL's result stays unbound.

use super::lifter_tests::{assert_reparses, lift};

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

const BANDS: &str = "<?php
final class Bands
{
    /** @param array<string, array{max: int, zone: string}> $bands */
    public function __construct(public array $bands)
    {
        foreach ($this->bands as $code => $band) {
            if ($band['max'] < 0) {
                echo $code . $band['zone'];
            }
        }
    }

    public function total(): int
    {
        $n = 0;
        foreach ($this->bands as $code => $band) {
            $n += $band['max'];
        }
        return $n;
    }
}";

#[test]
fn a_binder_over_a_declared_property_reads_its_fields() {
    let out = lift(BANDS);
    // In the constructor (a promoted parameter) and in an ordinary method alike.
    assert!(out.contains("band.max < 0"), "{out}");
    assert!(out.contains("band.zone"), "{out}");
    assert!(out.contains("n = n + band.max"), "{out}");
    assert!(!out.contains("band[\"max\"]"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_binder_over_a_declared_plain_property_reads_its_fields_too() {
    let out = lift(
        "<?php
final class Rows
{
    /** @var list<array{id: int, name: string}> */
    private array $rows = [];

    public function names(): string
    {
        $s = '';
        foreach ($this->rows as $row) {
            $s .= $row['name'];
        }
        return $s;
    }
}",
    );
    assert!(
        out.contains("s + row.name") || out.contains("row.name"),
        "{out}"
    );
    assert!(!out.contains("row[\"name\"]"), "{out}");
    assert_reparses(&out);
}

#[test]
fn a_returned_builder_local_takes_its_writes_as_the_declared_tuples() {
    let out = lift(
        "<?php
/** @return array<int, array{tag: string, text: string}> */
function collect(string $s): array
{
    $hits = [];
    $hits[3] = ['tag' => 'a', 'text' => $s];
    if ($s !== '') {
        $hits[7] = ['tag' => 'b', 'text' => $s];
    }
    return $hits;
}",
    );
    assert!(out.contains("hits[3] = (tag: \"a\", text: s)"), "{out}");
    assert!(out.contains("hits[7] = (tag: \"b\", text: s)"), "{out}");
    checks_clean(&out);
}

/// A conditional write — `$hits[$k] = $sure ? [...] : [...]` — shapes each arm (scout
/// `TenureClassifier.php`, three census errors on one line).
#[test]
fn both_arms_of_a_conditional_write_take_the_declared_tuple() {
    let out = lift(
        "<?php
/** @return array<int, array{tag: string, text: string}> */
function collect(string $s, bool $sure): array
{
    $hits = [];
    $hits[1] = $sure ? ['tag' => 'a', 'text' => $s] : ['tag' => '?', 'text' => $s];
    return $hits;
}",
    );
    assert!(out.contains("(tag: \"a\", text: s)"), "{out}");
    assert!(out.contains("(tag: \"?\", text: s)"), "{out}");
    checks_clean(&out);
}

/// A builder that is NOT returned has no declaration to read: its writes stay keyed arrays and the
/// checker keeps asking for the empty literal's type (DEC-166).
#[test]
fn an_unreturned_empty_local_is_not_shaped() {
    let out = lift(
        "<?php
/** @return array<int, array{tag: string, text: string}> */
function collect(string $s): array
{
    $tmp = [];
    $tmp[3] = ['tag' => 'a', 'text' => $s];
    return [];
}",
    );
    assert!(out.contains("tmp[3] = [\"tag\" => \"a\""), "{out}");
}

/// The constructor path reset NONE of the per-function shape maps, so a `$row` shape declared by the
/// previous class's method answered inside the next class's constructor — `$row['bp']` on a plain
/// map lifted to `row.bp`, which no map has.
#[test]
fn a_constructor_does_not_inherit_the_previous_methods_shapes() {
    let out = lift(
        "<?php
final class A
{
    /** @param array{bp: int} $row */
    public function m(array $row): int { return $row['bp']; }
}
final class B
{
    public function __construct()
    {
        $row = ['bp' => 1];
        echo $row['bp'];
    }
}",
    );
    assert!(out.contains("return row.bp;"), "{out}");
    assert!(out.contains("var row = [\"bp\" => 1];"), "{out}");
}
