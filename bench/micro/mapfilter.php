<?php
// Idiomatic PHP counterpart of mapfilter.phg (hand-authored). `array_filter` with a value predicate that
// closes over a data-dependent `$bump` (`$i % 2`) so the survivor set changes each iteration and cannot
// be folded. The checksum folds the survivor cardinality.
function bench(int $iters): int {
    $m = [
        "a" => 1, "b" => 2, "c" => 3, "d" => 4,
        "e" => 5, "f" => 6, "g" => 7, "h" => 8,
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $bump = $i % 2;
        $kept = array_filter($m, fn($v) => ($v + $bump) % 2 == 0);
        $acc = $acc + count($kept);
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapfilter\t%d\t%d\n", $d + $guard, $acc);
