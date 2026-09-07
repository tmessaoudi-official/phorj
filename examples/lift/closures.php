<?php

/**
 * Block-bodied closures — `function (…) [use (…)] : T { … }`.
 *
 * phorj HAS this shape (`function(int x): int { … }`); what it has ruled OUT is by-REFERENCE
 * capture, and `phg lift` says which is which. Lifted with `phg lift closures.php`.
 */
function main(): void
{
    // A statement body with a declared return type.
    $classify = function (int $x): string {
        if ($x < 0) {
            return "negative";
        }
        if ($x === 0) {
            return "zero";
        }
        return "positive";
    };
    echo $classify(-2), "\n";
    echo $classify(0), "\n";
    echo $classify(7), "\n";

    // `static` only stops PHP binding `$this`; a phorj lambda never binds one, so it is dropped.
    $double = static function (int $x): int { return $x * 2; };
    echo $double(21), "\n";

    // A BY-VALUE `use (…)` list is what a phorj lambda does anyway, so the list is dropped and the
    // captured name stays in scope. (A by-REFERENCE `use (&$x)` is refused by name — DEC-506.)
    $offset = 100;
    $shift = function (int $x) use ($offset): int { return $x + $offset; };
    echo $shift(5), "\n";
}
