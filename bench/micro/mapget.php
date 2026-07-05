<?php
// Idiomatic PHP counterpart of mapget.phg (hand-authored). Same fixed assoc array + key rotation.
function bench(int $iters): int {
    $m = ["a" => 10, "b" => 20, "c" => 30, "d" => 40];
    $keys = ["a", "b", "c", "d"];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $k = $keys[$i % 4];
        $acc = $acc + $m[$k];
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("mapget\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
