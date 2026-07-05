<?php
// Idiomatic PHP counterpart of interp.phg (hand-authored). Same interpolated string, length folded.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $s = "v=$i";
        $acc = $acc + strlen($s);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("interp\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
