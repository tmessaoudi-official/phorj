<?php
// Idiomatic PHP counterpart of userhof.phg (hand-authored): a user-written higher-order function
// taking a `callable`, with an arrow-fn passed at the call site — the same shape, written the way a
// PHP developer would write it.
//
// `callable` (not `Closure`) is the faithful twin of phorj's `(int) => int` function type: it is what
// PHP code actually declares, and it is the FASTER of the two here, which raises the bar on phorj
// rather than lowering it. The arrow fn `fn(int $x): int => ...` is allocated at the call site in
// both legs, so neither side gets a hoisting advantage the other lacks.
function applyTwice(callable $f, int $x): int {
    return $f($f($x));
}
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = applyTwice(fn(int $x): int => $x * 2 + 1, $i) % 1000003;
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("userhof\t%d\t%d\n", $d + $guard, $acc);
