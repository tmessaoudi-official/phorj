<?php
// Idiomatic PHP counterpart of closurecall.phg (hand-authored). Arrow-fn closure, called per iteration.
function bench(int $iters): int {
    $f = fn($x) => $x * 2 + 1;
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + $f($i % 100);
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("closurecall\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
