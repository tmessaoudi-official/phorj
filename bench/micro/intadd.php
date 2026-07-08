<?php
// Idiomatic PHP counterpart of intadd.phg (hand-authored — NOT transpiled — so no __phorj_* helper
// weight skews the baseline). Same shape: warm call, then a timed call; result printed as a checksum.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + ($i * 3 - 1);
    }
    return $acc;
}

$iters = 5000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("intadd\t%d\t%d\n", $d + $guard, $acc);
