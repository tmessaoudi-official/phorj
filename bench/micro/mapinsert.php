<?php
// Idiomatic PHP counterpart of mapinsert.phg (hand-authored): `$m[$k] = $v` on a string-keyed
// zend array — insert + overwrite mix with a periodic read + reset.
function bench(int $iters): int {
    $keys = ["alpha", "beta", "gamma", "delta", "epsi", "zeta", "eta", "theta"];
    $m = ["alpha" => 0];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $k = $keys[$i % 8];
        $m[$k] = $i;
        if ($i % 64 == 63) {
            $acc = $acc + $m["alpha"] + $m["theta"];
            $m = ["alpha" => 0];
        }
    }
    return $acc;
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapinsert\t%d\t%d\n", $d + $guard, $acc);
