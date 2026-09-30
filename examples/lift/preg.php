<?php
// `preg_match` compared with 1 or 0 is an EXISTENCE test, and `phg lift` maps it onto Core.Regex
// (scout row 4l-a, DEC-540). The pattern is read at lift time — a literal, or a string constant of
// the class being lifted — and translated: PHP's `$` also matches before a final newline, so it
// becomes `\n?\z`; without the `u` modifier `\d` is ASCII, so it becomes `[0-9]`. A pattern whose
// meaning would change (a `.` counting bytes, `i` without `u`, `\b` without `u`, the `m` modifier)
// is refused by name. A third argument (`$m`, the captures array) lifts onto a typed
// `Regex.first` hoisted in front of the statement (row 4l-b2, DEC-554): `$m[k]` reads `m.at(k)`.
// `preg_match_all` lifts onto a hoisted `Regex.all` (row 4l-b3): the count is `m.length()` and `$m[k]` is
// the COLUMN of group k across every match; with PREG_OFFSET_CAPTURE each cell is (text, BYTE offset).
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

    /** @return list<string> */
    public static function digits(string $s): array {
        if (preg_match_all('/[A-Z]{3}-(\d{4})/u', $s, $m) < 1) {
            return [];
        }
        return $m[1];
    }

    public static function firstOffset(string $s): int {
        if (preg_match_all('/rdc/u', $s, $m, PREG_OFFSET_CAPTURE) < 1) {
            return -1;
        }
        foreach ($m[0] as [$text, $at]) {
            return $at;
        }
        return -1;
    }
}

foreach (['ABC-1234', "ABC-1234\n", 'ABC-12345', 'abc-1234'] as $s) {
    echo Sku::valid($s) ? 'valid ' : 'invalid ';
}
echo "\n";
echo Sku::mentionsRdc('au rdc') ? 'rdc' : 'no', ' ', Sku::mentionsRdc('ardcx') ? 'rdc' : 'no', "\n";
echo Sku::number('ABC-1234'), ' ', Sku::number('abc'), "\n";
echo implode(',', Sku::digits('ABC-1234 x DEF-5678')), ' ', count(Sku::digits('none')), "\n";
echo Sku::firstOffset('é rdc'), ' ', Sku::firstOffset('none'), "\n";
