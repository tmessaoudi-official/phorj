# UFCS completion, import-gating, and the LSP-as-expert-companion bar (DEC-342 / DEC-346 / DEC-375, RULED 2026-07-26)

> **Status:** RULED by the developer 2026-07-26, **not yet built**. Canonical home for these rules
> (Invariant 19). Identity + status: the DEC-342, DEC-346 and DEC-375 rows in
> `docs/research/full-audit/raw/C-decisions.md`.

## The standing bar (DEC-375) — developer-ruled

> **The LSP and the editors are the expert companion.** They must be **flawless and fluent**: complete
> and suggest wherever a completion is possible, propose the imports a completion requires, and surface
> a diagnostic that names the fix. Anything an expert user would know, the editor should offer.

This is a **quality bar on every editor-facing slice**, not a one-off feature. It composes with
Invariant 17 (`phg check` ≡ LSP diagnostics — DEC-252) and DEC-181 (editors updated in the same change).

## The rule already enforced — and the gap that is not

**Import-gated UFCS is already the checker's behaviour.** Verified 2026-07-26:

```phg
string line = "  hi  ";
Output.printLine("[{line.trim()}]");
```
- **without** `import Core.String;` → `type error: type `string` has no method `trim``
- **with** it → `[hi]`

So the *language* rule the developer asked for ("to use `line.` the module must be imported") **exists**.
What is missing is the editor side and the message quality.

## Receiver types and the modules that contribute UFCS members

Native counts verified 2026-07-26 by registry grep. A member becomes available as `x.member(…)` when the
module is imported **and** its first parameter accepts the receiver type.

| Receiver | Contributing modules | Examples (import required) |
|---|---|---|
| `string` | **Core.String** (45), Core.Validation, Core.Path, Core.Regex, Core.Hash, Core.Encoding, Core.Conversion (20) | `line.trim()`, `email.isEmail()`, `p.basename()`, `s.matches(re)`, `s.sha256()`, `s.base64()`, `s.toInt()` |
| `List<T>` | **Core.List** (44) | `xs.map(f)`, `xs.filter(p)`, `xs.reduce(0, f)`, `xs.sort()` |
| `Map<K,V>` | **Core.Map** (14) | `m.keys()`, `m.merge(other)`, `m.get(k)` |
| `Set<T>` | **Core.Set** (12) | `s.union(t)`, `s.contains(x)` |
| `bytes` | **Core.Bytes** (6) | `b.toString()`, `b.length()` |
| `int` / `float` | **Core.Math** (37), Core.Conversion (20) | `n.abs()`, `x.sqrt()`, `n.toString()` |
| `decimal` | **Core.Decimal** (3) | `d.round(2)` |
| `Json` | **Core.Json** (5) | `j.stringify()` |
| `T?` | **Core.Option** (6) | `maybe.orElse(d)` |
| `Result<T,E>` | **Core.Result** (8) | `r.unwrapOr(d)` |
| Time / Csv / Ini / Uri | their own modules | `t.format(f)`, `row.toCsv()` |
| **User types** | the user's own free functions — these **already win** over a native of the same name (`src/checker/calls/ufcs.rs:38-44`) | `order.total()` from `function total(Order o): decimal` |

## What gets built (DEC-342)

1. **Receiver completion, import-aware.** `x.` unions the members of **every imported module** whose
   first parameter accepts the receiver's type — for **every** row above, not just `string`. Today `line.`
   returns **0 items**.
2. **Import-gating in the other direction too.** `String.` must stop suggesting members when
   `Core.String` is not imported. Both directions are one ruling (they are the same bug from two sides).
3. **A diagnostic that names the fix.** `line.trim()` without the import currently says *"type `string`
   has no method `trim`"* — misleading, because the method exists. It must say **``trim` exists in
   `Core.String` — add `import Core.String;`"** and the LSP must offer that as a **quick-fix code
   action** (DEC-375's bar: propose the import, don't just report the error).
4. **Fix the span.** The error anchors at `1:10` (`package Main;`) instead of the call site.

## UFCS ambiguity across modules — RULED (A)

Import-gating plus completion makes collisions reachable: several modules can contribute to the same
receiver type, so two imported modules could both define e.g. `hash` for `string`. The checker is
**single-overload only** today (`ufcs.rs:38-40`; multi-module UFCS was deferred as F-004).

> **Ruled: ambiguity is an ERROR that names both candidates and the qualified escape.**
> e.g. *"`hash` is ambiguous: `Core.Hash.hash` or `Core.Cryptography.hash` — call it qualified."*

Rejected: **first-import-wins** (silent and order-dependent — exactly the bug class this whole agenda has
been removing), and **hard-error on any second candidate** (rejects legitimate code).

## UFCS migration (DEC-346)

**Ruled: tooling first, then the 391 zero-judgement sites.** Order: DEC-342's completion + the import
hint + a formatter lint, *then* migrate. Corpus: **2223** qualified call sites in examples.

> **`Output.printLine` stays QUALIFIED — developer-ruled.** It is **1231 of the 2223 sites (55.4%)** and
> the single most-read line in every example. UFCS reads well when the receiver is the subject
> (`line.trim()`, `xs.map(…)`); for output the **sink** is the subject, so `"hello".printLine()` inverts
> it. No codemod touches `Output.printLine`.

## Definition of done

1. Receiver completion for every row in the table, gated on imports, driven by the same catalog the
   checker uses (never a second source of truth).
2. `String.`-style module completion gated on the import.
3. The "exists in `Core.X` — add the import" diagnostic **plus** the LSP quick-fix, span at the call site.
4. Ambiguity error naming both candidates.
5. `phg check` ≡ LSP diagnostics re-verified for every new code (Invariant 17 / DEC-252), and both
   editors updated in the same change (DEC-181).
6. Migration of the 391 zero-judgement sites, with `Output.printLine` untouched.
