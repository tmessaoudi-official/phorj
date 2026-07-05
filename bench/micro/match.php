<?php
// Idiomatic PHP counterpart of match.phg (hand-authored). PHP 8 `match` expression, same arms.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $w = match ($i % 4) {
            0 => 7,
            1 => 3,
            2 => 5,
            default => 1,
        };
        $acc = $acc + $w;
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("match\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
