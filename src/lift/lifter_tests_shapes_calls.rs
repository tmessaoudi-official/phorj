//! Row 4p4, DEC-550 — a `foreach` binder over a STATICALLY resolved call reads the callee's DECLARED
//! `@return` shape: `$this->m()`, `self::m()` / `Class::m()` on the enclosing class, and a free
//! function of the same file. Never a shape inferred from the callee's body (DEC-166), and nothing for
//! a call the lift cannot resolve (another object's method), which stays an index read.

use super::lifter_tests::lift;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

const LABELS: &str = "<?php
final class Labels
{
    /** @return list<array{tag: string, n: int}> */
    private function hits(): array
    {
        return [['tag' => 'a', 'n' => 1]];
    }

    /** @return array<string, array{tag: string, n: int}> */
    private static function keyed(): array
    {
        $out = [];
        $out['k'] = ['tag' => 'b', 'n' => 2];
        return $out;
    }

    public function sum(): int
    {
        $total = 0;
        foreach ($this->hits() as $h) {
            $total += $h['n'];
        }
        foreach (self::keyed() as $k => $h) {
            $total += $h['n'];
        }
        return $total;
    }
}";

#[test]
fn a_binder_over_this_or_self_method_reads_its_declared_return() {
    let out = lift(LABELS);
    assert_eq!(out.matches("total = total + h.n").count(), 2, "{out}");
    assert!(!out.contains("h[\"n\"]"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_binder_over_a_same_file_function_reads_its_declared_return() {
    let out = lift(
        "<?php
/** @return list<array{x: int}> */
function rows(): array { return [['x' => 1], ['x' => 2]]; }
function total(): int {
    $n = 0;
    foreach (rows() as $r) { $n += $r['x']; }
    return $n;
}",
    );
    assert!(out.contains("n = n + r.x"), "{out}");
    checks_clean(&out);
}

/// Through phorj's propagation marker too — the census shape, `foreach (this.matchLabels(f)? as …)`.
#[test]
fn a_binder_over_a_propagated_call_reads_its_declared_return() {
    let out = lift(
        "<?php
final class BadException extends \\RuntimeException {}
final class Scan
{
    /** @return list<array{tag: string}> */
    private function hits(string $s): array
    {
        if ($s === '') { throw new BadException('empty'); }
        return [['tag' => $s]];
    }

    public function first(string $s): string
    {
        foreach ($this->hits($s) as $h) { return $h['tag']; }
        return '';
    }
}",
    );
    assert!(out.contains("this.hits(s)?"), "{out}");
    assert!(out.contains("return h.tag;"), "{out}");
    checks_clean(&out);
}

/// Another object's method is not resolved statically (DEC-550's scope): its binder stays unbound.
#[test]
fn a_binder_over_another_objects_method_binds_nothing() {
    let out = lift(
        "<?php
final class Box
{
    /** @return list<array{tag: string}> */
    public function hits(): array { return [['tag' => 'a']]; }
}
final class User
{
    public function first(Box $b): string
    {
        foreach ($b->hits() as $h) { return $h['tag']; }
        return '';
    }
}",
    );
    assert!(out.contains("return h[\"tag\"];"), "{out}");
}
