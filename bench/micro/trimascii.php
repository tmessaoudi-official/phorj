<?php
// Idiomatic PHP counterpart of trimascii.phg (hand-authored): `trim()` with its default character list,
// which `String.trimAscii` transpiles to. Four inputs rotate (`$xs[$i % 4]`) so the call cannot be
// hoisted; the checksum folds each trimmed length (form feed kept, NUL and vertical tab stripped).
function bench(int $iters): int {
    $xs = ["  padded both  ", "\t\ntabbed\r\n", "\x0Ckept\x0C", " \0nul\x0B "];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + strlen(trim($xs[$i % 4]));
    }
    return $acc;
}
$iters = 1000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("trimascii\t%d\t%d\n", $d + $guard, $acc);
