# Block-scope shadowing — the redeclaration rule (DEC-339, RULED 2026-07-26)

> **Status:** RULED by the developer 2026-07-26, **not yet built**. Canonical home for the rule
> (Invariant 19: one canonical place). The decision *identity + status* is the DEC-339 row in
> `docs/research/full-audit/raw/C-decisions.md`; the *original P0 evidence* is
> `docs/research/2026-07-25-global-review/P0-block-shadow-byte-identity.md` (immutable);
> **this file is the rule**.

## Why

Phorj has true lexical block scoping. PHP has function scoping only. The transpiler emits a nested
declaration as a plain `$a = …`, which clobbers the enclosing binding — so a program that is correct
under both Rust backends produces **different output** under the transpiled PHP leg. That breaks
Invariant 1 (byte-identity spine).

It escaped the gate because `tests/differential.rs` globs `examples/**/*.phg` and **no example
shadows a variable** — block scoping had zero spine coverage.

The 2026-07-26 probing session widened the known blast radius from the 6 shapes recorded in the P0
report to **10**, adding four declaration forms nobody had considered: the `for…in` loop *variable*
itself, `match` arm bindings, binding-`if` bindings, and `catch` bindings. One shape (nested `for`
reusing a counter) **changes control flow**, not just printed output.

## THE RULE

> A declaration is **rejected** if its name is already bound by a **live local or parameter binding**
> in the same function — whether in the **same** scope or an **enclosing** one.
>
> - **Class fields are never local bindings**, so they never conflict.
> - **A lambda starts a new function**, so enclosing names do not reach into it.

Scopes are opened by: the function body, a bare block, an `if`/`else` body, a `while` body, a `for`
header+body, a `for…in` header+body, a `match` arm, a `catch` clause, and a binding-`if` body.

The rule was chosen over alpha-renaming in the transpiler because shadowing is not one construct but
**ten declaration forms**: a renamer must be correct in every one of them, forever, while the rule
makes all ten *unrepresentable* at a single chokepoint. It also protects any future backend or
codegen path for free.

## Enforcement site

**The checker** (`phg check`) — not the transpiler. That is the one pipeline every surface shares, so
one implementation reaches `run`, `run --tree-walker`, `transpile`, `build --php`, the LSP, the
formatter and the test runner. Invariant 17 pins `phg check` ≡ LSP diagnostics, so the editor squiggle
comes from the same code that fails the build. A transpiler-only guard would let `phg run` accept a
program that cannot transpile — a worse asymmetry than the bug being fixed.

## The complete case list

Every row below was executed on all three legs — `target/release/phg` @ `d77eeaf` vs `php-8.5.8`
(the transpile-floor oracle). "vm" and "tw" agreed on every single row; only the PHP leg diverges.

### REJECTED — 1-10 diverge today (correctness), 11-14 are byte-identical (hygiene)

| # | Case | Example | Today (vm/tw → php) |
|---|---|---|---|
| 1 | `if` block shadows an outer local | `int a=1; if(true){int a=2;}` | `1` → **`2`** |
| 2 | `while` body shadows an outer local | `int a=1; while(…){int a=9;}` | `1` → **`9`** |
| 3 | Nested bare blocks shadow an outer local | `int a=1; {{{int a=3;}}}` | `1` → **`3`** |
| 4 | Nested `for` reuses the counter name | `for(mutable int i…){for(mutable int i…){}}` | `6` → **`3`** — **changes iteration count** |
| 5 | `for` counter shadows an outer local | `int i=42; for(mutable int i=0;…)` | `42` → **`3`** |
| 6 | `for…in` body local shadows an outer local | `int x=7; for(int v in …){int x=v;}` | `7` → **`2`** |
| 7 | `for…in` loop **variable** shadows an outer local | `int v=77; for(int v in [1,2]){}` | `77` → **`2`** |
| 8 | `match` arm binding shadows an outer local | `float r=100.0; match(s){Circle(r)=>…}` | `100` → **`2`** |
| 9 | binding-`if` shadows an outer local | `int x=100; if(var x=j){…}` | `100` → **empty** — clobbers even when the bind **fails** |
| 10 | `catch` binding shadows an outer local | `int e=7; try{…}catch(BoomError e){…}` | `7` → **exception dump + stack trace + absolute path** |
| 11 | Same-scope redeclaration | `int a=1; int a=2;` | identical (`2`) — rejected as the "meant to assign, accidentally re-declared, possibly at another type" typo class |
| 12 | Local redeclares a parameter | `function f(int a){ int a=2; }` | identical (`2`) — the argument is silently discarded |
| 13 | Local redeclares a **non-promoted** ctor param | `constructor(int seed){ int seed=5; }` | identical — same as 12 |
| 14 | Local redeclares a **promoted** ctor param | `constructor(public int myVar){ int myVar=5; }` | identical — see note below |

### ACCEPTED — all verified byte-identical, all must keep working

| # | Case | Example | Why it is safe |
|---|---|---|---|
| 15 | Sibling blocks reuse a name (even at different types) | `{int a=1;…} {string a="x";…}` | The first binding is dead — "it can be declared after". |
| 16 | Sequential `for` loops reuse the counter | `for(mutable int i…){} for(mutable int i…){}` | Same; ubiquitous idiom. |
| 17 | Sibling `match` arms reuse a binding name | `match(s){Circle(v)=>…, Square(v)=>…}` | Arms are siblings; never both live. |
| 18 | Sibling binding-`if`s reuse a name | `if(var x=a){…} if(var x=b){…}` | Same. |
| 19 | Lambda param shadows an outer local (expr body) | `int x=100; …function(int x) => x*2` | Emits PHP `fn($x) => …`; arrow-fn params shadow correctly and auto-capture excludes params. |
| 20 | Lambda param shadows an outer local (block body) | `…function(int x): int { … }` | Capture list is `free_vars` **minus params** (`src/transpile/expr.rs:474+`). |
| 21 | Nested lambda params shadow each other | inner `function(int v)` inside outer `function(int v)` | A new function boundary each time. |
| 22 | Method local named like a class field | field `n` + `int n = 99` in a method | `this.n` is **mandatory** (Invariant 12) — the field is not a local binding, so nothing is shadowed. |
| 23 | Loop body re-declares per iteration | `for(…){ int a = i; }` | Safe **because** `int a;` (uninitialized) is a **parse error** — a declaration always initializes, so a stale PHP `$a` is never readable. |

**Row 14 vs row 22 — why they differ.** A promoted ctor param is *also* still a live parameter,
bare-readable in the constructor body (`constructor(public int myVar) { … myVar … }` prints the value
on all three legs). So inside the constructor there **is** a live binding named `myVar`, and a local
of that name redeclares it (row 12). In a *method*, by contrast, the field name is not a local
binding at all — nothing is shadowed, and rejecting it would poison every field name inside every
method of the class for zero correctness gain. The dividing line is exactly the rule's own words:
*is there a live local binding with that name?*

Rejected alternatives for row 14, recorded so they are not re-litigated:
- *Carve promoted params out of the rule* — byte-identical and free, but the rule gains an exception.
- *Make promoted params field-only inside the body* (bare `myVar` stops resolving) — keeps the rule
  exception-free, but **breaks a form that works today** and that every peer language allows.

**Row 23's dependency is load-bearing.** If uninitialized declarations were ever admitted (`int a;`),
loop-body declarations would stop being safe and would need their own rule. Any future proposal to
allow them must revisit this file.

## Definition of done

1. The rule implemented in the checker, one diagnostic code, spans anchored at the **offending
   declaration** with a secondary note pointing at the binding it collides with.
2. A differential example per rejected shape 1-10, so all ten gain permanent spine coverage
   (Invariant 9 — the reason the P0 survived is that no example exercised them).
3. Faults 11-14 are compile-time rejections, so they cannot be runnable examples — capture them in an
   `examples/README.md` entry instead (Invariant 9's stated carve-out).
4. `phg check` ≡ LSP diagnostics verified for the new code (Invariant 17 / DEC-252).
5. The lifter kept honest — see the adjacent bug below.

## Adjacent bugs found while probing (separate rows, not folded into DEC-339)

- **The lifter emits non-compiling phorj for ordinary function-scoped PHP.** A `$b` first assigned
  inside an `if` and used after it lifts to `mutable var b = 5;` *inside* the block plus `b = 7;`
  outside → `E-ASSIGN-UNKNOWN` + `E-UNKNOWN-IDENT`. This is the same PHP-function-scope-vs-phorj-
  block-scope insight from the other direction: the lifter must **hoist** the declaration to the
  outermost use. Live Invariant-17 gap.
- **`implements Error` + `getMessage()` → PHP fatal.** Such a class transpiles to one extending
  `Exception` and overriding **`final Exception::getMessage()`**, so the PHP leg dies at runtime while
  both Rust backends run the program fine. `src/checker/common.rs:432` guards builtin *class-name*
  collisions but not final-*method* collisions. Its own Invariant-1 violation.
