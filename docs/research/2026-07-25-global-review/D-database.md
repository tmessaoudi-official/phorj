# D — Database module review: naming, prepared statements, savepoints

Scope: the developer's three questions. Read-only investigation of `/home/user/phorj`
(HEAD `25053be`). Every claim below cites `path:LINE` and carries an evidence grade.
Probes ran against the shipped release binary `/home/user/phorj/target/release/phg`
(SQLite driver only — see D8); probe sources under
`/tmp/claude-0/-home-user-phorj/4519ba2a-7bcc-54d2-80b5-d8fbd68ed10d/scratchpad/probe-db/`.

**No design ruling is made anywhere in this document (Invariant 15).** Options + one
recommendation + the why; the developer decides.

---

## Module map (types + full method table)

**How a phorj user writes it** — [Verified: `examples/database/basic.phg:16-22`, and every other
`examples/database/*.phg`]:

```phorj
import Core.DatabaseModule;                    // module import (mandatory — pulls the prelude)
import Core.DatabaseModule.Database;           // member imports ("nothing in the wind")
import Core.DatabaseModule.Statement;
import Core.DatabaseModule.Row;
import Core.DatabaseModule.DatabaseError;
import Core.DatabaseModule.UniqueViolationError;
import Core.DatabaseModule.Naming;             // member-gated (prelude comment, prelude.rs:35)
```

The raw natives live at `Core.Native.Database` (whole-module import only, `E-IMPORT-NATIVE-MEMBER`)
under the internal qualifier `NativeDatabase` — [Verified: `src/ext/database/prelude.rs:13`;
`src/ext/database/natives/registry.rs:4-5,31-33`].

**Where the code lives** — [Verified: `find /home/user/phorj/src/ext/database -type f`]:

| File | Role | Lines |
|---|---|---|
| `src/ext/database/mod.rs` | extension entry, re-exports `database_natives` | 8 |
| `src/ext/database/prelude.rs` | THE user-facing surface — all phorj-source classes | 391 |
| `src/ext/database/natives/registry.rs` | `Core.Native.Database` conn/stmt native rows | ~250 |
| `src/ext/database/natives/registry_rows.rs` | Row-accessor native rows | 243 |
| `src/ext/database/natives/wrappers.rs` | `wrap`/`wrap_unit`/HigherOrder wrappers + `db_transaction` | 138 |
| `src/ext/database/natives/ops.rs` | conn/stmt/transaction operation bodies | 452 |
| `src/ext/database/natives/handles.rs` | `DbConn`/`DbStmt`/`DbCursor`/`Binds` | 235 |
| `src/ext/database/natives/rows.rs` | row getters | — |
| `src/ext/database/natives/{driver,sqlite,postgres,mysql,postgres_sql,mysql_sql}.rs` | multi-driver seam + drivers | — |
| `src/checker/desugar_db.rs` | `queryInto`/`queryOneInto`/`queryScalar`/`queryMap`/`streamInto` desugar | — |
| `src/checker/calls/lint.rs` | `W-SQL-INJECTION` compile-time lint | 87 |

Docs: `FEATURES.md:103` · `docs/EXTENSIONS.md:17` · `docs/specs/UNIFIED-SPEC.md:1236-1287` ·
`KNOWN_ISSUES.md:690-760` · register `docs/research/full-audit/raw/C-decisions.md:481-545`
(DEC-208), `:2039` (DEC-278 rename), `:136` (DEC-284 flag rename), `:212` (DEC-292 perf) ·
11 examples in `examples/database/`.

### Public types (ALL of them)

[Verified: read `src/ext/database/prelude.rs` in full, lines 12-390]

| Type | Kind | Line |
|---|---|---|
| `Database` | class — the connection façade | `prelude.rs:280` |
| `Statement` | class — the prepared statement | `prelude.rs:158` |
| `Row` | class — one materialized result row | `prelude.rs:82` |
| `RowStream` | class `implements Iterator<Row>` | `prelude.rs:236` |
| `DatabaseStream<T>` | class `implements Iterator<T>` | `prelude.rs:266` |
| `Naming` | enum `{ Exact(), SnakeToCamel() }` | `prelude.rs:43` |
| `DatabaseError` | `open class implements Error` + `static fail(msg): never` | `prelude.rs:45` |
| `UniqueViolationError` `ConstraintViolationError` `ConnectionError` `SerializationFailureError` `TimeoutError` `SyntaxError` | `extends DatabaseError` | `prelude.rs:75-80` |
| `DatabaseResult<T>` | prelude-LOCAL internal carrier (not user API) | `prelude.rs:31` |
| `DatabaseHandle` | reserved opaque type (`Value::Db`) | `registry.rs:12-14` |

### `Database` — full signatures

| Signature | Line |
|---|---|
| `constructor(string dsn, public Naming naming = new Naming.Exact()) throws DatabaseError` | `prelude.rs:288` |
| `static function withPassword(string dsn, Secret<string> password, Naming naming = new Naming.Exact()): Database throws DatabaseError` | `prelude.rs:298` |
| `function prepare(string sql): Statement throws DatabaseError` | `prelude.rs:301` |
| `function lastInsertId(): int throws DatabaseError` | `prelude.rs:307` |
| `function timeout(int ms): Database throws DatabaseError` | `prelude.rs:312` |
| `function onQuery((string, int) => void hook): Database` | `prelude.rs:320` |
| `function begin(): void throws DatabaseError` | `prelude.rs:336` |
| `function commit(): void throws DatabaseError` | `prelude.rs:339` |
| `function rollback(): void throws DatabaseError` | `prelude.rs:342` |
| `function rollbackQuiet(): void` (never throws) | `prelude.rs:349` |
| `function transaction<T>(() => T throws DatabaseError fn, int retries = 0): T throws DatabaseError` | `prelude.rs:371` |
| `function close(): void` (idempotent, never throws) | `prelude.rs:386` |
| public field `naming: Naming` | `prelude.rs:288` |

**Absent** (checked by grep + full read of the prelude): no `savepoint(name)`, no `rollbackTo(name)`,
no `rollbackAll()`, no `inTransaction()`, no `transactionDepth()`, no `isolation(...)`, no
`Closable`/`using` support (DEC-203, deferred — `KNOWN_ISSUES.md:735-739`).
[Verified: `grep -rn "rollbackAll\|rollbackTo\|savepoint(\|abortAll" docs/ src/` → no matches]

### `Statement` — full signatures

| Signature | Line |
|---|---|
| `constructor(private DatabaseHandle raw, public Naming naming = new Naming.Exact())` | `prelude.rs:162` |
| `function bind(string \| int \| float \| bool value): Statement throws DatabaseError` | `prelude.rs:163` |
| `function bindNamed(string name, string \| int \| float \| bool value): Statement throws DatabaseError` | `prelude.rs:170` |
| `function bindList<T>(List<T> values): Statement throws DatabaseError` | `prelude.rs:178` |
| `function exec(): int throws DatabaseError` | `prelude.rs:181` |
| `function executeMany<T>(List<List<T>> rows): int throws DatabaseError` | `prelude.rs:188` |
| `function execReturningId(): int throws DatabaseError` | `prelude.rs:193` |
| `function namingStrategy(Naming strategy): Statement` (copy-builder) | `prelude.rs:207` |
| `function query(): List<Row> throws DatabaseError` | `prelude.rs:208` |
| `function stream(): RowStream throws DatabaseError` | `prelude.rs:216` |

Plus five **desugar-only** pseudo-methods (no prelude declaration; rewritten by
`src/checker/desugar_db.rs:718-722`): `queryInto<T>()`, `queryOneInto<T>()`, `queryScalar<T>()`,
`queryMap<K,V>()`, `streamInto<T>()`.

**Absent on `Statement`**: no `reset()`, no `clearBinds()`, no `close()`, no `bindAll(list)`.
[Verified: `grep -rn "reset\|clearBind\|clear_bind" src/ext/database/` → only comments in
`sqlite.rs:193`, `ops.rs:419`, `wrappers.rs:121`; nothing user-facing]

### `Row` — 22 accessors

`getInt` `getString` `getFloat` `getBool` (+ `…OrNull` ×4) · `getDecimal` `getDecimalOrNull` ·
`getIntList` `getStringList` `getFloatList` `getBoolList` (+ `…OrNull` ×4) · `columnNames(): List<string>` ·
`isNull(string): bool`. All `throws DatabaseError`. [Verified: `prelude.rs:82-156`]

---

## Q1 naming: what the object REALLY is + cross-language scan + options

### The developer's question, restated

> *"should we not have the `Core.DatabaseModule.Database` be `Connection` instead? is that more
> correct and more clear?"*

### What the object REALLY is — the crux, settled by code

**It is exactly ONE connection. Not a pool, not a datasource, not a driver-manager.**
[Verified — four independent lines of evidence:]

1. `open_inner` builds a single `Box<dyn DriverConn>` from the DSN and wraps it in
   `Rc<RefCell<Option<…>>>` — one driver, one handle, no vector, no free-list, no acquire/release:
   ```rust
   // src/ext/database/natives/ops.rs:22-28
   let driver = open_driver(dsn)?;
   Ok(Value::Db(Rc::new(DbConn {
       driver: Rc::new(RefCell::new(Some(driver))),
       tx_depth: Rc::new(Cell::new(0)), hook: …, timeout_ms: …,
   })))
   ```
2. `DbConn`'s own doc-comment: *"A live database connection handle … cloning the `Value::Db` shares
   this `Rc`, so all bindings name the same connection."* [`handles.rs:77-79`]
3. Connection-scoped mutable state lives directly on it — `tx_depth`, `hook`, `timeout_ms`
   [`handles.rs:89-103`]. A pool cannot hold a single `tx_depth`; that field only makes sense for one
   session.
4. `grep -rn "pool\|Pool" src/ext/database/` → **zero matches**, and the spec lists pooling under
   **"Out of scope (userland or later)"**: *"pooling/replica routing/caching"*
   [`docs/specs/UNIFIED-SPEC.md:1285-1286`].

`close()` sets the single `Option` to `None`, invalidating every derived `Statement`
[`ops.rs:443-451`] — connection semantics, not pool semantics.

**Conclusion: `Connection` would be the technically correct name; `Database` currently names a
connection.** [Verified: per the four points above]

### The counter-consideration the developer should weigh

`Database` is not *arbitrarily* wrong — it is PDO-mimetic. DEC-208's ruled surface was explicitly
"a strongly-typed PDO", and PHP's `PDO` object is likewise one connection wearing a
database-y name [Verified: register `C-decisions.md:492-495` — *"**Connection:** `Db db = new
Db("sqlite:app.db")`"*. Note the register's own prose calls the row "**Connection:**" while naming
the type `Db` — the concept/name mismatch is visible in the original ruling itself].

### Cross-language naming scan (Invariant 16)

[Grade for this table: **Unverified** — recalled from API knowledge; `WebFetch` to
dev.mysql.com and mariadb.com both returned HTTP 403 in this session, so no vendor doc could be
opened to confirm. The *pattern* it establishes (single-session objects are named `Connection`;
`Database`/`DB` names are reserved for pools or façades) is high-confidence; individual method
spellings are not.]

| Ecosystem | Pool / factory | **Single session** | Statement | Result | Tx |
|---|---|---|---|---|---|
| PHP PDO | — | **`PDO`** | `PDOStatement` | `PDOStatement` (as cursor) | methods on `PDO` |
| Laravel / Illuminate | `DatabaseManager` (`DB` facade) | **`Connection`** | — | — | `Connection::transaction()` |
| JDBC | `DataSource` | **`Connection`** | `PreparedStatement` | `ResultSet` | `Connection` + `Savepoint` |
| Go `database/sql` | **`sql.DB` (a POOL)** | **`sql.Conn`** | `sql.Stmt` | `sql.Rows` | `sql.Tx` |
| Rust sqlx | `Pool`/`PgPool` | **`PgConnection`** | `Query`/`QueryAs` | `PgRow` | `Transaction` |
| Rust diesel | (r2d2 pool) | **`PgConnection`** | query DSL | `QueryResult` | `conn.transaction()` |
| Python DB-API (PEP 249) | — | **`Connection`** | (`Cursor` doubles as it) | `Cursor` | `Connection` |
| C# ADO.NET | (built-in pooling) | **`DbConnection`** | `DbCommand` | `DbDataReader` | `DbTransaction` |
| Node `pg` | `Pool` | **`Client`** | (query config obj) | `Result` | `client.query('BEGIN')` |
| Kotlin Exposed | — | — | — | — | `Transaction` |

Two things fall out:

* **`Connection` is the overwhelming convention for "one session".** Eight of ten name it
  `Connection`/`Conn`/`Client`/`PgConnection`.
* **`Database`/`DB` is the convention for a POOL or a FAÇADE — the opposite of what phorj's object
  is.** Go's `sql.DB` is a pool. Kotlin Exposed's `Database` is a handle over a datasource, not a
  connection. Laravel's `DB` is a manager façade that *hands out* `Connection`s. So keeping
  `Database` isn't merely "less precise" — in the two ecosystems where that exact word is used, it
  means something phorj's object is not. **[Inferred: from the table above; the Go `sql.DB`-is-a-pool
  and Laravel-`DB`-is-a-manager facts are the load-bearing ones]**

### The DEC-278 interaction — the strongest structural argument, and it cuts toward `Connection`

`Core.Db` was renamed `Core.DatabaseModule` under **DEC-278**, whose entire stated rationale is the
*namesake* collision:

> *"The SEVEN modules whose headline type shares the module leaf (Fs, Db, Uri, Session, Debug,
> HttpClient, Iterator) rename to `Core.FileSystemModule`, `Core.DatabaseModule`, … — so
> `import Core.FileSystemModule.FileSystem;` is fully explicit; **non-namesake modules stay bare**."*
> [Verified: `docs/research/full-audit/raw/C-decisions.md:2036-2044`]

Claude challenged the `Module` suffix at the time ("a zero-information suffix + mixed suffixed/bare
surface"); the developer heard it and confirmed [Verified: same rows, `:2046-2049`].

**Renaming the type `Database` → `Connection` dissolves the namesake collision for this one module** —
`Core.Database.Connection` is not a namesake pair, so by DEC-278's own rule the module would
qualify to *stay bare*, i.e. `import Core.Database; import Core.Database.Connection;`. That is the
awkward `DatabaseModule` string the developer's own question stumbled over
(`Core.Databas4Module.Databse`) disappearing as a *consequence*, not a separate change.
[Inferred: applying DEC-278's stated non-namesake rule to the post-rename names]

### Register / spec state for Q1

**No existing row.** [Verified: `grep -n "DatabaseModule" C-decisions.md` → only DEC-284 (:136) and
DEC-278 (:2039); neither considers `Database`→`Connection`. `grep` for a `Connection` naming
discussion → nothing.] Note `ConnectionError` already exists as an error subtype
[`prelude.rs:77`] — a `Connection` class would sit beside `ConnectionError`, which reads naturally,
and there is no name clash (different identifiers).

### Options — Q1

| # | Option | Blast radius | Trade |
|---|---|---|---|
| **A** | Rename type `Database` → `Connection`; keep module `Core.DatabaseModule` | prelude + 11 examples + tests + docs + register row | Type is correct; module keeps a suffix whose namesake rationale no longer applies (inconsistent with DEC-278's own rule) |
| **B** | Rename type → `Connection` **AND** module → bare `Core.Database` (DEC-278 non-namesake path) | A + the module-name codemod (checker registry, `CORE_MODULES`, gate tables, `Core.Native.Database` stays) + a DEC-278 amendment row | Fully consistent: correct type name AND the suffix drops out by DEC-278's own rule. Biggest one-time churn |
| **C** | Keep `Database` | zero | PDO-mimetic; but collides with the Go/Exposed meaning of the word (pool/façade), and today's object is provably a single connection |
| **D** | Rename → `Connection` and *reserve* `Database` for a future pool type (`Database.connect(): Connection`) | A now; the pool is later, out-of-scope per spec | Names both concepts correctly and leaves the pool door open — but adds a type that does not exist yet, and pooling is explicitly userland/later |

**Recommendation: B**, with A as the low-churn fallback. Why B: the object is a single connection by
four independent code proofs, `Connection` is the near-universal name for that, `Database` is
elsewhere the name for the *pool* (the thing phorj explicitly does not have), and — decisively —
DEC-278's `Module` suffix exists *only* because of the namesake collision that the rename removes,
so B leaves the surface internally consistent while A leaves a suffix with no remaining
justification. B is also the change that actually answers the friction in the developer's own
question. **This is a recommendation, not a ruling — Invariant 15 reserves it for the developer,
and it needs a DEC-278 amendment row either way.**

---

## Q2 prepare/PreparedStatement: current reality + v1 design options

### The developer's question, restated

> *"And for `.prepare(...)` it returns what? do we have an object now? let's say I have an array of
> binded and I have to go through it and bind one by one! do we have like a `PreparedStatement`?"*

### Direct answer, part 1: YES, the object exists

`db.prepare(sql)` returns a real **`Statement`** object — a phorj-source class over an opaque
`DatabaseHandle` [Verified: `prelude.rs:301-303`, `prelude.rs:158-229`]:

```phorj
// prelude.rs:301
function prepare(string sql): Statement throws DatabaseError {
  return match (NativeDatabase.prepare(this.raw, sql)) {
    DatabaseResult.Ok(h) => new Statement(h, this.naming),
    DatabaseResult.Err(e) => DatabaseError.fail(e)? };
}
```

It has `bind`/`bindNamed`/`bindList`/`exec`/`executeMany`/`execReturningId`/`query`/`stream`/
`namingStrategy` — see the table above. It is **not** fire-and-forget: nothing executes until
`exec()`/`query()`/`stream()`/`executeMany()`/`execReturningId()`. Both binding styles exist —
positional `?` via `bind(v)` and named `:name` via `bindNamed(n, v)` — and mixing them on one
statement is a catchable error [Verified: `ops.rs:79-83`, `ops.rs:132-136`].

So: **`Statement` *is* phorj's `PreparedStatement`. It just isn't a *reusable* one.**

### Direct answer, part 2: the developer's exact scenario FAILS today (D1 — P1)

`bind` **appends** to an accumulator and there is **no reset**:

```rust
// src/ext/database/natives/ops.rs:75-84
let mut binds = stmt.binds.borrow_mut();
match &mut *binds {
    Binds::None => *binds = Binds::Positional(vec![PosBind::One(val.clone())]),
    Binds::Positional(v) => v.push(PosBind::One(val.clone())),   // ← append, forever
    Binds::Named(_) => return Err("… cannot mix positional bind() with named bindNamed()"),
}
```

`exec_inner`/`query_inner` merely *read* `stmt.binds` and never clear it
[Verified: `ops.rs:154-163`, `ops.rs:202-211`].

Probe — hold one statement, loop over an array, bind one-by-one, execute
(`probe-db/reuse.phg`):

```
$ target/release/phg run reuse.phg
inserted x -> 1
DB error: Core.DatabaseModule: 2 bound value(s) but 1 ? placeholder(s) in the SQL
```

[Verified: ran on BOTH backends — `phg run` and `phg run --tree-walker` produce byte-identical
output, so this is a surface-design gap, not a backend divergence.]

**This is precisely the developer's scenario, and it hard-errors on iteration 2.**

The register makes this a promise gap, not just a missing nicety. DEC-208's ruled surface rejected
the one-shot alternative *for this exact reason*:

> *"shape 2 one-shot `db.query(sql, binds)` (**rejected — no Statement reuse**)"*
> [Verified: `C-decisions.md:515`]

Statement reuse was the stated rationale for the shape that shipped, and it does not work.

### D2 (P1) — the two bind styles behave *differently* under reuse, silently

Named binds accumulate into a `Vec<(String, Value)>` [`handles.rs:131`] and the SQLite driver
converts **all** accumulated pairs on every execute, so duplicates are last-wins
[Verified: `sqlite.rs:210-218`]. Net effect: the *identical* loop shape **works** with `:name`
and **hard-errors** with `?` (`probe-db/named.phg`):

```
$ target/release/phg run named.phg
inserted x -> 1
inserted y -> 1
inserted z -> 1
count=3
row x / row y / row z
```

[Verified: ran]

### D3 (P1) — and the style that "works" is quadratic + leaks memory

Because the named-bind `Vec` grows by one entry per loop iteration and every `exec` re-converts the
whole vector, the Nth execute does N conversions.

Measured [Verified: `time` on `probe-db/namedperf.phg` and `probe-db/reprep.phg`, release binary,
SQLite `:memory:`]:

| Pattern | 4 000 iters | 8 000 iters | Scaling |
|---|---|---|---|
| "Reuse" one `Statement` + `bindNamed` in a loop | **1.135 s** | **4.469 s** | ~4× for 2× work → **quadratic** |
| Re-`prepare` per iteration + positional `bind` | 0.049 s | 0.059 s | linear |

At 8 000 rows the "reuse" path is **~75× slower** than just re-preparing. The reuse pattern is
therefore not merely unsupported — it is an active performance trap, and the accumulator is an
unbounded memory growth per statement lifetime.

Corroborating internal evidence that the team already routes *around* reuse:
* the project's own macro-bench re-prepares every row and rationalises it as "like a naive request
  handler" [`bench/micro/dbwork.phg:11-13,25-28`];
* DEC-292's forward note is *"chase the prepare Statement-instance alloc"* — i.e. optimise
  re-prepare, not enable reuse [`C-decisions.md:212`];
* the SQLite driver already uses `prepare_cached`, so re-preparing identical SQL skips SQLite's
  recompile [`sqlite.rs:188-196`]. **This matters for the fix: the expensive part of reuse is
  already handled at the driver layer. A reusable statement is a *surface / bind-lifecycle*
  change, not a driver rewrite.** [Inferred: `prepare_cached` is keyed by SQL text and resets on
  return-to-cache per that comment, so a per-execute bind reset is the only missing piece]

### What DOES work today

[Verified: `probe-db/reuse2.phg` → `a1=1 a2=1`, `b n1=2 n2=2`]

* A statement with **no** binds re-executes fine (`INSERT … VALUES(7)` twice; `SELECT` twice).
* `executeMany(rows)` is the sanctioned bulk path: prepare once, many bind-sets, one `phorj_bulk`
  savepoint, atomic [`ops.rs:226-257`]. It **rejects** any statement that also has accumulated
  binds [`ops.rs:243-248`] — which is the module implicitly admitting the accumulator is
  single-shot.
* Re-`prepare` per iteration (fast, thanks to `prepare_cached`).

### What a `PreparedStatement` v1 would need

Design axes, with the byte-identity/lift shape noted (grade [Speculative] — these are design
judgments, not checkable facts):

1. **Bind lifecycle** — the actual decision. Three shapes:
   * *(a) implicit reset* — `exec`/`query` clears `binds` after a successful execute. Smallest
     diff (one `*stmt.binds.borrow_mut() = Binds::None;` after execute). Silently changes today's
     accumulate-then-execute semantics for anyone relying on multi-`bind`-then-one-`exec`… which is
     the *normal* case, so this must clear only *after* the execute consumed them — safe, but it
     also silently fixes/changes D2's last-wins named path.
   * *(b) explicit `reset()`/`clearBinds()`* — additive, zero behaviour change, but the developer's
     loop needs an extra call and forgetting it reproduces D1.
   * *(c) `bindAll(List<T>)` + execute-per-set* — i.e. `executeMany` generalised to reads. Doesn't
     answer "bind one by one".
2. **Placeholder style** — positional `?` and named `:name` both already exist and are mutually
   exclusive per statement; a v1 must make reuse behave *identically* for both (D2).
3. **Re-execution** — SQLite already resets the cached statement per execute
   [`sqlite.rs:193-194`]; Postgres/MySQL use `prep`/prepared-statement objects per driver, so the
   surface is uniform.
4. **`close`/lifetime** — there is no `Statement.close()`. A `Statement` holds
   `Rc<RefCell<Option<Box<dyn DriverConn>>>>` and dies with the connection [`handles.rs:141`];
   `db.close()` invalidates all derived statements [`ops.rs:443-451`]. A reusable statement makes
   lifetime user-visible → `Closable` (DEC-203) becomes more relevant.
5. **Error surface** — unchanged: natives return `DatabaseResult<T>` values, the prelude `match`es
   and throws a catchable typed `DatabaseError` [`handles.rs:14-35`, `prelude.rs:58-66`]. A
   bind-arity mismatch stays a runtime `DatabaseError` (the compile-time arity check is explicitly
   deferred — `UNIFIED-SPEC.md:1256`).
6. **Transpile shape (byte-identity)** — **this axis is already closed, and the spec is wrong
   about it.** `Core.DatabaseModule` is LADDER **case 2, native-only**: transpiling hard-errors
   `E-TRANSPILE-DB` [Verified: `src/cli/pipeline.rs:596-600`; `FEATURES.md:103`]. The PHP emitters
   in the registry are explicit placeholders [`registry_rows.rs:26-27`, `registry.rs:130-132`]. So
   PDO's `PDOStatement` mapping (`prepare`/`bindValue`/`execute`/`fetch*` — which *would* map a
   reusable statement almost 1:1, since `PDOStatement` is itself reusable via `execute()` per
   bind-set) is **not** a constraint on the design today. See D6 for the spec divergence.
7. **Lift shape (Invariant 17)** — there is **no** PDO lifter support at all
   [Verified: `grep -rn "PDO\|pdo" src/lift/` → zero matches; every DB native carries
   `lift_from: &[]`]. So a `PreparedStatement` v1 has no lift leg to keep current — but it also
   inherits an existing Invariant-17 hole (D7).

### Register / spec state for Q2

**No existing row for statement reuse or a bind reset.** [Verified: grep of `C-decisions.md`,
`KNOWN_ISSUES.md`, `UNIFIED-SPEC.md`, `FEATURES.md` for `reset`/`reuse`/`clearBind` in a DB context
→ only `C-decisions.md:515` (reuse as the *rationale* for the chosen shape) and the deferred
placeholder-arity note.] The gap is undocumented — `KNOWN_ISSUES.md` does not disclose it.

### Options — Q2

| # | Option | Diff size | Trade |
|---|---|---|---|
| **A** | **Reset binds after each successful execute** (`exec`/`query`/`stream`/`execReturningId`) — `Statement` becomes genuinely reusable, both styles identically | Tiny in `ops.rs` (4 sites, or one shared helper); + tests + example + KNOWN_ISSUES/spec/register rows | Makes the developer's loop just work; fixes D1+D2+D3 in one move; sanctions the reuse DEC-208 already promised. Behaviour change for any code relying on the current append-across-executes (which errors anyway for `?`, and is the quadratic trap for `:name`) |
| **B** | Add explicit `Statement.reset(): Statement` and document the current accumulate semantics | Tiny, purely additive | Zero risk of changing existing behaviour; but the footgun (D1's error, D2's asymmetry, D3's quadratic) stays reachable by default |
| **C** | A/B + `Statement.close()` and pursue `Closable`/`using` (DEC-203) | Larger, touches a language slice | Correct lifetime story; but couples the DB fix to a deferred language feature |
| **D** | Do nothing; document the limitation in `KNOWN_ISSUES.md` and steer users to `executeMany` / re-`prepare` | Doc-only | Honest, cheap; but leaves the register's own "Statement reuse" rationale unfulfilled and the quadratic named-bind trap live |

**Recommendation: A, plus the D-style disclosure as its docs leg.** Why: it is the smallest change
that fixes all three findings at once, the driver layer already resets and caches so there is no
perf cost, and it delivers the reuse the register cited as the reason for rejecting the one-shot
shape. B is the safe fallback if the developer wants zero behaviour change; D alone leaves a
75×-slowdown trap on the only pattern that appears to work. **Developer's call (Invariant 15) —
this is user-visible semantics.**

---

## Q3 savepoints: semantics + the abort-everything answer + any bug found

### The developer's question, restated

> *"And for the database transactions with savepoints! is there a way to rollback all and discard
> savepoints?"*

### Direct answer: **NO. There is no "abort everything" call today.**

The API is strictly one-level-at-a-time. `begin` pushes, `commit`/`rollback` pop exactly one level:

```rust
// src/ext/database/natives/ops.rs:375-390 (begin)
let sql = if depth == 0 { "BEGIN".to_string() } else { format!("SAVEPOINT phorj_sp_{depth}") };
// :395-413 (commit)  → remaining==0 ? "COMMIT" : format!("RELEASE phorj_sp_{remaining}")
// :420-438 (rollback)→ remaining==0 ? "ROLLBACK"
//                     : format!("ROLLBACK TO phorj_sp_{remaining}; RELEASE phorj_sp_{remaining}")
```
[Verified: read `ops.rs:363-438`]

Probe from depth 3 (`probe-db/sp.phg`) [Verified: ran]:

```
in-tx count=3
after 1 rollback count=2     ← ROLLBACK TO phorj_sp_2 — pops ONE level
after 2 rollbacks count=1
after 3 rollbacks count=0    ← only now is the top-level ROLLBACK issued
after 4th (no-op) count=0    ← depth-0 rollback is a guarded no-op
```

So the developer must know the depth and call `rollback()` exactly that many times. There is also
**no way to observe the depth from phorj**: `begin_inner` *returns* the new depth
[`ops.rs:389`], but the prelude discards it (`DatabaseResult.Ok(_) => Database.ok()`,
`prelude.rs:337`), and there is no `inTransaction()`/`transactionDepth()`. [Verified]

### The SQL semantics the developer is reasoning from — confirmed correct

The developer's intuition ("a top-level ROLLBACK discards the whole transaction including all
savepoints") is right, and phorj *can* express it — it just isn't reachable as one call:

* SQLite: `ROLLBACK` (no `TO`) reverts the entire transaction and, per SQLite's savepoint semantics,
  destroys all savepoints created within it. [Inferred: the module itself relies on exactly this —
  `ops.rs:418-419` comments *"a doomed transaction is reset by SQLite regardless"* — and the probe
  above shows the 3rd `rollback()` (which emits bare `ROLLBACK`) clearing everything. Vendor doc
  could not be opened: `WebFetch` → HTTP 403.]
* `ROLLBACK TO SAVEPOINT x` keeps `x` alive (which is why `ops.rs:434` follows it with an explicit
  `RELEASE`) — so it is the *partial* form, exactly as the developer supposed. [Verified: the code's
  own `ROLLBACK TO …; RELEASE …` pairing only makes sense under this semantics, and the probe
  confirms the outer level survives an inner rollback.]

### Today's only workaround — verified to work

Because a depth-0 `rollback` is a guarded no-op [`ops.rs:427-428`], a blind loop of
`rollbackQuiet()` is a correct (if inelegant) abort-everything:

```phorj
function abortAll(Database db): void {
    mutable int i = 0;
    while (i < 64) { db.rollbackQuiet(); i = i + 1; }   // depth-0 rollbacks are no-ops
}
```

Probe (`probe-db/abortall.phg`) [Verified: ran]:
```
in-tx = 3
after abortAll = 0
after reuse = 1          ← connection fully usable afterwards
```

This works but is a hack: it hard-codes a depth ceiling, and `rollbackQuiet` swallows real driver
errors by design [`prelude.rs:349-351`].

### D4 (P1, and the answer to "could nested bookkeeping leave stale state?") — YES, and it silently persists data

The task asked whether savepoint depth bookkeeping can leave stale state after a rollback. It can —
and the reachable path is the **closure form**, which *advertises* auto-rollback.

`db_transaction` rolls back exactly **one** level when the closure throws:

```rust
// src/ext/database/natives/wrappers.rs:133-136
Err(e) => {
    let _ = rollback_inner(std::slice::from_ref(db));   // ← ONE level only
    Err(e)
}
```

If the closure opened an unmatched inner `begin()` (directly, or via any helper that begins and
throws before committing — a completely ordinary composition), that single pop rolls back the
*inner savepoint* and leaves the **outer transaction OPEN** with the closure's earlier writes
uncommitted but live.

Probe (`probe-db/leak.phg`) [Verified: ran]:

```
caught: boom
visible rows after 'rollback' = 1     ← the pre-inner-begin INSERT survived
after stray commit = 1                ← a later commit() PERSISTS it
```

And the variant proving the transaction is genuinely still open (`probe-db/leak2.phg`)
[Verified: ran]:

```
caught: boom
visible rows after 'rollback' = 1
after outer rollback = 0              ← a SECOND rollback is what actually clears it
```

**Impact.** The caller catches the typed `DatabaseError`, reasonably believes "the transaction
rolled back" (that is the documented contract — `KNOWN_ISSUES.md:713-716`, `prelude.rs:352-360`),
and is instead left with: (a) partially-applied writes visible on the connection, (b) an open
transaction holding a write lock, and (c) a `tx_depth` of 1 such that any later `commit()` — or a
`transaction()` at what the user thinks is top level — **commits the supposedly-rolled-back work**.
Severity **P1** (silent data persistence after a reported rollback), grade **[Verified: two probes
above, on the shipped release binary]**.

The root cause is structural, not a typo: the auto-rollback native pops one level because that is
all the single-level API can express. **An abort-everything primitive is the fix for D4 as well as
the answer to Q3** — which is why the developer's two intuitions are the same intuition.

### D5 (P1) — nested savepoints look broken on the MySQL driver, and are untested

> **RESOLVED 2026-07-30** — folded into DEC-351 and BUILT. The tables and line references below are
> the AS-FOUND state (kept as the evidence for the finding); the SQL is now single-sourced in
> `src/ext/database/natives/savepoint.rs`, emitting only the three-dialect intersection, with a
> source-scan ratchet over every emitter. Current status lives in the decision register (DEC-351)
> and `docs/plans/SLICE-STATE.md` — not here.

The generic transaction-control SQL in `ops.rs` and the MySQL driver's own bulk path disagree about
MySQL's savepoint grammar, in the same module:

| Site | Emitted SQL |
|---|---|
| `ops.rs:408` (nested `commit`) | `RELEASE phorj_sp_1` |
| `ops.rs:434` (nested `rollback`) | `ROLLBACK TO phorj_sp_1; RELEASE phorj_sp_1` (one string) |
| `mysql.rs:156-157` (bulk, MySQL-aware) | `RELEASE SAVEPOINT phorj_bulk` / `ROLLBACK TO SAVEPOINT phorj_bulk`, issued as **separate** statements |
| `postgres.rs:187-188` | `RELEASE phorj_bulk` / `ROLLBACK TO phorj_bulk; RELEASE phorj_bulk` |

[Verified: read all four sites]

Two distinct problems for MySQL:

1. **Missing `SAVEPOINT` keyword.** MySQL's grammar is `RELEASE SAVEPOINT identifier` — the keyword
   is required there (unlike `ROLLBACK … TO [SAVEPOINT] id`, where it is optional, and unlike
   SQLite/Postgres, which accept bare `RELEASE id`). `ops.rs:408` emits the bare form.
   **[Inferred — strong]**: the vendor doc could not be fetched (`dev.mysql.com` and `mariadb.com`
   both HTTP 403 in this session), *but* the module's own MySQL driver deliberately writes the full
   `RELEASE SAVEPOINT` form for its bulk savepoint while Postgres's driver, three files away, uses
   the bare form — an author-intentional divergence that only makes sense if MySQL requires the
   keyword.
2. **Multi-statement string.** `MySqlConn::control` runs `self.conn.borrow_mut().query_drop(sql)`
   [Verified: `mysql.rs:199-201`] — a single-statement call. `ops.rs:434`'s `";"`-joined pair is not
   a single statement, and `CLIENT_MULTI_STATEMENTS` is not enabled anywhere in the driver
   [Verified: `grep` of `mysql.rs` — no multi-statement flag]. Postgres escapes this because its
   `control` uses `batch_execute` [`postgres.rs:245-249`], and SQLite because it uses
   `execute_batch` [`sqlite.rs:345-347`].

Consequence, if the grammar inference holds: on MySQL a **nested** `commit()` and a **nested**
`rollback()` both fail — and `rollback_inner` decrements `tx_depth` **before** issuing the SQL
[`ops.rs:429-436`], so the failure leaves the counter desynchronised from the real transaction
state. That is the stale-bookkeeping bug in its most direct form.

**Untested.** The MySQL live round-trip exercises only a top-level `db.transaction` (depth 0 →
`BEGIN`/`COMMIT`) and `executeMany`'s own depth-0 path — there is no nested-`begin`/savepoint case
[Verified: read `tests/database_mysql.rs:29-113`]. The Postgres round-trip likewise only notes the
bulk savepoint [`tests/database_postgres.rs:86`]. The only savepoint tests are SQLite
[`tests/database.rs:784`, `:881`]. And both live tests skip unless `PHORJ_MYSQL_TEST_DSN` /
`PHORJ_PG_TEST_DSN` is set, so the full gate never touches this path.

### Register / spec state for Q3

**No existing row for abort-everything, and no row for D4/D5.** [Verified:
`grep -rn "rollbackAll\|rollbackTo\|abortAll\|discard savepoint" docs/ src/` → zero matches.] What
*is* recorded: savepoint-aware nesting (DEC-208 slice C, `C-decisions.md:535-545`;
`KNOWN_ISSUES.md:700-704`), the closure form + retry (`C-decisions.md:782-790`), and isolation
levels as deferred (`KNOWN_ISSUES.md:740-742`). Nothing discloses the one-level unwind or the
MySQL grammar divergence.

### Options — Q3

| # | Option | Trade |
|---|---|---|
| **A** | Add `db.rollbackAll(): void throws DatabaseError` — issue a single top-level `ROLLBACK` and set `tx_depth = 0` (plus `rollbackAllQuiet()` for `finally`) | Exactly what the developer asked for; one SQL statement regardless of depth (SQL already destroys all savepoints); makes D4 fixable by pointing `db_transaction`'s error arm at it when the closure leaked levels. Naming/shape is user-visible → developer's ruling |
| **B** | Laravel-style depth parameter: `db.rollback(int toLevel = -1)` where `0` = abort everything | One method, PHP-ecosystem precedent, DEC-249 method defaults already shipped [`C-decisions.md:1383`]. But an int level is opaque, and the developer cannot currently *see* the depth |
| **C** | Named savepoints, JDBC/C#-style: `db.savepoint(name)` / `db.rollbackTo(name)` / `db.release(name)` alongside a plain `rollback()` = abort all | Most expressive, matches JDBC `Savepoint` and ADO.NET `SqlTransaction.Save/Rollback`; largest surface addition and re-opens the `begin()`-is-a-savepoint design |
| **D** | Fix D4 only — make `db_transaction`'s error arm unwind to the depth it entered at — and leave manual abort-all to the `rollbackQuiet()` loop | Closes the silent-persistence bug with no new surface; but manual users still have no abort-all, and Q3 is answered "no, use a loop" |
| **E** | Expose `db.transactionDepth(): int` / `inTransaction(): bool` (the value is already computed and thrown away at `ops.rs:389`) | Cheap, additive, lets users write a correct unwind loop themselves; a building block rather than an answer |

**Recommendation: A, bundled with D, and E as a cheap companion.** Why: A is the literal answer to
the question and is one SQL statement at any depth (SQL's own semantics do the savepoint discarding —
no per-level loop needed); D is a P1 correctness fix that A makes trivial to implement (the
closure's error arm unwinds to its entry depth rather than popping one level); E costs almost
nothing because `begin_inner` already returns the depth and the prelude simply discards it. B is a
reasonable alternative if the developer prefers one method over two. **D5 (MySQL) should be fixed
independently of the option chosen** — route `commit`/`rollback`'s control SQL through the
`DriverConn` seam (each driver spelling its own savepoint grammar, as `mysql.rs` already does for
`phorj_bulk`) rather than assuming one portable string, and add a nested-savepoint case to
`tests/database_mysql.rs` + `tests/database_postgres.rs`. **All of this is options + a
recommendation; Invariant 15 reserves the ruling.**

---

## Additional findings (D1..Dn, severity + grade)

Findings D1–D5 are stated in full above; recapped here with severity for the register.

| # | Finding | Sev | Grade |
|---|---|---|---|
| **D1** | `Statement` binds **append and never reset**; the developer's hold-statement-and-loop-binding scenario hard-errors on iteration 2 (`2 bound value(s) but 1 ? placeholder(s)`). DEC-208 rejected the one-shot shape *because* it had "no Statement reuse" (`C-decisions.md:515`) — the promise is unfulfilled. Undocumented in `KNOWN_ISSUES.md` | **P1** | [Verified: `ops.rs:75-84`, `ops.rs:154-211`; probe `reuse.phg` on both backends] |
| **D2** | The two bind styles diverge silently under reuse: the identical loop **works** with `bindNamed` (last-wins) and **errors** with `bind` | **P1** | [Verified: `handles.rs:131`, `sqlite.rs:210-218`; probes `named.phg` vs `reuse.phg`] |
| **D3** | The named-bind reuse path is **quadratic** and grows memory unboundedly: 4 000→1.135 s, 8 000→4.469 s, vs 0.049 s / 0.059 s for re-`prepare`. ~75× slower at 8 000 rows. Invariant 18 (WIN-OR-FLAG) exposure | **P1** | [Verified: measured, `namedperf.phg` vs `reprep.phg`] |
| **D4** | `db.transaction(fn)` auto-rollback pops **one** level (`wrappers.rs:133-136`). A closure that leaked an inner `begin()` leaves the outer transaction OPEN with partial writes live; the caught error reads as "rolled back" but a later `commit()` persists the data | **P1** | [Verified: probes `leak.phg`, `leak2.phg`] |
| **D5** | Nested savepoint control SQL is not MySQL-portable: `ops.rs:408` emits bare `RELEASE id` (MySQL needs `RELEASE SAVEPOINT id`) and `ops.rs:434` emits a `;`-joined pair through `query_drop` (single-statement). The module's OWN `mysql.rs:156-157` uses the correct full forms as separate statements. `rollback_inner` decrements the depth *before* the SQL, so failure desyncs `tx_depth`. **Zero nested-savepoint coverage on MySQL or Postgres** | **P1** | [Inferred — strong: four code sites read; in-repo self-contradiction is the evidence. Vendor doc unreachable (HTTP 403). Coverage gap: Verified from `tests/database_mysql.rs`, `tests/database_postgres.rs`] |
| **D6** | **Spec/code divergence (Invariant 19)**: `docs/specs/UNIFIED-SPEC.md:1283` says *"Spine/LADDER: case-1 (faithful → PHP PDO transpile)"*, but the shipped behaviour is LADDER **case 2** — `E-TRANSPILE-DB` hard error (`src/cli/pipeline.rs:596-600`), and `FEATURES.md:103` correctly says *"native-only (`E-TRANSPILE-DB`, §14 LADDER)"*. Every registry `php:` emitter is an admitted placeholder (`registry_rows.rs:26-27`, `registry.rs:130-132`). Three surfaces, two answers | **P2** (doc, but Inv-19 is explicit about zero divergence) | [Verified: read all three] |
| **D7** | **Invariant 17 lift gap**: no PDO→phorj lifting exists at all — `grep -rn "PDO\|pdo" src/lift/` returns zero matches, and every DB native carries `lift_from: &[]`. A PHP program using PDO lifts to nothing recognisable, so `Core.DatabaseModule` is unreachable from lifted code | **P2** | [Verified: grep + `registry.rs`/`registry_rows.rs` `lift_from` fields] |
| **D8** | The shipped release binary carries **SQLite only**: `database` is a default feature, `database-postgres`/`database-mysql` are not (`Cargo.toml:51,100,106,110`). A `postgres://` DSN yields *"the postgres driver is not compiled in (build with --features database-postgres)"* — a clean, actionable `ConnectionError`, correctly never falling through to the SQLite file path | Info (good behaviour) | [Verified: ran `probe-db/pg.phg` with both DSN schemes] |
| **D9** | No `db.transactionDepth()` / `inTransaction()`. `begin_inner` computes and returns the new depth (`ops.rs:389`) and the prelude throws it away (`prelude.rs:337`), so a user cannot write a correct unwind loop — only a blind bounded `rollbackQuiet()` loop | **P2** | [Verified: read both sites; probe `abortall.phg` shows the blind-loop workaround] |
| **D10** | No `Statement.close()`; a `Statement` outlives nothing and dies with the connection (`handles.rs:141`, `ops.rs:443-451`). `db.close()` correctly invalidates all derived statements → `ConnectionError`. Becomes user-visible if D1 is fixed (a long-lived reusable statement). `using`/`Closable` is DEC-203, ruled-but-unbuilt | **P3** | [Verified: read; `KNOWN_ISSUES.md:735-739`] |

### Cross-cutting posture notes (asked for explicitly)

* **Connection lifecycle.** `close()` is idempotent and never throws [`prelude.rs:386-388`]; the
  shared `Rc<RefCell<Option<…>>>` means one `close()` invalidates every derived `Statement`
  [`ops.rs:448`]. No scope-bound cleanup (DEC-203). `close()` also resets `tx_depth` to 0
  [`ops.rs:449`] — i.e. closing silently abandons an open transaction (SQLite/driver-level
  implicit rollback), which is defensible but undocumented. [Verified]
* **Error model.** Fault-based, not `Result`-based, and deliberately so: natives *never* hard-fault
  on a DB error — they return a prelude-local `DatabaseResult<T>` **value**, and the phorj-source
  prelude `match`es it and `throw`s a real catchable `DatabaseError` via the single classification
  point `DatabaseError.fail` [Verified: `handles.rs:14-35`, `prelude.rs:58-66`]. This was ruled as
  DEC-208's "error-mechanism = Option A" because the native ABI has no throws channel
  [`C-decisions.md:531-545`]. The taxonomy (6 subtypes, all `extends DatabaseError`) means
  `catch (DatabaseError e)` still catches everything while `catch (UniqueViolationError e)` is
  precise — genuinely better than PDO's silent `false`/`null`.
* **SQL-injection posture — the "better than PHP" doctrine is honoured.** Prepared-first: there is
  *no* `db.query(sql)` one-shot at all, so every path goes through `prepare` + binds [Verified:
  the `Database` method table above]. On top of that, a compile-time `W-SQL-INJECTION` lint fires
  when `prepare` receives an interpolated string literal with any non-constant hole — type-directed
  (receiver must be `Database`) and import-gated (program must import `Core.DatabaseModule`), so a
  user class named `Database` is never hijacked; non-fatal, so the deliberate-query escape hatch
  stays open [Verified: `src/checker/calls/lint.rs:14-86`]. `bindList` also gives typed `IN (?)`
  expansion, which PDO cannot do [`ops.rs:91-116`]. **One caveat**: the lint only inspects the
  literal passed *directly* to `prepare` — `string sql = "… {userInput}"; db.prepare(sql);` is not
  a `Expr::Str` at the call site and does not warn [Inferred: `lint.rs:38-43` requires
  `Expr::Str` as `args.first()`]. That is the standard limitation of a syntactic taint check, worth
  a KNOWN_ISSUES line.
* **`withTransaction(fn)` vs manual — both ship, deliberately.** `db.transaction(fn, retries = 0)`
  (commit-on-return, auto-rollback + re-throw the ORIGINAL typed error, nested = savepoint, retry
  on `SerializationFailureError` only) *and* manual `begin`/`commit`/`rollback`/`rollbackQuiet` —
  the developer ruled BOTH [Verified: `prelude.rs:352-381`, `KNOWN_ISSUES.md:721-723`]. The
  throw-preservation mechanism is genuinely careful: `rollback_inner` never re-enters the backend,
  so `pending_throw` survives and the caller catches the exact typed error
  [`wrappers.rs:103-137`]. D4 is the one hole in that otherwise-solid design.
* **Register coverage for the three questions: none of them has an existing row or spec entry.**
  Q1 (`Database`→`Connection`), Q2 (statement reuse / bind reset), Q3 (abort-everything) are all
  **new** questions. [Verified: greps documented in each section above.] Under Invariant 19 each
  ruling needs a register row + MASTER-PLAN/SLICE-STATE reflection in the same change.

---

## Options & recommendation per question

| Q | Recommendation (developer rules — Invariant 15) | Why, in one line |
|---|---|---|
| **Q1 naming** | **Option B** — rename the type `Database` → `Connection` **and** drop the module to bare `Core.Database`; fallback **A** (type only) | The object is provably ONE connection (4 code proofs, pooling explicitly out of scope), `Connection` is the near-universal name for that, `Database`/`DB` elsewhere means the *pool*, and DEC-278's `Module` suffix exists solely for the namesake collision the rename removes |
| **Q2 prepared statements** | **Option A** — reset binds after each successful execute so `Statement` is genuinely reusable for BOTH bind styles; ship the KNOWN_ISSUES/spec/register disclosure with it; fallback **B** (explicit `reset()`) | Smallest change that fixes D1+D2+D3 together; the driver already caches + resets (`prepare_cached`), so there is no perf cost; delivers the "Statement reuse" DEC-208 cited as its reason for the chosen shape |
| **Q3 savepoints** | **Option A + D**, with **E** as a cheap companion — add `rollbackAll()` (+ a quiet form), make `db_transaction`'s error arm unwind to its entry depth, and expose the depth `begin_inner` already computes; fix **D5** (MySQL savepoint grammar via the `DriverConn` seam + nested-savepoint tests) independently | The answer to the question is currently "no"; one top-level `ROLLBACK` discards every savepoint regardless of depth, so `rollbackAll()` is one statement not a loop — and it is also the clean fix for the P1 silent-persistence bug D4 |

**Priority ordering if only some land**: D4 (silent data persistence after a reported rollback) →
D5 (nested savepoints broken + untested on MySQL) → D1/D2/D3 (the developer's actual Q2 scenario,
plus a 75× perf trap) → Q1 naming → D6/D7/D9/D10.

---

*Method note: 8 probe programs written and run against the shipped release binary
(`target/release/phg`, SQLite driver); the reuse probe was additionally confirmed byte-identical
on `phg run --tree-walker`. No cargo build was run (disk constraint). No repo file was modified.
Cross-language naming details are graded [Unverified] — `WebFetch` to dev.mysql.com and
mariadb.com both returned HTTP 403 in this session, so no vendor documentation could be opened;
the MySQL savepoint-grammar claim rests on the module's own self-contradiction instead and is
graded [Inferred — strong].*
