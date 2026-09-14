<?php
// Row 5f: PHP's `usort` and `array_map` lift to phorj's receiver-form `Core.List` calls.
//
// `usort($xs, $cmp);` sorts its argument BY REFERENCE and returns `bool`, so only the statement
// form has a phorj counterpart: `xs = xs.sortWith(cmp);`. Both sorts are STABLE, which is why
// "fig" and "yam" (and "pear", "plum", "kiwi") keep their input order below.
//
// `array_map($f, $xs)` takes the callable FIRST; phorj's `map` is receiver-first: `xs.map(f)`.
//
// The sort copies its parameter into a local first: a phorj parameter is immutable, so
// `usort($words, …)` on the parameter itself would lift to a reassignment `phg check` rejects.

/**
 * @param list<string> $words
 * @return list<string>
 */
function byLength(array $words): array
{
    $out = $words;
    usort($out, fn(string $a, string $b): int => strlen($a) <=> strlen($b));
    return $out;
}

/**
 * @param list<string> $words
 * @return list<int>
 */
function lengths(array $words): array
{
    return array_map(fn(string $w): int => strlen($w), $words);
}

foreach (byLength(["pear", "fig", "plum", "kiwi", "yam"]) as $w) {
    echo $w . "\n";
}
foreach (lengths(["pear", "fig"]) as $n) {
    echo $n . "\n";
}
