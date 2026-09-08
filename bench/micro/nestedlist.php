<?php
// Idiomatic PHP counterpart of nestedlist.phg (hand-authored — NOT transpiled). The positional
// sibling of namedtuplefield.php: a list of LIST arrays instead of a list of ASSOCIATIVE arrays,
// so the pair isolates PHP's string-key hash from the array indexing underneath it, exactly as the
// phorj pair isolates the named tuple from its erased list. Same loop, same checksum.
function bench(int $iters): int {
    $rows = [
        [1, 3], [2, 1], [3, 4], [4, 1],
        [5, 5], [6, 9], [7, 2], [8, 6],
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $idx = ($i + $acc) % 8;
        $acc = $acc + $rows[$idx][1] * 2 - $rows[$idx][0];
    }
    return $acc;
}

$iters = 5000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("nestedlist\t%d\t%d\n", $d + $guard, $acc);
