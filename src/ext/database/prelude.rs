//! `Core.DatabaseModule` (DEC-208) — the enhanced-PDO surface: `Database`/`Statement`/`Row`/`DatabaseError` phorj-source classes
//! wrapping the opaque `DatabaseHandle` and the internal `Core.Native.Database` natives. Each method calls a
//! `NativeDatabase.*` native (which returns `Result<T, string>` — never a hard fault) and `match`es it, throwing
//! a catchable `DatabaseError` on `Failure` (DEC-208 error-mechanism = prelude-wrapper; a phorj-source `throw`
//! is a real `Op::Throw`, byte-identical across both backends). `import Core.DatabaseModule` transitively imports
//! `Core.Native.Database` (the natives) + `Core.Result` (the carrier), so this module runs BEFORE them.
//!
//! DEC-273 wave 3: colocated with the `database` extension. Compiled UNCONDITIONALLY (the
//! `CORE_MODULES` const array references it on every build; the disabled-import gate rejects the
//! import on gated builds before the prelude matters).

pub const PRELUDE: &str = r#"
import Core.Native.Database as NativeDatabase;
import Core.List;
import Core.String;
import Core.IteratorModule;
import Core.Abort.panic;
// `Core.Map` is imported for the `queryMap<K,V>` hydration helpers the `desugar_db` pass generates
// into a `Core.DatabaseModule` program (they build the result `Map` via `Map.set`); like the `Core.List` import
// above it makes the module's ops available to the generated helpers (and, as with `List`, to user
// code under an `import Core.DatabaseModule`).
import Core.Map;
// `Core.Secret` (Fork B) provides the opaque `Secret<T>` credential wrapper used by the `Database.withPassword`
// factory (DEC-208 slice G): a connection password is passed as a `Secret<string>` so it never sits in
// plaintext in user code, and — because the driver parses it out of the DSN and retains only a redacted
// DSN — is masked in every connect error / log. Secret is registered before Database (see CORE_MODULES order),
// so this transitive import injects the class here exactly as List/String/Map above.
import Core.Secret;

// Prelude-local result carrier (NOT Core.Result — see the native docs on injection order).
enum DatabaseResult<T> { Ok(T value), Err(string message) }

// Column NAMING STRATEGY (DEC-208 slice B2; DEC-258 combined model): the mapping between DB column
// names and phorj field names. Zero-payload variants (construct with `new Naming.SnakeToCamel()`,
// like `RoundingMode`). Member-gated (`import Core.DatabaseModule.Naming;`) — nothing in the wind.
// DEC-258: no longer compile-time-only — `naming` is a REAL promoted field on `Database` (ctor
// default `new Naming.Exact()`) that `prepare` copies onto each `Statement`, so the strategy
// follows the VALUE across any scope. The `desugar_db` pass still BAKES column literals when the
// strategy is statically visible (zero runtime cost); when the connection is not traceable it
// emits both baked variants and dispatches on the statement's `naming` field at run time (one
// branch per hydration call — never per-row string work). Per-statement `namingStrategy(...)`
// overrides either way.
enum Naming { Exact(), SnakeToCamel() }

open class DatabaseError implements Error {
  constructor(public string message) {}
  // `throw` is a statement, not an expression, so it cannot be a `match` arm value directly. This
  // `never`-returning helper lets a `DatabaseResult.Err(e)` arm raise a catchable exception as an expression
  // (`DatabaseResult.Err(e) => DatabaseError.fail(e)`) — a call to a `never` function types as the bottom type,
  // unifying with the success arm's value type.
  //
  // It is ALSO the single classification point (DEC-208 slice C, spec §6): the native tags a driver
  // error with a `<<Kind>>` marker prefix (`src/ext/database/natives.rs` `err_kind`), and `fail` strips the marker
  // and throws the matching TYPED subtype. Because every Row/Statement/Database method — including the S2
  // `queryInto` hydration helpers — funnels its `DatabaseResult.Err` through here, they all yield the precise
  // `catch (UniqueViolationError e)` type with zero change at the call sites. An untagged message (a logic /
  // usage error, e.g. mixed bind styles, or a plain SQLite failure) throws the base `DatabaseError`.
  static function fail(string message): never throws DatabaseError {
    if (String.startsWith(message, "<<UniqueViolationError>>")) { throw new UniqueViolationError(String.removePrefix(message, "<<UniqueViolationError>>")); }
    if (String.startsWith(message, "<<ConstraintViolationError>>")) { throw new ConstraintViolationError(String.removePrefix(message, "<<ConstraintViolationError>>")); }
    if (String.startsWith(message, "<<ConnectionError>>")) { throw new ConnectionError(String.removePrefix(message, "<<ConnectionError>>")); }
    if (String.startsWith(message, "<<SerializationFailureError>>")) { throw new SerializationFailureError(String.removePrefix(message, "<<SerializationFailureError>>")); }
    if (String.startsWith(message, "<<TimeoutError>>")) { throw new TimeoutError(String.removePrefix(message, "<<TimeoutError>>")); }
    if (String.startsWith(message, "<<SyntaxError>>")) { throw new SyntaxError(String.removePrefix(message, "<<SyntaxError>>")); }
    throw new DatabaseError(message);
  }
}

// Typed error taxonomy (DEC-208 slice C, spec §6). Each `extends DatabaseError`, so `catch (DatabaseError e)`
// still catches EVERY DB error while `catch (UniqueViolationError e)` catches exactly one kind. The native
// maps rusqlite (extended) result codes to the marker `DatabaseError.fail` reads. `SerializationFailureError` is
// the transient class `retry` targets (SQLite `SQLITE_BUSY`/`SQLITE_LOCKED`) — it is the spec's
// `Deadlock` under a single name. `TimeoutError` is part of the taxonomy contract; SQLite has no source for
// it yet (it arrives with query `.timeout(ms)`, slice D), so it is currently only ever caught, not thrown.
class UniqueViolationError extends DatabaseError { constructor(string message) { parent.constructor(message); } }
class ConstraintViolationError extends DatabaseError { constructor(string message) { parent.constructor(message); } }
class ConnectionError extends DatabaseError { constructor(string message) { parent.constructor(message); } }
class SerializationFailureError extends DatabaseError { constructor(string message) { parent.constructor(message); } }
class TimeoutError extends DatabaseError { constructor(string message) { parent.constructor(message); } }
class SyntaxError extends DatabaseError { constructor(string message) { parent.constructor(message); } }

class Row {
  constructor(private DatabaseHandle raw) {}
  function getInt(string column): int throws DatabaseError {
    return match (NativeDatabase.getInt(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getString(string column): string throws DatabaseError {
    return match (NativeDatabase.getString(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getFloat(string column): float throws DatabaseError {
    return match (NativeDatabase.getFloat(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getBool(string column): bool throws DatabaseError {
    return match (NativeDatabase.getBool(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Nullable accessors (DEC-208 S2): a SQL NULL yields `null` rather than throwing; a wrong non-null
  // storage type still throws. Used by the dynamic path and by the `queryInto` hydration of `T?` fields.
  function getIntOrNull(string column): int? throws DatabaseError {
    return match (NativeDatabase.getIntOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getStringOrNull(string column): string? throws DatabaseError {
    return match (NativeDatabase.getStringOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getFloatOrNull(string column): float? throws DatabaseError {
    return match (NativeDatabase.getFloatOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getBoolOrNull(string column): bool? throws DatabaseError {
    return match (NativeDatabase.getBoolOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Decimal accessors (DEC-208 slice E): a `decimal`/`decimal?` hydration field maps its column here —
  // exact money (a TEXT column is parsed exactly; never through float). Used by the dynamic path and by
  // the `queryInto` hydration of a `decimal` field (via `desugar_db`'s `accessor_for`).
  function getDecimal(string column): decimal throws DatabaseError {
    return match (NativeDatabase.getDecimal(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getDecimalOrNull(string column): decimal? throws DatabaseError {
    return match (NativeDatabase.getDecimalOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Typed ARRAY-column accessors (DEC-208 slice K) — a Postgres `int[]`/`text[]`/`float8[]`/`bool[]`
  // column reads as a typed `List<scalar>`. STRICT: a non-array column, a wrong element type, or a
  // NULL element throws a catchable DatabaseError (filter NULL elements in SQL: `array_remove(col, NULL)`);
  // the `OrNull` forms admit a whole-array SQL NULL. Numeric/decimal arrays: select `col::text[]` and
  // read `getStringList` (the slice-E decimal-as-text discipline, element form).
  function getIntList(string column): List<int> throws DatabaseError {
    return match (NativeDatabase.getIntList(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getStringList(string column): List<string> throws DatabaseError {
    return match (NativeDatabase.getStringList(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getFloatList(string column): List<float> throws DatabaseError {
    return match (NativeDatabase.getFloatList(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getBoolList(string column): List<bool> throws DatabaseError {
    return match (NativeDatabase.getBoolList(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getIntListOrNull(string column): List<int>? throws DatabaseError {
    return match (NativeDatabase.getIntListOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getStringListOrNull(string column): List<string>? throws DatabaseError {
    return match (NativeDatabase.getStringListOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getFloatListOrNull(string column): List<float>? throws DatabaseError {
    return match (NativeDatabase.getFloatListOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function getBoolListOrNull(string column): List<bool>? throws DatabaseError {
    return match (NativeDatabase.getBoolListOrNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Column introspection (DEC-208 slice B) — the desugared `queryScalar`/`queryMap`/nested-hydration
  // helpers use these. `columnNames` is selection-ordered; `isNull` tests a column for SQL NULL.
  function columnNames(): List<string> throws DatabaseError {
    return match (NativeDatabase.columnNames(this.raw)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function isNull(string column): bool throws DatabaseError {
    return match (NativeDatabase.isNull(this.raw, column)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
}

class Statement {
  // DEC-258: `naming` rides the statement (copied from the connection by `Database.prepare`, or
  // replaced by `namingStrategy`) so an untraceable-connection hydration can dispatch on it at
  // run time. Public — the desugar's dynamic dispatcher reads it.
  constructor(private DatabaseHandle raw, public Naming naming = new Naming.Exact()) {}
  function bind(string | int | float | bool value): Statement throws DatabaseError {
    // The native binds onto the SHARED raw handle in place (interior-mutable accumulator) and returns
    // that same handle, so `this` already reflects the bind — return it (an Rc bump) instead of
    // allocating a fresh `new Statement(...)` per chained bind (DEC-266 dbwork alloc lever). Byte-
    // identical: same raw handle + same naming; validated by tests/database.rs on both backends.
    return match (NativeDatabase.bind(this.raw, value)) { DatabaseResult.Ok(_) => this, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function bindNamed(string name, string | int | float | bool value): Statement throws DatabaseError {
    return match (NativeDatabase.bindNamed(this.raw, name, value)) { DatabaseResult.Ok(_) => this, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Typed IN-list bind (DEC-208 slice D, spec §2): occupies one positional `?` slot (left-to-right
  // with bind()) that expands to `(?,?,…)` — one placeholder per value — at execute time; an empty list
  // becomes `(NULL)` (a never-true IN). Strictly safer than PDO (which cannot bind an array to IN).
  // Generic over the element type (a `List<int>`/`List<string>`/… all bind); a non-scalar element is a
  // runtime DatabaseError (an invariant `List<bindable>` union cannot accept a homogeneous list argument).
  function bindList<T>(List<T> values): Statement throws DatabaseError {
    return match (NativeDatabase.bindList(this.raw, values)) { DatabaseResult.Ok(_) => this, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function exec(): int throws DatabaseError {
    return match (NativeDatabase.exec(this.raw)) { DatabaseResult.Ok(n) => n, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Bulk write (DEC-208 slice D, spec §4): prepare ONCE, execute for each row of positional binds,
  // inside one savepoint (atomic + far faster than a loop). `rows` carries ALL binds (do not also call
  // bind()/bindNamed()). Returns the total affected rows. Generic over the row element type (same
  // reason as bindList); a non-scalar bind value is a runtime DatabaseError.
  function executeMany<T>(List<List<T>> rows): int throws DatabaseError {
    return match (NativeDatabase.executeMany(this.raw, rows)) { DatabaseResult.Ok(n) => n, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Exec an INSERT and return the auto-generated rowid / PK (DEC-208 slice D, spec §4) — exec + the
  // connection's last insert id in one call. (Database.lastInsertId() reads the same value standalone.)
  function execReturningId(): int throws DatabaseError {
    return match (NativeDatabase.execReturningId(this.raw)) { DatabaseResult.Ok(id) => id, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Column naming strategy (DEC-208 slice B2; DEC-258 combined model) — chainable, per query:
  // `stmt.namingStrategy(new Naming.SnakeToCamel()).queryInto()` maps a `userName` field from a
  // `user_name` column. A REAL copy-builder since DEC-258 (formerly a compile-time-only no-op):
  // the returned Statement carries the strategy in its `naming` field. The `desugar_db` pass still
  // BAKES the transformed column-name literals when the argument is a `new Naming.X()` literal in
  // the query's own chain (zero runtime cost, unchanged); a runtime `Naming` value — or a stored
  // `Statement s = stmt.namingStrategy(...); s.queryInto();` split — now dispatches on the field at
  // run time instead of being rejected/reverting (the old `E-DB-NAMING-NOT-CONST` and the
  // stored-statement-reverts-to-Exact footgun are both retired). Applies only to by-field-name
  // hydration (`queryInto`/`queryOneInto`, a `queryMap` entity value, `streamInto`);
  // `queryScalar`/scalar map values read by column position and ignore it.
  function namingStrategy(Naming strategy): Statement { return new Statement(this.raw, strategy); }
  function query(): List<Row> throws DatabaseError {
    return match (NativeDatabase.query(this.raw)) { DatabaseResult.Ok(rows) => Statement.wrapRows(rows), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Streaming (DEC-208 item H, DEC-257 reshape): run the query and deliver rows ONE AT A TIME via
  // the `Iterator<Row>` pull protocol (`hasNext()`/`next()` — foreach-able) instead of
  // materializing a `List<Row>` in user code. The typed form `stmt.streamInto<T>()` (desugar_db)
  // wraps this in a `DatabaseStream<T>` (an `Iterator<T>`) that hydrates each row only when pulled —
  // early exit skips the remaining rows' hydration entirely.
  function stream(): RowStream throws DatabaseError {
    return match (NativeDatabase.stream(this.raw)) { DatabaseResult.Ok(h) => new RowStream(h), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  private static function wrapRows(List<DatabaseHandle> rows): List<Row> {
    mutable List<Row> out = new List<Row>();
    mutable int i = 0;
    int n = List.length(rows);
    while (i < n) {
      out = List.append(out, new Row(rows[i]));
      i = i + 1;
    }
    return out;
  }
}

// A row-at-a-time result cursor (DEC-208 item H, RESHAPED by DEC-257) — the untyped streaming
// surface, an `Iterator<Row>`: `for (Row r in stmt.stream()) { … }` just works. `hasNext()` pulls
// one row ahead and caches it (so it carries the `throws` — the pull is where the driver can
// fail); `next()` hands over the cached row, or FAULTS "iterator exhausted" past the end (the
// DEC-257 misuse contract — foreach never triggers it).
class RowStream implements Iterator<Row> {
  constructor(private DatabaseHandle raw) {}
  private mutable Row? ahead;
  function hasNext(): bool throws DatabaseError {
    if (var cached = this.ahead) { return true; }
    DatabaseHandle? h = match (NativeDatabase.streamNext(this.raw)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
    if (var handle = h) {
      this.ahead = new Row(handle);
      return true;
    }
    return false;
  }
  function next(): Row throws DatabaseError {
    bool has = this.hasNext()?;
    if (has) {
      if (var r = this.ahead) {
        this.ahead = null;
        return r;
      }
    }
    panic("iterator exhausted");
  }
}

// The TYPED streaming surface (DEC-208 item H, RESHAPED by DEC-257): a lazy, hydrate-on-pull
// stream of `T` built by the `stmt.streamInto<T>()` desugar (which supplies the per-class
// hydration closure) — an `Iterator<T>`: `for (User u in stmt.streamInto<User>()) { … }` just
// works. Laziness is EXACT: `hasNext()` only asks the underlying cursor to pull a raw row ahead;
// hydration happens solely in `next()` — rows never pulled via `next()` are never hydrated.
// Past the end, `next()` FAULTS "iterator exhausted" (the DEC-257 misuse contract).
class DatabaseStream<T> implements Iterator<T> {
  constructor(private RowStream rows, private (Row) => T throws DatabaseError hydrate) {}
  function hasNext(): bool throws DatabaseError {
    bool has = this.rows.hasNext()?;
    return has;
  }
  function next(): T throws DatabaseError {
    Row row = this.rows.next()?;
    (Row) => T throws DatabaseError f = this.hydrate;
    T v = f(row)?;
    return v;
  }
}

class Database {
  // DEC-221: opening a connection can fail, so the constructor itself declares `throws DatabaseError` and
  // opens directly — `new Database(dsn)` (fail-fast, exactly like PHP's `new PDO`). No static factory. The
  // handle is COMPUTED in the body (not a promoted param), so the field is `mutable` (set once here).
  // DEC-258: `naming` is a promoted field with a variant default — the connection-level column
  // naming strategy. `prepare` copies it onto every Statement, so it follows the value into any
  // scope; per-statement `namingStrategy(...)` overrides it.
  private mutable DatabaseHandle raw;
  constructor(string dsn, public Naming naming = new Naming.Exact()) throws DatabaseError {
    this.raw = match (NativeDatabase.connect(dsn)) { DatabaseResult.Ok(h) => h, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Credential-safe connect (DEC-208 slice G, spec §1). The password is supplied as a `Core.Secret` —
  // kept out of plaintext in user code — and injected into the DSN only at the connect boundary. It is
  // NEVER retained: the driver parses it back out into its connection config and stores only a redacted
  // DSN, so a connect error prints the host but never the password (unlike PDO, which leaks the DSN in
  // exceptions). Use for a `postgres://user@host/db` DSN (no inline password); SQLite has no password,
  // so the DSN is passed through unchanged. Example:
  //   `Database db = Database.withPassword("postgres://app@db.host:5432/prod", new Secret(env));`
  static function withPassword(string dsn, Secret<string> password, Naming naming = new Naming.Exact()): Database throws DatabaseError {
    return new Database(NativeDatabase.dsnWithPassword(dsn, password.expose()), naming)?;
  }
  function prepare(string sql): Statement throws DatabaseError {
    return match (NativeDatabase.prepare(this.raw, sql)) { DatabaseResult.Ok(h) => new Statement(h, this.naming), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }

  // --- Writes & robustness (DEC-208 slice D, spec §4/§7). ---
  // The auto-generated rowid / PK of the most recent INSERT on this connection.
  function lastInsertId(): int throws DatabaseError {
    return match (NativeDatabase.lastInsertId(this.raw)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Arm a query timeout (ms): a bounded lock-wait (SQLite busy_timeout). Once set, a busy/locked
  // failure surfaces as `TimeoutError` rather than `SerializationFailureError`. Chainable (returns this).
  function timeout(int ms): Database throws DatabaseError {
    match (NativeDatabase.timeout(this.raw, ms)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
    return this;
  }
  // Register a per-query observability hook (logging / metrics / slow-query). The `(string sql, int
  // ms) => void` closure fires after each query/exec with the SQL text + elapsed ms. A logging hook is
  // `void` (cannot throw a checked error), so registration never fails. Chainable (returns this).
  // NOTE: `ms` is wall-clock (non-deterministic) — do not print it raw in a byte-identity example.
  function onQuery((string, int) => void hook): Database {
    match (NativeDatabase.onQuery(this.raw, hook)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(_) => Database.ok() };
    return this;
  }

  // A void no-op, used as the success arm of the `void`-returning transaction methods below — a `match`
  // arm must be an EXPRESSION, so `DatabaseResult.Ok(_) => Database.ok()` yields `void` cleanly (a bare `{}` block
  // is not an expression here). The `?` in the error arm makes that arm `never`, unifying to `void`.
  private static function ok(): void {}

  // --- Transactions & correctness (DEC-208 slice C, spec §5). Manual, PDO-faithful control. A nested
  // begin() opens a SAVEPOINT (composable partial rollback); commit()/rollback() release / roll back the
  // innermost level. commit()/rollback() at depth 0 are best-effort no-ops (the native guards the depth),
  // so a secondary fault can never mask the original. The closure form `db.transaction(fn)` + retry are
  // BLOCKED on phorj lambdas being unable to propagate a checked exception (see docs/KNOWN_ISSUES) —
  // recorded as a PENDING adjudication; this manual surface is what the closure form would build on. ---
  function begin(): void throws DatabaseError {
    match (NativeDatabase.begin(this.raw)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function commit(): void throws DatabaseError {
    match (NativeDatabase.commit(this.raw)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  function rollback(): void throws DatabaseError {
    match (NativeDatabase.rollback(this.raw)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(e) => DatabaseError.fail(e)? };
  }
  // Best-effort rollback that NEVER throws — safe inside a `finally` (a throwing rollback there would
  // mask the original exception). The auto-rollback idiom is: `db.begin(); mutable bool ok = false;
  // try { …work…; db.commit(); ok = true; } finally { if (!ok) db.rollbackQuiet(); }` — demonstrated in
  // examples/database/transactions.phg.
  function rollbackQuiet(): void {
    match (NativeDatabase.rollback(this.raw)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(_) => Database.ok() };
  }
  // Closure-form transaction (DEC-208 slice C, spec §5 — unblocked by DEC-222 throwing closures). BEGIN,
  // run the closure, COMMIT on a normal return (returning its value), auto-ROLLBACK + re-throw the
  // ORIGINAL typed error on a throw (`NativeDatabase.transaction` preserves the thrown value through the rollback
  // via the backend's `pending_throw`, so the caller catches the exact `DatabaseError` the closure threw — not
  // a generic one). A NESTED `db.transaction(fn)` opens a SAVEPOINT (composable partial rollback), so
  // transactions compose. The closure is a THROWING function type — `db.transaction(function(): T throws
  // DatabaseError { … })` — since DB work raises a checked `DatabaseError` (a non-throwing closure is also accepted:
  // fewer-throws variance). BOTH this closure form AND the manual begin()/commit()/rollback() above are
  // supported (developer ruled BOTH).
  // DEC-249 resolved the recorded SURFACE PENDING the ambitious way: method default parameters
  // landed, so the spec's single-method shape is real — `db.transaction(fn)` runs once;
  // `db.transaction(fn, retries)` re-runs the WHOLE transaction up to `retries` extra times on the
  // transient `SerializationFailureError` ONLY (SQLite SQLITE_BUSY/LOCKED — the class Serializable
  // isolation needs); any OTHER `DatabaseError` (and an exhausted retry budget) rolls back and
  // propagates immediately. The retry loop lives HERE, not in the native, because only phorj
  // source can `catch` the TYPED error (the thrown value is backend-side `pending_throw`,
  // invisible to a native). The former distinct `transactionRetry(fn, retries)` is RETIRED.
  // NOTE (timeout): with `db.timeout(ms)` armed a transient busy is reclassified `TimeoutError`, not
  // `SerializationFailureError` (slice D) — so it is NOT retried; leave the timeout unset when relying on retry.
  function transaction<T>(() => T throws DatabaseError fn, int retries = 0): T throws DatabaseError {
    mutable int attempt = 0;
    while (true) {
      try {
        return match (NativeDatabase.transaction(this.raw, fn)) { DatabaseResult.Ok(v) => v, DatabaseResult.Err(e) => DatabaseError.fail(e)? };
      } catch (SerializationFailureError e) {
        if (attempt >= retries) { throw e; }
        attempt = attempt + 1;
      }
    }
  }
  // Deterministic close (spec §1): idempotent, never throws. After close(), any further use of this
  // connection (or a Statement derived from it) fails with `ConnectionError`. The `using`/`Closable`
  // sugar that would call this automatically at scope exit is DEC-203 — a separate language slice
  // (see KNOWN_ISSUES); until then, call close() explicitly (or rely on drop at program end).
  function close(): void {
    match (NativeDatabase.close(this.raw)) { DatabaseResult.Ok(_) => Database.ok(), DatabaseResult.Err(_) => Database.ok() };
  }
}
"#;
