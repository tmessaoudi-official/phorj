<?php
// Idiomatic PHP counterpart of listindex.phg (hand-authored). Same fixed list, same i%len read/fold.
function bench(int $iters): int {
    $xs = [3, 1, 4, 1, 5, 9, 2, 6];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + $xs[$i % 8];
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("listindex\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
