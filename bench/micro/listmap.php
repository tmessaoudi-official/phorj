<?php
// Idiomatic PHP counterpart of listmap.phg (hand-authored). `array_map` with a closure, allocating a
// fresh array per call; data-dependent transform `$x + ($i % 3)` and a data-dependent read of the
// result so neither backend can fold the map to a closed form.
function bench(int $iters): int {
    $xs = [1, 2, 3, 4, 5, 6, 7, 8];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $bump = $i % 3;
        $ys = array_map(fn($x) => $x + $bump, $xs);
        $acc = $acc + $ys[$i % 8];
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("listmap\t%d\t%d\n", $d + $guard, $acc);
