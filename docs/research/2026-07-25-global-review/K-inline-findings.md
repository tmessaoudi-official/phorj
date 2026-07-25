# K — Inline findings (orchestrator's own probes, 2026-07-25)

Everything here was probe-verified with `target/release/phg` + `php-8.5.8`. Separate files hold the two
biggest inline results: `P0-block-shadow-byte-identity.md` and `F-block-visibility-research.md`.

---

## K-1 — Direct answer to developer question #7 (loop syntax) — **NOTHING was retired; both forms live**

> *"Did we not retire `for .. in` and kept only `for .. as`?? or did we retire `foreach .. in`? remind me?"*

⚠ **CORRECTED.** My first pass concluded "`as` was retired" — that was **WRONG**, produced by crossing the
keyword with the wrong separator. Agent E caught it; I then re-verified directly. The true rule:

**Both loop forms are live, and each keyword is locked to ONE separator.** [Verified — all four probes run]

| Form | Result |
|---|---|
| `for (int x in xs)` | ✅ runs (prints `1`,`2`) |
| `foreach (xs as int x)` | ✅ runs (prints `1`,`2`) |
| `foreach (int x in xs)` | ✗ `parse error: expected 'as' after the foreach iterable (e.g. `foreach (xs as x)`)` |
| `for (xs as int x)` | ✗ `parse error: expected 'in' in for-loop header` |

So: **`for` … `in`** and **`foreach` … `as`**. Neither was removed; only the *cross* combinations are errors.
(Credit: both error messages are genuinely good — the `foreach` one even prints the correct form as a hint.)

**The retirement the developer is remembering is real but UNBUILT:** DEC-248 ruled `for (T x in xs)` retired
behind a new `E-RETIRED-FORIN` code — and `grep -rn 'E-RETIRED-FORIN' src/` returns **0 hits** [Verified].
Its `foreach` half did ship. Register **Conflict C-2** is this same open question, unadjudicated since 06-25.

**Census makes the decision awkward:** examples use **87 `for…in` vs 8 `foreach…as`** [Verified] — i.e. the
form DEC-248 ruled *retired* is the one the corpus overwhelmingly teaches. That is a strong argument for
amending DEC-248 to "keep both" rather than executing the retirement, but it is the developer's call
(Invariant 15).

*Process note:* this is the second time tonight a hasty single-probe conclusion was wrong (see K-4). Both
were caught. A single failing probe proves that ONE spelling is invalid — never that its alternative is
"the surviving form".

---

## K-2 — Dual return-type syntax: `->` is retired-per-spec but still fully accepted [Verified]

`function main() -> void` and `function main(): void` **both type-check clean**. The spec already knows:
`docs/specs/UNIFIED-SPEC.md:162` — *"Return annotation … **superseded**: canonical `: T`; `->` retired
(W2-4, parser-reject pending)"*, and `MASTER-PLAN.md:1556` tracks **W2-4** as *"superseded by UA-1.5's ruled
sequence (docs first → parser-reject → individual fixes)"*.

**So this is TRACKED debt, not a new bug.** The value I can add is *quantifying the cost*, which was not
recorded anywhere:

| Surface | `->` occurrences | Files |
|---|---|---|
| `.phg` sources | **87** | `examples/guide` (26), `examples/web` (7), `selftest` (1) + comment prose |
| `.rs` inline test fixtures | **2 068** | **90 files** (worst: `checker/tests/generics.rs` 46, `checker/tests/overloading.rs` 45) |

**Key enabler nobody has recorded: `phg format` ALREADY normalizes `->` → `:`.** [Verified: ran
`phg format` on an arrow-form file; output came back `function main(): void`]. So the `.phg` half of the
migration is a mechanical formatter sweep, and the pre-commit hook already runs `.phg format --check`. Only
the 2 068 Rust-string fixtures need a scripted rewrite. That makes W2-4 substantially cheaper than its
"parser-reject pending" status suggests — worth surfacing as a ready-to-schedule slice.

⚠ **Caveat before scheduling:** some of the 87 `.phg` hits are `->` inside *comment prose* describing
function types (e.g. `examples/web/middleware.phg:5` *"A middleware is `(Request, next) -> Response`"*) and
`selftest/faults.phg:6` (`Test.assertFaults(() -> T)`). Those are documentation of a *fn-type* arrow, a
DIFFERENT use than the return annotation, and a naive sweep would corrupt them. The spec (`:20`) notes
`HISTORICAL` blocks may keep retired syntax. **Any W2-4 execution must separate return-annotation `->`
from fn-type/prose `->` before rewriting.** [Verified: read the offending lines]

---

## K-3 — Live Rust test fixtures depend on the retired syntax (latent coupling) [Verified]

The 2 068 fixture occurrences mean **landing W2-4's parser-reject instantly breaks ~90 test files**. This
coupling is not recorded in the W2-4 entry. Notably `tests/differential.rs:265-275` — the `#[ignore]`d
W5-13 ready-gate — is itself written in the retired form (`function main() -> void`) *and* uses `var`.
Because it is `#[ignore]`d it never runs, so nothing would flag it going stale.
**Recommendation:** when W2-4 is scheduled, order it as: (1) scripted fixture rewrite, (2) parser-reject,
(3) un-ignore/refresh dormant tests — and add a cheap grep gate so no NEW `->` fixture can be added.

---

## K-4 — SELF-CORRECTION: two earlier claims of mine were wrong (recorded so they don't propagate)

I initially reported "**0** `var` declarations and **0** `->` uses in `examples/`". **Both were false** —
artifacts of running `grep … examples/` from a scratch probe directory instead of the repo root, so the
path did not exist and grep returned nothing, which I misread as "feature unused".

Corrected, from `/home/user/phorj`: **`var` appears 321 times** across `examples/` (`database/typed.phg`,
`database/mapping.phg`, `random/dice.phg`, `process/args-env.phg`, …). `var` is a **documented, well-covered
feature** — `FEATURES.md:32` *"Local type inference: `var x = …;` ✅ inferred from the initializer; still
fully static + immutable"*. **There is no Invariant-9 example gap for `var`.**

*Process lesson worth keeping:* a relative-path grep that returns zero is indistinguishable from a real
absence. Any "feature X is unused/uncovered" claim must be made from a verified absolute cwd — a
zero-result grep is **not** evidence until the path is confirmed to exist.

---

## K-5 — Confirmed-tracked (do NOT re-report as new): VM interpolation fault-line skew

A fault raised inside a `"{…}"` interpolation reports **line 1 on the VM** but the true line on the
tree-walker (probe: overflow inside `Output.printLine("{big + 1}")` → vm `line 1`, tw `line 8`).

This is **already known, disclosed, and gated**: `tests/differential.rs:250-260` documents it as **W0-5 /
H §5**, disclosed in `KNOWN_ISSUES` + G-1.1, fix scheduled **W5-13** (needs VM debug symbols / scope IP
ranges), with an `#[ignore]`d ready-gate test `interpolation_fault_line_matches_between_backends` covering
three shapes. Message, FaultKind, and exit code all agree; only the line diverges.
**Credit where due: this is exactly the right way to carry a known parity gap** — reproduced, scoped,
disclosed, and pre-wired with the test that will prove the fix. Nothing to add except that arithmetic
overflow is a 4th shape not in the ignored test's case list (trivial to add when W5-13 lands).

---

## K-6 — Positive attestations from my probes (do not re-litigate)

These were tested and are **correct**; recording them so future sessions don't re-investigate:

| Behaviour | Result |
|---|---|
| **Closure capture parity** | Captures by value at creation; emitted as PHP arrow `fn($v) => …`. Mutating the captured var after creation gives `kept=2\|kept2=0` **identically on vm, tree-walker, and php-8.5.8** ✅ |
| **Object aliasing** | `Box q = p; q.v = 7;` → `p.v=7` on both vm and php — reference semantics agree ✅ |
| **Fault line numbers on plain statements** | vm ≡ tree-walker exactly, including multi-frame stack traces (`→ inner line 5`) ✅ |
| **Immutable-by-default locals** | `int k = 0; k = k + 1;` → `[E-ASSIGN-IMMUTABLE]` **with a helpful hint** (*"declare it `mutable`"*). A real better-than-PHP win ✅ |
| **`E-UNUSED-IMPORT`** | Enforced as a hard error with an actionable message ✅ (note: `SLICE-STATE` lists a "W-UNUSED-IMPORT family" follow-up — it is an **error**, not a warning, today) |
| **`E-ATTR-TARGET`** | `#[…]` off a top-level `function`/`class` is rejected with a hint — confirms agent A's A8 (no file-level attribute grammar exists) ✅ |
| **Overflow is checked, not wrapped** | `int` overflow faults on all three legs (PHP via `__phorj_checked_add` → `OverflowException`) — a genuine PHP-beating guarantee ✅ |

---

## K-8 — Entry ceremony: `#[Entry]` costs TWO imports; minimal program is 6 lines vs PHP's 2 [Verified]

The developer asked for "syntax non-intuitive things". This is the clearest one I found by probing.

A minimal runnable phorj program:

```phorj
package Main;
import Core.Output;
import Core.Runtime.Entry;        // ← required
import Core.Runtime.EntryKind;    // ← ALSO required
#[Entry(kind: EntryKind.Cli)]
function main(): void { Output.printLine("hi"); }
```

**Both imports are mandatory, and each omission is a separate hard error** [Verified]:

| Omission | Error |
|---|---|
| no `Core.Runtime.Entry` | `Entry` is an injected `Core.Runtime` type used bare without importing it |
| no `Core.Runtime.EntryKind` | `EntryKind` is used without importing it |
| no `Core.Output` | `unknown identifier Output` |

So **4 of 6 lines are ceremony**; PHP's equivalent is 2 lines (`<?php` + `echo`). This is a direct
"better-than-PHP" tension: DEC-337 deliberately bought explicitness (killing bare magic `kind: Cli`), and
that was the right call for *clarity* — but the bill is two imports for one attribute, paid in **every
single file that runs**.

Worth noting the design is otherwise sound: `#[Entry]` genuinely frees the entry's NAME (verified —
`function startHere()` with the attribute runs fine, printing `from startHere`), and a file with no entry
gives an excellent error: *"no entry point: running needs an `#[Entry(kind: EntryKind.Cli)]` function
(DEC-331). A library or web file still type-checks and transpiles — use `phg check` / `phg transpile`"*.

**Options for the developer (not ruled):** (a) auto-inject `Core.Runtime.{Entry,EntryKind}` into scope
since they are already *injected* types (the error text literally calls `Entry` "an injected
`Core.Runtime` type"), so requiring an explicit import for a compiler-injected symbol is arguably
self-contradictory — **recommended**; (b) allow one combined `import Core.Runtime;` to cover both;
(c) keep as-is and accept the ceremony as the price of explicitness. Note (a)/(b) interact with the
`E-UNIMPORTED`/`E-INJECTED-VARIANT-BARE` machinery DEC-337 just built, so this is a real design question,
not a trivial tweak.

⚠ Related, and confirmed independently by agent E: **`main` IS still reserved** in the checker
(`checker/program/type_bodies.rs:347` forces any function named `main` into the entry signature even
without `#[Entry]`) — so the developer's finding #14 is correct, and the freedom `#[Entry]` promises is
only partial.

## K-7 — Minor finding: bad span on some UFCS type errors [Verified]

When a UFCS method is unresolved because its module is not imported, *some* diagnostics carry a correct
span and others point at line 1:

```
type error at 9:11: type `List<int>` has no method `push`      ← correct span
    b.push(2);
type error at 1:9: type `List<int>` has no method `length`     ← WRONG: points at `package Main;`
package Main;
```

Both errors came from the same file and the same cause. Severity **P2** (diagnostic quality, no
correctness impact), but it directly undercuts the "better than PHP diagnostics" goal, and a
`1:9`-anchored error is actively confusing in an editor (the squiggle lands on the package declaration).
Likely the same missing-span-propagation class as the K-5 interpolation skew, so the two may share a fix.
**Recommendation:** treat as a small diagnostics slice; find the UFCS-resolution error path that fails to
thread the call-site span and give it the receiver's span.
