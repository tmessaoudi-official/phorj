<?php
// Idiomatic PHP counterpart of stringcontains.phg (hand-authored). `str_contains` is the PHP 8 C
// builtin; the needle rotates (`$needles[$i % 6]`, constant haystack) so it cannot be hoisted. The
// checksum counts hits. A native-vs-builtin substring-search probe.
function bench(int $iters): int {
    $hay = "the quick brown fox jumps over the lazy dog";
    $needles = ["fox", "cat", "lazy", "zzz", "the", "qux"];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        if (str_contains($hay, $needles[$i % 6])) {
            $acc = $acc + 1;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("stringcontains\t%d\t%d\n", $d + $guard, $acc);
