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
| the whole file | `namespace a\b;` → `package A.B;` (PascalCase-ized); no namespace → `package Main;` |
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

## `namespace` / `use` — file-level declarations (LIFT-NS, 2026-08-04)

`namespaces.php` / `namespaces.phg`. Both keywords were outside the Tier-1 subset until this slice, which
made the lifter unusable on real-world PHP: a namespaced file failed at the PARSER, before anything else
could be attempted.

> **Honest scope.** Both mandatory PSR-12 prologue lines now lift — `declare(strict_types=1);` was
> closed by DEC-401, which also has the TRANSPILER emit it into every generated file. What is still
> open: a lifted `import` cannot resolve in a flat file (`E-MODULE-NOT-FOUND`), so the `use` half needs
> project-aware lifting before it pays off — which is why the example below shows the unused-import DROP
> rather than an emitted import. `#[...]` attributes are still swallowed (`KNOWN_ISSUES` LIFT-ATTR).

- `declare(strict_types=1);` → consumed and discarded (phorj is always strictly typed, so it states what
  is permanently true). `strict_types=0`, `ticks` and `encoding` are REFUSED — they carry meaning phorj
  cannot express.
- `namespace a\b;` → `package A.B;` — segments PascalCase-ized (`E-PKG-CASE` is enforced and PHP does not
  guarantee PascalCase), `snake_case`/`kebab` treated as word boundaries (`cli_tools` → `CliTools`), an
  already-upper segment left alone (`ORM` stays `ORM`). No namespace at all still yields `package Main;`.
- `use A\B\C;` → `import A.B.C;`, and `use A\B\C as D;` → `import A.B.C as D;` (phorj supports import
  aliases natively). A leading `\` root marker is not part of the path.
- Only the namespace segments are reshaped; the LAST segment is the class's own name and is left verbatim.
- **An unreferenced `use` is dropped.** `E-UNUSED-IMPORT` is a hard error in phorj and an unused `use` is
  legal and common in PHP, so keeping it would emit a draft that fails `phg check`. It is lossless — a
  `use` only creates a local alias.

Refused loudly, with the reason, rather than half-lifted: a braced `namespace A { … }` (phorj has one
`package` per file), a second `namespace` in one file, a `namespace` after a declaration,
`use function` / `use const` (they import a symbol, not a type), and the grouped `use A\{B, C};` form.

> **Review the draft.** A lifted program that type-checks is *structurally* sound, but `lift` cannot
> prove it preserves the original PHP's behavior — that is the `// lifted (verify)` contract.
