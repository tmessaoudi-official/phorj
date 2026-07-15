<?php
// Idiomatic PHP counterpart of jsonround.phg (hand-authored): json_decode → field reads → build a
// response array → json_encode. This is the HARDEST fair baseline — both codecs are C natives.
function bench(int $iters): int {
    $doc = '{"id": 7, "qty": 3, "name": "widget", "tags": ["a", "b"], "price": 9.5}';
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $j = json_decode($doc, true);
        if ($j !== null) {
            $id = is_int($j['id'] ?? null) ? $j['id'] : 0;
            $qty = is_int($j['qty'] ?? null) ? $j['qty'] : 0;
            $body = json_encode(["ok" => true, "id" => $id, "total" => $id * $qty]);
            $acc = $acc + strlen($body) + $id + $qty;
        }
    }
    return $acc;
}
$iters = 200000;
$warm = bench($iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($iters);
$d = hrtime(true) - $t;
printf("jsonround\t%d\t%d\n", $d + $guard, $acc);
