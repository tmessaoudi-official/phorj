<?php

/**
 * PHP spread `...` over lists (DEC-538, scout row 5u) lifts to `List.concat` / `List.flatten` —
 * phorj has no spread syntax and needs none. `array_merge(...array_values($m))` flattens a map's
 * values in insertion order, and `array_unshift($v, ...$xs)` prepends. A spread of a map, or into
 * any other call, is refused by name. Lifted with `phg lift spread.php`.
 */

/**
 * @param list<int> $low
 * @param list<int> $high
 * @return list<int>
 */
function around(array $low, array $high, int $mid): array
{
    return [...$low, $mid, ...$high];
}

/** @return list<string> */
function tiers(): array
{
    $byTier = [2 => ['label'], 1 => ['field', 'form'], 3 => ['source']];
    $byTier[1] = ['field'];
    return array_merge(...array_values($byTier));
}

/**
 * @param list<string> $extra
 * @return list<string>
 */
function reasons(array $extra): array
{
    /** @var list<string> $out */
    $out = ['checked'];
    array_unshift($out, ...$extra);
    return $out;
}

/** @param list<string> $xs */
function joined(array $xs): string
{
    $s = "";
    foreach ($xs as $x) {
        $s = $s . $x . " ";
    }
    return $s;
}

function main(): void
{
    $nums = around([1, 2], [8, 9], 5);
    echo count($nums), " ", $nums[2], "\n";
    echo joined(tiers()), "\n";
    echo joined(reasons(['doubt', 'conflict'])), "\n";
}
