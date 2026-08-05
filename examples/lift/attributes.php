<?php
// LIFT-ATTR: PHP 8 attributes survive the lift.
//
// Before this slice a bare `#` was a line COMMENT to the lift lexer, so `#[Audited("billing")]` was
// silently eaten — the file lifted and quietly meant less than it did. `#[` is now its own token, and
// the attribute NAME is resolved the way PHP resolves a class name: through `use`, then the current
// namespace. See README.md for what happens to the positions phorj has no target for.
declare(strict_types=1);

namespace App\Meta;

use Attribute;

#[Attribute]
class Audited
{
    public function __construct(public string $reason) {}
}

#[Audited("billing")]
class Invoice
{
    public function __construct(public string $ref) {}

    public function label(): string
    {
        return "invoice " . $this->ref;
    }
}

#[Audited("entry")]
function main(): void
{
    $i = new Invoice("A-1");
    echo $i->label(), "\n";
}
