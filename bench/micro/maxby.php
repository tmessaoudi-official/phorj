<?php
// Idiomatic PHP counterpart of maxby.phg (hand-authored). `__phorj_max_by` is phorj's gated erasure
// (there is no single builtin — PHP `max`/`array_reduce` don't give first-wins-by-key), inlined here as
// the natural foreach. The selector `($x + $bump) % 7` is data-dependent so the fold cannot be hoisted.
// Isolates phorj's per-element re-entrant callback vs php's foreach + strict-`>` first-wins fold.
function bench(int $iters): int {
    $xs = [5, 2, 8, 1, 9, 3, 7, 4];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $bump = $i % 3;
        $f = fn($x) => ($x + $bump) % 7;
        $best = null; $bk = null; $has = false;
        foreach ($xs as $x) { $k = $f($x); if (!$has || (is_string($k) ? strcmp($k, $bk) : ($k <=> $bk)) > 0) { $best = $x; $bk = $k; $has = true; } }
        $acc = $acc + ($best ?? 0);
    }
    return $acc;
}
$iters = 500000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("maxby\t%d\t%d\n", $d + $guard, $acc);
