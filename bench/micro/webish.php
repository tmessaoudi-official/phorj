<?php
// Idiomatic PHP counterpart of webish.phg (hand-authored — NOT transpiled). Same shape: route a path
// via a map, render a templated string, fold length + handler into a checksum, in a loop; warm call
// then a timed call. The CPU-realistic web-response slice — routing + templating, no I/O — which is
// where a JIT can actually help a real request (the DB/network can't be JITted away).
function bench(int $iters): int {
    $routes = ["/" => 1, "/users" => 2, "/posts" => 3, "/about" => 4];
    $paths = ["/", "/users", "/posts", "/about"];
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $path = $paths[$i % 4];
        $handler = $routes[$path];
        $body = "handler=$handler path=$path";
        $acc = $acc + strlen($body) + $handler;
    }
    return $acc;
}

$iters = 2000000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("webish\t%d\t%d\n", intdiv($d, $iters) + $guard, $acc);
