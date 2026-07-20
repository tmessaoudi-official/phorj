<?php
// Idiomatic-equivalent PHP twin of isurl.phg: the SAME anchored `preg_match(…/D)` that
// Core.Validation.isUrl transpiles to (byte-identical semantics). Rotating probe so it cannot hoist.
function bench(int $iters): int {
    $probes = [
        "https://x.io/p",
        "http://a.b:8080/x",
        "notaurl",
        "https://ok.example.org",
        "ftp://no.go",
        "https://bad host/x",
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        if (preg_match('/^https?:\/\/[A-Za-z0-9.-]+(:[0-9]+)?(\/[^\x00-\x20]*)?$/D', $probes[$i % 6]) === 1) {
            $acc = $acc + 1;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("isurl\t%d\t%d\n", $d + $guard, $acc);
