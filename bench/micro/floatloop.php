<?php
// Idiomatic PHP counterpart of floatloop.phg (hand-authored). Same float-driven loop with a
// float compare + reset; identical arithmetic ⇒ identical checksum.
function bench(int $iters): int {
    $x = 0.0;
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $x = $x + 1.5;
        if ($x > 1000000.0) {
            $x = 0.5;
            $acc = $acc + 1;
        }
    }
    return $acc + intval($x);
}
$iters = 5000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("floatloop\t%d\t%d\n", $d + $guard, $acc);
