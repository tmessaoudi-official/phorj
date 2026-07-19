<?php
// Idiomatic PHP counterpart of mapmap.phg (hand-authored). `array_map` with a closure over a
// data-dependent `$bump` (`$i % 3`) transforms the values (keys preserved). The result values are
// materialized (`array_values`) and indexed so the mapped array cannot be folded.
function bench(int $iters): int {
    $m = [
        "a" => 1, "b" => 2, "c" => 3, "d" => 4,
        "e" => 5, "f" => 6, "g" => 7, "h" => 8,
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $bump = $i % 3;
        $mapped = array_map(fn($v) => $v + $bump, $m);
        $vs = array_values($mapped);
        $acc = $acc + $vs[$i % count($vs)];
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapmap\t%d\t%d\n", $d + $guard, $acc);
