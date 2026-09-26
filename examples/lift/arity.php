<?php
// Builtins whose phorj form depends on the arity (row 4g). Lift it: `phg lift examples/lift/arity.php`.

/**
 * One argument: `min`/`max` take an ARRAY and lift to `xs.min()!` — `!` because an empty list has no
 * minimum (PHP throws `ValueError`; phorj faults on the unwrap).
 *
 * @param list<int> $xs
 */
function spread(array $xs): int
{
    return max($xs) - min($xs);
}

// Three or more values: a left fold of the two-value form, `a.min(b).min(c)`.
function clamp(int $v, int $lo, int $hi): int
{
    return max($lo, min($v, $hi, 100));
}

// `substr` with no length runs to the end: `s.substring(i, 9223372036854775807)`.
function tail(string $s, int $i): string
{
    return substr($s, $i);
}

echo spread([4, 1, 7]), "|", clamp(250, 0, 150), "|", clamp(-5, 0, 150), "|", tail("hello", 1), "|", tail("hello", -2), "\n";
