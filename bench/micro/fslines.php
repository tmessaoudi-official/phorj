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
        // `fgets` KEEPS the terminator; phorj's iterator strips it, so trim to compare like for like
        // (and `\r` too, for the same reason phorj strips it).
        $acc += mb_strlen(rtrim($line, "\r\n"));
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
