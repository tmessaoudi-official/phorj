<?php
// PHP's untyped `[]` where the program declared the collection (row 4h). Lift it:
// `phg lift examples/lift/empty.php`.

/**
 * A strict `=== []` on a variable declared as a list or map is `List.isEmpty` / `Map.isEmpty` — the
 * QUALIFIED native, so a name the draft rebinds to anything else fails `phg check` instead of changing
 * meaning. A loose `== []` is left alone: it is also true for `null`, `false`, `0` and `""`.
 *
 * @param list<string> $tags
 * @param array<string, int> $counts
 */
function describe(array $tags, array $counts): string
{
    $t = $tags === [] ? 'untagged' : 'tagged';
    $c = [] !== $counts ? 'counted' : 'uncounted';
    return $t . '/' . $c;
}

/**
 * A `return [];` at any depth IS the declared return type, so it is constructed as one.
 *
 * @return list<int>
 */
function evens(int $n): array
{
    if ($n < 1) {
        return [];
    }
    /** @var list<int> $out */
    $out = [];
    for ($i = 2; $i <= $n; $i += 2) {
        $out[] = $i;
    }
    return $out;
}

/** @var list<string> $none */
$none = [];
/** @var array<string, int> $zero */
$zero = [];
echo describe($none, $zero), "|", describe(['a'], ['k' => 1]), "|", count(evens(0)), "|", count(evens(7)), "\n";
