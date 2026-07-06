<?php
// Idiomatic PHP counterpart of trycatch.phg (hand-authored). `step` throws on odd inputs; `bench`
// wraps each call in try/catch, so every iteration pays try-block cost and ~3-in-7 pay a throw+catch.
class Odd extends Exception {
    public int $n;
    public function __construct(string $message, int $n) {
        parent::__construct($message);
        $this->n = $n;
    }
}
function step(int $x): int {
    if ($x % 2 === 1) {
        throw new Odd("odd", $x);
    }
    return $x;
}
function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        try {
            $acc = $acc + step($i % 7);
        } catch (Odd $e) {
            $acc = $acc + $e->n;
        }
    }
    return $acc;
}
$iters = 3000000;
$warm = bench($iters); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($iters); $d = hrtime(true) - $t;
printf("trycatch\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
