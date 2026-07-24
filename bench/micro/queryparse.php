<?php
// Idiomatic PHP counterpart of queryparse.phg (hand-authored): the SAME per-request job the rich
// Request does eagerly — split head/body, request line, lowercased header map, cookie bag, and a
// hand-rolled first-wins query parse (parse_str mangles keys and is last-wins, so a careful PHP
// app rolls its own exactly like this).
function parse_query_pairs(string $s): array {
    $out = [];
    foreach (explode('&', $s) as $seg) {
        if ($seg === '') { continue; }
        $eq = strpos($seg, '=');
        $k = urldecode($eq === false ? $seg : substr($seg, 0, $eq));
        $v = urldecode($eq === false ? '' : substr($seg, $eq + 1));
        if (!array_key_exists($k, $out)) { $out[$k] = []; }
        $out[$k][] = $v;
    }
    return $out;
}
function parse_request(string $raw): ?array {
    $sep = strpos($raw, "\r\n\r\n");
    if ($sep === false) { return null; }
    $head = substr($raw, 0, $sep);
    $body = substr($raw, $sep + 4);
    $lines = explode("\r\n", $head);
    $rl = explode(' ', $lines[0]);
    if (count($rl) < 2) { return null; }
    $headers = [];
    for ($i = 1; $i < count($lines); $i++) {
        $ci = strpos($lines[$i], ':');
        if ($ci === false) { continue; }
        $k = strtolower(trim(substr($lines[$i], 0, $ci)));
        $headers[$k][] = trim(substr($lines[$i], $ci + 1));
    }
    $cookies = [];
    foreach ($headers['cookie'] ?? [] as $line) {
        foreach (explode(';', $line) as $piece) {
            $p = trim($piece);
            if ($p === '') { continue; }
            $eq = strpos($p, '=');
            $k = $eq === false ? $p : substr($p, 0, $eq);
            $cookies[$k][] = $eq === false ? '' : substr($p, $eq + 1);
        }
    }
    $target = $rl[1];
    $qpos = strpos($target, '?');
    $path = rawurldecode($qpos === false ? $target : substr($target, 0, $qpos));
    $query = parse_query_pairs($qpos === false ? '' : substr($target, $qpos + 1));
    return ['method' => $rl[0], 'path' => $path, 'query' => $query,
            'headers' => $headers, 'cookies' => $cookies, 'body' => $body];
}
function bench(int $iters): int {
    $raw = "GET /s?page=2&tag=a+b&tag=c%21&q=hello%20world&eq=1=2 HTTP/1.1\r\nHost: localhost\r\n\r\n";
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $req = parse_request($raw);
        $q = $req['query'];
        $acc += strlen($q['q'][0] ?? '') + count($q['tag'] ?? []) + strlen($q['eq'][0] ?? '');
    }
    return $acc;
}
$iters = 200000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("queryparse\t%d\t%d\n", $d + $guard, $acc);
