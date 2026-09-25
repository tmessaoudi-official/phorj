<?php
// Idiomatic PHP counterpart of mapunion.phg (hand-authored). Array `+` is union: the LEFT operand wins
// on a shared key; the checksum folds the union's cardinality plus `$u["a"]` (always the left `1`). The
// second operand rotates (`$others[$i % 3]`) so the union cannot be hoisted out of the loop.
function bench(int $iters): int {
    $a = ["a" => 1, "b" => 2, "c" => 3];
    $others = [
        ["b" => 20, "d" => 4],
        ["c" => 30, "e" => 5, "f" => 6],
        ["a" => 10, "g" => 7],
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $u = $a + $others[$i % 3];
        $acc = $acc + count($u) + $u["a"];
    }
    return $acc;
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mapunion\t%d\t%d\n", $d + $guard, $acc);
