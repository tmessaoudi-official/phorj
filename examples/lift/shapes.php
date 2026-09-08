<?php

/**
 * PHP array SHAPES — `array{…}` — and why they split in two.
 *
 * A POSITIONAL shape is a tuple, which phorj has. A KEYED shape needs a named-field tuple, which
 * phorj does not have yet (DEC-504), so `phg lift` refuses it BY NAME rather than as a generic
 * Tier-2 wall. Lifted with `phg lift shapes.php`.
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
}
