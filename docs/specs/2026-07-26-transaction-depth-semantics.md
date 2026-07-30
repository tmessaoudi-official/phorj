# Transaction depth semantics — auto-rollback unwinds to the ENTRY depth (DEC-340, RULED 2026-07-26)

> **Status:** RULED 2026-07-26; **items 1, 2, 4, 5 BUILT 2026-07-29. Item 3 (the PHP leg) is BLOCKED on
> a question the developer must answer — see "Item 3 is blocked" at the bottom of this file.** Canonical home for the rule
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


## Item 3 is blocked — the PHP leg needs a Ladder ruling first (found 2026-07-29)

Definition-of-done item 3 asks for the savepoint helper "with a test that nested begin/rollback composes
under PDO". Building it surfaced a conflict this file could not have known about.

**`Core.Database` is not merely unimplemented on the PHP leg — it is deliberately QUARANTINED.**
Transpiling any program that imports it is a clean `E-TRANSPILE-DB` Ladder case-2 error
[Verified: `phg transpile examples/database/transactions.phg` → *"cannot transpile a program importing
`Core.Database` … native-only: live database I/O cannot be byte-identical across the phorj drivers
and PHP PDO, so transpiling it is refused rather than silently diverging (THE LADDER RULE)"*]. That
quarantine was ruled deliberately (register ~:1005, leg 2 of Invariant 14), and its stated reason —
live DB I/O is not byte-identical — is unrelated to savepoints.

So this file's description of the emitter as "a literal placeholder comment" is accurate but incomplete:
the placeholder was **unreachable**, because the quarantine fires first. Emitting a correct helper does
not make the leg work; only LIFTING the quarantine would, and that is a Ladder case-2 → case-1 move for
the entire database module. Invariants 14 and 16 both put that squarely with the developer.

**What was built anyway, and why it is not dead weight.** `src/transpile/db_php.rs` now contains the
complete `__phorj_db_*` savepoint helper set — depth counter per PDO handle in a `SplObjectStorage`
(mirroring the Rust `Rc<Cell<u32>>` sharing), `begin`/`commit`/`rollback` composing via
`SAVEPOINT phorj_sp_N` with **the same savepoint names the Rust legs emit**, plus `unwind_to` and
`rollback_all`. The three `php:` emitters for `begin`/`commit`/`rollback` were repointed at it, replacing
the non-nesting `->beginTransaction()`/`->commit()`/`->rollBack()` mapping that could not express phorj's
nesting semantics at all. It is gated behind `uses_db`, which the quarantine keeps unreachable today — so
it is the prerequisite, staged and ready, not a claim that the leg works.

**The question for the developer:** does `Core.Database` stay Ladder case 2 (native-only, quarantined
— in which case item 3 should be struck from this file as unsatisfiable and the helpers either kept staged
or removed), or does it move to case 1 for the transaction/savepoint surface now that the helpers exist —
accepting that live DB I/O still cannot be byte-identical, so the differential quarantine would remain
even if the transpile error were lifted?


## Item 3, resolved as far as it can be without a ruling (2026-07-29)

The helpers are now **verified against the real oracle**, not merely written: `tests/db_savepoints.rs`
executes the transpiler's own helper source under `php-8.5.8` + PDO/SQLite and asserts nested
begin/rollback composes, that `unwind_to` restores a caller-owned entry depth (555 survives and commits),
that `rollback_all` flattens every level, that the depth counter is per-HANDLE rather than global, and
that the savepoint NAMES still match `ops_tx.rs`. The source is read from the transpiler, so the test
cannot pass against a stale copy.

**That test immediately found a defect that reading the code would not have.**
`SplObjectStorage::contains()` is **deprecated as of PHP 8.5** — which is the transpile floor — so every
depth read printed a deprecation notice onto **stdout**. Had the leg been lifted with that in place, it
would have broken byte-identity outright, in the subtlest possible way. Now `offsetExists()`.

### The case-1 move: scoped, so it is a slice rather than a vibe

The developer's instinct is that the transaction surface should become Ladder case 1. Two findings say the
destination is right but the distance is longer than it looks:

1. **The transaction surface cannot move alone.** `E-TRANSPILE-DB` fires on the IMPORT, and a transaction
   body necessarily contains `prepare`/`exec`/`query`. There is no partial lift — the unit is the module.
2. **The savepoint helper is a small fraction of the work.** The blocker is the ERROR CONTRACT. The Rust
   legs reconstruct a 7-kind taxonomy (`ConnectionError`, `ConstraintViolationError`,
   `SerializationFailureError`, `SyntaxError`, `TimeoutError`, `UniqueViolationError`, `DatabaseError`)
   and every native returns `DatabaseResult.Ok/Err` which the prelude turns into a typed throw. The PHP
   emitters are raw PDO calls with **zero** mapping. Lift as-is and `catch (UniqueViolationError)` never
   matches, and `db.transaction(fn, retries)` — which retries ONLY `SerializationFailureError` — silently
   never retries. That is the same silent downgrade Invariant 14 forbids, relocated from "no transactions"
   to "wrong error types", which is worse because it looks like it works.

**Not an obstacle:** value types. [Verified: PHP 8.5 + `pdo_sqlite` returns native `int`/`float`, not
strings — I had assumed otherwise and was wrong.] The one real value gap is `decimal`: PDO yields float
`19.99` where phorj is exact fixed-point.

**So the case-1 slice is: port `DatabaseResult` + the 7-kind SQLSTATE→kind classifier to the PHP leg,
decide the `decimal` mapping, then lift the quarantine** — after which the differential quarantine still
stands on its own separate reason (live DB I/O is not byte-identical), which is unaffected either way.


## Case-1 slice, step 1 of 3 BUILT (2026-07-29) — the SQLSTATE classifier

Developer ruled: go for case 1. Step 1 is the error contract's front half, and it is the piece everything
else depends on.

**Built:** `__phorj_db_classify(PDOException)` in `src/transpile/db_php.rs` maps a real PDO exception onto
the SAME 7-kind taxonomy the Rust drivers produce, tagged with the same `<<Kind>>` marker. That marker is
the whole mechanism: the prelude's `DatabaseError.fail` parses it, and the prelude IS phorj source, so it
already runs on the PHP leg. Driver-agnostic SQLSTATE first (Postgres/MySQL agree), then the SQLite driver
code. An unmatched error stays UNTAGGED deliberately — the prelude maps that to the base `DatabaseError`,
exactly as an unmatched Rust error does. Inventing a kind would be worse than staying generic.

**Verified against real PDO exceptions, not synthesised codes** — and that found a second real defect, in
the same spirit as the `SplObjectStorage::contains()` one. Keying "unique" on SQLite's driver code 19
mis-classified a **NOT NULL** violation as a unique violation: 19 is the GENERIC `SQLITE_CONSTRAINT`, and
the extended codes the Rust driver keys on (2067 `_UNIQUE`, 1555 `_PRIMARYKEY`) are **not exposed through
PDO's `errorInfo`**. So on SQLite the message is the only discriminator. MySQL's 1062 genuinely is
unique-specific and stays. A handler catching the wrong error type is precisely the silent-downgrade
failure this slice exists to prevent, and it would have shipped un-noticed without a real driver.

A drift guard asserts the PHP classifier can still produce every kind the Rust side tags — if a kind is
added there and not here, the PHP leg would silently degrade it to the base `DatabaseError`.

### Remaining before the quarantine can be lifted

- **Step 2 — `DatabaseResult` construction.** Every `Core.Native.Database` `php:` emitter must return
  `DatabaseResult.Ok(v)` / `Err(<<Kind>>msg)` (the enum-scoped variant classes per DEC-329.3) instead of a
  raw PDO value, wrapping its call in `try/catch` and routing the exception through the classifier above.
  ~20 emitters; mechanical, but each needs its Ok payload shape checked against the prelude's `match`.
- **Step 3 — the `decimal` mapping.** [Verified: PDO+SQLite returns native `int`/`float`, so ints and
  floats are NOT a problem — I had assumed otherwise and was wrong. But a `NUMERIC` column comes back as
  float `19.99` where phorj `decimal` is exact fixed-point.] Needs a ruling: bind/fetch decimals as TEXT
  and reconstruct exactly (matching the Postgres `::numeric` cast the driver already uses), or accept float
  on the PHP leg and disclose it.
- **Then** flip `E-TRANSPILE-DB` off. The DIFFERENTIAL quarantine stays regardless — it rests on its own
  separate reason (live DB I/O is not byte-identical across drivers), which none of this changes.
