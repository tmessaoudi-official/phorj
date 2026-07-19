<?php
// Idiomatic PHP counterpart of listcontains.phg (hand-authored). `in_array(..., true)` strict linear
// search; data-dependent needle `$i % 12` (constant list) so it cannot be hoisted, no per-iter alloc.
function bench(int $iters): int {
    $xs = [3, 1, 4, 1, 5, 9, 2, 6];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        if (in_array($i % 12, $xs, true)) {
            $acc = $acc + 1;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("listcontains\t%d\t%d\n", $d + $guard, $acc);
