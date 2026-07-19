<?php
// Idiomatic PHP counterpart of setcontains.phg (hand-authored). A PHP set is a hash keyed by its
// elements (`array_flip` of the element list), and membership is `isset($s[$k])` — the idiomatic,
// performant PHP set-lookup. Data-dependent needle `$i % 16` (constant set) so it cannot be hoisted.
function bench(int $iters): int {
    $s = array_flip([3, 1, 4, 1, 5, 9, 2, 6]);
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        if (isset($s[$i % 16])) {
            $acc = $acc + 1;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("setcontains\t%d\t%d\n", $d + $guard, $acc);
