<?php
// PHP error handling, of the shape `phg lift` meets in real code: a `throw` of a builtin exception,
// a typed `catch`, and a rethrow that wraps one kind of failure in another.
//
// Every exception class named here is a PHP BUILTIN, so `phg lift` maps each onto phorj's standard
// taxonomy (`Core.ErrorModule`, DEC-421) and emits the imports the draft needs:
//
//     \InvalidArgumentException -> InvalidValueError
//     \DivisionByZeroError      -> MathError
//     \RuntimeException         -> RuntimeError
//
// A class phorj has NO counterpart for (a framework or app exception) is left named as it is, with a
// `// CANNOT LIFT:` note at the top of the draft — never coerced into the nearest phorj type.
//
// Run `phg lift errors.php` to see the draft; the committed `errors.phg` is that draft with the one
// thing lift cannot infer filled in by hand (see the README — PHP has no checked exceptions, so it
// carries nothing a `throws` clause could be derived from).

function half(int $n): int {
    if ($n < 0) {
        throw new \InvalidArgumentException("negative: $n");
    }
    if ($n % 2 !== 0) {
        throw new \DivisionByZeroError("odd: $n");
    }
    return intdiv($n, 2);
}

function halfOrZero(int $n): int {
    try {
        return half($n);
    } catch (\DivisionByZeroError $e) {
        throw new \RuntimeException("not halvable");
    }
}

function main(): void {
    foreach ([8, 4] as $n) {
        echo "half($n) = ", halfOrZero($n), "\n";
    }
}
