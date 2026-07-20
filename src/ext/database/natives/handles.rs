//! The backend-agnostic handle layer for `Core.DatabaseModule` (DEC-208): the `DatabaseResult` value
//! wrappers ([`success`]/[`failure`]/[`wrap`]/[`wrap_unit`]), the opaque connection / statement / cursor
//! handles ([`DbConn`]/[`DbStmt`]/[`DbCursor`], carried by [`Value::Db`] via [`DbObject`]), the bind
//! accumulator ([`Binds`]/[`PosBind`]), the downcast helpers ([`as_conn`]/[`as_stmt`]/[`as_cursor`]),
//! and the closed-connection fault string ([`conn_closed`]).

use super::driver::DriverConn;
use crate::phstr::PhStr;
use crate::value::{DbObject, EnumVal, Value};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Wrap a success payload as `DatabaseResult.Ok(v)`. The natives here NEVER fault on a DB error (a native
/// `Err(String)` is an uncatchable hard fault — `vm/exec.rs`); instead they return this `DatabaseResult<T>`
/// VALUE, and the phorj-source `Core.DatabaseModule` prelude `match`es it and `throw`s a catchable `DatabaseError`
/// (DEC-208 error-mechanism = prelude-wrapper). `DatabaseResult` is a PRELUDE-LOCAL enum (defined in
/// DB_PRELUDE, injected with it) — NOT `Core.Result`, whose injection sits earlier in the module chain
/// and so is not pulled in by `Core.DatabaseModule`'s transitive import (importer-after-imported doesn't inject).
pub(super) fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "DatabaseResult".into(),
        variant: "Ok".into(),
        payload: crate::value::Payload::One(v),
    }))
}

/// Wrap a DB error message as `DatabaseResult.Err(msg)` — the prelude turns this into `throw DatabaseError`.
pub(super) fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "DatabaseResult".into(),
        variant: "Err".into(),
        payload: crate::value::Payload::One(Value::Str(msg.into())),
    }))
}

thread_local! {
    /// One cached `DatabaseResult.Ok(null)` carrier, reused (Rc bump, no alloc) by every op whose Ok
    /// payload the prelude discards — `bind`/`bindNamed`/`bindList` (`Ok(_) => this`) and
    /// `begin`/`commit`/`rollback`/`timeout`/`onQuery` (`Ok(_) => Database.ok()`). `bind` alone runs
    /// ~40k times in the dbwork macro-bench; skipping its per-call carrier allocation (and the
    /// discarded handle clone in `bind_inner`) is the dbwork alloc lever (DEC-292). Never dropped to
    /// zero (the thread-local holds one ref), so cloning it is a pure refcount bump.
    static OK_UNIT: Value = Value::Enum(Rc::new(EnumVal {
        ty: "DatabaseResult".into(),
        variant: "Ok".into(),
        payload: crate::value::Payload::One(Value::Null),
    }));
}

/// The cached unit success carrier (see [`OK_UNIT`]) — an Rc bump, not an allocation.
fn success_unit() -> Value {
    OK_UNIT.with(|v| v.clone())
}

/// Like [`wrap`], but for ops whose Ok payload the prelude ignores: the success arm returns the
/// cached [`success_unit`] carrier instead of allocating a fresh one, and the inner body's Ok value
/// (if any) is discarded. The Err path is unchanged (a real message the prelude throws on).
pub(super) fn wrap_unit(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(_) => success_unit(),
        Err(msg) => failure(msg),
    }
}

/// Map an inner body's `Result<payload, db-error-message>` onto the returned `Result<T, string>` VALUE:
/// `Ok(v) → Success(v)`, `Err(msg) → Failure(msg)`. A DB error thus becomes a value the prelude throws
/// on, never an uncatchable native fault. (An arity/shape bug the checker forbids stays an `Err` — a
/// hard fault — because it is a program-construction error, not a recoverable DB error.)
pub(super) fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

/// A live database connection handle (`Value::Db` payload). Shared-mutable: cloning the `Value::Db`
/// shares this `Rc`, so all bindings name the same connection.
///
/// The driver is wrapped in an `Option` so `close()` (DEC-208 slice C) can deterministically drop it —
/// every derived `DbStmt` shares the same `Rc<RefCell<Option<Box<dyn DriverConn>>>>`, so closing
/// invalidates all of them (a later op then faults with `<<ConnectionError>>`). `tx_depth` is the
/// transaction / savepoint nesting level (0 = no open transaction), shared (`Rc<Cell>`) across every
/// binding of the connection AND every derived statement: `begin` opens `BEGIN` at depth 0 and a
/// `SAVEPOINT` deeper, `commit`/`rollback` `RELEASE`/`ROLLBACK TO` the innermost level — so transactional
/// helpers compose (an inner rollback never aborts the outer), and a statement's `executeMany` can tell
/// whether a caller transaction is already open (load-bearing for the Postgres driver's bulk savepoint).
#[derive(Debug)]
pub(super) struct DbConn {
    pub(super) driver: Rc<RefCell<Option<Box<dyn DriverConn>>>>,
    pub(super) tx_depth: Rc<Cell<u32>>,
    /// The `onQuery` observability hook (DEC-208 slice D, spec §7): a `(string sql, int ms) => void`
    /// phorj closure invoked after each `query`/`exec`, or `None`. Held behind a SHARED `Rc<RefCell>`
    /// so a [`DbStmt`] derived from this connection observes the SAME hook — a statement carries only
    /// the connection's shared cells (not the whole `DbConn`), and a hook registered AFTER `prepare`
    /// must still fire. Storing a `Value::Closure` is cheap (an `Rc` bump) and never inspected here.
    pub(super) hook: Rc<RefCell<Option<Value>>>,
    /// The connection's query timeout in ms (DEC-208 slice D, spec §7), `0` = unset. Shared with
    /// derived statements (same rationale as `hook`). Setting it also arms SQLite's `busy_timeout`; when
    /// `> 0`, a transient `busy`/`locked` failure is reclassified `SerializationFailureError` → `TimeoutError`
    /// (the bounded lock-wait was exceeded). See [`super::ops::remap_timeout`].
    pub(super) timeout_ms: Rc<Cell<i64>>,
}

impl DbObject for DbConn {
    fn kind(&self) -> &'static str {
        "db-connection"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// One positional bind entry. A plain `bind(v)` is [`One`](PosBind::One) — it fills a single `?`. A
/// `bindList([..])` (DEC-208 slice D) is [`List`](PosBind::List) — it also occupies exactly ONE `?`
/// slot (left-to-right with `One`), but that `?` is EXPANDED to `(?,?,…)` at execute time, one `?` per
/// value (`expand_placeholders`), giving a typed `IN`-list bind PDO cannot do.
#[derive(Debug, Clone)]
pub(super) enum PosBind {
    One(Value),
    List(Vec<Value>),
}

/// Accumulated bind parameters for a prepared statement. Positional and named are mutually exclusive
/// per statement (the surface's contract) — mixing is a DB error (`Failure`, catchable).
#[derive(Debug, Default, Clone)]
pub(super) enum Binds {
    #[default]
    None,
    Positional(Vec<PosBind>),
    Named(Vec<(String, Value)>),
}

/// A lazily-executed prepared statement handle. rusqlite's `Statement` borrows its `Connection`, so
/// storing a live one in a `Value` would leak a lifetime; instead the handle keeps the connection
/// `Rc`, the SQL text, and the accumulated binds, and prepares+binds+executes eagerly at `query`/
/// `exec` (fetch-all semantics, like PDO). `binds` is interior-mutable so a chained `.bind(v)` mutates
/// in place and returns the same shared handle.
#[derive(Debug)]
pub(super) struct DbStmt {
    pub(super) driver: Rc<RefCell<Option<Box<dyn DriverConn>>>>,
    /// The statement SQL, held as the shared `PhStr` from the `prepare(sql)` argument — a clone is an
    /// Rc bump (heap variant) or a small inline copy (≤22 bytes), never a fresh `String` heap alloc per
    /// prepare (DEC-266 dbwork lever). Derefs to `&str` for the driver, so the exec/query sites are
    /// unchanged.
    pub(super) sql: PhStr,
    pub(super) binds: RefCell<Binds>,
    /// The originating connection's `onQuery` hook and query timeout, shared by `Rc` (see [`DbConn`]).
    /// A statement carries these (not the whole `DbConn`) so `query`/`exec` can fire the hook and apply
    /// the timeout classification without a back-reference to the connection object.
    pub(super) hook: Rc<RefCell<Option<Value>>>,
    pub(super) timeout_ms: Rc<Cell<i64>>,
    /// The shared transaction depth (see [`DbConn::tx_depth`]) — `executeMany` reads it to decide whether
    /// to open its own transaction (Postgres) or ride an existing one (savepoint).
    pub(super) tx_depth: Rc<Cell<u32>>,
}

impl DbObject for DbStmt {
    fn kind(&self) -> &'static str {
        "db-statement"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A streaming result cursor (DEC-208 item H): `stmt.stream()` → one of these; `streamNext` pulls one
/// row at a time (`null` when exhausted). The SURFACE contract is row-at-a-time delivery + lazy
/// per-row hydration (a `streamInto<T>` stream hydrates only the rows actually pulled — early exit
/// skips the rest). DISCLOSED LIMIT (KNOWN_ISSUES): today both drivers MATERIALIZE the result set at
/// `stream()` — rusqlite's and postgres's incremental row iterators borrow their statement/connection,
/// a self-referential lifetime a `#![deny(unsafe_code)]` handle cannot hold — so the cursor walks an
/// owned buffer. A driver that gains true incremental stepping (e.g. Postgres portals via a dedicated
/// `DriverConn` method) upgrades underneath this same surface with no user-visible change.
#[derive(Debug)]
pub(super) struct DbCursor {
    pub(super) rows: RefCell<std::vec::IntoIter<Value>>,
}

impl DbObject for DbCursor {
    fn kind(&self) -> &'static str {
        "db-cursor"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(super) fn as_cursor(v: &Value) -> Result<&DbCursor, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<DbCursor>()
            .ok_or_else(|| "Core.DatabaseModule: expected a stream cursor".to_string()),
        other => Err(format!(
            "Core.DatabaseModule: expected a stream cursor, got {}",
            other.type_name()
        )),
    }
}

/// Downcast a `Value::Db` handle to a concrete resource, or a clean fault (checker-unreachable once the
/// surface enforces the receiver types, but the natives stay total).
pub(super) fn as_conn(v: &Value) -> Result<&DbConn, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<DbConn>()
            .ok_or_else(|| "Core.DatabaseModule: expected a connection".to_string()),
        other => Err(format!(
            "Core.DatabaseModule: expected a connection, got {}",
            other.type_name()
        )),
    }
}

pub(super) fn as_stmt(v: &Value) -> Result<&DbStmt, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<DbStmt>()
            .ok_or_else(|| "Core.DatabaseModule: expected a statement".to_string()),
        other => Err(format!(
            "Core.DatabaseModule: expected a statement, got {}",
            other.type_name()
        )),
    }
}

/// The catchable message for using a connection (or a statement derived from it) after `close()`.
/// Tagged `ConnectionError` so `catch (ConnectionError e)` is precise.
pub(super) fn conn_closed() -> String {
    "<<ConnectionError>>Core.DatabaseModule: the connection is closed".to_string()
}
