<?php
// PHP decodes string escapes differently in each quote style, and `phg lift` decodes them exactly as
// PHP does (scout row 5n). Double-quoted: "\u{2019}" is one character, "\x41" and "\101" are `A`,
// "\$" is a dollar. Single-quoted: only \\ and \' decode, so '\n' stays a backslash and an `n`.
// "\{" is not an escape — the backslash stays and `$first` still interpolates after it.
// Run `phg lift escapes.php` to see the draft (escapes.phg).

function apostrophes(): string {
    return "\u{2018}\u{2019}";
}

function possessive(string $first): string {
    return "{$first}\u{2019}s \x41\101 \{$first}";
}

echo apostrophes(), "\n";
echo possessive("Ana"), "\n";
echo 'single: \n\t\'', "\n";
echo "tab:[\t] dollar:\$x", "\n";
