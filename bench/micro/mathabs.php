<?php
// Idiomatic PHP counterpart of mathabs.phg (hand-authored). `abs()` is a PHP C builtin; the operand
// is data-dependent (`$i % 2000 - 1000`, spanning negatives) so the call cannot be hoisted, and never
// reaches PHP_INT_MIN. Isolates the native-call overhead: phorj's VM native dispatch vs php's C builtin.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + abs($i % 2000 - 1000);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mathabs\t%d\t%d\n", $d + $guard, $acc);
