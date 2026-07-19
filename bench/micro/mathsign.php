<?php
// Idiomatic PHP counterpart of mathsign.phg (hand-authored). Phorj's `Math.sign` erases to PHP's
// spaceship `<=> 0` (single-evaluating, yields -1/0/1). The operand is data-dependent (`$i % 3 - 1`,
// yielding -1/0/1) so the call cannot be hoisted. Isolates the native-call overhead: phorj's VM native
// dispatch vs php's C spaceship.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + (($i % 3 - 1) <=> 0);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("mathsign\t%d\t%d\n", $d + $guard, $acc);
