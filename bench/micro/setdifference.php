<?php
// Idiomatic PHP counterpart of setdifference.phg (hand-authored). Int sets as plain arrays;
// `array_diff($a, $b)` returns the elements of `$a` absent from `$b` and the checksum folds its
// cardinality. The second operand rotates (`$bs[$i % 4]`) so the difference cannot be hoisted.
function bench(int $iters): int {
    $a = [1, 2, 3, 4, 5, 6, 7, 8];
    $bs = [
        [3, 4, 5, 6, 7, 8, 9, 10],
        [9, 10, 11, 12],
        [1, 2, 3, 4],
        [13, 14, 15, 16, 17],
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $b = $bs[$i % 4];
        $acc = $acc + count(array_diff($a, $b));
    }
    return $acc;
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("setdifference\t%d\t%d\n", $d + $guard, $acc);
