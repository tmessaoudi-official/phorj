<?php
// A small, typed PHP program — the kind `phg lift` handles 1:1 (Tier-1).
// Run `phg lift sample.php` to see the Phorge draft (committed alongside as sample.phg).

function greet(string $name): string {
    return "Hello, " . $name;
}

class Counter {
    public function __construct(private int $start) {}

    public function next(): int {
        return $this->start + 1;
    }
}

echo greet("Phorge");
