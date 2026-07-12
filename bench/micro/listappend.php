<?php
// Idiomatic PHP counterpart of listappend.phg (hand-authored): `$xs[] = $v` push — the
// amortized-O(1) zend array append this micro races against.
function bench(int $iters): int {
    $xs = [0];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $xs[] = $i;
        if (count($xs) >= 256) {
            $acc = $acc + count($xs) + $xs[0] + $xs[255];
            $xs = [0];
        }
    }
    return $acc + count($xs);
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("listappend\t%d\t%d\n", $d + $guard, $acc);
