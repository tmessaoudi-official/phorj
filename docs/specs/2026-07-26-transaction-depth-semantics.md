# Transaction depth semantics — auto-rollback unwinds to the ENTRY depth (DEC-340, RULED 2026-07-26)

> **Status:** RULED by the developer 2026-07-26, **not yet built**. Canonical home for the rule
> (Invariant 19). Decision *identity + status* = the DEC-340 row in
> `docs/research/full-audit/raw/C-decisions.md`.

## The bug (P1, silent data loss — reproduced live)

`db.transaction(fn)` calls `rollback_inner` exactly **once** on the throw path
(`src/ext/database/natives/wrappers.rs:132-135`), and `rollback` unwinds only the *innermost* level
(`src/ext/database/natives/ops.rs:415-419`). So a `begin()` leaked anywhere inside the closure —
including inside a helper the closure calls — consumes that single rollback, leaving the
transaction's **own** level open with its work live. The next `commit()` from unrelated code makes it
permanent, after the error handler was told the transaction rolled back.

Reproduced on both Rust backends (`target/release/phg` @ `27f08cb`, bundled SQLite `sqlite::memory:`),
row starting at `bal = 100`:

```phg
discard db.transaction(function(): int throws DatabaseError {
    run(db, "UPDATE acct SET bal = 999 WHERE id = 1")?;
    db.begin()?;                                  // LEAKED
    run(db, "UPDATE acct SET bal = 777 WHERE id = 1")?;
    throw new UniqueViolationError("abort");
});
```
```
caught — the transaction reported itself rolled back
bal right after the failed transaction = 999      <-- expected 100
bal after a later commit()             = 999      <-- expected 100, now PERMANENT
```

The single rollback discarded the inner `777` and stopped; `999` survived at the transaction's own
level and was then committed.

**Correction to the register's framing.** `docs/research/2026-07-25-completeness-register.md` §2 GR-2
describes this as "leaving an outer tx open". There is **no outer transaction** in the repro — the
leak is *inside* the closure, and the work that persists is the transaction's own. The trigger is
therefore ordinary, not an exotic nesting scenario.

## THE RULE

> **Auto-rollback unwinds to the depth recorded on ENTRY** — "restore the depth I found" — not to
> depth 0.

The entry depth is read from `conn.tx_depth.get()` before `begin_inner`; `tx_depth` is an
`Rc<Cell<u32>>` shared across every binding of the connection and every derived statement
(`src/ext/database/natives/handles.rs:82-91`).

**Why not depth 0** (the register's original recommendation, explicitly **REJECTED**): a caller that
owns an outer transaction —

```phg
db.begin();            // caller's own transaction, with its own work
db.transaction(fn);    // fn throws
```

— would have *its* transaction rolled back too, destroying work `db.transaction` was never given
authority over. That trades this bug for a rarer but worse one. Today that nesting case is handled
correctly, and entry-depth unwinding keeps it correct while still fixing the leak.

## API additions

| API | Semantics |
|---|---|
| `rollbackAll(): void throws DatabaseError` | Unwind to depth 0. For the **manual** `begin`/`commit` path, where the caller does own the outermost level. Not what auto-rollback uses. |
| `transactionDepth(): int` | Current depth (0 = no open transaction). Depth is currently **unobservable from phorj** — the native returns it and the prelude discards the payload (`ops.rs:374`), so the invariant cannot be asserted in a test or by user code. |

## The PHP leg — emit a savepoint helper (ruled)

The closure form is **not implemented on the PHP leg at all**: the emitter is a literal placeholder,
`php: |a| format!("/* db.transaction finalized in transpile slice */ {}", a[0])`
(`src/ext/database/natives/registry.rs:300`), and `begin()` maps to PDO `->beginTransaction()`
(`registry.rs:246`), which has **no nesting** — a nested `begin` throws in PDO.

**Ruled: emit a `__phorj_*` savepoint helper** so PDO composes and the depth semantics above hold on
the PHP leg too. Invariant 16 explicitly admits a `__phorj_*` helper as a legitimate tool with the
trade surfaced; the rejected alternative was to keep the quarantine behind a hard `E-TRANSPILE-*`
error. Shipping the current placeholder is not an option under Invariant 14 — a comment plus the raw
closure is precisely the "silent semantic downgrade" that rule forbids.

## Sequencing

**GR-26 / DEC-364 (`using` / `defer` scope guards) is sequenced immediately after this slice.** A
leaked `begin()` is only expressible because no scope guard exists; fixing the depth arithmetic is
necessary and closes the live data-loss bug, but it does not prevent the next leak. DEC-364's own
recommendation — sequence it before the slices that keep hand-rolling `try`/`finally` — is honoured by
putting it next rather than first, because GR-2 is live data loss today.

## Definition of done

1. Auto-rollback unwinds to the entry depth; a regression test asserts the repro above yields `100`
   twice.
2. `rollbackAll()` + `transactionDepth()` on the prelude, both exercised by `tests/db.rs` on **both**
   Rust backends (`run` ≡ `run --tree-walker`).
3. The PHP leg emits the savepoint helper, with a test that nested begin/rollback composes under PDO.
4. `examples/database/transaction-closure.phg` gains the leaked-`begin` case (Invariant 9).
5. The register's §2 GR-2 "unwind to depth 0" wording superseded by a pointer to this file.
