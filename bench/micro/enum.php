<?php
// Idiomatic PHP counterpart of enum.phg (hand-authored). PHP has no payload-carrying enums, so the
// idiomatic equivalent of constructing + matching a sum type is a lightweight tag `match` — the
// LEANEST PHP for this logic (no object alloc), i.e. the hardest baseline for phorj's enum to beat.
// Same arithmetic ⇒ identical checksum.
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $r = $i % 10;
        $a = match ($i % 3) {
            0 => $r * $r,
            1 => $r * $r * 2,
            default => 1,
        };
        $acc = $acc + $a;
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("enum\t%d\t%d\n", $d + $guard, $acc);
