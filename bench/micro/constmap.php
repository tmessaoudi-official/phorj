<?php
// Idiomatic PHP counterpart of constmap.phg (hand-authored).
final class Fold { public const array TABLE = [0 => 3, 1 => 5, 2 => 7, 3 => 11]; }
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + Fold::TABLE[$i % 4];
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("constmap\t%d\t%d\n", $d + $guard, $acc);
