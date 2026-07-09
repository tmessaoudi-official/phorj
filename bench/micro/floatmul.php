<?php
// Idiomatic PHP counterpart of floatmul.phg (hand-authored — NOT transpiled — so no __phorj_* helper
// weight skews the baseline). Same shape: an IIR recurrence acc = acc*r + 0.5 (loop-carried f64), a
// warm call then a timed call; the checksum is (int)$acc (truncate toward zero), matching phorj's
// Conversion.truncate. f64 arithmetic in the same order is bit-identical, so the checksum agrees.
function bench(int $iters, float $r): float {
    $acc = 0.0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc * $r + 0.5;
    }
    return $acc;
}

$iters = 2000000;
$r = 1.0000001;
$warm = bench($iters, $r);
$guard = (int)($warm - $warm);
$t = hrtime(true);
$acc = bench($iters, $r);
$d = hrtime(true) - $t;
$cs = (int)$acc;
printf("floatmul\t%d\t%d\n", $d + $guard, $cs);
