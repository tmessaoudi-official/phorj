<?php
// Idiomatic PHP counterpart of throwexpr.phg (hand-authored).
final class MissError extends \RuntimeException {}
function orThrow(?int $n): int { return $n ?? throw new MissError('none'); }
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + orThrow($i % 100);
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("throwexpr\t%d\t%d\n", $d + $guard, $acc);
