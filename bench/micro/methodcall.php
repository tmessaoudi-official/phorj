<?php
// Idiomatic PHP counterpart of methodcall.phg (hand-authored). Same shape: warm call, timed call,
// checksum print.
final class Box {
    public function __construct(private int $v) {}
    public function get(): int {
        return $this->v;
    }
}

function bench(int $iters): int {
    $b = new Box(7);
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $acc = $acc + $b->get();
    }
    return $acc;
}

$iters = 5000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("methodcall\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
