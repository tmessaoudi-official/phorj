<?php
// Idiomatic PHP counterpart of strbuild.phg (hand-authored): the classic `.=` accumulator —
// PHP appends IN PLACE when the string's refcount is 1 (amortized O(1)), its signature string
// advantage. Same logic ⇒ identical checksum.
function bench(int $iters): int {
    $parts = ["alpha", "beta", "gamma", "delta"];
    $s = "";
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $s .= $parts[$i % 4];
        if (strlen($s) > 512) {
            $acc += strlen($s);
            $s = "";
        }
    }
    return $acc + strlen($s);
}
$iters = 2000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("strbuild\t%d\t%d\n", $d + $guard, $acc);
