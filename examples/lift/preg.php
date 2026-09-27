<?php
// `preg_match` compared with 1 or 0 is an EXISTENCE test, and `phg lift` maps it onto Core.Regex
// (scout row 4l-a, DEC-540). The pattern is read at lift time — a literal, or a string constant of
// the class being lifted — and translated: PHP's `$` also matches before a final newline, so it
// becomes `\n?\z`; without the `u` modifier `\d` is ASCII, so it becomes `[0-9]`. A pattern whose
// meaning would change (a `.` counting bytes, `i` without `u`, `\b` without `u`, the `m` modifier)
// is refused by name. A third argument (`$m`, the captures array) lifts onto a typed
// `Regex.first` hoisted in front of the statement (row 4l-b2, DEC-554): `$m[k]` reads `m.at(k)`.
// Run `phg lift preg.php` to see the draft (preg.phg).

final class Sku {
    private const string CODE = '/^[A-Z]{3}-\d{4}$/';
    private const string CODE_PARTS = '/^(?<prefix>[A-Z]{3})-(\d{4})$/';

    public static function valid(string $s): bool {
        return 1 === preg_match(self::CODE, $s);
    }

    public static function mentionsRdc(string $s): bool {
        return preg_match('/(?<![a-z])rdc(?![a-z])/u', $s) === 1;
    }

    public static function number(string $s): string {
        if (1 !== preg_match(self::CODE_PARTS, $s, $m)) {
            return 'none';
        }
        return $m['prefix'] . '/' . $m[2];
    }
}

foreach (['ABC-1234', "ABC-1234\n", 'ABC-12345', 'abc-1234'] as $s) {
    echo Sku::valid($s) ? 'valid ' : 'invalid ';
}
echo "\n";
echo Sku::mentionsRdc('au rdc') ? 'rdc' : 'no', ' ', Sku::mentionsRdc('ardcx') ? 'rdc' : 'no', "\n";
echo Sku::number('ABC-1234'), ' ', Sku::number('abc'), "\n";
