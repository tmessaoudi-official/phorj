<?php
// DEC-397: PHP has FUNCTION scope, phorj has BLOCK scope. A variable first assigned inside a block
// used to be DECLARED inside it, so every later use was `E-ASSIGN-UNKNOWN` / `E-UNKNOWN-IDENT`.
//
// The hoist is deliberately restricted to blocks that ALWAYS execute. The ruled shape was "hoist any
// literal first assignment", but that is unsound when the block is conditional:
//
//     function g(bool $c): int { if ($c) { $b = 5; } return $b + 0; }
//
// `g(false)` prints 0 in PHP — reading an unassigned $b gives null, and null + 0 is 0. A hoisted
// `mutable var b = 5;` would print 5: the draft would COMPILE and be WRONG. Those cases get a
// `// CANNOT LIFT:` note naming the variable instead of a silent wrong answer.
function pick(): string
{
    // `if (true)` always runs, so the declaration can move out of it soundly.
    if (true) {
        $chosen = "first";
    }
    $chosen = "second";
    return $chosen;
}

echo pick() . "\n";
