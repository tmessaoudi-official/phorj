<?php
// Callable SIGNATURES (row 4e): a `callable` or `\Closure` typed by a PHPStan/Psalm docblock
// signature lifts to a phorj function type. Lift it: `phg lift examples/lift/callables.php`.

final class Profile { public function __construct(public int $bp) {} }

final class Registry {
    /** @param callable(string): ?Profile $profileFor resolves a key to its profile */
    public static function effective(string $key, callable $profileFor): int {
        $p = $profileFor($key) ?? new Profile(-1);
        return $p->bp;
    }
}

final class Classifier {
    /** @var \Closure(string): ?Profile */
    private \Closure $profileFor;

    /** @param (callable(string): ?Profile)|null $profileFor */
    public function __construct(?callable $profileFor = null) {
        $this->profileFor = $profileFor === null
            ? static fn (string $key): ?Profile => null
            : \Closure::fromCallable($profileFor);
    }

    public function classify(string $key): int {
        return Registry::effective($key, $this->profileFor);
    }
}

// A first-class callable of a NAMED function (row 4f) is a phorj function reference: `lens` lifts to
// `xs.map(len)`. A builtin's `strlen(...)` is still refused (row 4g).
function len(string $s): int { return strlen($s); }

/**
 * @param list<string> $xs
 * @return list<int>
 */
function lens(array $xs): array { return array_map(len(...), $xs); }

$none = new Classifier();
$some = new Classifier(function (string $k): ?Profile {
    if ($k === 'a') {
        return new Profile(7);
    }
    return null;
});
echo $none->classify('a'); echo "|"; echo $some->classify('a'); echo "|"; echo $some->classify('b'); echo "\n";
$ns = lens(['a', 'bcd']);
echo $ns[0]; echo "|"; echo $ns[1]; echo "\n";
