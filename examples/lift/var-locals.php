<?php

/**
 * `@var`-declared LOCALS keep their shape (DEC-515, scout row 5d). The declared type stays on the
 * local's declaration, its reads become field reads, and a keyed literal written to it becomes a
 * tuple. A local with no `@var` stays `var` — nothing is inferred. Lifted with `phg lift var-locals.php`.
 */

/** @return list<array{id: int, name: string}> */
function load(): array
{
    return [['id' => 3, 'name' => 'c'], ['id' => 7, 'name' => 'g'], ['id' => 5, 'name' => 'e']];
}

/** A nullable local replaced by a keyed literal under a `=== null ||` guard. */
/** @param list<array{id: int, name: string}> $rows */
function best(array $rows): string
{
    /** @var array{id: int, name: string}|null $seen */
    $seen = null;
    foreach ($rows as $r) {
        if ($seen === null || $r['id'] > $seen['id']) {
            $seen = ['id' => $r['id'], 'name' => $r['name']];
        }
    }
    if ($seen !== null) {
        return $seen['name'];
    }
    return "-";
}

/** A call-initialized list read by element field, and an empty list filled from a binder. */
function total(): int
{
    /** @var list<array{id: int, name: string}> $rows */
    $rows = load();
    /** @var list<array{id: int, name: string}> $kept */
    $kept = [];
    foreach ($rows as $r) {
        if ($r['id'] > 4) {
            $kept[] = $r;
        }
    }
    $sum = 0;
    foreach ($kept as $k) {
        $sum += $k['id'];
    }
    return $sum + $rows[0]['id'];
}

function main(): void
{
    /** @var list<array{id: int, name: string}> $none */
    $none = [];
    echo best(load()), " ", total(), " ", best($none), "\n";
}
