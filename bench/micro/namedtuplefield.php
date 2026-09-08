<?php
// Idiomatic PHP counterpart of namedtuplefield.phg (hand-authored — NOT transpiled). PHP has no
// named tuple; the idiomatic named record is an associative array, which is also the shape the
// scout corpus writes and that `phg lift` turns into a named tuple. Same loop, same checksum.
function bench(int $iters): int {
    $rows = [
        ['tier' => 1, 'bp' => 3], ['tier' => 2, 'bp' => 1], ['tier' => 3, 'bp' => 4], ['tier' => 4, 'bp' => 1],
        ['tier' => 5, 'bp' => 5], ['tier' => 6, 'bp' => 9], ['tier' => 7, 'bp' => 2], ['tier' => 8, 'bp' => 6],
    ];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $idx = ($i + $acc) % 8;
        $acc = $acc + $rows[$idx]['bp'] * 2 - $rows[$idx]['tier'];
    }
    return $acc;
}

$iters = 5000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("namedtuplefield\t%d\t%d\n", $d + $guard, $acc);
