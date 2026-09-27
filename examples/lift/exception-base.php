<?php
// A user exception class extending a PHP builtin (row 4j). Lift it:
// `phg lift examples/lift/exception-base.php`.

// `extends \RuntimeException` becomes `extends RuntimeError`, the DEC-421 counterpart: an `open`
// class whose `message` constructor the subclass inherits, so `new self(...)` still works.
final class QuotaExceededException extends \RuntimeException
{
    public static function over(int $limit): self
    {
        return new self('over the quota of ' . $limit);
    }
}

try {
    throw QuotaExceededException::over(3);
} catch (QuotaExceededException $e) {
    echo 'caught by its own type';
}
try {
    throw new QuotaExceededException('again');
} catch (\RuntimeException $e) {
    echo ' | caught by its base', "\n";
}
