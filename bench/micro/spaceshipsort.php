<?php
// Idiomatic PHP counterpart of spaceshipsort.phg (hand-authored). The two-key comparator written the
// way PHP writes it — `[$a->tier, $a->position] <=> [$b->tier, $b->position]` — which is the exact
// line `phg lift` turns into a tuple comparison. `usort` sorts IN PLACE by reference, so the array is
// copied first, matching phorj's `sortWith` returning a new list; both sides therefore pay for one
// list per iteration and allocate two small arrays/tuples per comparison.
class Row {
    public function __construct(public int $tier, public int $position) {}
}

function bench(array $rows, int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $copy = $rows;
        usort($copy, fn($a, $b) => [$a->tier, $a->position] <=> [$b->tier, $b->position]);
        foreach ($copy as $s) {
            $acc = ($acc + $s->tier * 7 + $s->position * 13) % 1000003;
        }
    }
    return $acc;
}

$rows = [];
for ($i = 0; $i < 1200; $i++) {
    $rows[] = new Row($i % 7, ($i * 37) % 101);
}
$iters = 40;
$warm = bench($rows, 2); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($rows, $iters); $d = hrtime(true) - $t;
printf("spaceshipsort\t%d\t%d\n", $d + $guard, $acc);
