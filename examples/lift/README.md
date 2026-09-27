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
`foreach` has no key binding yet), untyped parameters, the elvis `?:`, an assignment used as a
sub-expression, and a non-literal `match` arm. (Backed enums, enum methods and default parameter
values were on this list and are not any more — see § "Enums" below and Lane R.)
Each is a clear `lift …` message naming what to do by hand.

A PHPStan/Psalm **pseudo-type** in a docblock — `numeric-string`, `array-key`, `non-empty-string`,
`class-string`, `positive-int` — is refused by name (twes row T2): it has no phorj form yet, so the
message says to declare its base type. `non-empty-list<T>` and `non-empty-array<…>` do lift, as their
base `List`/`Map`. A ternary used as a STATEMENT (`$c ? $a->m() : $b->m();`) lifts to an `if`
statement (twes row T1).

A PHP **static local** (`static $keys = null;` inside a function or method) is refused by name
(DEC-539): phorj has no static locals, so the message names the two rewrites — move the value to a
private static field, or drop the memo when the value it caches is pure. A static *property* and a
`static fn` closure are different constructs and are unaffected.

Interpolation is lifted only within PHP's *actual* grammar — a `$`-rooted access chain (`$x`,
`$o->p`, `$a[$k]`, `$o->m()`). The forms PHP itself rejects or that coerce silently are refused
loudly: a top-level operator inside `{$…}` (a PHP parse error too), the removed `${…}`
variable-variable form, and a simple-syntax bareword subscript `"$a[key]"` (whose key silently
becomes the string `'key'` — use the explicit `"{$a['key']}"` form).

## Enums — cases, `self`, and methods (DEC-509, 2026-09-07)

`enums.php` / `enums.phg`. PHP enums do three things phorj has no direct spelling for, and the
lifter got two of them wrong and refused the third outright. All three sat on the path to a real
codebase's domain enum.

**A case is not a class constant.** PHP spells `Tenure::PLAI` and `Limits::MAX` identically, so
telling them apart needs to know which names are enums. The lifter used to emit `Tenure::PLAI`
verbatim — which is not phorj syntax at all, so the draft "lifted" and then failed `phg check` with
`E-UNKNOWN-IDENT`. A payload-less variant is **constructed**, like everything else in phorj
(mandatory `new`):

```php
return $this->tenure === Tenure::PLAI;      // PHP
```
```phorj
return this.tenure == new PLAI();           // lifted
```

In **pattern** position it becomes a variant pattern carrying its enum as the qualifier, so a case
from the wrong enum is still an error rather than an arm that silently never matches:

```phorj
return match (tenure) { PLAI() => true, default => false };
```

A class constant is untouched: `Limits::MAX` still lifts to `Limits::MAX`.

**`self` inside an enum body.** `self::PLAI` refused with *"`self` outside a class body"* — a message
that reads like the file is malformed rather than like a lifter gap, because the parser never
recorded the enclosing enum. An enum body is a class body for `self` now, in expression *and* type
position, which is R-7's rule reaching enums at last. `static` keeps its refusal, here as in a
class: late static binding means the receiver's class, and the enclosing name would narrow it.

**A method has no phorj form, so it lowers to a free function.** Phorj enums carry no methods, by
design — and UFCS is the substitute, so the call site does not change:

```php
public function isExcluded(): bool { return match ($this) { self::PLAI => true, default => false }; }
```
```phorj
function isExcluded(Tenure tenure): bool { return match (tenure) { PLAI() => true, default => false }; }
```

`$t->isExcluded()` in PHP reads as `t.isExcluded()` in the draft and resolves through UFCS to that
function. A **static** enum method has no `$this`, so it lowers with no receiver parameter at all.

Two refusals guard the lowering, because the lifter never invents a name:

- two enums in one file declaring the same method name would emit two free functions of one name —
  refused, naming the method;
- a method that already has a parameter called `tenure` (the receiver name for `enum Tenure`) —
  refused, naming the parameter.

**Outside `package Main`, the functions get their own file (DEC-526, scout row 5i).** A public enum
and public free functions cannot share a file in any package but `Main` (`E-FILE-MIXED-PUBLIC`), so
the lowered draft above checked only while the enum lived in `Main`, and every real PHP enum lives in
a namespace. A directory lift (`phg lift <dir> -o <out>`) therefore writes each enum's lowered methods
to a sibling `<Enum>Functions.phg` in the same package. Both files carry the source header of the PHP
file they came from, and each gets exactly the imports its own items use: an import only a method
needs moves with the method, because `E-UNUSED-IMPORT` is a hard error. A same-package caller reaches
the functions exactly as before, `t.isExcluded()` through UFCS or `fallback()` as a direct call.
`project/lift-enum-functions/` is the lifted output of such a tree, run on all three legs.

Three cases keep the single-file shape:
- an enum in `package Main`, where mixing is legal;
- an enum in the directory lift's ENTRY file, because the entry is re-packaged as `Main`, which
  would strand a companion in a package its enum no longer has;
- the single-file `phg lift foo.php`, which prints one draft to stdout and has nowhere to put a
  second file.

A PHP file already named `TenureFunctions.php` is not overwritten. The companion goes through the same
collision guard as every draft, so whichever of the two is written second is renamed (files are
lifted in path order, so that is usually the real file's draft, as `Core_TenureFunctions.phg`), and
`LIFT-REPORT.md` says so. A caller in
ANOTHER package still fails to check. UFCS through a function import is deferred to L7 (DEC-527), and
the lifter does not yet emit function imports for a lowered static method.

## Block-bodied closures, and the capture phorj rules out (DEC-506, 2026-09-07)

`closures.php` / `closures.phg`. 23 of a 120-file real codebase write a `function (…) { … }`
closure, and every one of them was refused with a flat *"Tier-2"* message. That message conflated
two very different answers:

- a **statement-bodied lambda**, which phorj HAS — `function(int x): int { … }` — and
- **by-reference capture** `use (&$x)`, which phorj has deliberately RULED OUT (it contradicts the
  value/handle split).

The first now lifts. The second is refused *by name*, saying which variable and that the rejection
is a ruling rather than a missing feature:

```console
$ phg lift byref.php
lift parse error: a closure capturing `$acc` by reference (`use (&$acc)`) has no phorj form —
references contradict the value/handle split and are RULED OUT (DEC-506), not merely
unimplemented. Rewrite the closure to return its result instead of mutating a captured variable.
```

The same ruling covers a by-reference PARAMETER (row 4o, DEC-543), and PHP's sentinel return
`string|false|null` is refused in DEC-544's words — collapsing `false` into `null` would merge two
outcomes the caller tells apart, so the port declares an enum. Any other union is refused as a gap
(phorj has `A | B`; the lifter does not map onto it yet):

```console
$ phg lift outparam.php
lift parse error: the parameter `$m` is taken by reference (`&$m`), which has no phorj form —
references contradict the value/handle split and are RULED OUT (DEC-506, DEC-543), not merely
unimplemented. Return the value instead of writing through the parameter (line 2)
$ phg lift sentinel.php
lift parse error: the union type `string|false|null` has a `false` member — PHP's sentinel return
has no phorj form (DEC-544): write `?T` when `false` is the only sentinel, or an enum with one
variant per outcome when `false` and `null` mean different things (line 2)
```

Two more rules fall out of the shape:

- **`static` is dropped, not refused.** It only stops PHP binding `$this`, and a phorj lambda never
  binds one — the modifier carries no information in the draft.
- **A BY-VALUE `use (…)` list is dropped** and the captured names simply stay in scope, because
  capturing enclosing locals by value is what a phorj lambda already does. Nothing is lost.
- **No `: T` is a refusal.** phorj requires a return type on a statement-bodied lambda, and the
  lifter does not infer one (DEC-166 — it never guesses).

## Array shapes, destructuring and `.` — `shapes.php` / `shapes.phg` (DEC-504/510/511/515, 2026-09-07, keyed half 2026-09-13)

PHP's `array{…}` docblock shape splits in two, and both halves now lift.

A **POSITIONAL** shape is a tuple, which phorj shipped as DEC-288 — so `@return array{int, string}`
lifts to `: (int, string)`, and the same shape written with explicit ascending indices from zero
(`array{0: float, 1: float}`, what PHPStan and Psalm emit) is the same tuple. A **KEYED** shape
(`array{tenure: string, bp: int}`) is a named-field tuple (**DEC-504**), `(tenure: string, bp: int)`.

**DEC-515 — the keyed shape's reads and writes.** A type alone lifts a draft that does not check, so
both value sides follow it:

- **Reads through a `foreach` binder.** Real code rarely indexes a keyed-shape *parameter*; it
  iterates a declared `list<array{…}>` (or `array<K, array{…}>`) and indexes the binder. The binder
  inherits the collection's ELEMENT shape, so `$row['bp']` lifts to `row.bp`. The binding lasts for
  the loop body only and a nested loop rebinding the same name restores the outer shape when it
  closes. A collection with no declared shape (bare `array`, an unresolved call's result) binds
  nothing — the lifter reads what the program declared and does not infer it (DEC-166).
- **Writes in return position.** `return ['tenure' => $t, 'bp' => $bp];` under a keyed `@return`
  lifts to `return (tenure: t, bp: bp);` — but only when the keys are exactly the declared fields **in
  the declared order**. Field order is part of the type and erasure is positional, so reordering the
  literal would silently change the evaluation order of its values; a reordered or short literal
  stays a map and `phg check` reports it.
- **Not yet:** `@var`-declared locals (their reads, and a keyed literal *assigned* to one) — tracked
  in the scout plan, row 5d.
- **Row 4p — two more declarations.** A binder over a declared PROPERTY (`foreach ($this->rows as
  $row)`, a typed property or a promoted constructor parameter with its `@param`) reads the element
  shape the same way, and the BUILDER local of a declared return — a top-level `$out = []` that the
  function returns, already constructed as the return type — takes a keyed literal written into it
  (`$out['rent'] = ['tenure' => …]`, each arm of `$c ? [...] : [...]` too) as the declared tuple.
  `Ledger` and `byTenure` in the pair. A keyed literal passed as an ARGUMENT still binds nothing.
- **Row 4p4 (DEC-550) — a call's DECLARED return.** A binder over a STATICALLY resolved call reads
  the callee's `@return` element shape: `$this->m()`, `self::m()` / `static::m()` / `Own::m()` on the
  enclosing class, and `f()` for a function this FILE declares — through a propagated `?` too.
  `Ledger::rentBp()` (over `$this->rents()`) and `main`'s loop over `byTenure(true)` in the pair.
  Never the callee's body (DEC-166). Unresolved, so still binding nothing: an inherited or
  `parent::` method, another object's method (`$box->hits()`), a nullsafe `$this?->m()`, a static
  call on ANOTHER class of the same file (`Other::m()`), and a function declared in another file.

A tuple TYPE is only half of it. The literal that satisfies it must lift to a tuple VALUE too, or
the draft lifts and then fails `phg check` with `expected (int, string), found List<int>` — one
draft whose two halves disagree. So a `return [$n, 'x'];` whose arity MATCHES the declared shape
becomes `return (n, "x")`, at any depth: `if ($y) { return [1, 'one']; }` is the shape real code
writes. A literal of the WRONG arity is left as a list — never padded or truncated into the declared
shape (DEC-166: the lifter does not guess), and the checker then reports the disagreement.

**DEC-510 — array destructuring.** `[$a, $b] = pair();` and `list($a, $b) = pair();` both lift to
`var (a, b) = pair();`. This is the ONLY way to read a positional shape: a phorj tuple has
destructuring-only access, so `$p[0]` has no form to lift to at all. A KEYED destructure
(`['k' => $v] = $m`) reads a map by key — a different construct, refused by name rather than
silently treated as positional.

**DEC-511 — `.` is an interpolation, not `+`.** PHP's `.` COERCES both operands to string; phorj's
`+` refuses to coerce anything. So `'n=' . $n` lifts to `"n={n}"`, and a CHAIN flattens into ONE
interpolation (`'a' . $x . 'b' . $y` → `"a{x}b{y}"`) rather than a nest, which is what PHP's own
left-associative evaluation produces. Arithmetic `+` is untouched — only `.` changes.

Two more "the draft must CHECK" fixes landed with this example, both found by running it:

| PHP | lifts to | why |
|---|---|---|
| `echo $n;` | `Output.print("{n}")` | the lifter cannot see whether `$n` is a string, and `Output.print` takes one. An interpolation is correct for BOTH — a bare `Output.print(n)` was a guess, and wrong on the int a lifted tuple produces |
| `$x === null` / `null === $x` | `x instanceof null` (phorj's `is null` test) | the checker rejects `T? == null` as a cross-type comparison. STRICT only: PHP's loose `== null` is also true for `0`, `""`, `[]` and `false`, so it stays an equality and the checker reports it |
| `$x !== null` | `!(x instanceof null)` | phorj has no `is not null` spelling |

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

## `@var`-declared locals — `var-locals.php` / `var-locals.phg` (scout row 5d, DEC-515, 2026-09-25)

A local's `/** @var … $x */` is read the way a parameter's `@param` is, when it names a keyed shape:

- **The type stays on the declaration.** `$rows = load();` under `@var list<array{id: int, name: string}>`
  lifts to `mutable List<(id: int, name: string)> rows = load();`, and `$seen = null;` under
  `@var array{…}|null` to `mutable (id: int, name: string)? seen = null;`. An empty `$kept = []` keeps
  printing `var kept = new List<(id: int, name: string)>()` — the initializer already spells the type.
- **Reads become field reads**: `$rows[0]['id']` → `rows[0].id`, `$seen['id']` → `seen.id`, and a
  `foreach` binder over the local inherits the element shape (`$k['id']` → `k.id`).
- **Writes become tuples**: a keyed literal assigned to the local (`$seen = ['id' => …, 'name' => …]`),
  appended to it (`$xs[] = […]`), or listed in a returned `list<array{…}>` literal becomes a tuple
  literal — only when its keys are the declared fields in declared order.

`$seen === null || $r['id'] > $seen['id']` reads `seen.id` behind a `||` — that checks because of
DEC-535 (row 5r). A local with no `@var`, a `@var` naming another variable, or a type with no keyed
shape (`@var int $n`) lifts exactly as before. Byte-identical on all three legs and against the PHP.

## Map union — `map-union.php` / `map-union.phg` (scout row 5q, DEC-534, 2026-09-25)

PHP spells array union and addition with one operator. The lifter has no types, so it rewrites `$a + $b`
to `Map.union(a, b)` only where it KNOWS both operands are maps: a keyed array literal, or a class
constant of the same file whose type lifts to a `Map` — `self::LOWER + self::UPPER` here. On the shared
key `é` the LEFT value wins, as PHP `+` does. A PHP `array` parameter or property may be a list, so
`$a + $b` on parameters stays `+` and `phg check` names it; a positional union (`[1, 2] + [3]`, by
index) has no phorj form and stays `+` too. Byte-identical on all three legs and against the PHP.

## Destructuring `foreach` — `foreach-destructure.php` / `foreach-destructure.phg` (scout row 5v, 2026-09-26)

`foreach (hits() as [$word, $at])` and `foreach (… as list($word, $at))` lift to phorj's tuple loop
`for ((word, at) in hits())` — DEC-510's positional destructure, in a loop header. The lifter emits the
same lowering the phorj parser builds and the formatter shows it sugared, so the nested pair reads as two
ordinary tuple loops. The binders belong to the loop: the `$word = "last"` after it is a fresh
declaration, where PHP would be overwriting the loop's last value. Byte-identical on all three legs and
against the PHP.

## Callable signatures — `callables.php` / `callables.phg` (scout row 4e, 2026-09-26)

A bare `callable` or `\Closure` says nothing about what it takes or returns, and a phorj function type
always does — so the lifter refuses one rather than invent a signature. PHPStan and Psalm spell the
signature in the docblock, `callable(A, B): R` or `\Closure(A): R`, and that is read exactly like a
docblock `list<T>` types an `array`: `@param callable(string): ?Profile $f` lifts `callable $f` to
`(string) => Profile? f`, and `@var \Closure(string): ?Profile` types a property the same way. A
parameter may be named in the signature (`callable(string $key): R`); the name is dropped.

A nullable callable is written grouped, `(callable(string): ?Profile)|null` on a `?callable`, and
lifts to `((string) => Profile?)?` — the parentheses matter: `?` binds to the nearest type, so without
them the outer `?` would attach to the return type instead of the function (`phg format` keeps them
since row 5y). `\Closure::fromCallable($f)`
lifts to `f`: once the parameter is typed from its signature it is already a function value.

Refused by name: a signature-less `callable`/`\Closure`, an optional (`int=`), variadic (`int...`) or
by-reference (`int &$x`) signature parameter, a signature with no `: R` (PHP reads that as `mixed`), and
a docblock whose type does not match the declared slot. A function-typed FIELD is passed on as a value
here, never called directly: `($this->f)($x)` has no phorj spelling yet (see the scout plan's Known
issues).

**First-class callables (row 4f).** `len(...)` of a NAMED function lifts to the phorj function reference
`len` — `array_map(len(...), $xs)` becomes `xs.map(len)`, and it transpiles back to `len(...)`. A
builtin's `strlen(...)` is still refused: which phorj native it means can depend on the arity it is
called with (`min($xs)` is a list minimum, `min($a, $b)` the lesser value), and that data is the next
row's. `$f(...)`, `$o->m(...)` and `C::m(...)` are refused too, and every one of these refusals names a
rewrite that lifts: a closure with a TYPED parameter, or a `foreach` where the parameter is an `array`.
Byte-identical on all three legs and against the PHP.

## Exception base classes — `exception-base.php` / `exception-base.phg` (scout row 4j, 2026-09-27)

A user exception class that extends a PHP builtin (`extends \RuntimeException`) lifts onto its
DEC-421 counterpart (`extends RuntimeError`): an `open` class whose `message` constructor the subclass
inherits, so `new self('…')` keeps working, and a `catch (\RuntimeException $e)` still catches it. The
draft imports the counterpart like any `throw`/`catch` site that names one. A base that is the
program's own class — or any class that is not a known PHP exception — keeps its name, and a class the
program declares is no longer reported as `// CANNOT LIFT` when it is thrown or caught.

One thing stays loud by ruling (DEC-546): phorj requires a throwable's name to end in `Error` or
`Exception` (`E-ERROR-NAME`), and the lift keeps the PHP name, since a rename would change the
transpiled PHP class. The `throws` a throwing function needs is now declared for it (row 4j2 — see
`throws.php` below). Byte-identical on all three legs and against the PHP.

## Declared and propagated `throws` — `throws.php` / `throws.phg` (scout row 4j2, 2026-09-27)

PHP exceptions are unchecked; phorj's are checked (DEC-068). So the lift works out, from the PHP
alone, what each function and method throws, and says so (DEC-547):

- **Direct.** A `throw` whose type the source names — `throw new X(…)`, `throw X::factory(…)` (a
  static method returning `self`/`static`/the class), or a rethrow `throw $e` of a caught exception —
  and that no enclosing `catch` covers, becomes `throws X` on the declaration.
- **Propagated.** A call the lift can resolve statically — `f()`, `self::`/`static::`/`parent::`/
  `Class::m()`, `$this->m()` — to a declaration that throws, outside a covering `catch`, is spelled
  `f(…)?` and its caller declares the type too; closed over the whole call graph, so a chain of any
  depth is covered. In a directory lift the graph spans the whole tree, and a type thrown from
  another namespace is imported. A propagated call used as a receiver is parenthesized:
  `(fold(s)?).length()`, since `f()?.m()` would read as safe navigation.
- **`$e->getMessage()`** on a variable a `catch` bound reads its `message` field: `e.message` (row
  4j3). Any other receiver keeps the call — a user class may declare its own `getMessage`.
- **Coverage is judged in phorj names**, as the checker judges it: `catch (\RuntimeException $e)`
  covers a class extending it, because both lift onto `RuntimeError`.

What cannot declare `throws` stays LOUD rather than guessed: `main` (so a throwing call at the top
level must sit in a `try` — `E-CALL-UNHANDLED` otherwise), a lambda, a constructor, a magic method
and a lowered enum method; so does a call through a value or an untyped receiver, a `throw` whose
type the source does not say, and a class or function whose name is declared in two namespaces (the
analysis cannot tell the two apart, so it does not pick one). `?` is a checker-only marker, erased before every backend: the run, the transpiled PHP
and the original PHP are byte-identical.

## Reassigned parameters — `reassigned-params.php` / `reassigned-params.phg` (scout row 4q, DEC-548, 2026-09-27)

A PHP parameter is an ordinary local and the body may reassign it; a phorj parameter is immutable,
there is no `mutable` parameter form, and a local may not shadow one. So a parameter the body WRITES
is copied into a mutable local on the first line, and the body uses the copy:

```phorj
function cap(int bp): int {
    mutable var bpLocal = bp;
    if (bpLocal > 9) {
        bpLocal = 9;
    }
    return bpLocal;
}
```

The signature is untouched, so the named argument `cap(bp: 12)` still binds. Every assignment form
counts — `=`, `+=`/`*=`, `++`, `$xs[] = …`, `$xs[$k] = …`, and the by-reference builtins the lift
lowers to a reassignment (`usort`, `array_unshift`); a read-only parameter keeps its name. A closure
capturing the parameter captures the copy, and the copy carries the parameter's declared type, so a
shaped or collection parameter's fields still read as fields. Not a write: `$box->x = …` (that writes
through the object) and an assignment inside a closure body (PHP captures by value — it is the
closure's own local). A `foreach`/`catch`/destructure binder over a parameter's name is a phorj
DECLARATION, not an assignment, and copying would not change that. The copy is `<name>Local`, or
`<name>Local2`… when the body already uses that name. Kotlin parameters are `val` and Swift removed
`var` parameters (SE-0003) — the copy is the idiom both leave. All three phorj legs print what PHP
prints.

## `trim` — PHP's own set (scout row 4n, DEC-545/549, 2026-09-27)

`trim($s)` lifts to `s.trimAscii()`, a native stripping exactly PHP's default list
`" \t\n\r\0\x0B"` and transpiling back to `trim($s)`. It deliberately does NOT lift to
`String.trim`, which strips the Unicode White_Space set (form feed, U+00A0, U+3000, …) — in a
byte-identity harness that would be a meaning change. The character-list form `trim($s, $chars)` has
another arity and stays the unresolved `trim(…)`, loud at `phg check` (DEC-312's arity rule), and
`trim(...)` as a first-class callable is refused like any registered builtin's (row 4f).

## Reordered builtins — `reordered.php` / `reordered.phg` (scout row 4i, 2026-09-27)

These PHP builtins have a phorj native whose PHP form IS the builtin, but with the arguments in
another order, so the registry could not invert them:

- **`in_array($x, $xs, true)`** → `List.contains(xs, x)`. Qualified, because `contains` also exists
  on `string`, where `in_array($x, $s, true)` is a PHP `TypeError`. A LOOSE (two-argument or
  non-`true`) `in_array` is refused by name: `in_array("1", ["01"])` is true in PHP.
- **`implode($sep, $xs)`** → `xs.join(sep)`; **`explode($sep, $s)`** → `s.split(sep)` (a limit is
  refused).
- **`str_replace`** → `replace`, and over a literal search ARRAY a chain of one `replace` per needle:
  PHP re-replaces left to right (`str_replace(['a', 'b'], ['b', 'c'], 'ab')` is `cc`), which is
  exactly what the chain does. The replacement arrays must be the same length, and a reused scalar
  replacement must be inert (the chain evaluates it once per needle); a count argument is refused.
- **`sprintf`** with a literal format (or a `.` chain of literals) of `%s` and `%%` only → the
  interpolation its `.` chain would lift to (DEC-511 — so a `%s` of a bool inherits `.`'s existing
  exposure, it is not new). `%d` is refused: it equals `%s` only for an int. So are width, flag and
  positional directives, a computed format, and a directive/value count mismatch.
- **`PHP_INT_MAX`** → `9223372036854775807`.

**Evaluation order.** The receiver form evaluates the receiver first; PHP evaluates arguments left to
right. A swap is only taken when there is at most one EFFECT (a call, `new`) among the arguments and
no other argument READS state it could change: `implode($this->sep, $this->build())` keeps its PHP
order because `build()` may set `$this->sep`, while `implode(', ', $this->build())` and
`str_replace($this->a, $this->b, $s)` are swapped (a literal, a local variable or a constant cannot
be changed by a call). Otherwise the call is left as the plain unresolved (loud) call. The same guard now covers `array_map`'s swap (row 5f). Byte-identical on all
three legs and against the PHP.

## Empty collections — `empty.php` / `empty.phg` (scout row 4h, 2026-09-27)

phorj has no untyped empty literal: a bare `[]` is `E-EMPTY-LITERAL` until it is constructed as
`new List<T>()` / `new Map<K, V>()` (DEC-214 part-2). PHP writes `[]` everywhere, so the lifter types
it wherever the PROGRAM declared the type — never by inference:

- **`$xs === []` / `$xs !== []`** on a variable declared a non-null list or map (parameter type or
  `@param`, `@var` local) → `List.isEmpty(xs)` / `!Map.isEmpty(m)`, either operand order. The call is
  QUALIFIED, not `xs.isEmpty()`: the declaration is a hint the lifter cannot follow through a `foreach`
  binder, a closure parameter, a `catch` or the top-level script (which sees the last function's
  declarations) that rebinds the name, and `isEmpty` also exists on
  `string`, where PHP's `"" === []` is false. `List.isEmpty` type-checks only on a list, so a stale
  hint makes the draft fail `phg check` — it never changes the answer. A LOOSE `== []` is left alone
  (it is also true for `null`, `false`, `0` and `""`), and so is a nullable collection, for which
  `null === []` is false.
- **`return [];`** at any depth of a function or method whose return type is declared (`@return
  list<int>`, `array<string, int>`, either nullable) → `return new List<int>();`. A closure's own
  `return` answers to the closure's signature, not the enclosing function's.
- **`/** @var list<T> $x */ $x = [];`** was already typed (Lane R-6).

Left loud, by design — each still reads `an empty collection needs its type` and the fix is an
annotation: an unannotated `$x = [];` local, `$this->prop === []` (property types are not tracked
yet), a `[]` branch of a ternary, and a bare `[]` passed as an argument. Byte-identical on all three
legs and against the PHP — the round-trip that pinned this also found scout row 4h0, where
`!xs.isEmpty()` transpiled to an always-false PHP expression.

## Builtins by arity — `arity.php` / `arity.phg` (scout row 4g, 2026-09-27)

The registry maps one phorj native to each PHP builtin, for ONE arity: `min($a, $b)` is `a.min(b)`,
`substr($s, $i, $n)` is `s.substring(i, n)`. The arities it cannot place used to lift to an unresolved call:

- **`min($xs)` / `max($xs)`** — the one-argument form takes an array, and lifts to `xs.min()!`. Three
  costs, disclosed: every site carries a `W-FORCE-UNWRAP` warning (the accepted cost, as `division.phg`'s
  redundant casts are); an empty list is a phorj unwrap fault where PHP throws `ValueError` — both
  fatal, a different class; and a list of strings orders byte-wise, not by PHP's numeric-string rules.
- **`min($a, $b, $c)`** — three or more values fold left, `a.min(b).min(c)`; the arguments are still
  evaluated in source order.
- **`substr($s, $i)`** — to the end, `s.substring(i, 9223372036854775807)`: PHP's own idiom for "no
  length", and `substring` clamps it exactly as PHP does (row 5z made that safe). Not `s.length()`,
  which would evaluate `$s` twice.

Refused by name: `array_filter($xs)` with no callback (it keeps the PHP-*truthy* values, and truthiness
depends on the element type — write the callback), and the two-argument `strtr($s, $pairs)` (replace
pairs longest-first; no phorj native does that yet). A spread or named argument is never read as one of
these shapes. Byte-identical on all three legs and against the PHP.

## Spread over lists — `spread.php` / `spread.phg` (scout row 5u, DEC-538, 2026-09-26)

phorj has no spread syntax; PHP's `...` over lists lifts to the `Core.List` calls that already mean
the same thing. `[...$low, $mid, ...$high]` becomes `List.flatten([low, [mid], high])` (two segments
would be `List.concat(a, b)`), `array_merge(...array_values($byTier))` becomes
`List.flatten(Map.values(byTier))` — the map keeps PHP's insertion order, including the key `1`
overwritten in place — and the statement `array_unshift($out, ...$extra)` becomes
`out = List.concat(extra, out)`. The checker is the judge of list-ness: `concat`/`flatten`/`values`
type-check only on the right collection, and a phorj List is sequential, so PHP's renumbering spread
agrees whenever the draft checks. A spread of a keyed literal, a map constant or a Map-declared
variable, a spread inside a keyed literal, and unpacking into any other call (`f(...$xs)`, DEC-299)
are refused by name. Byte-identical on all three legs and against the PHP.

## String escapes — `escapes.php` / `escapes.phg` (scout row 5n, 2026-09-25)

PHP decodes escapes by quote style, and the lift now decodes them exactly as PHP does, so the draft
prints the same bytes. Double-quoted: `"\u{2019}"` is `’`, `"\x41"` and `"\101"` are `A`, `"\$"` is
`$`, and `"\{"` is NOT an escape (the backslash stays, and `$first` after it still interpolates).
Single-quoted: only `\\` and `\'` decode, so `'\n'` stays two characters. The draft re-escapes the
decoded text the phorj way — `’` appears as itself, a tab as `\t`, a backslash as `\\`. Bytes a phorj
string cannot hold (`"\x80"`, a surrogate `\u{D800}`) and octal escapes past `\377` are refused by
name rather than guessed at (KNOWN_ISSUES §LIFT-UNICODE-ESCAPE, FIXED). Byte-identical on all three
legs and against the original PHP.

## Division — `division.php` / `division.phg` (scout row 5g, 2026-09-24)

PHP `/` is float division (`7 / 2` is `3.5`). Phorj's `/` on two ints is integer division, so the
lift makes every operand float (DEC-523, built as DEC-529):

- an int literal becomes a float literal: `100` → `100.0`, `-2` → `-2.0`;
- an operand that is already float stays as written: a float literal, a `(float)` cast, another `/`;
- anything else gets `as float`: `$a / $b` → `(a as float) / (b as float)`.

`$x /= 2` lifts to `x = (x as float) / 2.0`. The lifter does not know variable types, so a float
variable gets a redundant cast, and `phg check` warns `W-REDUNDANT-CAST` but still passes. PHP's
exact `6 / 3` is int `2`; the lift gives float `2.0`, which prints `2` too. The one difference is an
`int` slot: `function f(int $a): int { return $a / 3; }` lifts to a draft that fails `phg check`
(`expected int, found float`). Use `intdiv($a, 3)` in the PHP when int division is what you mean.

## Sorting and mapping — `sorting.php` / `sorting.phg` (scout row 5f, 2026-09-13)

PHP's two most common array callbacks lift to phorj's receiver-form `Core.List` calls:

- **`usort($xs, $cmp);` → `xs = xs.sortWith(cmp);`.** PHP sorts its argument BY REFERENCE and returns
  `bool`, so the lift maps only the statement form. `usort` used as a value, or on a field or element
  target (`usort($this->items, …)`, `usort($xss[0], …)`), stays an unresolved call that `phg check`
  reports. The lift does not guess a write.
- **`array_map($f, $xs)` → `xs.map(f)`.** PHP takes the callable first. A call over several arrays
  (PHP zips them) stays unmapped, so no second list is dropped.

Both sorts are **stable**. That is why `fig`/`yam` and `pear`/`plum`/`kiwi` keep their input order
in the output, and the DEC-505 differential test pins it on all three legs.

The sort copies its parameter into a local first. A phorj parameter is immutable, so
`usort($words, …)` on the parameter itself lifts to a reassignment that `phg check` rejects
(`E-ASSIGN-IMMUTABLE`). That is the same result any lifted `$param = …` gets.

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
| `/** @var list<string> $words */ $words = [];` | `mutable var words = new List<string>();` |
| `$words[] = 'lift'` | `words = List.append(words, "lift")` |
| `(float) $n / 2.0` | `(n as float) / 2.0` |
| `new Ranking(bonus: 2)` (named argument) | `new Ranking(bonus: 2)` |
| `1_000` | `1000` |
| `echo $r->score($words) . "\n"` | `Output.print("{r.score(words)}\n")` |
| `private int $n = 0;` (property default) | `private mutable int n = 0;` (field initializer) |
| `public function add(int $k): self` | `function add(int k): Tally` — `self` is the enclosing class, exactly |
| `$t = $t->add(3)->add(4)` (fluent chain) | the same chain, and the second `$t =` is an ASSIGNMENT |
| `$r instanceof \Main\Scorer` | `r instanceof Scorer` — a root-qualified name in the file's OWN namespace is not an import |

The `.phg` is the lifter's output byte for byte — `src/lift/tests_examples.rs` re-lifts every pair
in this directory and fails if a shipped `.phg` drifts from what `phg lift` produces (only
`errors.phg` is exempt, by name, because its `throws` clauses are hand-finished — see
KNOWN_ISSUES §LIFT-THROWS). It checks clean and prints the same eight lines on the interpreter, the
VM, the transpiled PHP and the original PHP — verified by running all four, not inferred.

The file declares `namespace Main;` on purpose: a lifted draft's `package` must match its directory
(`E-PKG-PATH`, folder = package), so a namespaced draft is checkable only inside the tree
`phg lift <dir> -o <out>` builds for it — `namespaces.php` is the pair that demonstrates that shape.
Here the point is that the draft RUNS from where it sits, so the paragraph above can be a claim
about behaviour rather than about shape.

Two lifter bugs were found by writing this example and running it, not by reading the lifter:
a root-qualified name pointing into the file's own namespace became an import of itself
(`import Main.Scorer;` → `E-MODULE-NOT-FOUND`), and every top-level statement got a fresh
already-declared set, so a second `$t = …` re-declared the variable (`E-SHADOW-LOCAL`) — a shape
almost every PHP script with top-level code contains.

The `/** @var list<string> $words */ $words = [];` at the entry is the empty-literal idiom (Lane
R-6): phorj needs an empty collection's type, and the lifter takes it ONLY from a declaration the
program wrote — a `@var` on the local, or the enclosing function's declared return when the local
is what gets returned — never from the elements. One shape is deliberately AVOIDED here because the
lifter cannot carry it faithfully: `(int)` of a float (the `as int` conversion is fallible on the
phorj side, `int?` — KNOWN_ISSUES §LIFT-CAST-FIDELITY).
