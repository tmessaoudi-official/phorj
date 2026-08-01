<?php
// Idiomatic PHP counterpart of fsforeachline.phg (hand-authored): the same `fgets` streaming loop as
// `fslines.php`, deliberately UNCHANGED between the two. PHP has one idiomatic way to read a file line
// by line, so the PHP baseline is identical and the only thing that varies across the pair is which
// phorj API is being measured against it.
//
// This is also what phorj's transpile leg emits for `forEachLine` (Invariant-14 ladder case 1), with
// the fold as a closure rather than inline — the shape `__phorj_fs_for_each_line` produces.
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
        // `fgets` KEEPS the terminator; phorj strips it, so trim to compare like for like (and `\r`
        // too, for the same reason phorj strips it).
        $acc += mb_strlen(rtrim($line, "\r\n"));
    }
    fclose($h);
    return $acc;
}
$path = sys_get_temp_dir() . '/phorj-bench-fsforeachline.txt';
fixture($path, 40000);
$warm = bench($path); $guard = $warm - $warm;
$t = hrtime(true); $acc = bench($path); $d = hrtime(true) - $t;
printf("fsforeachline\t%d\t%d\n", $d + $guard, $acc);
unlink($path);
