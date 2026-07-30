# `using` — the scope guard (DEC-364 / DEC-203)

> **Status: RULED, DESIGNED, NOT YET BUILT.** Ruling identity + status = the DEC-364 row in
> `docs/research/full-audit/raw/C-decisions.md`. This file is the canonical design (Invariant 19) and the
> measured blast radius, written 2026-07-30 so the build starts from decided state instead of re-deriving
> the shape. **Nothing is half-built** — the tree is green; only this document landed.

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

- The type is **mandatory** and must implement `Core.Closable`. Rejecting anything else at compile time is
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

Adding the variant and letting `rustc` enumerate gives **35 sites** [Verified 2026-07-30: added
`Stmt::Using`, collected every `E0004` location, then reverted]. Grouped:

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

- **Lexer:** `using` becomes a keyword. Check the de-reservation question first (DEC-344 de-reserves
  `main`) — a new reserved word is a breaking change for any identifier named `using`.
- **`Core.Closable`** in the prelude, plus `Connection`/`Statement`/file handles declaring it. The DB
  prelude already anticipates this (`src/ext/database/prelude.rs:396` names `using`/`Closable`).
- **Lift** (Invariant 17): PHP has no `using`, so the lifter must recognise the `try`/`finally`-with-a-
  single-`close()` shape and raise it back. A feature that runs but does not lift is not done.
- **LSP + BOTH editors, same change** (Invariant 17's 100% rule): 8 LSP surfaces, 3 editor grammar files.
- **Examples** (Invariant 9): a runnable `examples/` program + a README entry, auto-gated by the
  differential glob.

## Definition of done

1. All 35 arms, with the checker enforcing `Closable` and rejecting non-conforming types.
2. `close()` runs on normal exit, `return`, `break`/`continue` out of the block, AND a throw — a
   differential case per path, since "every exit path" is the entire promise.
3. Byte-identical across `run` / `--tree-walker` / transpiled PHP; lift round-trips.
4. LSP + both editors + grammars updated in the SAME change.
5. Example + README entry; `FEATURES.md` row; spec status flipped here.
6. Full ALL-FEATURES gate green.

## Open question for the developer, before the build starts

**Is `using` a reserved word, or contextual?** Reserving it is simpler to implement and matches C#, but it
breaks any existing identifier spelled `using` — and DEC-344 is simultaneously *de*-reserving `main`, so
the project is currently moving in the opposite direction. A contextual keyword (only significant before
`(`) costs parser lookahead but reserves nothing. Not ruled; do not decide it in passing.
