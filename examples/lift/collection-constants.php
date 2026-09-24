<?php
// PHP 8.3 typed `const array` constants (DEC-533) lift to phorj collection constants that keep their
// names. The element types are read from the literal: one type per level, to any depth, and one
// scalar type plus `null` becomes `T?`. A negative number is a literal constant.
final class Text {
    private const array FOLD = ['à' => 'a', 'é' => 'e'];
    public const array BANDS = ['A' => [10, 20], 'B' => [30]];
    private const array NAMES = ['inli' => 'inli', 'x' => null];
    private const int FLOOR = -5;
    public static function fold(string $c): string { return self::FOLD[$c]; }
    public static function band(string $k, int $i): int { return self::BANDS[$k][$i]; }
    public static function name(string $k): string { return self::NAMES[$k] ?? 'none'; }
    public static function floor(): int { return self::FLOOR * 2; }
}
echo Text::fold('é'), "|", Text::band('A', 1), "|", Text::name('x'), "|", Text::name('inli'), "|", Text::floor(), "\n";
