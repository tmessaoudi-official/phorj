//! Row 4q, DEC-548 — a PHP parameter the body REASSIGNS. phorj parameters are immutable, there is
//! no `mutable` parameter form and shadowing is refused, so the lift copies such a parameter into a
//! mutable local at the top of the body (`mutable var bpLocal = bp;`) and the body uses the copy.
//! The signature — and so every named-argument call — is untouched. Only a WRITTEN parameter is
//! copied; a read-only one, or one whose object is written through (`$p->x = …`), keeps its name.

use super::lifter_tests::lift;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

#[test]
fn a_reassigned_parameter_is_copied_and_the_signature_kept() {
    let out = lift(
        "<?php
function cap(int $bp): int { if ($bp > 9) { $bp = 9; } return $bp; }
echo cap(bp: 12);",
    );
    assert!(out.contains("function cap(int bp): int {"), "{out}");
    assert!(out.contains("mutable var bpLocal = bp;"), "{out}");
    assert!(out.contains("bpLocal = 9;"), "{out}");
    assert!(out.contains("return bpLocal;"), "{out}");
    assert!(out.contains("cap(bp: 12)"), "{out}");
    checks_clean(&out);
}

/// Every assignment form counts — `=`, `+=`, `++` — and only a written parameter is copied. A write
/// inside a closure is to the closure's own local (PHP captures by value), so it copies nothing.
#[test]
fn every_write_form_counts_and_only_written_parameters_are_copied() {
    let out = lift(
        "<?php
function f(int $a, int $b, int $c, int $d): int {
    $a += 1;
    $c++;
    $g = function (): int { $d = 5; return $d; };
    return $a + $b + $c + $d + $g();
}",
    );
    for copy in ["aLocal = a;", "cLocal = c;"] {
        assert!(out.contains(copy), "{copy}\n{out}");
    }
    assert!(!out.contains("bLocal") && !out.contains("dLocal"), "{out}");
    checks_clean(&out);
}

/// A collection parameter appended to keeps its declared shape on the copy: the append takes the
/// keyed literal as the declared tuple, and a binder over the copy reads fields.
#[test]
fn a_copied_collection_parameter_keeps_its_declared_shape() {
    let out = lift(
        "<?php
/** @param list<array{x: int}> $rows */
function g(array $rows): int {
    $rows[] = ['x' => 2];
    $n = 0;
    foreach ($rows as $r) { $n += $r['x']; }
    return $n;
}",
    );
    assert!(
        out.contains("rowsLocal = List.append(rowsLocal, (x: 2));"),
        "{out}"
    );
    assert!(out.contains("n = n + r.x"), "{out}");
    checks_clean(&out);
}

/// The copy's name never collides with a local the body already uses.
#[test]
fn the_copy_name_skips_a_taken_one() {
    let out = lift(
        "<?php
function h(int $v): int { $vLocal = 1; $v = 2; return $v + $vLocal; }",
    );
    assert!(out.contains("mutable var vLocal2 = v;"), "{out}");
    assert!(out.contains("return vLocal2 + vLocal;"), "{out}");
    checks_clean(&out);
}

/// A closure capturing the reassigned parameter captures the copy; a write THROUGH a parameter
/// object is not a reassignment of the parameter.
#[test]
fn a_closure_sees_the_copy_and_a_write_through_an_object_is_not_a_reassignment() {
    let out = lift(
        "<?php
final class Box { public int $x = 0; }
function k(int $n, Box $b): int {
    $n = $n * 2;
    $b->x = $n;
    $f = fn (int $m): int => $m + $n;
    return $f(1) + $b->x;
}",
    );
    assert!(out.contains("m + nLocal"), "{out}");
    assert!(out.contains("b.x = nLocal;"), "{out}");
    assert!(!out.contains("bLocal"), "{out}");
    checks_clean(&out);
}

/// A by-reference builtin the lift lowers to a reassignment of its first argument — `usort`,
/// `array_unshift` — writes that parameter too (scout `TenureClassifier::resolve`).
#[test]
fn a_by_reference_builtin_writes_its_parameter() {
    let out = lift(
        "<?php
/**
 * @param list<int> $xs
 * @param list<int> $ys
 * @return list<int>
 */
function s(array $xs, array $ys): array {
    usort($xs, fn (int $a, int $b): int => $a <=> $b);
    array_unshift($ys, ...$xs);
    return $ys;
}",
    );
    assert!(out.contains("xsLocal = xsLocal.sortWith("), "{out}");
    assert!(
        out.contains("ysLocal = List.concat(xsLocal, ysLocal);"),
        "{out}"
    );
    checks_clean(&out);
}
