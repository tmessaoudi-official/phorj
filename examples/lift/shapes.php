<?php

/**
 * PHP array SHAPES — `array{…}` — and why they split in two.
 *
 * A POSITIONAL shape is a tuple (DEC-288). A KEYED shape is a named-field tuple (DEC-504), and its
 * reads and writes lift with it (DEC-515): `$row['bp']` on a `foreach` binder over a declared
 * `list<array{…}>` becomes `row.bp`, and a keyed literal returned under a keyed `@return` becomes a
 * tuple literal. Lifted with `phg lift shapes.php`.
 */

/** @return array{int, string} */
function pair(int $n): array
{
    return [$n, "n={$n}"];
}

/** The same shape with explicit ascending indices — what PHPStan and Psalm emit. */
/** @return array{0: float, 1: float} */
function origin(): array
{
    return [0.0, 0.0];
}

/** A declared `?array` plus a docblock `|null` are ONE nullable, not two. */
/** @return array{int, string}|null */
function maybePair(bool $yes): ?array
{
    if ($yes) {
        return [1, 'one'];
    }
    return null;
}

/** A KEYED literal in declared field order IS the named tuple its `@return` describes. */
/** @return array{tenure: string, bp: int} */
function verdict(string $tenure, int $bp): array
{
    return ['tenure' => $tenure, 'bp' => $bp];
}

/** The binder inherits the ELEMENT shape of the declared collection, so `$row['bp']` is a field read. */
/** @param list<array{tenure: string, bp: int}> $rows */
function totalBp(array $rows): int
{
    $n = 0;
    foreach ($rows as $row) {
        $n = $n + $row['bp'];
    }
    return $n;
}

function main(): void
{
    // A positional shape is READ by destructuring — a phorj tuple has no index access.
    [$n, $label] = pair(7);
    echo $n, " ", $label, "\n";
    [$x, $y] = origin();
    echo $x, " ", $y, "\n";
    // (A ternary inside `echo` is KNOWN_ISSUES LIFT-ECHO-TERNARY, so this uses an `if`.)
    if (maybePair(false) === null) {
        echo "none\n";
    } else {
        echo "some\n";
    }
    $total = totalBp([verdict('rent', 3), verdict('own', 4)]);
    echo $total, "\n";
}
