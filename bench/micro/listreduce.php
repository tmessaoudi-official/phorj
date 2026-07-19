<?php
// Idiomatic PHP counterpart of listreduce.phg (hand-authored). `array_reduce` with a closure;
// data-dependent seed `$i % 7` so the fold cannot be precomputed to a constant.
function bench(int $iters): int {
    $xs = [1, 2, 3, 4, 5, 6, 7, 8];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $seed = $i % 7;
        $total = array_reduce($xs, fn($a, $x) => $a + $x, $seed);
        $acc = $acc + $total;
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("listreduce\t%d\t%d\n", $d + $guard, $acc);
