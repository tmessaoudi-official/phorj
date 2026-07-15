<?php
// Idiomatic PHP counterpart of dbwork.phg (hand-authored): PDO sqlite::memory:, per-row
// prepare+bind+execute (the same naive-handler shape), aggregate read back. Same embedded SQLite —
// the measured delta is the language-side binding/dispatch overhead.
function bench(PDO $db, int $iters): int {
    $acc = 0;
    for ($i = 0; $i < $iters; $i++) {
        $db->exec("DELETE FROM t");
        for ($r = 0; $r < 50; $r++) {
            $stmt = $db->prepare("INSERT INTO t(k, v) VALUES(?, ?)");
            $stmt->bindValue(1, $r, PDO::PARAM_INT);
            $stmt->bindValue(2, $r * 3 + $i % 7, PDO::PARAM_INT);
            $stmt->execute();
        }
        $q = $db->query("SELECT SUM(v) FROM t");
        $total = (int) $q->fetchColumn();
        $acc = $acc + $total;
    }
    return $acc;
}
$db = new PDO("sqlite::memory:");
$db->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
$db->exec("CREATE TABLE t(k INTEGER, v INTEGER)");
$iters = 400;
$warm = bench($db, $iters);
$guard = $warm - $warm;
$t = hrtime(true);
$acc = bench($db, $iters);
$d = hrtime(true) - $t;
printf("dbwork\t%d\t%d\n", $d + $guard, $acc);
