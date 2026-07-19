<?php
// Idiomatic PHP counterpart of listfilter.phg (hand-authored). `array_filter` with a closure;
// data-dependent predicate `($x + $bump) % 2 == 0` so the survivor set cannot be folded.
function bench(int $iters): int {
    $xs = [1, 2, 3, 4, 5, 6, 7, 8];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $bump = $i % 2;
        $ys = array_filter($xs, fn($x) => ($x + $bump) % 2 == 0);
        $acc = $acc + count($ys);
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("listfilter\t%d\t%d\n", $d + $guard, $acc);
