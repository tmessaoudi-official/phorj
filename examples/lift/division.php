<?php
// PHP `/` is FLOAT division: `7 / 2` is 3.5. Phorj's `/` on two ints is integer division, so
// `phg lift` makes every operand float (DEC-523, built as DEC-529): an int literal becomes a
// float literal (`2` -> `2.0`, `-2` -> `-2.0`), an operand that is already float is left alone,
// and anything else gets `as float`. Run `phg lift division.php` to see the draft (division.phg).

class Classification {
    public function __construct(private int $confidenceBp) {}

    public function confidence(): float {
        return $this->confidenceBp / 100;
    }
}

function ratio(int $a, int $b): float {
    return $a / $b;
}

function halvedTwice(int $n): float {
    $x = (float) $n;
    $x /= 2;
    return $x / -2;
}

$c = new Classification(8750);
echo $c->confidence(), "\n";
echo ratio(7, 2), " ", ratio(-7, 2), " ", ratio(6, 3), " ", ratio(1, 4), "\n";
echo halvedTwice(10), "\n";
