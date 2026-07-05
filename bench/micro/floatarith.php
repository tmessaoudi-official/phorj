<?php
// Idiomatic PHP counterpart of floatarith.phg (hand-authored — NOT transpiled). Same shape and same
// float-fold-into-int checksum: f64 IEEE arithmetic in the same order is bit-identical, so `(int)$acc`
// matches the phorj `Conversion.truncate`.
function bench(int $iters): int {
    $acc = 0.0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + (float) $i * 0.5;
    }
    return (int) $acc;
}

$iters = 2000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("floatarith\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
