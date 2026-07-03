# W3-1 · Database access — design doc

> Status: **DESIGN — not yet implemented.** Contains ONE hard developer adjudication (the dependency
> fork, §6-Q1) that an autonomous session records as PENDING per invariant 15 (Adjudication Rule) and
> must NOT rule alone. Everything downstream of that fork is designed conditionally on its answer.
>
> Grounds: MASTER-PLAN §Wave-3 W3-1 (lines 590–604); §9 charter Bucket-1 "DB 10"; INVARIANTS 1/2/8/10/14;
> dependency policy (`docs/specs/2026-06-27-dependency-policy.md`); DEC-007 (Determinism Partition),
> DEC-009 (dep policy), DEC-133 (concurrency oracle-quarantine precedent); the `Transport` seam
> (`src/serve.rs:59`); the `pure:false` auto-quarantine (`tests/differential.rs:1027 uses_impure_native`);
> `Value::Channel`/`Value::Task` opaque-handle precedent (`src/value.rs:150–164`); native-module recipe
> (`src/native/file.rs`, `src/native/mod.rs`). D-Db verified **OPEN** (not yet ruled — P-plan-verdicts.md
> line 231/373; "resolved as each module is built"). FN-DB verified as a **count of 10**, "entire database
> surface absent" (M-gap-matrix line 221) — the matrix does not itemize the ten, so the surface below maps
> to the PDO core method set that constitutes those 10 rows.

---

## 0. TL;DR

- **Two modules, two tiers.** `Core.Sql` = Tier A **pure** parameterized-query *value* construction
  (byte-identity-gated, ships FIRST, needs no dependency and no adjudication). `Core.Db` = Tier B
  **impure** execution (`pure:false` → auto-quarantined from the differential, fixture-tested).
- **The fork (§6-Q1, the #1 adjudication).** There is **no std-only native DB engine path** (hand-rolling
  SQLite's file format + query VDBE is infeasible and itself a data-corruption/security hazard), and
  the policy's clause 3 **forbids** transpile-only delegation. So the fork is **binary**: *expand the
  dependency policy to admit an embeddable-storage-engine domain (SQLite first), or defer W3-1.* Every
  other option violates a stated invariant.
- **DB is Tier B (byte-identity-quarantined) but NOT ladder-quarantined-from-transpile.** Unlike
  concurrency (DEC-133, invariant 14 case-2 `E-CONCURRENCY-NO-PHP`), DB **has a faithful idiomatic PHP
  mapping — PDO** — so it transpiles cleanly; the quarantine is only from the *live byte-identity
  differential* (impure IO), exactly like `Core.File` mutation ops and `Core.Process`. This distinction
  is load-bearing: invariant 14 does **not** fire here.

---

## 1. API surface

Naming per invariant 12 (types PascalCase, fns camelCase, everything namespaced; `Core.` reserved).
Parameterized-only by construction — **there is no string-concatenation query API**, so the SQL-injection
class is removed at the type level (the LENS in MASTER-PLAN 591).

### 1.1 Types

| Type | Tier | Representation | Cloning |
|------|------|----------------|---------|
| `Query` | A (pure value) | SQL template `string` + ordered/`named` bound-param list (a plain immutable record `Value`) | value-copy (immutable; `bind` returns a new `Query`) |
| `Db` | B (handle) | **new opaque `Value` variant** — see §1.4 | shares the connection (`Rc<RefCell<..>>`), like `Value::Channel` |
| `Statement` | B (handle) | optional (prepared-statement cache) opaque handle | shares |
| `Row` | A (value) | `Map<string, Scalar?>` (column → value); typed-class decode is a later phase (§5) | value-copy |

`ResultSet` is simply `List<Row>` (= `List<Map<string, ...>>`) — no dedicated type needed.

### 1.2 `Core.Sql` — Tier A, pure builder (ships first, no dep)

Pure value construction — appears in the `examples/**/*.phg` glob, **byte-identity-gated across all three
legs** (interpreter ≡ VM ≡ PHP), because it produces a data value and touches no IO.

```phorj
import Core.Sql;

// A parameterized query is a VALUE. The text is a static template; params are separate.
let q: Query = Sql.query("SELECT id, name FROM users WHERE age > ? AND city = ?");
let bound: Query = q.bind(18).bind("Paris");        // UFCS on the Query value; each bind → new Query
// Named form (recommended default — order-independent, self-documenting):
let q2 = Sql.query("... WHERE id = :id").bindNamed("id", 42);
```

- `Sql.query(text: string) -> Query` — construct a template.
- `Query.bind(value) -> Query` / `Query.bindNamed(name: string, value) -> Query` — append a positional /
  named binding; returns a **new** `Query` (immutable; invariant-friendly).
- `Query.sql() -> string` / `Query.params() -> List` — introspection (pure; used by `Core.Db` and tests).
- **Recommendation:** ship the *parameterized-query value* first; a fluent typed `SELECT/FROM/WHERE`
  builder is deferred (§5) — building the full fluent surface now is XL scope creep and orthogonal to
  the injection-safety guarantee (which the template+params split already delivers). (§6-Q3.)

### 1.3 `Core.Db` — Tier B, execution (behind the dep adjudication)

All `pure:false` → the whole module is quarantined from the differential (`uses_impure_native` derives
the impure set from the flag — no harness edit needed). Errors surface as **faults** (`E-DB-*`
`FaultKind`), matching PDO's `ERRMODE_EXCEPTION` and Phorj's three-tier error model (§6-Q6).

| Phorj native | Semantics | Maps to PDO (§2) | one of the 10 FN-DB rows |
|---|---|---|---|
| `Db.openSqlite(path: string) -> Db` | open/create a SQLite database | `new PDO("sqlite:$path", …)` | connect |
| `Db.execute(db: Db, q: Query) -> int` | run INSERT/UPDATE/DELETE/DDL; returns rows affected | `prepare` + `execute`; `rowCount()` | prepare, execute, rowCount |
| `Db.fetchAll(db: Db, q: Query) -> List<Map>` | run SELECT; all rows | `prepare`+`execute`+`fetchAll(PDO::FETCH_ASSOC)` | query, fetchAll |
| `Db.fetchOne(db: Db, q: Query) -> Map?` | first row or `null` | `fetch(PDO::FETCH_ASSOC) ?: null` | fetch |
| `Db.lastInsertId(db: Db) -> int` | id of last INSERT | `lastInsertId()` | lastInsertId |
| `Db.transaction(db: Db, body: () -> T) -> T` | scoped tx: commit on Ok, rollback on fault | `beginTransaction`/`commit`/`rollBack` | beginTransaction, commit, rollBack |
| `Db.close(db: Db) -> void` | release the connection (interim; superseded by `using`, §5) | `$pdo = null;` | (lifecycle) |

The ten FN-DB rows (M-gap count) reconcile to the PDO core method set: **connect, prepare, execute,
query, fetch, fetchAll, rowCount, lastInsertId, beginTransaction, commit/rollBack**. `Db.transaction`
folds the last three into one better-than-PHP closure form (no dangling half-open transaction on a
fault — the LENS). `PDO::quote` is **deliberately absent** — offering it would reintroduce the
string-concat injection path the design removes.

### 1.4 `Value` handle representation (design decision — verified precedent)

A live connection must live in `Value`, which is `Clone` and matched **exhaustively** (no `_` arms in the
`Op`/kernel surfaces). Verified: `Value::Channel(ChanId, Rc<RefCell<VecDeque<Value>>>)` and
`Value::Task(TaskId)` are **always-present enum variants** (NOT `cfg`-gated — the `green` feature gates
the *natives/scheduler*, not the variant; `src/value.rs:150–164`). Follow that precedent exactly:

- Add **`Value::Db(Rc<RefCell<DbConn>>)`** as an **always-present** variant (so no `cfg` arm poisons every
  `match Value` site). Only the `Core.Db` *natives* are behind the `db` feature; with the feature off the
  variant is simply unconstructable (like `Channel` on a non-`green` build).
- **Opaque to the value kernels** (arithmetic/compare/display) and **never transpiled** (the transpiler
  emits PDO calls directly from the `Core.Db` native `php` closures — it never serializes a `Value::Db`),
  mirroring the `Channel`/`Task` "never transpiled" contract.
- **Cloning shares** the same connection (`Rc<RefCell<DbConn>>`), like `Channel`'s shared buffer — a `Db`
  passed to two calls is one connection, not two. `as_type_name()` → `"db"`.

---

## 2. PHP transpile mapping (PDO) — byte-identity-critical framing

Each `Core.Db` native's `php` closure emits idiomatic PDO. Connection opens with the two attributes that
make PDO behave like the native engine and keep prepared-statement safety:

```php
new PDO("sqlite:$path", null, null, [
    PDO::ATTR_ERRMODE            => PDO::ERRMODE_EXCEPTION,   // faults, not silent false
    PDO::ATTR_EMULATE_PREPARES   => false,                   // real parameterized queries
]);
```

| Phorj | Emitted PHP (PDO idiom) |
|---|---|
| `Db.openSqlite(p)` | `new PDO("sqlite:" . $p, …attrs…)` |
| `Db.execute(db, q)` | `(function($p,$s,$a){$st=$p->prepare($s);$st->execute($a);return $st->rowCount();})($db, q.sql(), q.params())` |
| `Db.fetchAll(db, q)` | `…$st->execute($a); $st->fetchAll(PDO::FETCH_ASSOC)` |
| `Db.fetchOne(db, q)` | `… ($r=$st->fetch(PDO::FETCH_ASSOC))===false ? null : $r` |
| `Db.lastInsertId(db)` | `(int)$db->lastInsertId()` |
| `Db.transaction(db,f)` | `$db->beginTransaction(); try { $r=f(); $db->commit(); } catch(\Throwable $e){ $db->rollBack(); throw $e; }` |

**Why this is not byte-identity-tested live but still ships:** DB is impure/non-deterministic IO → Tier B
(DEC-007). The `php` closures **ship** (transpiled programs run under PHP+PDO), but the differential glob
does **not** drive them with a live database — `pure:false` on every `Core.Db` native routes any importing
program through `uses_impure_native` → quarantined (same mechanism as `Core.File` mutation ops,
`Core.Process`). Live equivalence is instead **fixture-tested** (§7). This is the DEC-007 Tier-B contract,
not a ladder-rule exclusion.

---

## 3. Dependency stance — the hard fork (crisp, binary)

**Question:** can the Rust interpreter/VM execute SQL *natively* (invariant 2 requires it — the
interpreter is the oracle) **without** an external crate?

**Answer: no, not realistically.**
- A native engine means implementing SQLite's on-disk B-tree file format, a SQL parser, a query planner,
  and a bytecode VDBE — tens of thousands of lines of new, storage-and-data-integrity-critical code.
  Infeasible for a single developer and *itself* the "never roll your own" hazard the policy exists to
  avoid (data corruption, subtle correctness bugs).
- Transpile-only DB (run only under PHP+PDO) is **explicitly forbidden** by dependency-policy clause 3:
  *"A feature that runs only after transpiling to PHP is a delegation and is disallowed"* — and by
  invariant 14 (native-first; silent semantic downgrade FORBIDDEN).

So the three apparent options collapse to a **binary fork**, because two of them each violate a stated,
non-negotiable rule:

| Option | Verdict |
|---|---|
| (a) Hand-roll a std-only SQL engine | Infeasible + a "roll-your-own" hazard. Rejected. |
| (b) Transpile-only (PDO on the PHP leg) | Violates dep-policy clause 3 + invariant 14. **Forbidden.** |
| (c) **Admit an embeddable-engine dependency (SQLite), feature-gated, quarantined** | The only path that keeps DB native on the Rust backends. **Requires developer adjudication (§6-Q1).** |

**Fair statement of the counter-argument** (so the developer rules on the real trade-off, not a stacked
deck): a SQL engine is *precisely* the "general-purpose / parsing-for-formats" class the policy was
written to **exclude** (clause 1 lists crypto / ReDoS-safe regex / signals / stackful coroutines and
"no others"; format/parser crates "do not qualify"). The policy's own process section is explicit:
anything outside the four domains *"requires revisiting this policy itself, not just adding a row."*
This is that revisit — a real policy amendment, not a routine per-dep authorization.

**Why the amendment is defensible (the case FOR (c)):** the admitted-domain shape is *"a primitive `std`
lacks that phorj cannot implement safely by hand."* A production storage engine fits that shape at least
as well as the existing four — there is genuinely no std path, and hand-rolling it is a data-integrity
hazard on the same axis as "never roll your own crypto." The `corosensei`/`ctrlc` precedent already
admits a **native-only, feature-gated, spine-quarantined** dependency whose `unsafe` is confined to
vetted code — SQLite-via-`rusqlite` is the same shape (unsafe FFI confined to the crate; phorj's own
`#![forbid(unsafe_code)]` intact). Cost of deferral: M TOP-20 **#1**, *"blocks essentially every real
app."*

**This is left OPEN as a PENDING adjudication (§6-Q1).** The autonomous session recommends (c) but does
not rule.

---

## 4. Determinism / Transport model for Tier B

- **DEC-007 Tier B via the `Transport`-style seam.** Introduce a `DbBackend` trait (the exact shape of
  `serve.rs`'s `Transport`: a narrow seam between the impure world and the deterministic core) with the
  real driver (`SqliteBackend`, crate-backed) as the production impl and an **in-memory / committed-fixture**
  impl for tests. The native eval calls the seam; **both backends (interpreter + VM) call the identical
  eval → identical `DbBackend` calls → identical results within a run** (invariant 1/2 parity preserved on
  the Rust side; the divergence is only vs the PHP engine, which the quarantine already excludes).
- **`pure:false` = the quarantine mechanism** (verified `tests/differential.rs:1027`): any program with
  `import Core.Db` is skipped by the differential glob and tested only under §7's controlled fixtures.
- **Invariant 10 (determinism):**
  - Fixture tests **must** use `ORDER BY` — SQL row order is otherwise unspecified and would make output
    hash-seed-like nondeterministic.
  - The Tier A `Query` value renders named params in **stable (sorted) order** if it ever iterates a
    `Map` internally — same rule as the checker's sorted missing-variant list.

---

## 5. Phasing

| Phase | Ships | Gate |
|---|---|---|
| **P1 — `Core.Sql` (Tier A)** | parameterized-`Query` value + `bind`/`bindNamed`; byte-identity-gated; runnable example | **none** — no dep, no adjudication. **Do this first regardless of the fork.** |
| **P2 — `Core.Db` SQLite (Tier B)** | `Value::Db` handle + `DbBackend` seam + `openSqlite`/`execute`/`fetchAll`/`fetchOne`/`lastInsertId`/`close`; fixture harness; `db` feature-gate | **§6-Q1 dep adjudication** (blocking) |
| **P3 — transactions** | `Db.transaction(closure)` (commit/rollback), later `using`/`defer` resource blocks (XL-018/XL-019 — MASTER-PLAN line 954 pairs them with the first handle-based IO) | after P2 |
| **P4 — typed row decode** | `Db.fetchAll<T>` decoding rows into a class via derive-style decode (W5-2 synergy) | after W5-2 |
| **P5 — Postgres** | `Db.openPostgres(dsn)`; a network wire-protocol impl — **separate** std-vs-dep decision (hand-rolled TCP wire protocol is plausible std-only; or another dep) | after P2; own adjudication |
| **P6 — fluent SELECT builder** | optional typed `select().from().where()` on top of `Query` | on demand |

---

## 6. Open questions for the developer (§15 — each with a recommended answer + why)

Per invariant 15, each ships to the developer with a **minimal current-syntax failing program** embedded
in the question and after-states in per-option previews; recommended option first with the why.

**Q1 (THE fork — blocking, biggest).** Amend the dependency policy to admit an embeddable-storage-engine
domain (SQLite), feature-gated `db` + quarantined Tier B — or defer W3-1?
→ **Recommend: admit (c).** Only native-capable path; cost of deferral is M TOP-20 #1. But it is a policy
*amendment* (a 5th domain), not a row-add — hence adjudication. Counter-argument stated fairly in §3.

**Q2 (driver sub-fork).** If Q1=admit: C-SQLite via `rusqlite` (bundled, mature, unsafe confined to the
crate — exact `corosensei` shape) vs a pure-Rust engine (`limbo`/`turso` — young, less proven)?
→ **Recommend: `rusqlite` bundled.** Maturity + determinism + `#![forbid(unsafe_code)]` preserved in
phorj's own code (unsafe FFI confined to the vetted crate). Flag the pure-Rust immaturity risk explicitly.

**Q3 (Sql surface).** Parameterized-`Query` value first, fluent `SELECT` builder deferred — or build the
fluent builder now?
→ **Recommend: `Query` value first.** Delivers the injection-safety guarantee with far less surface; fluent
builder is orthogonal scope creep (P6).

**Q4 (param binding).** Positional `?`+`bind` vs named `:name`+`bindNamed` as the default idiom?
→ **Recommend: ship both, document named as the default** (order-independent, self-documenting, maps
cleanly to PDO named params).

**Q5 (connection lifecycle).** Explicit `Db.close` now, adopt `using`/`defer` (XL-019) when it lands?
→ **Recommend: yes** — `Db.close` interim + `Db.transaction` closure now; `using db = Db.openSqlite(...)`
scoped-release when XL-019 ships (MASTER-PLAN line 954 pairs them). Better-than-PHP end-state.

**Q6 (error model).** DB errors as **faults** (`E-DB-*` FaultKind) vs `T?`/Result returns?
→ **Recommend: faults**, matching PDO `ERRMODE_EXCEPTION` and the three-tier error model; keeps `fetchOne`
free to use `Map?` for the legitimate "no row" case (a `null`, not an error) — distinct from a query fault.

**Q7 (DSN).** Typed per-driver constructors (`Db.openSqlite(path)`) vs a PDO-style DSN string?
→ **Recommend: typed constructors** (better-than-PHP LENS — no DSN string soup) that *transpile to* the
PDO DSN string.

---

## 7. Acceptance & tests

- **`Core.Sql` (Tier A):** byte-identity-gated across all three legs (pure value construction; a runnable
  `examples/db/query-builder.phg` under the differential glob) + unit tests on `sql()`/`params()`.
- **`Core.Db` (Tier B):** new `tests/db.rs` harness (the `tests/process.rs` / `tests/filesystem.rs`
  precedent), **NOT** in `differential.rs` (auto-quarantined via `pure:false`). It must:
  1. **Seed each run from a fresh copy** of a committed fixture (a `:memory:` DB loaded from a committed
     `.sql`, or a `cp`-per-run of a committed `.sqlite`) — **critical:** DB mutation means running `run`
     and `runvm` against the *same* file would let the first run's writes corrupt the second's reads, a
     silent parity break. Fresh state per backend-run.
  2. Assert **run ≡ runvm** on that fixture (both backends, same seed, invariant 1 on the Rust side).
  3. Use `ORDER BY` in every query (invariant 10).
  4. **(Stronger, recommended)** a `tests/db_php.rs` that transpiles the same program, runs the emitted
     PDO under real `php` (PHORJ_PHP=…8.5) against the *same* SQLite fixture, and compares — the closest
     achievable analog to byte-identity for a Tier-B module (SQLite works under both `rusqlite` and PDO's
     `pdo_sqlite`, so this is genuinely a same-engine comparison).
- **Ladder check (invariant 14):** explicitly record that DB **does NOT trigger case-2** (`E-TRANSPILE-DB`
  is NOT created) — it has a faithful PDO mapping and transpiles. The only quarantine is DEC-007 Tier-B
  (byte-identity), not ladder.
- **Examples ship with features** (invariant 9): P1 lands `examples/db/` guide + runnable `.phg` +
  `examples/README.md` entry with the `pure:false` convention note; flagship (W6-2) consumes `Core.Db`.
- **Feature-gate:** `db` off for `phorj-playground` (WASM stays tiny) — verified against the playground
  build, per dep-policy clause 4 and the process checklist.
- **Standing rule:** `cargo build --release`, report `target/release/phg`.

---

STATUS: Designed — not yet implemented. §6-Q1 (dependency-policy amendment) is a blocking PENDING
adjudication for the developer; `Core.Sql` (Tier A, P1) can proceed with no adjudication once approved.
