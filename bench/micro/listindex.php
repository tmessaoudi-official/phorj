<?php
// Idiomatic PHP counterpart of listindex.phg (hand-authored). Data-dependent index `($i + $acc) % 8`
// so the read cannot be precomputed to a closed form (a periodic constant index can be folded).
function bench(int $iters): int {
    $xs = [3, 1, 4, 1, 5, 9, 2, 6];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $idx = ($i + $acc) % 8;
        $acc = $acc + $xs[$idx];
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("listindex\t%d\t%d\n", $d + $guard, $acc);
