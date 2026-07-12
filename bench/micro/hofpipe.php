<?php
// Idiomatic PHP counterpart of hofpipe.phg (hand-authored): array_map with a capturing
// closure, then a counting filter — the zend closure + array-function pipeline.
function bench(int $iters): int {
    $xs = [3, 1, 4, 1, 5, 9, 2, 6];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $k = $i % 7 + 1;
        $ys = array_map(fn($x) => $x * $k, $xs);
        $n = 0;
        foreach ($ys as $y) {
            if ($y % 2 == 0) { $n++; }
        }
        $acc = $acc + $n;
    }
    return $acc;
}
$iters = 300000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("hofpipe\t%d\t%d\n", $d + $guard, $acc);
