<?php
// Idiomatic PHP counterpart of stringconcat.phg (hand-authored). Two fixed strings, concat length folded.
function bench(int $iters): int {
    $a = "hello"; $b = "world";
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $s = $a . $b;
        $acc = $acc + strlen($s);
    }
    return $acc;
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("stringconcat\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
