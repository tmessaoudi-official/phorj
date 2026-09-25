<?php
// PHP array `+` is UNION: the left operand's entries, then the right's new keys; the LEFT value wins
// on a shared key. `phg lift` maps it to `Map.union` (DEC-534) where it knows both operands are maps —
// a keyed array literal, or a class constant whose type lifts to a Map, as here. A PHP `array`
// parameter could be a list, so `$a + $b` on parameters stays `+` and `phg check` names it.
// Run `phg lift map-union.php` to see the draft (map-union.phg).

final class Accents {
    private const array LOWER = ['à' => 'a', 'é' => 'e'];
    private const array UPPER = ['À' => 'A', 'é' => 'E'];

    public static function show(): string {
        $out = "";
        foreach (self::LOWER + self::UPPER as $k => $v) {
            $out .= $k . "->" . $v . " ";
        }
        return $out;
    }
}

echo Accents::show(), "\n";
