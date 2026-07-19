<?php
// Idiomatic PHP counterpart of mapmerge.phg (hand-authored). `array_merge` combines two assoc arrays
// (later keys win) and the checksum folds the merged-key cardinality. The second operand rotates
// (`$others[$i % 3]`) so the merge cannot be hoisted out of the loop.
function bench(int $iters): int {
    $a = ["a" => 1, "b" => 2, "c" => 3];
    $others = [
        ["b" => 20, "d" => 4],
        ["c" => 30, "e" => 5, "f" => 6],
        ["a" => 10, "g" => 7],
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $merged = array_merge($a, $others[$i % 3]);
        $acc = $acc + count($merged);
    }
    return $acc;
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapmerge\t%d\t%d\n", $d + $guard, $acc);
