<?php
// Idiomatic PHP counterpart of sumby.phg (hand-authored). `array_sum(array_map($fn, $xs))` is the
// natural php spelling; the projection `$x + $bump` is data-dependent so the map cannot be hoisted.
// Isolates phorj's per-element re-entrant callback vs php's array_map+array_sum.
function bench(int $iters): int {
    $xs = [1, 2, 3, 4, 5, 6, 7, 8];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $bump = $i % 2;
        $acc = $acc + array_sum(array_map(fn($x) => $x + $bump, $xs));
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("sumby\t%d\t%d\n", $d + $guard, $acc);
