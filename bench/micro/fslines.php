<?php
// Idiomatic PHP counterpart of fslines.phg (hand-authored): the `fgets` streaming loop, which is also
// exactly what phorj's transpile leg emits for `FileSystem.lines` (Invariant-14 ladder case 1).
function fixture(string $path, int $lines): void {
    $body = '';
    for ($i = 0; $i < $lines; $i++) {
        $body .= "row $i the quick brown fox jumps over the lazy dog\n";
    }
    file_put_contents($path, $body);
}
function bench(string $path): int {
    $acc = 0;
    $h = fopen($path, 'rb');
    while (($line = fgets($h)) !== false) {
        // `strlen`, NOT `mb_strlen`: phorj's `String.length` is documented BYTE length, so `strlen` is
        // the faithful twin — and it is FASTER, which raises the bar on phorj rather than lowering it.
        // The bench used `mb_strlen` until 2026-08-01 and was therefore comparing against a handicapped
        // PHP (measured: 4.31 ms vs 3.55 ms median, JIT on). Any loss measured before that date was
        // understated by ~20%. `rtrim` stays: `fgets` keeps the terminator and phorj strips it.
        $acc += strlen(rtrim($line, "\r\n"));
    }
    fclose($h);
    return $acc;
}
$path = sys_get_temp_dir() . '/phorj-bench-fslines.txt';
fixture($path, 40000);
$warm = bench($path); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($path); $d = hrtime(true) - $t;
printf("fslines\t%d\t%d\n", $d + $guard, $acc);
unlink($path);
