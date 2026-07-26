# P0 — Variable shadowing in ANY nested block breaks the Invariant-1 byte-identity spine

**Found:** 2026-07-25, inline (while researching the developer's question #8 "visibility/access in blocks").
**Severity: P0** — silent WRONG OUTPUT on the PHP leg. Violates **Invariant 1** (`phg run` ≡
`phg run --tree-walker` ≡ transpiled PHP under a real `php`, identical stdout) — the project's #1 delivery
invariant. Not a fault-message mismatch: the program prints different *values*.

## Reproducer (minimal, current syntax)

```phorj
package Main;
import Core.Output;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;
#[Entry(kind: EntryKind.Cli)]
function main(): void {
    int a = 1;
    if (true) { int a = 2; Output.printLine("in={a}"); }
    Output.printLine("out={a}");
}
```

| Backend | Output |
|---|---|
| `phg run` (VM) | `in=2` / **`out=1`** |
| `phg run --tree-walker` (oracle) | `in=2` / **`out=1`** |
| `phg transpile` → `php-8.5.8` | `in=2` / **`out=2`** ← WRONG |

[Verified: all three legs executed; `php-8.5.8` at `/stack/tools/phpbrew/php/php-8.5.8/bin/php`]

## Blast radius — every nested-block form, verified individually

| Shadowing site | vm | tree-walker | php | Verdict |
|---|---|---|---|---|
| bare block `{ … }` | `out=1` | `out=1` | `out=2` | **DIVERGES** |
| `if (…) { … }` body | `out=1` | `out=1` | `out=2` | **DIVERGES** |
| `for (…;…;…) { … }` body | `out=99` | `out=99` | `out=1` | **DIVERGES** |
| `while (…) { … }` body | `out=1` | `out=1` | `out=5` | **DIVERGES** |
| **function parameter** shadowed in an inner block | `outer v=7` | `outer v=7` | `outer v=42` | **DIVERGES** |
| 3-deep nested blocks | `d3=3,d2=2,d1=1` | same | `d3=3,d2=3,d1=3` | **DIVERGES** |
| sibling blocks, no live outer shadow | `b1=1,b2=s` | same | same | OK (no divergence) |

[All Verified — probe files under `scratchpad/probe-blocks/`, each run on all three legs]

## Root cause [Verified]

Phorj has **true lexical block scoping**: an inner `int a = 2;` creates a NEW binding, and reading `a`
after the block sees the outer one. Proof that scoping is real, not accidental: a block-local is
*unreachable* after its block — `{ int b = 2; } Output.printLine("{b}")` is rejected with
`[E-UNKNOWN-IDENT] unknown identifier 'b'`.

**PHP has no block scope** — only function scope. The transpiler emits the phorj local's name verbatim as
a PHP variable, so the inner declaration becomes a plain **assignment to the same `$a`**, clobbering the
outer binding. Emitted PHP for the reproducer:

```php
function main(): void {
    $a = 1;
    if (true) { $a = 2; echo "in={$a}", "\n"; }   // <-- same $a, no new scope
    echo "out={$a}", "\n";
}
```

So the divergence is structural: **any** shadowing of a still-live outer local/param is mistranspiled.

## Why the gate never caught it [Verified]

`tests/differential.rs` globs `examples/**/*.phg`, so the spine is only enforced over shipped examples —
and **no example shadows a variable in a nested block**. The bug lives entirely in the untested gap.
This is the more important structural lesson: the differential harness's coverage is exactly the example
corpus, so a whole language feature (block scoping) has zero spine coverage.

## No prior record [Verified: grepped register + UNIFIED-SPEC + KNOWN_ISSUES]

Shadowing appears in the register only in unrelated senses: `DEC-027` `E-SHADOW-IMPORT` (a binding may not
shadow an *imported qualifier*), `DEC-064` (trait-ctor shadowing warnings), `C-4` (the `text`/`string`
naming rationale). Register line 330 notes a `W-BINDING-SHADOWS-TYPE` warning "was possible and was not
chosen — **highest silent-bug surface**". **Local-shadowing-vs-PHP-function-scope is unrecorded** — this is
a new finding, not known debt.

## Fix options (developer rules — Invariant 15)

1. **Alpha-rename shadowed locals in the transpiler (RECOMMENDED).** When emitting a block-scoped
   declaration whose name is already live in an enclosing scope, emit a deterministic unique PHP name
   (e.g. `$a__b1`) and rewrite references within that scope. Restores byte-identity, keeps the language
   surface untouched, zero runtime cost, and it's the standard technique for targeting a scope-less
   language. Cost: the transpiler must track a scope stack + rename map (it already tracks `locals:
   Vec<HashSet<String>>` and `local_kinds`, so the scaffolding exists). Must be deterministic
   (Invariant 10) and must not collide with user names — the existing `$__phorj_` reserved prefix
   convention gives a safe namespace.
2. **Reject shadowing with a new checker error `E-SHADOW-LOCAL`.** Simplest, fully sound, and arguably
   better style (many linters ban shadowing). But it *removes* a working language capability that the
   Rust backends already implement correctly, and it is a breaking surface change. Rust/C#/Kotlin all
   permit shadowing, so this makes phorj stricter than its peers.
3. **Warn (`W-SHADOW-LOCAL`) and keep the divergence** — REJECTED as an option under **Invariant 14's**
   "silent semantic downgrade: FORBIDDEN" (this is worse than silent: it's wrong output).
4. Wrap blocks in PHP closures — rejected: changes by-reference semantics, heavy runtime cost.

**Recommendation: Option 1**, plus (regardless of choice) **a differential example that shadows in every
block form**, so the spine covers block scoping permanently. If the developer prefers Option 2, the
example becomes a negative/conformance test instead.

## Related follow-up discovered in the same probe

> ## ⚠ CORRECTED — THE CONCLUSION BELOW WAS **WRONG**. Do not act on it.
>
> **Both loop forms are live.** Each keyword is locked to exactly one separator: **`for` … `in`** and
> **`foreach` … `as`**. Only the *crossed* combinations are parse errors — which is all my probe actually
> demonstrated. Verified by running all four: `for (int x in xs)` ✅ runs · `foreach (xs as int x)` ✅ runs ·
> `foreach (int x in xs)` ✗ *"expected 'as' after the foreach iterable"* · `for (xs as int x)` ✗
> *"expected 'in' in for-loop header"*. Independently re-confirmed by the certification pass.
>
> The retirement the developer remembers is **DEC-248** (it ruled `for (T x in xs)` retired behind
> `E-RETIRED-FORIN`) — and that code has **0 occurrences in `src/`**, i.e. the ruling was never built.
> Register **Conflict C-2** is the same open question. Census: **87 `for…in` vs 8 `foreach…as`**.
>
> Canonical answers: `K-inline-findings.md` **K-1** and the completeness register **§1 #7**; the ruling is
> **GR-5 / DEC-343**. *(Kept visible rather than deleted so the reasoning failure stays auditable: a single
> failing spelling proves that spelling invalid — never that its alternative is "the survivor".)*

~~`for (xs as int x)` is a **parse error**: `expected 'in' in for-loop header`. So the surviving loop form is
`for (item in collection)` and the `as` form is retired — this answers the developer's question #7
directly (his recollection was inverted).~~ **[RETRACTED — see the correction above]**
