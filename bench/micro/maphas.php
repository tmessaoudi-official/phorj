<?php
// Idiomatic PHP counterpart of maphas.phg (hand-authored). `array_key_exists` is the exact key-presence
// test matching Map.has. Data-dependent probed key `$probes[$i % 6]` (constant map) so it cannot hoist.
function bench(int $iters): int {
    $m = ["a" => 10, "b" => 20, "c" => 30, "d" => 40];
    $probes = ["a", "b", "c", "d", "e", "f"];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        if (array_key_exists($probes[$i % 6], $m)) {
            $acc = $acc + 1;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("maphas\t%d\t%d\n", $d + $guard, $acc);
