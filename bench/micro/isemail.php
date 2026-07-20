<?php
// Idiomatic-equivalent PHP twin of isemail.phg: the SAME anchored `preg_match(…/D)` that
// Core.Validation.isEmail transpiles to (byte-identical semantics — deliberately NOT
// filter_var(FILTER_VALIDATE_EMAIL), which accepts dotless domains isEmail rejects). Rotating probe so
// it cannot hoist.
function bench(int $iters): int {
    $probes = [
        "a@b.co",
        "user@localhost",
        "a..b@c.com",
        "x.y+z@mail.example.org",
        "no-at-sign",
        "bad@dom..com",
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        if (preg_match('/^(?!.*\.\.)[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}$/D', $probes[$i % 6]) === 1) {
            $acc = $acc + 1;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("isemail\t%d\t%d\n", $d + $guard, $acc);
