<?php
// Builtins whose phorj native takes its arguments in another order (row 4i). Lift it:
// `phg lift examples/lift/reordered.php`.

/**
 * Strict `in_array` is `List.contains` (qualified: `contains` also exists on strings); `implode` and
 * `explode` are `join` and `split` on the receiver.
 *
 * @param list<string> $tags
 */
function summary(array $tags, string $line): string
{
    $mark = in_array('urgent', $tags, true) ? '!' : '-';
    return $mark . ' ' . implode(', ', explode(' ', $line));
}

// `str_replace` over a literal array re-replaces left to right: exactly a chain of `replace`.
function slug(string $s): string
{
    return str_replace([' ', '_'], '-', str_replace(['à', 'é'], ['a', 'e'], $s));
}

// A `%s`-only `sprintf` is the `.` chain it formats, so it lifts to an interpolation.
function report(string $who, string $what): string
{
    return sprintf('%s: %s (100%% ' . 'done)', $who, $what);
}

echo summary(['urgent', 'home'], 'fix the roof'), "|", slug('café à_emporter'), "|", report('Ann', 'roof'), "|", PHP_INT_MAX, "\n";
