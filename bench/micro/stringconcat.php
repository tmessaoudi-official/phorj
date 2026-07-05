<?php
// Idiomatic PHP counterpart of stringconcat.phg (hand-authored). Same index-varying operands so the
// tracing JIT cannot hoist the concat (a loop-invariant concat folds to a constant — measuring nothing).
function bench(int $iters): int {
    $parts = ["alpha", "beta", "gamma", "delta"];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $s = $parts[$i % 4] . $parts[($i + 1) % 4];
        $acc = $acc + strlen($s);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("stringconcat\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
