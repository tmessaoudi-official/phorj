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
    Output.print("{greet("Phorj")}");
    Output.print(" Counter starts at {c.start}, next is {c.next()}.");
}
```

Both print `Hello, Phorj! Counter starts at 41, next is 42.` The lifted `sample.phg` is part of the
example suite, so it is byte-identity-gated on both backends **and** real PHP like every other
example.

## `phg lift <dir>` — a whole PROJECT (DEC-439)

Lifting one file at a time could not resolve anything the file *referenced*, for one reason: a file
cannot see its siblings. `import App.Support.Money;` was `E-MODULE-NOT-FOUND` and `#[App.Meta.Audited]`
was `E-UNKNOWN-ATTRIBUTE` — both correct, both unfixable one file at a time. Lifting the tree in ONE
pass fixes both, because the files that *declare* those symbols are now in the project beside the
files that use them.

```console
$ phg lift ./my-symfony-app -o ./lifted
```

`-o` is required: a directory lift writes a whole tree, so where it lands is never implied. The output
must be empty — it will not overwrite an existing project.

> **No companion fixture for this section, unlike the others.** A directory lift's artifact is a
> *tree* plus two reports, not a `.php` → `.phg` pair, so there is nothing here for the byte-identity
> example glob to gate. The transcript below is real output, reproduced from the integration fixture
> in [`tests/lift_project.rs`](../../tests/lift_project.rs) — which *is* gated.

### It lifts what lifts, and NAMES the rest

A real Symfony/Laravel app contains plenty of Tier-2 PHP, so an all-or-nothing lift would produce
nothing at all on any real input. Every file that fails is listed in `LIFT-REPORT.md` with its reason
— which doubles as the ranked worklist of what the lifter still cannot do. `VENDOR-REPORT.md` lists
every composer symbol the app references, attributed to the package that ships it and ranked by
reference count. Nothing is faked and nothing is silently skipped.

### The files that are not in `autoload` — and not all the same thing

`autoload.psr-4` maps `src/`, so what happens to Symfony's `public/index.php`, `bin/console`,
`migrations/`, `config/*.php`, or Laravel's `artisan` and `routes/web.php`? A rule matching those
**names** would be a list of the frameworks the lifter happens to know, and wrong for the next one. So
they are classified by **content** instead, and no framework path is hardcoded anywhere:

| Shape | Role | What happens |
|---|---|---|
| declares a class / interface / trait / enum / function | code | **lifted** — it is the app's own code however composer maps it |
| top-level `return` of DATA | configuration | reported, with `#[Config]` (DEC-318) as its replacement |
| anything else with no declarations | bootstrap | reported, with `#[Entry(kind: …)]` as its replacement |
| declared by composer's `autoload-dev` | test | reported, with `phg test` as its replacement |

Two consequences worth stating, because both were wrong before they were measured:

* **Doctrine's `migrations/Version*.php` is LIFTED.** It declares a class, so it is code — and the
  lifter says nothing about Doctrine anywhere.
* **A returned closure is a factory, not configuration.** Symfony's `public/index.php`
  (`return function (array $context) {…}`) and a `config/*.php` file (`return [ … ]`) are *both* a
  top-level `return`. A rule that stopped there told the developer to re-express their front
  controller as typed configuration — wrong advice, confidently given.

**Test code is the one role NOT decided by content**, and it cannot be: a PHPUnit class declares a
class like any other, so content alone calls it application code and lifts it — producing a draft
whose `extends \PHPUnit\Framework\TestCase` references a framework that will never be ported. It comes
instead from composer's own `autoload-dev` declaration, which is still machine-readable metadata and
not a guess at a directory named `tests/`. The honest limit: test code in a project that declares no
`autoload-dev` is indistinguishable from application code, and *is* lifted.

Dropping `autoload-dev` from the *walk* does not drop it from *namespace recognition* — those are two
different questions. Test code is the app's own even though it is not lifted, so a reference into the
test namespace is a sibling reference, not a composer dependency, and must not appear in
`VENDOR-REPORT.md`.

`bin/console` and `artisan` have **no extension at all**, so PHP-ness is decided by content (an
opening `<?php`, allowing a `#!` shebang line) as well as by suffix. And composer's `bin` key is read
but deliberately kept *out* of the code surface: `autoload` says "this is my code", `bin` says "this is
a command", and feeding a console script to the lifter produced `lift parse error: require is Tier-2`
where the right answer was "this is a bootstrap script, here is the entry that replaces it".

On the Symfony-shaped fixture that is:

```console
$ phg lift ./app -o ./lifted
lifted 2/2 PHP file(s) into `./lifted`
  no entry — a LIBRARY project (no file had top-level code)
  4 framework file(s) to RE-EXPRESS, not lift (bootstrap / PHP config) — each paired
    with its phorj counterpart in `LIFT-REPORT.md`
  0 vendor symbol(s) referenced — ranked in `VENDOR-REPORT.md` (nothing was stubbed)
```

| File | Role | phorj counterpart |
|---|---|---|
| `bin/console` | bootstrap | `#[Entry(kind: EntryKind.Cli)]` |
| `config/framework.php` | configuration | a `#[Config]` class, read at the entry |
| `public/index.php` | bootstrap | `#[Entry(kind: EntryKind.Web)]` |
| `routes/web.php` | bootstrap | an entry plus `#[Route]` handlers |

…with `src/Entity/Post.php` **and** `migrations/Version20260805.php` lifted into
`lifted/src/App/Entity/Post.phg` and `lifted/src/DoctrineMigrations/Version20260805.phg`.

A tree whose PHP is *entirely* bootstrap and configuration is refused with exactly that reason — not
with "no `.php` files found", which would send you looking for a file that is not missing.

### The entry, and collisions

A PHP script with top-level code IS an entry, so the first one becomes `src/main.phg`,
`package Main;`, at the source root. That is not cosmetic: a dotted package must sit in a matching
subdirectory (`E-PKG-PATH`) while `package Main` is exempt, so an entry left in its namespace package
makes the whole project fail to **load**. PHP allows any number of such scripts; phorj has one entry
per role, so further ones are left in place and *reported* — that choice is the developer's.

Two sources that map to the same package and file stem are **renamed, never overwritten**, and the
rename is disclosed. Legacy PHP hits this constantly: every namespace-less file lands in `package Main`
and collides on its bare stem.

### Vendor: reported by default, stubbed only on request

`--vendor=stub` would declare each vendor symbol as a foreign PHP symbol (`declare class` /
`declare function`, M8.5). It is ruled but not yet built, and it refuses with that reason rather than
quietly behaving like the default. The reason it must stay opt-in is measured, not stylistic: a program
carrying foreign declarations cannot run on **either** phorj engine (`E-FOREIGN-RUNTIME`), so stubs
trade the VM, the JIT and the byte-identity spine for a draft that type-checks. Invariant 14 forbids
making that trade silently.

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

## Real-code shapes in one file — `real-shapes.php` / `real-shapes.phg` (Lane R, 2026-09-05)

The wave-2 lift slices were measured on a real application (`scout`, 120 files of strict PHP 8.5),
one wall at a time, and this pair exercises every shape those slices taught the lifter, in one
ordinary-looking file:

| PHP shape | lifted to |
|---|---|
| `interface Scorer { … }` with a bodiless method | `interface Scorer { function score(…): int; }` |
| `final readonly class … implements Scorer` | a class whose fields carry no `mutable` |
| `public const string NAME = 'lengths'` (PHP 8.3 typed constant) | `const string NAME` |
| `public function __construct(public int $bonus = 1)` | a promoted parameter with its default |
| `/** @param list<string> $words */ … array $words` | `List<string> words` |
| `/** @return array<string, int> */ … : array` | `Map<string, int>` |
| `static fn (string $a, string $b): bool => …` | `function(string a, string b): bool => …` |
| `$words[] = 'lift'` | `words = List.append(words, "lift")` |
| `(float) $n / 2.0` | `(n as float) / 2.0` |
| `new Ranking(bonus: 2)` (named argument) | `new Ranking(bonus: 2)` |
| `1_000` | `1000` |
| `echo $r->score($words) . "\n"` | `Output.print("{r.score(words)}\n")` |

The `.phg` is the lifter's output byte for byte — `src/lift/tests_examples.rs` re-lifts every pair
in this directory and fails if a shipped `.phg` drifts from what `phg lift` produces (only
`errors.phg` is exempt, by name, because its `throws` clauses are hand-finished — see
KNOWN_ISSUES §LIFT-THROWS). It checks clean and prints the same six lines on the interpreter, the VM,
the transpiled PHP and the original PHP.

Two shapes were deliberately AVOIDED here because the lifter does not carry them yet, and both are
the next Lane R items: a local initialised as `$xs = []` and filled later (phorj needs the empty
literal's type — a `/** @var list<T> $xs */` on the local is the mechanical fix), and `(int)` of a
float (the `as int` conversion is fallible on the phorj side, `int?` — KNOWN_ISSUES
§LIFT-CAST-FIDELITY).
