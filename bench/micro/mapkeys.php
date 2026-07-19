<?php
// Idiomatic PHP counterpart of mapkeys.phg (hand-authored). `array_keys` materializes the key list; the
// map operand rotates (`$maps[$i % 3]`, varied sizes) so it cannot be hoisted, and the result is indexed
// (`$ks[$i % count($ks)]`) so the list must materialize. The checksum folds a byte-length read.
function bench(int $iters): int {
    $m1 = ["a" => 1, "b" => 2];
    $m2 = ["a" => 1, "b" => 2, "c" => 3, "d" => 4, "e" => 5];
    $m3 = ["solo" => 9];
    $maps = [$m1, $m2, $m3];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $m = $maps[$i % 3];
        $ks = array_keys($m);
        $acc = $acc + strlen($ks[$i % count($ks)]);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapkeys\t%d\t%d\n", $d + $guard, $acc);
