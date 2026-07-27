# Writing to a captured local is rejected (DEC-357, RULED 2026-07-26)

> **Status:** RULED by the developer 2026-07-26, **not yet built**. Canonical home for the rule
> (Invariant 19). Decision *identity + status* = the DEC-357 row in
> `docs/research/full-audit/raw/C-decisions.md`. Original analysis:
> `docs/research/2026-07-25-completeness-register.md` §6.4 (GR-19, finding I19).

## The bug

```phg
mutable int total = 0;
discard List.map(nums, function(int x): int { total = total + x; return x; });
Output.printLine("total={total}");     // total=0
```

`total=0` on **all three legs** — no error, no warning. — *[Verified 2026-07-26: `phg run`,
`run --tree-walker`, and transpiled PHP under php-8.5.8, `target/release/phg` @ `ea28687`.]*

This is **not** an Invariant-1 break: the backends agree. It is worse in a different way — the code
compiles, runs, and does nothing, and the programmer gets no signal. A dead assignment that reads as
live is the silent-wrong-answer class Invariant 14 exists to prevent.

## Why this is a narrow fix and not a semantics question

**By-value capture is already the documented, intended semantics:** `FEATURES.md:37` —
*"capture enclosing locals **by value**"*. So the language already answered "should writes escape?"
(no). The only open question was whether a provably-dead write should be a compile error. It should.

**The escape hatch already exists and is already taught.**
`examples/database/transaction-closure.phg:45-47` ships an `Attempts` class with a `mutable int n`
field, commented *"objects are reference-shared, so the mutation persists between the retry loop's
re-invocations"*. A user needing a mutable cell across invocations has an idiomatic, shipped pattern.

## THE RULE

> **Assignment to a captured local — the local variable itself — is rejected at check time,** with a
> hint naming the object-field pattern.

### The boundary — must be exact

| Form | Verdict |
|---|---|
| `total = …` where `total` is a captured enclosing local | **REJECT** |
| Mutating a captured **object's field** (`counter.n = …`, or a mutating method call on it) | **LEGAL** — this is the reference-shared workaround `transaction-closure.phg` depends on |
| Assigning the lambda's **own** parameters or its own locals | **LEGAL** — not captures |

Getting this boundary wrong breaks a shipped example, so the differential/db tests for
`transaction-closure.phg` are the regression guard that matters.

### Rejected alternatives

- **By-reference capture** (PHP's `use (&$x)`) — **rejected as out of scope here.** It would contradict
  `FEATURES.md`'s documented by-value semantics, so it is a language redesign, not a bug fix: aliasing
  rules, VM and JIT semantics for the reference, and PHP-parity for the reference itself. If it is ever
  wanted it gets its own spec and its own ruling.
- **A warning instead of a hard error** — a lost write is a correctness bug, not a style preference, and
  warnings are ignorable. Reconsider only if the migration measurement below comes back large.

## Definition of done

1. The diagnostic in the **checker** (one chokepoint → all surfaces, Invariant 17), with a hint naming
   the object-field pattern and the "return the value instead" alternative.
2. The boundary table above covered by tests, including a positive test that captured-object-field
   mutation still works.
3. **Migration measured before landing:** whether anything in `examples/` or `tests/` writes to a
   capture. Not reliably greppable — it needs the diagnostic to exist. Note that any hit is a **bug
   found**, not a migration burden, since the write is already a no-op.
4. Faults cannot be runnable examples — capture the case in a README entry (Invariant 9's carve-out).
5. `FEATURES.md:37`'s "by value" wording gains a pointer to the diagnostic, so the documented semantics
   and the enforced semantics are visibly the same thing.

---

## Companion: `Core.Mutable<T>` (RULED 2026-07-26, same session)

The rejection above needs somewhere to point. Ruled: a stdlib `Mutable<T>`.

### What reframed the design

**`List.reduce` already exists** — `reduce(list, initial, fn(acc, x) -> acc)`, generic, pure, mapping to
PHP `array_reduce` (`src/native/list_registry.rs:230-245`), alongside `sum`, `sumBy`, `count`, `countBy`.
So the motivating case for mutable capture —

```phg
mutable int total = 0;
discard List.map(nums, function(int x): int { total = total + x; return x; });   // silently 0
```

— already has a better, pure answer today:

```phg
int total = List.reduce(nums, 0, function(int acc, int x): int => acc + x);      // 6
```

**Most legitimate mutable-capture uses are a missing-fold smell, and the fold already ships.** The
wrapper is only for the genuine residual: reporting state out of a callback you do not control (the
`db.transaction` retry counter — `examples/database/transaction-closure.phg`'s `Attempts`),
multi-accumulator loops, and early-exit flags. **The real deliverable is therefore the diagnostic's
routing, not the type.**

### The name — `Mutable<T>`, and why not `Ref` or `Cell`

| Candidate | Verdict |
|---|---|
| **`Mutable<T>`** | **RULED.** Uses **phorj's own vocabulary** — the language already teaches `mutable int n`, so there is no new concept *and* nothing to unlearn. It also teaches a real distinction cleanly: `mutable` = the *binding* may be reassigned; `Mutable<T>` = the *contents* may change and be shared. |
| `Ref<T>` | Rejected. Familiar to PHP developers (`&$x` is "by reference") — but what it makes familiar is **wrong**: a PHP reference *aliases an existing variable*, while `Mutable<T>` **owns** its value. `new Ref(total)` copies, so `r.set(9)` silently leaves `total` untouched — and the checker **cannot** catch it, since `r.set(9)` is legal code. Borrowing PHP's most confusing feature's name for something that behaves differently is the wrong trade for a language whose premise is "PHP-inspired but clearer". |
| `Cell<T>` | Rejected. Rust jargon for interior mutability; the audience does not arrive knowing it. |
| `Slot<T>` | Rejected. Most precise, zero baggage, but unfamiliar and it borrows from nothing the language already teaches. |

### Surface — **AMENDED 2026-07-26 (developer-ruled, supersedes the get/set draft)**

**ONE form.** `Mutable<T>` is the single by-reference notion in phorj; `&$var` is merely its PHP spelling at
the foreign-interop boundary. A `ref x` operator was proposed and **REJECTED by the developer as ambiguous**.

| Item | Ruling |
|---|---|
| Type | `Mutable<T>`, `import Core.Mutable;` |
| Construction | `new Mutable(v)` — the only way to make one |
| **Access** | **`.value`, a public mutable field — REPLACES `get()`/`set()`** |
| `update(fn)` | Dropped — `List.reduce` covers accumulation; YAGNI |
| `ref x` | **REJECTED** (ambiguous). Purely ergonomic anyway, and purely additive if ever revisited |
| Implementation | Prelude, phorj source — no native code |
| PHP leg, phorj-owned | **`final class Mutable`** — an ORDINARY transpiled prelude class, **no `__phorj_` helper** (corrected 2026-07-26: verified that prelude classes emit as `final class Regex` / `final class RegexMatch`, unprefixed) |
| PHP leg, foreign boundary | **`&$var`** — a `Mutable<T>` param on a `declare function` maps to a by-ref arg (DEC-374) |
| Task safety | **Not** synchronised — must not cross a task boundary once DEC-370 lands |
| The capture-write diagnostic | Routes by shape: accumulation → `List.reduce`/`sumBy`/`count`; shared state → `Mutable<T>` |

**Why `.value` and not `get`/`set`** — verified 2026-07-26: `class Box { constructor(public mutable int value) {} }`
with `b.value = b.value + 1` written from **inside a function and from the caller** works today and prints the
mutated value. So `.value` costs **zero new machinery**, and because it is a plain typed expression, *everything*
that applies to `T` applies to it — arithmetic, UFCS, indexing, interpolation, passing to any function. A bespoke
get/set pair is strictly worse.

**No `__phorj_` helper is involved** (developer rule, DEC-377): `Mutable` is prelude *phorj source*, so it transpiles exactly like any other phorj class. An earlier draft of this spec wrongly said `final class __phorj_Mutable`; verified 2026-07-26 that prelude classes emit unprefixed (`final class Regex`).

**Why NOT `&$param` for phorj-owned values** — two PHP shapes for one phorj value is the **DEC-329.3 bug class**
(same value, different PHP shape ⇒ byte-identity regression). PHP references also cannot be a list element, an
object field, or a return type, so they cannot express the type at all. `&` appears **only** where a foreign
callee demands it, which is why it creates no second representation.

### Usage — every case

```phg
import Core.Mutable;

// 1. A counter you own
Mutable<int> hits = new Mutable(0);
hits.value = hits.value + 1;

// 2. A function that mutates the caller's value
function bump(Mutable<int> n): void { n.value = n.value + 1; }

// 3. Report state out of a callback you do not control  <- the reason it exists
Mutable<int> tries = new Mutable(0);
db.transaction(function(): void throws DatabaseError { tries.value = tries.value + 1; });

// 4. Stored, returned, collected — it is a real type
class Session { mutable Mutable<int> hits; }
List<Mutable<int>> counters = [new Mutable(0)];
function makeCounter(): Mutable<int> { return new Mutable(0); }

// 5. Collections: the BOX is shared; the collection inside stays immutable
Mutable<List<int>> xs = new Mutable([1, 2]);
xs.value = List.append(xs.value, 9);      // replaces the list — there is no in-place push

// 6. Accumulation — use neither; the fold already ships
int total = List.reduce(nums, 0, function(int acc, int x): int => acc + x);

// 7. Foreign PHP out-param (DEC-374)
declare function preg_match(string re, string subject, Mutable<List<string>> matches): int;
```

### Superseded draft (kept for the record)

### Surface

| Item | Ruling |
|---|---|
| Type | `Mutable<T>` |
| Import | `import Core.Mutable;` |
| API | `new Mutable(v)`, `get(): T`, `set(T v): void` — **nothing else** |
| `update(fn)` | **Dropped** — `List.reduce` removed most of its use; YAGNI, matching the `sameSite` precedent in `Cookie` |
| Implementation | **Prelude, phorj source** — no native code, all three legs identical by construction, transpiles to an ordinary PHP object (reference-shared for free, so no `__phorj_*` helper and no Invariant-14 ladder analysis) |
| Task safety | Explicitly **not** synchronised — single-task only |
| The capture-write diagnostic | **Routes by shape:** accumulation → point at `List.reduce`/`sumBy`/`count`; genuine shared state → point at `Mutable<T>` |
