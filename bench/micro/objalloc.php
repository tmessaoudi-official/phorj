<?php
// Idiomatic PHP counterpart of objalloc.phg (hand-authored). Same shape: warm call, timed call, checksum.
final class Cell {
    public function __construct(private int $v) {}
    public function sq(): int {
        return $this->v * $this->v;
    }
}

function bench(int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $c = new Cell($i);
        $acc = $acc + $c->sq();
    }
    return $acc;
}

$iters = 2000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("objalloc\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
