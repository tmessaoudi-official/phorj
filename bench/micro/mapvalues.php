<?php
// Idiomatic PHP counterpart of mapvalues.phg (hand-authored). `array_values` materializes the value
// list; the map operand rotates (`$maps[$i % 3]`) so it cannot be hoisted, and the result is indexed
// (`$vs[$i % count($vs)]`) so the list must materialize. The checksum folds that read.
function bench(int $iters): int {
    $m1 = ["a" => 11, "b" => 22];
    $m2 = ["a" => 3, "b" => 5, "c" => 7, "d" => 9, "e" => 13];
    $m3 = ["solo" => 99];
    $maps = [$m1, $m2, $m3];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $m = $maps[$i % 3];
        $vs = array_values($m);
        $acc = $acc + $vs[$i % count($vs)];
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapvalues\t%d\t%d\n", $d + $guard, $acc);
