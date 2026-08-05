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
> rather than an emitted import. `#[...]` attributes now lift (see below).

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

## `#[…]` attributes (LIFT-ATTR, 2026-08-05)

`attributes.php` / `attributes.phg`. A bare `#` is a line COMMENT in PHP, and the lift lexer treated
`#[Audited("billing")]` as exactly that — **silently swallowing it**. That is the worst failure shape for
a tool whose contract is "refuse loudly, never guess": the file lifted, and quietly meant less. `#[` is
now its own token; a bare `#` is still a comment.

An attribute name is a CLASS name, so it is resolved the way PHP resolves one — `use` map first, then the
current namespace, and a leading `\` means the root. Only then is it spelled for phorj:

| Resolved to | Emitted as | Why |
|---|---|---|
| root `Attribute` / `Deprecated` | `Core.Runtime.Attribute` / `Core.Runtime.Deprecated` | same concept under the same name; the dotted form is self-gating, so no import is synthesized |
| a class in THIS file's package (or the root) | the bare leaf — `#[Audited("billing")]` | a single-file compile keys classes bare, so `#[App.Meta.Audited]` would match nothing and land on `E-ATTR-TARGET`. The bare form matches both keyings |
| a class from anywhere else | the FULL path — `#[Doctrine.ORM.Mapping.Column]` | phorj matches a built-in attribute as a segment-boundary SUFFIX, so a Symfony `#[Route("/home")]` lifted bare would bind to phorj's own `Core.Http.Route` — a different class taking different arguments, checking clean and meaning something else |

`#[A, B]` (several attributes in one group) is flattened to one `#[…]` per line, and PHP 8.0 **named
arguments** lift 1:1 (`#[Tag(order: 3, name: "late")]`) — phorj spells them the same way, so nothing is
reordered; the checker normalizes them into their constructor slots.

**The other direction closed too (DEC-437, developer-ruled).** `transpile` now RE-EMITS attributes into
the PHP, so `PHP → phorj → PHP` keeps the metadata and PHP-side reflection can read it
(`ReflectionAttribute::newInstance()` works — which is why the `#[Attribute]` marker is emitted as PHP's own
`#[\Attribute]`). Two things are deliberately left out of the PHP, both for byte-identity: phorj's built-ins
(compile-time machinery), and **any attribute whose argument has no PHP CONSTANT form** — PHP would fatal
the whole file — with the omission disclosed in the output. See `CHANGELOG.md` / DEC-437.

**Arguments are never rewritten, dropped or reordered.** `#[Attribute(Attribute::TARGET_CLASS)]`
therefore lifts to a phorj marker that the CHECKER rejects (`E-ATTRIBUTE-ARGS` — target restriction is
not implemented yet) rather than the lifter quietly dropping the restriction; likewise
`#[Deprecated(since: "8.4")]` fails on the argument phorj does not have. A draft that fails `phg check`
with a precise message is in-contract; one that checks clean and means less is not.

Refused loudly, with the position named:

| Shape | Why |
|---|---|
| an attribute on a method, property or class constant | phorj allows `#[…]` on a top-level `function` or `class` only (`E-ATTR-TARGET`) — and `#[ORM\Column]` on a property *is* the meaning of that line, so dropping it is a silent loss |
| an attribute on a parameter, an enum, or an enum case | same target rule |
| an unqualified name equal to a phorj built-in attribute (`#[Route]`, `#[Config]`, … in a file with no namespace or `use` for it) | phorj resolves the unqualified name to the BUILT-IN, so the lifted program would mean something different. Qualifying it instead is not a fix — `#[App.Route]` resolves only under a project compile and is `E-ATTR-TARGET` in the flat draft `phg lift` emits |
| a non-ASCII class name (`#[Café]`) | legal PHP; phorj's lexer rejects `é`, and a LEX error suppresses every other diagnostic in the file |

## The function-scope hoist (DEC-397, 2026-08-04)

`hoist.php` / `hoist.phg`. PHP has FUNCTION scope; phorj has BLOCK scope. A variable first assigned
inside a block was declared inside it, so every later use failed.

**Hoisted** when the first assignment is a literal in a block that ALWAYS executes — the function body,
a bare `{ … }`, or `if (true)` with no `elseif`/`else`.

**Refused, with a `// CANNOT LIFT:` note naming the variable**, in every other case:

| Shape | Why not |
|---|---|
| first assignment in a CONDITIONAL block, read outside | PHP reads the unassigned variable as null; hoisting a literal changes the answer. `if ($c) { $b = 5; } return $b + 0;` prints `0` in PHP for `$c = false`, and `5` hoisted |
| non-literal right-hand side | hoisting `$b = g();` moves a CALL out of its branch — a relocated side effect |
| a read precedes the first assignment | that read is of an unassigned variable in PHP |
| `while` / `for` / `foreach` / `try` / `catch` / `finally` body | a loop may run zero times; a `try` body may throw part-way |

**Never touched:** a parameter (already declared — a second declaration is `E-SHADOW-LOCAL`, which
DEC-397 explicitly forbids the lifter from emitting), a `foreach`/`catch` binding (the construct declares
it), and a block-local variable (nothing is broken, so hoisting would only add noise).

The refused cases still fail `phg check` — in-contract for a `// lifted (verify)` draft. What would not
be acceptable is failing it *silently*, or worse, passing it with the wrong answer.

> **Review the draft.** A lifted program that type-checks is *structurally* sound, but `lift` cannot
> prove it preserves the original PHP's behavior — that is the `// lifted (verify)` contract.
