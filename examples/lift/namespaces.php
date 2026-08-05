<?php
// LIFT-NS: what `phg lift` does with PHP's file-level declarations.
//
// `namespace` and `use` were outside the lifter's Tier-1 subset until 2026-08-04, so a file shaped
// like this one could not be lifted AT ALL — it failed at the PARSER, before anything else.
//
// BOTH mandatory PSR-12 prologue lines now lift: `declare(strict_types=1);` (DEC-401, which the
// TRANSPILER also emits into every generated file) and `namespace`.
declare(strict_types=1);

namespace app\cli_tools;

// A `use` whose local name is never referenced. PHP allows this freely (editors add them, code moves
// on), but phorj's `E-UNUSED-IMPORT` is a HARD error — so the lifter DROPS an unreferenced import
// rather than emitting a draft that fails the very check it should pass. Dropping is lossless: a
// `use` only creates a local alias, so an unused one carries no behaviour.
use App\Support\Money as Cash;

function label(string $who): string
{
    return "hi " . $who;
}

echo label("phorj") . "\n";
