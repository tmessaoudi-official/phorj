<?php
// Idiomatic PHP counterpart of mathmin.phg (hand-authored). `min()` is a PHP C builtin; both operands
// are data-dependent (`$i % 1000`, `($i * 3) % 1000`) so the call cannot be hoisted. Isolates the
// native-call overhead: phorj's VM native dispatch vs php's C builtin.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + min($i % 1000, ($i * 3) % 1000);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mathmin\t%d\t%d\n", $d + $guard, $acc);
