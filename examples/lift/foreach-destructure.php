<?php

/**
 * A `foreach` that destructures each element — `[$word, $at]` or `list($word, $at)` — lifts to
 * phorj's tuple loop `for ((word, at) in …)` (scout row 5v, DEC-510's positional form). The binders
 * belong to the loop: a name reused after it is a fresh declaration. A keyed pattern, a nested one
 * and a skipped slot (`[, $x]`) are refused by name. Lifted with `phg lift foreach-destructure.php`.
 */

/** @return array{string, int} */
function hit(string $word, int $at): array
{
    return [$word, $at];
}

/** @return list<array{string, int}> */
function hits(): array
{
    return [hit('loyer', 4), hit('social', 19), hit('pls', 31)];
}

function main(): void
{
    $last = 0;
    foreach (hits() as [$word, $at]) {
        echo $word, "@", $at, " ";
        $last = $at;
    }
    echo "\n";
    foreach (hits() as list($word, $at)) {
        foreach (hits() as [$other, $pos]) {
            if ($pos > $at) {
                echo $word, "<", $other, " ";
            }
        }
    }
    echo "\n";
    $word = "last";
    echo $word, "=", $last, "\n";
}
