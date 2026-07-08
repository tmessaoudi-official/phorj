<?php
// Idiomatic PHP counterpart of fibrec.phg (hand-authored — NOT transpiled — so no __phorj_* helper
// weight skews the baseline). Same shape: warm call, then a timed call; result printed as a checksum.
// Recursive int Fibonacci — the exponential-call workload PHP's tracing JIT handles well, which is
// exactly why it is the honest bar for phorj's unboxed native codegen.
function fib(int $n): int {
    if ($n < 2) {
        return $n;
    }
    return fib($n - 1) + fib($n - 2);
}

$n = 32;
$warm = fib($n);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = fib($n);
$d = hrtime(true) - $t;
printf("fibrec\t%d\t%d\n", $d + $guard, $acc);
