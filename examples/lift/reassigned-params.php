<?php

/**
 * A PHP parameter is an ordinary local — the body may reassign it. phorj parameters are immutable
 * (Kotlin's `val`; Swift removed `var` parameters, SE-0003), so the lift copies a parameter the body
 * WRITES into a mutable local on the first line and the body uses the copy (DEC-548). The signature
 * is untouched, so the named argument `cap(bp: 12)` still binds. Lifted with
 * `phg lift reassigned-params.php`.
 */

/** Clamp: the classic reassigned parameter. */
function cap(int $bp): int
{
    if ($bp > 9) {
        $bp = 9;
    }
    return $bp;
}

/**
 * Every assignment form counts — `[]=`, `++`, `*=` — and a read-only parameter keeps its name.
 *
 * @param list<int> $xs
 */
function grow(array $xs, int $n, int $scale): int
{
    $xs[] = $n;
    $n++;
    $n *= $scale;
    $total = 0;
    foreach ($xs as $x) {
        $total += $x;
    }
    return $total + $n;
}

/** A closure capturing a reassigned parameter captures the copy. */
function shifted(int $k): int
{
    $k = $k * 3;
    $add = fn (int $m): int => $m + $k;
    return $add(1);
}

function main(): void
{
    echo cap(bp: 12), " ", cap(4), "\n";
    echo grow([1, 2], 3, 2), "\n";
    echo shifted(2), "\n";
}
