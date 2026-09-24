<?php
// PHP lets a method, a property and a variable be named `match`, `type`, `new` or `class`; phorj
// reserves those words (DEC-531). Members keep their name — phorj accepts a reserved word where
// nothing else can stand: after `function` in a class, as a field or promoted parameter, after
// `.`/`::`, as a `with` field or a named argument. Locals and plain parameters are private to their
// function, so `phg lift` renames them with a `_`: `$type` -> `type_`. Run `phg lift
// keyword-names.php` to see the draft (keyword-names.phg).

final class Rule {
    public function __construct(public string $type, public int $floor) {}

    public static function match(int $rent, int $floor): bool {
        return $rent >= $floor;
    }

    public function open(int $new): string {
        $verdict = 'no';
        if (self::match($new, $this->floor)) {
            $verdict = 'yes';
        }
        return $this->type . ':' . $verdict;
    }
}

function describe(string $type, int $class): string {
    $new = "$type/$class";
    foreach ([1, 2] as $match) {
        $new = $new . '+' . $match;
    }
    return $new;
}

$r = new Rule(type: 'lease', floor: 800);
echo $r->open(900), "\n";
echo $r->open(700), "\n";
echo describe(class: 3, type: 'T'), "\n";
