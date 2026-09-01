# `using` — the scope guard (DEC-364 / DEC-203)

> **Status: BUILT 2026-07-31.** Ruling identity + status = the DEC-364 row in
> `docs/research/full-audit/raw/C-decisions.md`; the build record (including the two pre-existing bugs
> this feature exposed) is the "DEC-364 BUILT" section at the end of that file. This file stays the
> canonical design (Invariant 19).
>
> **Three things below were corrected BY the build and are marked inline:** the interface's import path
> is `Core.ClosableModule` (not `Core.Closable`) per DEC-278's namesake rule; the radius is **34**
> sites, not 35; and there is **one** shared editor grammar, not three. **LIFT is deliberately NOT
> done** — see "Beyond the 35".

## The ruling, restated

`using (T h = expr) { … }` closes `h` at **every** exit path from the block, including a throw. Build it
**before** DEC-347 (`FileSystem.lines`) and DEC-348 (`withLock`), so those land on a real release
guarantee rather than hand-rolled `try`/`finally` — which is what every open slice does today.

`defer` was re-examined under DEC-371 (its "no PHP analog" reason was struck as illegitimate) and is
**still rejected on its real merits**: LIFO ordering plus capture timing is a genuine footgun, and `using`
covers the same need with block-scoped clarity. One mechanism beats two. So this is the whole scope-guard
surface — there is no second slice waiting behind it.

## Surface

```phorj
using (Connection db = new Connection(":memory:")) {
    db.exec("INSERT INTO t VALUES (1)");
}   // db.close() has run — on the normal path, on a `return`, and on a throw
```

- The type is **mandatory** and must implement `Core.ClosableModule`'s `Closable` (**corrected from
  `Core.Closable`**: DEC-278 gives a namesake module the `Module` suffix, and a module leaf equal to its
  one bound type is exactly that case). Rejecting anything else at compile time is
  what makes the `close()` call total; there is no runtime "does it have close()?" probe.
- The binding is scoped to the block, so it cannot outlive its own release. Invariant 12's naming applies:
  the contract is `Closable` (PascalCase) with one method `close()` (camelCase).

## AST

```rust
Using {
    ty: Type,
    name: String,
    init: Expr,
    body: Vec<Stmt>,
    span: Span,
}
```

**No new `Op` and no new `Value`.** It lowers to the shape a hand-written guard already has —
`try { … } finally { h.close(); }` — so all three backends reuse `Stmt::Try`'s machinery and the PHP leg
is a literal `try`/`finally`. That keeps Invariant 1 cheap: there is no new failure ordering to reconcile,
because the ordering is `Try`'s, which is already differentialled.

## Blast radius — MEASURED, not estimated

Adding the variant and letting `rustc` enumerate gives **34 sites** [Verified 2026-07-31 during the
build: `cargo check --all-targets --all-features` after adding `Stmt::Using` reported `E0004` at 34
distinct locations. The 2026-07-30 estimate of 35 was one high]. Grouped:

| group | what each needs |
|---|---|
| `ast/walk.rs` ×4 | recurse into `init` + `body` (free-vars, `this`-use, task-use, sub-expressions) |
| 15 checker rewriters | the same total-walk arm they give `Stmt::For` — recurse `init` and `body` |
| `checker/stmt/core.rs` ×2 | the real work: scope the binding, require `Core.Closable`, check the body |
| `compiler/stmt/core.rs` | emit as `Try` with a synthesized `finally` calling `close()` |
| `format/printer/` ×3 | print it back idempotently (the formatter sweep gates on this) |
| `cli/rewrite_new.rs`, `checker/rewrite_new.rs` | `new`-unwrapping inside `init` |

**This count is the receipt for DEC-356.** Before that slice landed this morning, most of those checker
walks carried `leaf => leaf`, so `Stmt::Using` would have compiled fine and been **silently passed
through** — generics erasure, DI desugaring, html resolution and UFCS would all have skipped the inside of
a `using` block, and nothing would have failed until a user hit it. The 35 compile errors are the feature
working as designed.

## Beyond the 35 — what the compiler CANNOT enumerate

- **Parser, NOT the lexer** — `using` is contextual (DEC-364.1 below), so no keyword is added and no
  identifier is reserved; the decision happens at statement position with one lookahead.
- **`Core.ClosableModule`** in the prelude, plus `Connection` declaring it. ✅ **DONE** — `Connection
  implements Closable`, and the deferral note that stood at `src/ext/database/prelude.rs` is replaced.
  (`Statement`/file handles are separate surfaces; neither has a `close()` today.)
- **Lift** (Invariant 17): ❌ **DEFERRED, with its reason.** The lifter has no `try`/`catch`/`finally`
  **at all** — the lift parser rejects the keyword and the lift printer lists `try` as outside its
  subset — so raising a PHP `try`/`finally`-with-one-`close()` back to `using` is blocked on the whole
  exception family entering the lift subset. `Stmt::Using` sits behind that same documented boundary.
  This is a lifter-wide gap, not a `using` gap; it is recorded in the register and `KNOWN_ISSUES.md`.
- **LSP + BOTH editors, same change** (Invariant 17's 100% rule): ✅ **DONE**, and cheaper than
  estimated — there is **ONE** grammar, not three: `editors/vscode/syntaxes/phorj.tmLanguage.json` is
  consumed by the JetBrains path too (it loads that directory as a TextMate bundle). The LSP work was
  the keyword list plus `lsp::scope::collect_bindings`, which had a catch-all and saw neither the
  `using` binding nor anything declared inside its body.
- **Examples** (Invariant 9): a runnable `examples/` program + a README entry, auto-gated by the
  differential glob.

## Definition of done — ALL MET except (3)'s lift half, deferred above with its reason

1. All 34 arms, with the checker enforcing `Closable` and rejecting non-conforming types.
2. `close()` runs on normal exit, `return`, `break`/`continue` out of the block, AND a throw — a
   differential case per path, since "every exit path" is the entire promise.
3. Byte-identical across `run` / `--tree-walker` / transpiled PHP; lift round-trips.
4. LSP + both editors + grammars updated in the SAME change.
5. Example + README entry; `FEATURES.md` row; spec status flipped here.
6. Full ALL-FEATURES gate green.

**Achieved:** every exit path verified byte-identical on all three legs; `E-USING-NOT-CLOSABLE`,
`E-USING-INFER` and `E-USING-CLOSE-THROWS` each have a test that fires AND a paired accepting case;
the DEC-364.1 pair (`int using = 1;` / `using (T h = …)`) is tested, plus `using(1);` staying a *call*
(the gate checks the header's `Type name =` shape, not merely the `(`, so a function named `using` keeps
working — the discipline `at_discard` documents for itself).

**One obligation the interface does NOT cover, found while building:** conformance compares parameters
and the return type but **not** `throws`, so an implementor may legally declare
`function close(): void throws IoError`. That call is synthesized into a `finally`, so without a check a
*checked* fault would escape a function that neither catches nor declares it. `E-USING-CLOSE-THROWS`
applies the rule DEC-257 already ruled for a throwing iterator's `foreach`: caught by an enclosing
`try`, or declared by the enclosing function.

## RULED 2026-07-30 (DEC-364.1) — `using` is CONTEXTUAL, not reserved

Significant only immediately before `(`; reserves nothing. Reserving would break any identifier spelled
`using`, and DEC-344 is simultaneously *de*-reserving `main` — a new reserved word would cut against the
direction the project is already moving. Cost: one parser lookahead branch.

**So the lexer gains NO keyword.** `using` stays an ordinary identifier token and the parser decides at
statement position. Both of these must parse, and each needs a test — the pair is this decision's
regression surface:

```phorj
int using = 1;                      // still a legal identifier
using (Connection db = open()) { }  // the scope guard
```
