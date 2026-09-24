<?php
// Idiomatic PHP counterpart of coalesceor.phg (hand-authored).
function orZero(?int $n): int { return $n ?? 0; }
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + orZero($i % 100);
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("coalesceor\t%d\t%d\n", $d + $guard, $acc);
