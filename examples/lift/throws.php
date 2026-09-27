<?php
// PHP exceptions are unchecked; phorj's are checked (row 4j2, DEC-547). Lift it:
// `phg lift examples/lift/throws.php`.

final class BadInputException extends \RuntimeException
{
    public static function at(string $where): self
    {
        return new self('bad input at ' . $where);
    }
}

// Throws through a static factory: the lift reads the type off the factory's return and declares
// `throws BadInputException`.
function fold(string $s): string
{
    if ($s === '') {
        throw BadInputException::at('fold');
    }
    return strtolower($s);
}

// Reaches `fold` through a call — propagation: `throws` is declared here too, and the call is
// spelled `fold(s)?`. The `strlen` receiver keeps its parentheses: `(fold(s)?).length()`.
function measure(string $s): int
{
    return strlen(fold($s));
}

final class Report
{
    // Through `$this->`, so the method declares it as well.
    public function line(string $s): string
    {
        return $this->label($s) . '=' . measure($s);
    }

    private function label(string $s): string
    {
        return fold($s);
    }

    // A `catch` of the MAPPED base covers the subclass: nothing propagates out of here.
    public function safe(string $s): string
    {
        try {
            return $this->line($s);
        } catch (\RuntimeException $e) {
            return 'skipped';
        }
    }
}

// `main` cannot declare `throws`, so a call that can throw must sit in a `try` here: an unguarded
// one stays a loud `E-CALL-UNHANDLED` in the draft rather than being guessed at.
$r = new Report();
try {
    echo $r->line('ABC'), ' | ', $r->safe(''), "\n";
    echo $r->line('');
} catch (BadInputException $e) {
    echo 'caught at the top', "\n";
}
