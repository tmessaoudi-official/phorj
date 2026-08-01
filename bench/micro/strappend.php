<?php
// Idiomatic PHP counterpart of strappend.phg (hand-authored): `.=`, which is PHP's one idiomatic way to
// grow a string and is amortized O(1) — the interpreter reallocs the buffer in place when the zend_string
// refcount is 1. That is exactly the optimization phorj's JIT performs and its VM does not, which is what
// this pair measures. `strlen` (not `mb_strlen`) is the faithful twin of phorj's byte-length
// `String.length` — see the note in fslines.php for why that distinction cost this project real numbers.
function bench(int $lines): int {
    $body = '';
    for ($i = 0; $i < $lines; $i++) {
        $body .= "row $i the quick brown fox jumps over the lazy dog\n";
    }
    return strlen($body);
}
$lines = 20000;
$warm = bench($lines); $guard = $warm - $warm;
$t = hrtime(true); $n = bench($lines); $d = hrtime(true) - $t;
printf("strappend\t%d\t%d\n", $d + $guard, $n);
