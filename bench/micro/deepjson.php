<?php
// Idiomatic PHP counterpart of deepjson.phg (hand-authored): json_decode a paginated user-list
// response, read only status + the first record's name/email. PHP's C json_decode ALWAYS materializes
// the whole nested array (every record + the meta block), so this is the deep/wide workload where a
// lazy Phorj Json (which skips the ~unread subtrees) has room to win. Same doc, same read pattern.
function bench(string $doc, int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $j = json_decode($doc, true);
        if ($j !== null) {
            $status = is_string($j['status'] ?? null) ? $j['status'] : "";
            $rec0 = (isset($j['data']) && is_array($j['data']) && count($j['data']) > 0) ? $j['data'][0] : null;
            $name = (is_array($rec0) && is_string($rec0['name'] ?? null)) ? $rec0['name'] : "";
            $email = (is_array($rec0) && is_string($rec0['email'] ?? null)) ? $rec0['email'] : "";
            $acc = $acc + strlen($status) + strlen($name) + strlen($email);
        }
    }
    return $acc;
}
$doc = '{"status": "ok", "page": 1, "total": 12, "meta": {"requestId": "abc-123", "tookMs": 4, "cached": false, "region": "eu-west"}, "data": ['
    . '{"id": 1, "name": "Ada", "email": "ada@x.io", "active": true, "age": 36, "city": "London"},'
    . '{"id": 2, "name": "Bob", "email": "bob@x.io", "active": false, "age": 41, "city": "Paris"},'
    . '{"id": 3, "name": "Cy", "email": "cy@x.io", "active": true, "age": 29, "city": "Berlin"},'
    . '{"id": 4, "name": "Di", "email": "di@x.io", "active": true, "age": 52, "city": "Madrid"},'
    . '{"id": 5, "name": "Ed", "email": "ed@x.io", "active": false, "age": 33, "city": "Rome"},'
    . '{"id": 6, "name": "Fi", "email": "fi@x.io", "active": true, "age": 47, "city": "Oslo"},'
    . '{"id": 7, "name": "Gu", "email": "gu@x.io", "active": true, "age": 38, "city": "Lyon"},'
    . '{"id": 8, "name": "Ha", "email": "ha@x.io", "active": false, "age": 25, "city": "Kyiv"},'
    . '{"id": 9, "name": "Ir", "email": "ir@x.io", "active": true, "age": 61, "city": "Porto"},'
    . '{"id": 10, "name": "Jo", "email": "jo@x.io", "active": true, "age": 44, "city": "Delft"},'
    . '{"id": 11, "name": "Ka", "email": "ka@x.io", "active": false, "age": 30, "city": "Gent"},'
    . '{"id": 12, "name": "Li", "email": "li@x.io", "active": true, "age": 55, "city": "Bonn"}'
    . ']}';
$iters = 100000;
$warm = bench($doc, $iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($doc, $iters);
$d = hrtime(true) - $t;
printf("deepjson\t%d\t%d\n", $d + $guard, $acc);
