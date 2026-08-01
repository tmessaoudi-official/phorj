# `phg lift` — PHP → Phorj

`lift` is the **inverse of `transpile`**: it reads PHP and emits a Phorj **draft**.

Where `transpile` is *total and byte-identity-verified* (every Phorj program has one correct PHP
translation), `lift` is **best-effort and review-required** — PHP is larger and dynamic, Phorj is
smaller and typed, so the map is partial by nature. The output is a scaffold a human checks, prefixed
`// lifted (verify)`. Anything outside the supported subset is refused with a clear `lift …` error
rather than guessed at — lift **never** silently produces wrong Phorj.

## Try it

```console
$ phg lift sample.php
```

Input — [`sample.php`](sample.php), ordinary typed PHP (note the double-quoted **interpolation**):

```php
function greet(string $name): string {
    return "Hello, $name!";
}

class Counter {
    public function __construct(public int $start) {}
    public function next(): int { return $this->start + 1; }
}

$c = new Counter(41);
echo greet("Phorj");
echo " Counter starts at $c->start, next is {$c->next()}.";
```

Output — [`sample.phg`](sample.phg), idiomatic Phorj (PHP is the *floor*, not the ceiling — lift
emits clean Phorj, it doesn't mirror PHP's quirks). PHP interpolation maps straight to Phorj holes:
`"$name"` → `"{name}"`, `"$c->start"` → `"{c.start}"`, `"{$c->next()}"` → `"{c.next()}"`:

```phorj
package Main;
import Core.Output;

function greet(string name) -> string {
    return "Hello, {name}!";
}

open class Counter {
    constructor(public mutable int start) {}
    public open function next() -> int {
        return this.start + 1;
    }
}

function main() -> void {
    mutable var c = new Counter(41);
    Output.print(greet("Phorj"));
    Output.print(" Counter starts at {c.start}, next is {c.next()}.");
}
```

Both print `Hello, Phorj! Counter starts at 41, next is 42.` The lifted `sample.phg` is part of the
example suite, so it is byte-identity-gated on both backends **and** real PHP like every other
example.

## What lift does (idiomatic, not a mirror)

| PHP | Phorj |
|---|---|
| top-level statements | a synthesized `function main()` (the runnable entry) |
| the whole file | `package Main;` (PHP has no packages) |
| `$x = e` | `mutable var x = e;` (PHP locals are freely reassignable) |
| `.` string concat / `===` / `!==` | `+` / `==` / `!=` (Phorj is typed) |
| `echo e;` | `Output.print(e);` (+ an automatic `import Core.Output;`) |
| `__construct` + promoted params | a `constructor` with promoted (mutable) fields |
| a non-`final` PHP class | an `open` class (Phorj is final-by-default) |
| `[a, b]` / `[k => v]` | a `List` / a `Map` |
| ternary `c ? a : b` / `match` | an expression `if` / a Phorj `match` |
| `"$name"` / `"$o->prop"` / `"{$o->m()}"` interpolation | Phorj `"{name}"` / `"{o.prop}"` / `"{o.m()}"` holes |
| `foreach ($xs as $x)` (keyless) | Phorj `foreach (xs as x)` — element type inferred (A-6) |

## Exceptions — PHP's builtins map onto `Core.ErrorModule` (DEC-421)

A second sample, [`errors.php`](errors.php), covers the error path: a `throw`, a typed `catch`, and a
rethrow that wraps one failure kind in another.

```console
$ phg lift errors.php
```

Phorj ships a **standard error taxonomy** — six types in `Core.ErrorModule` — and lift maps PHP's
builtin exception classes onto it. Before that existed, a lifted `catch (\RuntimeException $e)`
produced valid phorj syntax that then failed `phg check` with `unknown type RuntimeException`: phorj
had an `Error` marker interface and user-declared errors and nothing in between.

| PHP builtin | phorj |
|---|---|
| `Throwable`, `Exception`, `Error`, `ErrorException`, `RuntimeException` | `RuntimeError` |
| `LogicException`, `BadFunctionCallException`, `BadMethodCallException` | `LogicError` |
| `ArithmeticError`, `DivisionByZeroError`, `OverflowException`, `UnderflowException`, `RangeException` | `MathError` |
| `TypeError` | `TypeMismatchError` |
| `ValueError`, `InvalidArgumentException`, `DomainException`, `LengthException`, `OutOfRangeException`, `OutOfBoundsException`, `UnexpectedValueException`, `JsonException` | `InvalidValueError` |
| *(no PHP counterpart — phorj's own)* | `IoError` |

The set is **flat**: none of the six extends another. PHP's `Throwable`/`Error`/`Exception` split was
deliberately not mirrored — it would import a much-criticised hierarchy into a language that does not
have one, and decide phorj's error model as a side effect of a lift feature. Flat also means `catch`
needs no subclass matching: a clause catches exactly the type it names.

Three names avoid a collision rather than reading oddly by choice. `ArithmeticError`, `TypeError` and
`ValueError` are real PHP **builtin classes**, so `E-RESERVED-NAME` rejects them — transpiling
`class TypeError extends \Exception` would redeclare PHP's own.

The mapping is **semantic, not hierarchical**. `InvalidArgumentException` lands on `InvalidValueError`
rather than `LogicError`: PHP files it under `LogicException` for hierarchy reasons, but what it
reports is a bad argument *value*, and a flat set should say what a thing means.

**An unmapped class is refused loudly, not guessed.** A framework or application exception keeps its
own name and the draft is prefixed with a note:

```
// CANNOT LIFT: `Acme\PaymentFailed` has no phorj counterpart — declare it, or catch one of
// `Core.ErrorModule`'s types instead.
```

### The one thing lift cannot infer: `throws`

A lifted `catch` now type-checks with **no hand edits**. A lifted `throw` does not, and cannot: phorj
has checked exceptions and PHP does not, so the PHP source carries nothing a `throws` clause could be
derived from. `phg check` says so precisely, at the exact statement:

```
type error at 22:9: `InvalidValueError` is thrown here but neither caught nor declared
  [E-THROW-UNDECLARED]
  hint: add `throws InvalidValueError` to the enclosing function, or wrap this in `try`/`catch`
```

The committed [`errors.phg`](errors.phg) is the draft with those clauses added (and one `int` →
string conversion `echo` needs). It is part of the example suite, so it is byte-identity-gated on
both backends **and** real PHP, and its output matches the original `errors.php` run under `php`.

## What lift refuses (loudly — the Tier-2 frontier)

Lift errors rather than guess when there is no faithful Phorj form *yet*: an `array` **type**
annotation (needs `List`/`Map`/`Set` inference), a **key/value** `foreach ($xs as $k => $v)` (Phorj's
`foreach` has no key binding yet), backed enums and enum methods, default parameter values, untyped
parameters, the elvis `?:`, an assignment used as a sub-expression, and a non-literal `match` arm.
Each is a clear `lift …` message naming what to do by hand.

Interpolation is lifted only within PHP's *actual* grammar — a `$`-rooted access chain (`$x`,
`$o->p`, `$a[$k]`, `$o->m()`). The forms PHP itself rejects or that coerce silently are refused
loudly: a top-level operator inside `{$…}` (a PHP parse error too), the removed `${…}`
variable-variable form, and a simple-syntax bareword subscript `"$a[key]"` (whose key silently
becomes the string `'key'` — use the explicit `"{$a['key']}"` form).

> **Review the draft.** A lifted program that type-checks is *structurally* sound, but `lift` cannot
> prove it preserves the original PHP's behavior — that is the `// lifted (verify)` contract.
