<?php
// Idiomatic PHP counterpart of forin.phg (hand-authored): `foreach` over a small array,
// repeated — the zend foreach iteration this micro races against.
function bench(int $iters): int {
    $xs = [3, 1, 4, 1, 5, 9, 2, 6];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        foreach ($xs as $x) {
            $acc = $acc + $x;
        }
    }
    return $acc;
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("forin\t%d\t%d\n", $d + $guard, $acc);
