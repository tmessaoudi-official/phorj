//! `Core.DatabaseModule` — the enhanced-PDO database primitive (DEC-208), a MULTI-DRIVER runtime behind a scheme-
//! dispatched [`DriverConn`] trait (DEC-208 slice I): `sqlite:…` → [`sqlite`] (bundled `rusqlite`),
//! `postgres://…` → [`postgres`] (the sync `postgres` crate, `db-postgres` feature).
//!
//! Feature-gated (`db`) and native-only. This module owns the BACKEND-AGNOSTIC layer: the opaque
//! connection / statement handles ([`DbConn`] / [`DbStmt`], carried by [`Value::Db`] via the
//! [`DbObject`] trait, each holding a `Box<dyn DriverConn>`); the bind accumulator ([`Binds`]); the
//! internal `Core.Native.Database` native bodies for connect / prepare / bind / bindNamed / query / exec; the Row
//! accessors; and the (portable) transaction-control SQL (`BEGIN`/`COMMIT`/`SAVEPOINT`/`RELEASE`/
//! `ROLLBACK TO`, which SQLite and Postgres both accept). Each concrete backend implements only its
//! genuinely dialect-specific pieces (value mapping, placeholder syntax, error-code taxonomy) in its own
//! submodule. The public `Core.DatabaseModule` SURFACE (`Database`/`Statement`/`Row` + `new Database(dsn)`) is the phorj-source
//! `DB_PRELUDE` (`src/cli/preludes.rs`) on top of these — the natives live under the `DbSys` qualifier so
//! a prelude `class Database` never collides with them.
//!
//! **Error mechanism (DEC-208 = prelude-wrapper).** phorj's native ABI has no throws channel: a native's
//! `Err(String)` is an uncatchable HARD fault (`vm/exec.rs`), so it cannot express the ruled catchable
//! `throws DatabaseError` (Q6). Instead every native here returns a `DatabaseResult<T>` VALUE (`DatabaseResult.Ok(payload)`
//! on success, `DatabaseResult.Err(message)` on any DB error — it NEVER faults on a DB error); the phorj-source
//! prelude `match`es it and `throw`s a catchable `DatabaseError` (a real `Op::Throw`). `DatabaseResult` is a
//! prelude-LOCAL enum (not `Core.Result`, whose injection sits earlier in the module chain and so is not
//! pulled in by `Core.DatabaseModule`'s transitive import). Only a checker-unreachable arity/shape bug returns `Err`.
//! Each driver prefixes a `<<Kind>>` taxonomy marker on a classified error; the prelude's single
//! `DatabaseError.fail` strips it and throws the matching typed subtype.
//!
//! **Spine treatment.** Every native is `pure: false`, so `uses_impure_native` auto-excludes any
//! `import Core.DatabaseModule` program from the byte-identity differential (live DB I/O can't be byte-identical
//! across the drivers and PHP PDO). Correctness: the in-module unit tests + the `tests/db.rs` fixture.
//! `run ≡ runvm` holds unconditionally (both backends call these one shared `eval` bodies). The `php`
//! emitters (faithful PDO, DEC-208 LADDER case 1) are finalized in the DEC-208 transpile slice.

#[cfg(feature = "db-mysql")]
mod mysql;
#[cfg(feature = "db-postgres")]
mod postgres;
mod sqlite;

use super::{ClosureInvoker, NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{DbObject, EnumVal, HKey, Value};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A live database connection behind one backend (SQLite / Postgres / …). This is the multi-driver seam
/// (DEC-208 slice I): the generic layer holds a `Box<dyn DriverConn>` (carried by [`DbConn`] / [`DbStmt`]
/// and set to `None` on `close()`) and threads the accumulated [`Binds`] + portable transaction-control
/// SQL through it, never touching a dialect detail. Methods take `&self` (a backend needing `&mut` — like
/// the `postgres` client — wraps it in a `RefCell` internally); a DB error is a plain `Err(String)` with
/// an optional leading `<<Kind>>` taxonomy marker (the generic layer wraps it into a `DatabaseResult.Err`).
trait DriverConn: std::fmt::Debug {
    /// Run a SELECT with the accumulated binds; returns `Ok(List<Row>)` (each Row a column→value `Map`).
    fn query(&self, sql: &str, binds: &Binds) -> Result<Value, String>;
    /// Run a write with the accumulated binds; returns the affected-row count.
    fn exec(&self, sql: &str, binds: &Binds) -> Result<i64, String>;
    /// Run an INSERT and return the auto-generated id (SQLite `last_insert_rowid()`; Postgres
    /// `RETURNING`/`lastval()`).
    fn exec_returning_id(&self, sql: &str, binds: &Binds) -> Result<i64, String>;
    /// The connection-level last-insert id.
    fn last_insert_id(&self) -> Result<i64, String>;
    /// Bulk insert: prepare once, execute for each positional-value row, atomically. `in_transaction`
    /// tells the backend whether a caller transaction is already open (Postgres opens its own `BEGIN` at
    /// depth 0 since it rejects a standalone `SAVEPOINT`; SQLite ignores the flag).
    fn execute_many(&self, sql: &str, rows: &[Value], in_transaction: bool) -> Result<i64, String>;
    /// Run a transaction-control statement (portable `BEGIN`/`COMMIT`/`SAVEPOINT`/`RELEASE`/`ROLLBACK`).
    fn control(&self, sql: &str) -> Result<(), String>;
    /// Arm a query/lock timeout in ms (`0` = unset). SQLite: `busy_timeout`; Postgres: `statement_timeout`.
    fn set_timeout(&self, ms: i64) -> Result<(), String>;
}

/// Dispatch a DSN onto its backend driver (DEC-208 slice I). `postgres://` / `postgresql://` →
/// [`postgres`] (feature `db-postgres`; a clear feature-gated `ConnectionError` when it is off — never a
/// fall-through to the SQLite file path); everything else (`sqlite:`, `:memory:`, `sqlite::memory:`, or a
/// bare path) → [`sqlite`], unchanged from the shipped runtime.
fn open_driver(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    if dsn.starts_with("postgres://") || dsn.starts_with("postgresql://") {
        return open_postgres(dsn);
    }
    if dsn.starts_with("mysql://") || dsn.starts_with("mariadb://") {
        return open_mysql(dsn);
    }
    sqlite::open(dsn)
}

#[cfg(feature = "db-mysql")]
fn open_mysql(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    mysql::open(dsn)
}

#[cfg(not(feature = "db-mysql"))]
fn open_mysql(_dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    Err(
        "<<ConnectionError>>Core.DatabaseModule: the mysql driver is not compiled in \
         (build with --features db-mysql)"
            .to_string(),
    )
}

#[cfg(feature = "db-postgres")]
fn open_postgres(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    postgres::open(dsn)
}

#[cfg(not(feature = "db-postgres"))]
fn open_postgres(_dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    Err(
        "<<ConnectionError>>Core.DatabaseModule: the postgres driver is not compiled in \
         (build with --features db-postgres)"
            .to_string(),
    )
}

/// Inject a password into a `postgres://` DSN's authority (DEC-208 slice G — the `Database.withPassword`
/// factory). The password is percent-encoded and placed as the userinfo password
/// (`postgres://user:PW@host/…`); an existing DSN password is replaced. This is a PURE string transform
/// (no `postgres` dep), so the `Database.withPassword` surface type-checks under the plain `db` feature; the
/// resulting DSN is consumed IMMEDIATELY by `new Database(...)` inside the factory (never surfaced to user
/// code), and the driver parses the password back OUT into its config and stores only a redacted DSN, so
/// nothing retains the plaintext. A non-postgres DSN is returned unchanged (SQLite has no password).
fn inject_pg_password(dsn: &str, pw: &str) -> String {
    // Every URL-authority DSN takes the injected credential: postgres AND mysql/mariadb (slice J) —
    // a `Database.withPassword` on a mysql DSN must never silently no-op. SQLite has no password; a bare
    // path / `sqlite:` DSN is returned unchanged.
    let url_scheme = ["postgres://", "postgresql://", "mysql://", "mariadb://"]
        .iter()
        .any(|s| dsn.starts_with(s));
    if !url_scheme {
        return dsn.to_string();
    }
    let Some(idx) = dsn.find("://") else {
        return dsn.to_string();
    };
    let enc = percent_encode(pw);
    let (scheme, rest) = dsn.split_at(idx + 3);
    let auth_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(auth_end);
    match authority.find('@') {
        Some(at) => {
            let userinfo = &authority[..at];
            let hostpart = &authority[at..]; // includes the '@'
            let user = userinfo.split(':').next().unwrap_or(userinfo);
            format!("{scheme}{user}:{enc}{hostpart}{tail}")
        }
        None => format!("{scheme}:{enc}@{authority}{tail}"),
    }
}

/// Replace the password component of a DSN with `***`, for use in connect diagnostics. Handles both the
/// URL form (`postgres://user:PASS@host/db`) and the keyword form (`host=… password=PASS …`,
/// space-delimited or single-quoted). Defense-in-depth: the password is also never RETAINED on the
/// handle (it lives only transiently in the [`Config`] during connect), so this scrubs the one place a
/// raw DSN could still surface — the connect-time error path.
#[cfg_attr(
    not(any(feature = "db-postgres", feature = "db-mysql")),
    allow(dead_code)
)]
fn redact_dsn_password(dsn: &str) -> String {
    // URL form: userinfo password is between the first ':' after "://" and the '@' terminating the
    // authority (which ends at '@', or at '/'/'?' if there is no '@').
    if let Some(scheme_end) = dsn.find("://") {
        let after = &dsn[scheme_end + 3..];
        let authority_end = after.find(['/', '?']).map_or(after.len(), |i| {
            i.min(after.find('@').map_or(after.len(), |a| a + 1))
        });
        let authority = &after[..after.find('@').map_or(authority_end, |a| a)];
        if let Some(colon) = authority.find(':') {
            let mut out = String::with_capacity(dsn.len());
            out.push_str(&dsn[..scheme_end + 3]);
            out.push_str(&authority[..colon]);
            out.push_str(":***");
            out.push_str(&after[authority.len()..]);
            return out;
        }
        return dsn.to_string();
    }
    // Keyword form: `password=VALUE` up to the next whitespace, or `password='…'` (single-quoted).
    if let Some(pos) = dsn.find("password=") {
        let start = pos + "password=".len();
        let rest = &dsn[start..];
        let end = if let Some(quoted) = rest.strip_prefix('\'') {
            // Closing quote at index `i` in `quoted` → past-the-closing-quote index in `rest` is `i + 2`
            // (the opening quote + the closing quote).
            quoted.find('\'').map_or(rest.len(), |i| i + 2)
        } else {
            rest.find(char::is_whitespace).unwrap_or(rest.len())
        };
        let mut out = String::with_capacity(dsn.len());
        out.push_str(&dsn[..start]);
        out.push_str("***");
        out.push_str(&rest[end..]);
        return out;
    }
    dsn.to_string()
}

/// Percent-encode a string for a DSN userinfo component (RFC 3986 unreserved set kept verbatim).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Wrap a success payload as `DatabaseResult.Ok(v)`. The natives here NEVER fault on a DB error (a native
/// `Err(String)` is an uncatchable hard fault — `vm/exec.rs`); instead they return this `DatabaseResult<T>`
/// VALUE, and the phorj-source `Core.DatabaseModule` prelude `match`es it and `throw`s a catchable `DatabaseError`
/// (DEC-208 error-mechanism = prelude-wrapper). `DatabaseResult` is a PRELUDE-LOCAL enum (defined in
/// DB_PRELUDE, injected with it) — NOT `Core.Result`, whose injection sits earlier in the module chain
/// and so is not pulled in by `Core.DatabaseModule`'s transitive import (importer-after-imported doesn't inject).
fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "DatabaseResult".into(),
        variant: "Ok".into(),
        payload: vec![v],
    }))
}

/// Wrap a DB error message as `DatabaseResult.Err(msg)` — the prelude turns this into `throw DatabaseError`.
fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "DatabaseResult".into(),
        variant: "Err".into(),
        payload: vec![Value::Str(msg.into())],
    }))
}

/// Map an inner body's `Result<payload, db-error-message>` onto the returned `Result<T, string>` VALUE:
/// `Ok(v) → Success(v)`, `Err(msg) → Failure(msg)`. A DB error thus becomes a value the prelude throws
/// on, never an uncatchable native fault. (An arity/shape bug the checker forbids stays an `Err` — a
/// hard fault — because it is a program-construction error, not a recoverable DB error.)
fn wrap(inner: Result<Value, String>) -> Value {
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
struct DbConn {
    driver: Rc<RefCell<Option<Box<dyn DriverConn>>>>,
    tx_depth: Rc<Cell<u32>>,
    /// The `onQuery` observability hook (DEC-208 slice D, spec §7): a `(string sql, int ms) => void`
    /// phorj closure invoked after each `query`/`exec`, or `None`. Held behind a SHARED `Rc<RefCell>`
    /// so a [`DbStmt`] derived from this connection observes the SAME hook — a statement carries only
    /// the connection's shared cells (not the whole `DbConn`), and a hook registered AFTER `prepare`
    /// must still fire. Storing a `Value::Closure` is cheap (an `Rc` bump) and never inspected here.
    hook: Rc<RefCell<Option<Value>>>,
    /// The connection's query timeout in ms (DEC-208 slice D, spec §7), `0` = unset. Shared with
    /// derived statements (same rationale as `hook`). Setting it also arms SQLite's `busy_timeout`; when
    /// `> 0`, a transient `busy`/`locked` failure is reclassified `SerializationFailureError` → `TimeoutError`
    /// (the bounded lock-wait was exceeded). See [`remap_timeout`].
    timeout_ms: Rc<Cell<i64>>,
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
/// value ([`expand_placeholders`]), giving a typed `IN`-list bind PDO cannot do.
#[derive(Debug, Clone)]
enum PosBind {
    One(Value),
    List(Vec<Value>),
}

/// Accumulated bind parameters for a prepared statement. Positional and named are mutually exclusive
/// per statement (the surface's contract) — mixing is a DB error (`Failure`, catchable).
#[derive(Debug, Default, Clone)]
enum Binds {
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
struct DbStmt {
    driver: Rc<RefCell<Option<Box<dyn DriverConn>>>>,
    sql: String,
    binds: RefCell<Binds>,
    /// The originating connection's `onQuery` hook and query timeout, shared by `Rc` (see [`DbConn`]).
    /// A statement carries these (not the whole `DbConn`) so `query`/`exec` can fire the hook and apply
    /// the timeout classification without a back-reference to the connection object.
    hook: Rc<RefCell<Option<Value>>>,
    timeout_ms: Rc<Cell<i64>>,
    /// The shared transaction depth (see [`DbConn::tx_depth`]) — `executeMany` reads it to decide whether
    /// to open its own transaction (Postgres) or ride an existing one (savepoint).
    tx_depth: Rc<Cell<u32>>,
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
struct DbCursor {
    rows: RefCell<std::vec::IntoIter<Value>>,
}

impl DbObject for DbCursor {
    fn kind(&self) -> &'static str {
        "db-cursor"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn as_cursor(v: &Value) -> Result<&DbCursor, String> {
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
fn as_conn(v: &Value) -> Result<&DbConn, String> {
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

fn as_stmt(v: &Value) -> Result<&DbStmt, String> {
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
fn conn_closed() -> String {
    "<<ConnectionError>>Core.DatabaseModule: the connection is closed".to_string()
}

// --- Internal bodies: `Ok(payload)` on success, `Err(db-error-message)` on a DB error. `wrap` maps
// these onto the `Result<T, string>` VALUE the public `__`-natives return (Success | Failure). ---

/// `new Database(dsn)` → open a connection, dispatching on the DSN scheme onto the right backend driver
/// ([`open_driver`]): `sqlite:PATH` / `sqlite::memory:` / a bare path → SQLite; `postgres://…` →
/// Postgres. The driver behind [`Value::Db`] is opaque to the generic layer.
fn open_inner(args: &[Value]) -> Result<Value, String> {
    let dsn = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.DatabaseModule.__open expects (string dsn)".into()),
    };
    let driver = open_driver(dsn)?;
    Ok(Value::Db(Rc::new(DbConn {
        driver: Rc::new(RefCell::new(Some(driver))),
        tx_depth: Rc::new(Cell::new(0)),
        hook: Rc::new(RefCell::new(None)),
        timeout_ms: Rc::new(Cell::new(0)),
    })))
}

/// `DbSys.dsnWithPassword(dsn, password)` → the DSN with `password` injected as its credential (DEC-208
/// slice G, the `Database.withPassword` factory). A pure string transform ([`inject_pg_password`]); the result
/// is consumed immediately by `new Database(...)` and never retained in plaintext (the driver parses the
/// password out and stores only a redacted DSN). A non-postgres DSN is returned unchanged.
fn dsn_with_password_inner(args: &[Value]) -> Result<Value, String> {
    let (dsn, pw) = match args {
        [Value::Str(d), Value::Str(p)] => (d.as_str(), p.as_str()),
        _ => {
            return Err(
                "Core.DatabaseModule.__dsnWithPassword expects (string dsn, string password)"
                    .into(),
            )
        }
    };
    Ok(Value::Str(inject_pg_password(dsn, pw).into()))
}

/// `db.prepare(sql)` → a lazily-executed statement handle carrying the connection driver + SQL.
fn prepare_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, sql) = match args {
        [c, Value::Str(s)] => (as_conn(c)?, s.as_str().to_string()),
        _ => return Err("Core.DatabaseModule.__prepare expects (Database, string sql)".into()),
    };
    // Reject preparing on a closed connection eagerly (the statement would otherwise fault only at
    // query/exec time).
    if conn.driver.borrow().is_none() {
        return Err(conn_closed());
    }
    Ok(Value::Db(Rc::new(DbStmt {
        driver: Rc::clone(&conn.driver),
        sql,
        binds: RefCell::new(Binds::None),
        hook: Rc::clone(&conn.hook),
        timeout_ms: Rc::clone(&conn.timeout_ms),
        tx_depth: Rc::clone(&conn.tx_depth),
    })))
}

/// `stmt.bind(value)` → append a positional bind; returns the same shared handle (chainable).
fn bind_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, val) = match args {
        [s, v] => (as_stmt(s)?, v),
        _ => return Err("Core.DatabaseModule.__bind expects (Statement, value)".into()),
    };
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Positional(vec![PosBind::One(val.clone())]),
        Binds::Positional(v) => v.push(PosBind::One(val.clone())),
        Binds::Named(_) => {
            return Err(
                "Core.DatabaseModule: cannot mix positional bind() with named bindNamed()".into(),
            )
        }
    }
    drop(binds);
    Ok(args[0].clone())
}

/// `stmt.bindList(values)` → record a list-valued positional bind (DEC-208 slice D, spec §2). It
/// occupies ONE positional `?` slot (left-to-right with `bind()`); at execute time that `?` expands to
/// `(?,?,…)` — one placeholder per value — so `… WHERE id IN (?)` binds the whole list, strictly safer
/// than PDO (which cannot bind an array to `IN`). An EMPTY list expands to `(NULL)`: `x IN (NULL)` is
/// never true, so an empty `IN` matches nothing (documented, sane default). Mixing with `bindNamed()`
/// is an error, exactly like `bind()`. Returns the same shared handle (chainable).
fn bind_list_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, vals) = match args {
        [s, Value::List(vs)] => (as_stmt(s)?, vs),
        _ => return Err("Core.DatabaseModule.__bindList expects (Statement, List<value>)".into()),
    };
    let entry = PosBind::List(vals.iter().cloned().collect());
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Positional(vec![entry]),
        Binds::Positional(v) => v.push(entry),
        Binds::Named(_) => {
            return Err(
                "Core.DatabaseModule: cannot mix positional bindList() with named bindNamed()"
                    .into(),
            )
        }
    }
    drop(binds);
    Ok(args[0].clone())
}

/// `stmt.bindNamed(name, value)` → append a named bind; returns the same shared handle (chainable).
fn bind_named_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, name, val) = match args {
        [s, Value::Str(n), v] => (as_stmt(s)?, n.as_str().to_string(), v),
        _ => {
            return Err(
                "Core.DatabaseModule.__bindNamed expects (Statement, string name, value)".into(),
            )
        }
    };
    let mut binds = stmt.binds.borrow_mut();
    match &mut *binds {
        Binds::None => *binds = Binds::Named(vec![(name, val.clone())]),
        Binds::Named(v) => v.push((name, val.clone())),
        Binds::Positional(_) => {
            return Err(
                "Core.DatabaseModule: cannot mix named bindNamed() with positional bind()".into(),
            )
        }
    }
    drop(binds);
    Ok(args[0].clone())
}

/// Borrow the live driver behind a statement, or a clean `<<ConnectionError>>` if the connection was
/// closed. The returned guard keeps the driver borrowed for the caller's operation.
fn stmt_driver(stmt: &DbStmt) -> Result<std::cell::Ref<'_, Option<Box<dyn DriverConn>>>, String> {
    let guard = stmt.driver.borrow();
    if guard.is_none() {
        return Err(conn_closed());
    }
    Ok(guard)
}

/// `stmt.query()` → run the prepared+bound statement and return `List<Row>` (fetch-all), delegating the
/// dialect-specific placeholder + value handling to the connection's driver.
fn query_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__query expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    driver.query(&stmt.sql, &binds)
}

/// `stmt.stream()` → run the prepared+bound statement and wrap the result set in a [`DbCursor`]
/// (DEC-208 item H). Runs the SAME driver query as `stmt.query()` (so the `onQuery` hook + timeout
/// classification apply identically); the difference is delivery — rows are pulled one at a time via
/// `streamNext`, and a typed `streamInto<T>` stream hydrates each row only when pulled.
fn stream_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__stream expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    let rows = match driver.query(&stmt.sql, &binds)? {
        Value::List(rc) => Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone()),
        other => {
            return Err(format!(
                "Core.DatabaseModule.__stream: driver returned {}, not a row list",
                other.type_name()
            ))
        }
    };
    Ok(Value::Db(Rc::new(DbCursor {
        rows: RefCell::new(rows.into_iter()),
    })))
}

/// `cursor.streamNext()` → the next row handle, or `null` when the result set is exhausted.
fn stream_next_inner(args: &[Value]) -> Result<Value, String> {
    let cursor = match args {
        [c] => as_cursor(c)?,
        _ => return Err("Core.DatabaseModule.__streamNext expects (cursor)".into()),
    };
    let next = cursor.rows.borrow_mut().next();
    Ok(next.unwrap_or(Value::Null))
}

/// `stmt.exec()` → run a write (INSERT/UPDATE/DELETE/DDL) and return the affected-row count.
fn exec_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__exec expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    driver.exec(&stmt.sql, &binds).map(Value::Int)
}

/// `stmt.execReturningId()` → run an INSERT and return the auto-generated rowid / PK (DEC-208 slice D,
/// spec §4). Backend-specific: SQLite `last_insert_rowid()`; Postgres `RETURNING`/`lastval()`.
fn exec_returning_id_inner(args: &[Value]) -> Result<Value, String> {
    let stmt = match args {
        [s] => as_stmt(s)?,
        _ => return Err("Core.DatabaseModule.__execReturningId expects (Statement)".into()),
    };
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    let binds = stmt.binds.borrow();
    driver.exec_returning_id(&stmt.sql, &binds).map(Value::Int)
}

/// `stmt.executeMany(rows)` → prepare ONCE and execute the statement for each row of binds (DEC-208
/// slice D, spec §4) — far faster than a per-row `prepare`+`exec` loop. `rows` is a `List<List<value>>`
/// (each inner list = one positional bind-set, matching the `?` count). The whole batch runs inside a
/// dedicated SAVEPOINT (`phorj_bulk`) for atomicity + speed: it commits (`RELEASE`) on success and
/// rolls back the entire batch on ANY row's failure. A savepoint composes with an outer `begin()`
/// transaction and never touches the `begin()`/`rollback()` depth counter. Returns the TOTAL affected
/// rows. `executeMany` carries all its binds via `rows`; a statement that also has accumulated
/// `bind()`/`bindNamed()` binds is a usage error (ambiguous).
fn execute_many_inner(args: &[Value]) -> Result<Value, String> {
    let (stmt, rows) = match args {
        [s, Value::List(rows)] => (as_stmt(s)?, rows),
        _ => {
            return Err(
                "Core.DatabaseModule.__executeMany expects (Statement, List<List<value>>)".into(),
            )
        }
    };
    if !matches!(&*stmt.binds.borrow(), Binds::None) {
        return Err(
            "Core.DatabaseModule.executeMany: pass all values via the rows argument, not bind()/bindNamed()"
                .into(),
        );
    }
    let guard = stmt_driver(stmt)?;
    let driver = guard.as_ref().expect("driver liveness checked");
    // The driver's bulk savepoint needs to know whether a caller transaction is already open (Postgres
    // opens its own `BEGIN` at depth 0; SQLite auto-txns a standalone savepoint and ignores the flag).
    let in_transaction = stmt.tx_depth.get() > 0;
    driver
        .execute_many(&stmt.sql, rows.as_slice(), in_transaction)
        .map(Value::Int)
}

/// `db.lastInsertId()` → the auto-generated rowid / PK of the most recent INSERT on this connection
/// (DEC-208 slice D, spec §4). Backend-specific: SQLite `last_insert_rowid()`; Postgres `lastval()`.
fn last_insert_id_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__lastInsertId expects (Database)".into()),
    };
    let guard = conn.driver.borrow();
    let driver = guard.as_ref().ok_or_else(conn_closed)?;
    driver.last_insert_id().map(Value::Int)
}

/// `db.timeout(ms)` → arm the connection's query timeout (DEC-208 slice D, spec §7). SQLite:
/// `busy_timeout(ms)` bounds how long a statement waits on a held lock before failing — a genuine
/// statement-runtime cap needs a progress-handler/interrupt (not wired here; the busy-wait cap is what
/// SQLite supports cleanly). Storing `timeout_ms > 0` makes a subsequent `busy`/`locked` failure
/// reclassify to `TimeoutError` ([`remap_timeout`]). A negative `ms` clamps to 0 (unset). Idempotent.
fn timeout_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, ms) = match args {
        [c, Value::Int(ms)] => (as_conn(c)?, *ms),
        _ => return Err("Core.DatabaseModule.__timeout expects (Database, int ms)".into()),
    };
    let clamped = ms.max(0);
    {
        let guard = conn.driver.borrow();
        let driver = guard.as_ref().ok_or_else(conn_closed)?;
        driver.set_timeout(clamped)?;
    }
    conn.timeout_ms.set(clamped);
    Ok(Value::Int(clamped))
}

/// `db.onQuery(hook)` → register the observability hook (DEC-208 slice D, spec §7). Stores the
/// `(string, int) => void` closure in the shared cell every derived statement reads; `query`/`exec`
/// then invoke it after each op with `(sql, elapsed_ms)`. Stored eagerly (an `Rc` bump); a re-register
/// replaces the previous hook. Never a DB error.
fn on_query_inner(args: &[Value]) -> Result<Value, String> {
    let (conn, hook) = match args {
        [c, h] => (as_conn(c)?, h.clone()),
        _ => return Err("Core.DatabaseModule.__onQuery expects (Database, hook)".into()),
    };
    *conn.hook.borrow_mut() = Some(hook);
    Ok(Value::Int(0))
}

/// When a query timeout is active (`db.timeout(ms)`), reclassify a transient `SerializationFailureError`
/// (SQLite `busy`/`locked`) as `TimeoutError`: `busy_timeout` bounded the lock-wait, so reaching it means
/// the wait was exceeded. CONSEQUENCE: with a timeout set you no longer observe `SerializationFailureError`
/// (the class a future closure-`retry` would target) — acceptable while retry is deferred, documented
/// in `KNOWN_ISSUES.md` + the spec.
fn remap_timeout(res: Result<Value, String>, active: bool) -> Result<Value, String> {
    if active {
        if let Err(msg) = &res {
            if let Some(rest) = msg.strip_prefix("<<SerializationFailureError>>") {
                return Err(format!("<<TimeoutError>>{rest}"));
            }
        }
    }
    res
}

/// Run a statement-executing inner body, then (a) reclassify a busy failure as `TimeoutError` when a
/// timeout is active and (b) fire the connection's `onQuery` hook with `(sql, elapsed_ms)`. This is why
/// `query`/`exec`/`executeMany`/`execReturningId` are `HigherOrder` natives: they must call BACK into
/// the calling backend to invoke the stored `Value::Closure` (the same re-entrant `invoke` the
/// interpreter/VM hand to `List.map`). A well-typed `(string, int) => void` hook cannot raise a checked
/// exception, so its error (reachable only via a hard fault / the throw sentinel) is PROPAGATED, never
/// swallowed — swallowing would strand the backend's throw sentinel. `elapsed_ms` is wall-clock and
/// thus NON-deterministic across the two backends: examples/tests must never print it raw, or
/// `run ≡ runvm` breaks. When no hook is set and no timeout is armed, this is byte-identical to the old
/// `Ok(wrap(inner(args)))`.
fn with_hook(
    args: &[Value],
    invoke: &mut ClosureInvoker,
    inner: fn(&[Value]) -> Result<Value, String>,
) -> Result<Value, String> {
    let stmt = args.first().and_then(|v| as_stmt(v).ok());
    let start = std::time::Instant::now();
    let mut result = inner(args);
    if let Some(s) = stmt {
        result = remap_timeout(result, s.timeout_ms.get() > 0);
        let ms = i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX);
        // TAKE the hook out of the shared cell for the duration of its own call: a hook that itself
        // issues a query/exec on the same connection would otherwise re-enter here and recurse without
        // bound (stack overflow). With it removed, the nested op sees no hook and runs normally; the
        // hook is restored afterward (even if it faulted, so the error still propagates).
        let hook = s.hook.borrow_mut().take();
        if let Some(h) = hook {
            let fired = invoke(&h, vec![Value::Str(s.sql.as_str().into()), Value::Int(ms)]);
            *s.hook.borrow_mut() = Some(h);
            fired?;
        }
    }
    Ok(wrap(result))
}

/// Run one portable SQL control statement (`BEGIN`/`COMMIT`/`SAVEPOINT`/`RELEASE`/`ROLLBACK[ TO]`) on
/// the live connection's driver, or a clean `<<ConnectionError>>` if the connection was closed. These
/// forms are accepted identically by SQLite and Postgres, so transaction management stays generic.
fn control(conn: &DbConn, sql: &str) -> Result<(), String> {
    let guard = conn.driver.borrow();
    let driver = guard.as_ref().ok_or_else(conn_closed)?;
    driver.control(sql)
}

/// `db.begin()` → open a transaction (DEC-208 slice C). At depth 0 this is a top-level `BEGIN`; nested,
/// it opens `SAVEPOINT phorj_sp_<depth>` so transactional helpers compose. Increments the depth only on
/// success. Returns the new depth (the prelude ignores the payload; it is handy for tests/debugging).
fn begin_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__begin expects (Database)".into()),
    };
    let depth = conn.tx_depth.get();
    let sql = if depth == 0 {
        "BEGIN".to_string()
    } else {
        format!("SAVEPOINT phorj_sp_{depth}")
    };
    control(conn, &sql)?;
    let new_depth = depth + 1;
    conn.tx_depth.set(new_depth);
    Ok(Value::Int(i64::from(new_depth)))
}

/// `db.commit()` → commit the innermost open transaction level. At the outermost level (depth 1) this is
/// `COMMIT`; nested, it `RELEASE`s the matching savepoint. A commit with no open transaction (depth 0) is
/// a best-effort no-op so a secondary fault can never mask an original one. Returns the remaining depth.
fn commit_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__commit expects (Database)".into()),
    };
    let depth = conn.tx_depth.get();
    if depth == 0 {
        return Ok(Value::Int(0));
    }
    let remaining = depth - 1;
    let sql = if remaining == 0 {
        "COMMIT".to_string()
    } else {
        format!("RELEASE phorj_sp_{remaining}")
    };
    control(conn, &sql)?;
    conn.tx_depth.set(remaining);
    Ok(Value::Int(i64::from(remaining)))
}

/// `db.rollback()` → roll back the innermost open transaction level. At the outermost level this is
/// `ROLLBACK`; nested, it `ROLLBACK`s to and `RELEASE`s the matching savepoint (so the outer transaction
/// survives an inner rollback). A rollback with no open transaction is a best-effort no-op. The depth is
/// decremented BEFORE issuing the SQL, so the counter stays consistent even if the driver rejects the
/// statement (a doomed transaction is reset by SQLite regardless). Returns the remaining depth.
fn rollback_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__rollback expects (Database)".into()),
    };
    let depth = conn.tx_depth.get();
    if depth == 0 {
        return Ok(Value::Int(0));
    }
    let remaining = depth - 1;
    conn.tx_depth.set(remaining);
    let sql = if remaining == 0 {
        "ROLLBACK".to_string()
    } else {
        format!("ROLLBACK TO phorj_sp_{remaining}; RELEASE phorj_sp_{remaining}")
    };
    control(conn, &sql)?;
    Ok(Value::Int(i64::from(remaining)))
}

/// `db.close()` → deterministically drop the connection (DEC-208 slice C, spec §1). Idempotent and
/// never a DB error: every `Value::Db`/`DbStmt` shares the same `Rc<RefCell<Option<…>>>`, so setting it
/// to `None` invalidates all of them — a later op faults with `<<ConnectionError>>`. Resets the tx depth.
fn close_inner(args: &[Value]) -> Result<Value, String> {
    let conn = match args {
        [c] => as_conn(c)?,
        _ => return Err("Core.DatabaseModule.__close expects (Database)".into()),
    };
    *conn.driver.borrow_mut() = None;
    conn.tx_depth.set(0);
    Ok(Value::Int(0))
}

/// Look up a column in a `Row` (a `Map`), or a DB error if the column is absent.
fn row_cell<'a>(args: &'a [Value], who: &str) -> Result<(&'a Value, &'a str), String> {
    match args {
        [Value::Map(pairs), Value::Str(key)] => {
            let k = key.as_str();
            pairs
                .iter()
                .find(|(hk, _)| matches!(hk, HKey::Str(s) if s.as_str() == k))
                .map(|(_, v)| (v, k))
                .ok_or_else(|| format!("Core.DatabaseModule.{who}: no column `{k}` in this row"))
        }
        _ => Err(format!(
            "Core.DatabaseModule.{who} expects (Row, string column)"
        )),
    }
}

fn get_int_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getInt")?;
    match v {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getInt: column `{k}` is NULL (use int?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getInt: column `{k}` is {}, not int",
            other.type_name()
        )),
    }
}

fn get_string_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getString")?;
    match v {
        Value::Str(s) => Ok(Value::Str(s.clone())),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getString: column `{k}` is NULL (use string?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getString: column `{k}` is {}, not string",
            other.type_name()
        )),
    }
}

fn get_float_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getFloat")?;
    match v {
        Value::Float(f) => Ok(Value::Float(*f)),
        // SQLite stores an integral REAL as INTEGER; widen int→float for a float column, matching PDO.
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getFloat: column `{k}` is NULL (use float?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getFloat: column `{k}` is {}, not float",
            other.type_name()
        )),
    }
}

fn get_bool_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getBool")?;
    match v {
        // SQLite has no bool: it round-trips as 0/1 integer (matching the `to_sql` bind side).
        Value::Int(0) => Ok(Value::Bool(false)),
        Value::Int(_) => Ok(Value::Bool(true)),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Null => Err(format!(
            "Core.DatabaseModule.getBool: column `{k}` is NULL (use bool?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.getBool: column `{k}` is {}, not bool",
            other.type_name()
        )),
    }
}

// --- Nullable Row accessors (DEC-208 S2): a `T?`-typed hydration field admits a SQL NULL, so these
// return `null` for a NULL column instead of faulting. A wrong non-null storage type is still a DB
// error, and a missing column is still a DB error (`row_cell`). Shared by the dynamic path and the
// generic hydration desugar. ---

fn get_int_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getIntOrNull")?;
    match v {
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getIntOrNull: column `{k}` is {}, not int",
            other.type_name()
        )),
    }
}

fn get_string_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getStringOrNull")?;
    match v {
        Value::Str(s) => Ok(Value::Str(s.clone())),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getStringOrNull: column `{k}` is {}, not string",
            other.type_name()
        )),
    }
}

fn get_float_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getFloatOrNull")?;
    match v {
        Value::Float(f) => Ok(Value::Float(*f)),
        // SQLite stores an integral REAL as INTEGER; widen int→float, matching the non-nullable accessor.
        Value::Int(n) => Ok(Value::Float(*n as f64)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getFloatOrNull: column `{k}` is {}, not float",
            other.type_name()
        )),
    }
}

fn get_bool_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getBoolOrNull")?;
    match v {
        // SQLite has no bool: it round-trips as 0/1 integer (matching the `to_sql` bind side).
        Value::Int(0) => Ok(Value::Bool(false)),
        Value::Int(_) => Ok(Value::Bool(true)),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Null => Ok(Value::Null),
        other => Err(format!(
            "Core.DatabaseModule.getBoolOrNull: column `{k}` is {}, not bool",
            other.type_name()
        )),
    }
}

// --- Decimal accessor (DEC-208 slice E): a `decimal`-typed hydration field maps its column here.
// Exact money, never float: a TEXT column is parsed EXACTLY via the shared `…d`-literal grammar
// (`value::decimal_of`) — the money path; an INTEGER is exact at scale 0. A REAL is converted through
// its shortest round-trip decimal string (a REAL column cannot store money exactly — store decimal
// columns as TEXT for guaranteed exactness; this is a best-effort convenience). A missing column /
// wrong storage type / NULL-into-non-optional is a strict DB error (no silent coercion); the nullable
// accessor admits NULL. Shared by the dynamic path and the generic hydration desugar. ---

/// Convert a fetched cell to a phorj `decimal` (DEC-208 slice E). See the section note for the
/// TEXT/INTEGER/REAL conventions. `null_ok` selects the `decimal?` (admit NULL) vs `decimal` (strict)
/// behaviour; `who` names the accessor for the error message.
fn decimal_from_cell(v: &Value, k: &str, who: &str, null_ok: bool) -> Result<Value, String> {
    match v {
        // Already a decimal (defensive — SQLite storage classes never produce this, but a row is a
        // general `Map`, so the accessor stays total).
        Value::Decimal { .. } => Ok(v.clone()),
        Value::Int(n) => Ok(Value::Decimal {
            unscaled: i128::from(*n),
            scale: 0,
        }),
        // The money path: parse the exact decimal grammar from the stored text (no float round-trip).
        Value::Str(s) => match crate::value::decimal_of(s) {
            Some((unscaled, scale)) => Ok(Value::Decimal { unscaled, scale }),
            None => Err(format!(
                "Core.DatabaseModule.{who}: column `{k}` value `{s}` is not a valid decimal"
            )),
        },
        // Best-effort REAL → shortest round-trip decimal string → exact decimal of THAT string.
        Value::Float(f) => match crate::value::decimal_of(&format!("{f}")) {
            Some((unscaled, scale)) => Ok(Value::Decimal { unscaled, scale }),
            None => Err(format!(
                "Core.DatabaseModule.{who}: column `{k}` REAL value cannot be represented as a decimal"
            )),
        },
        Value::Null if null_ok => Ok(Value::Null),
        Value::Null => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is NULL (use decimal?)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is {}, not decimal",
            other.type_name()
        )),
    }
}

fn get_decimal_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getDecimal")?;
    decimal_from_cell(v, k, "getDecimal", false)
}

fn get_decimal_or_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, k) = row_cell(args, "getDecimalOrNull")?;
    decimal_from_cell(v, k, "getDecimalOrNull", true)
}

// --- Typed ARRAY-column accessors (DEC-208 slice K): a Postgres `int[]`/`text[]`/`float8[]`/`bool[]`
// cell arrives as a `Value::List` (see `postgres::pg_cell`); these read it as a typed `List<scalar>`.
// STRICT like the scalar accessors: a non-array column, a wrong element type, or a NULL ELEMENT is a
// clean catchable DatabaseError (Postgres arrays are nullable per element; phorj `List<int>` elements are
// not — the error steers to filtering NULLs in SQL, e.g. `array_remove(col, NULL)`). The `OrNull`
// variants admit a whole-array SQL NULL (→ `null`), never NULL elements. SQLite/MySQL never produce
// a list cell, so on those drivers the error reads "not an array" — the honest cross-driver story
// (arrays are a Postgres capability; the SAME class hydrates everywhere else via scalar columns).

/// Validate every element of an array cell with `check`, or explain which element broke.
fn list_from_cell(
    v: &Value,
    k: &str,
    who: &str,
    elem: &str,
    or_null: bool,
    check: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    match v {
        Value::List(items) => {
            for (i, it) in items.iter().enumerate() {
                if matches!(it, Value::Null) {
                    return Err(format!(
                        "Core.DatabaseModule.{who}: column `{k}` has a NULL element at [{i}] — filter them in \
                         SQL (e.g. array_remove({k}, NULL)) or select a non-null projection"
                    ));
                }
                if !check(it) {
                    return Err(format!(
                        "Core.DatabaseModule.{who}: column `{k}` element [{i}] is {}, not {elem}",
                        it.type_name()
                    ));
                }
            }
            Ok(v.clone())
        }
        Value::Null if or_null => Ok(Value::Null),
        Value::Null => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is NULL (use List<{elem}>? / the OrNull accessor)"
        )),
        other => Err(format!(
            "Core.DatabaseModule.{who}: column `{k}` is {}, not an array",
            other.type_name()
        )),
    }
}

macro_rules! list_accessor_inner {
    ($fn_name:ident, $who:literal, $elem:literal, $or_null:literal, $pat:pat) => {
        fn $fn_name(args: &[Value]) -> Result<Value, String> {
            let (v, k) = row_cell(args, $who)?;
            list_from_cell(v, k, $who, $elem, $or_null, |it| matches!(it, $pat))
        }
    };
}
list_accessor_inner!(
    get_int_list_inner,
    "getIntList",
    "int",
    false,
    Value::Int(_)
);
list_accessor_inner!(
    get_string_list_inner,
    "getStringList",
    "string",
    false,
    Value::Str(_)
);
list_accessor_inner!(
    get_float_list_inner,
    "getFloatList",
    "float",
    false,
    Value::Float(_)
);
list_accessor_inner!(
    get_bool_list_inner,
    "getBoolList",
    "bool",
    false,
    Value::Bool(_)
);
list_accessor_inner!(
    get_int_list_or_null_inner,
    "getIntListOrNull",
    "int",
    true,
    Value::Int(_)
);
list_accessor_inner!(
    get_string_list_or_null_inner,
    "getStringListOrNull",
    "string",
    true,
    Value::Str(_)
);
list_accessor_inner!(
    get_float_list_or_null_inner,
    "getFloatListOrNull",
    "float",
    true,
    Value::Float(_)
);
list_accessor_inner!(
    get_bool_list_or_null_inner,
    "getBoolListOrNull",
    "bool",
    true,
    Value::Bool(_)
);

// --- Column introspection (DEC-208 slice B): two capabilities the desugared `queryScalar` /
// `queryMap` / nested-hydration helpers need, routed through the SAME `DatabaseResult`/`wrap` protocol as
// the accessors (NOT a duplication of `getX` — genuinely new operations). `columnNames` gives the
// ORDERED column names of a row (the row is an insertion-ordered `Map`, so selection order is
// preserved) — `queryScalar` reads the sole column whose name is unpredictable (`COUNT(*)`), and
// `queryMap` keys on the first / reads the second. `isNull` reports whether a column is SQL NULL
// (type-agnostic) — the nested-optional-entity hydration tests "all this entity's columns are NULL"
// (a LEFT JOIN miss → the whole entity is `null`); it cannot use `== null` (phorj rejects a
// cross-type `T? == null` comparison), so this boolean primitive is required. ---

/// `row.columnNames()` → the ordered `List<string>` of this row's column names (selection order).
fn column_names_inner(args: &[Value]) -> Result<Value, String> {
    match args {
        [Value::Map(pairs)] => {
            let names: Vec<Value> = pairs
                .iter()
                // Column names are always text from SQL; the non-Str arms are unreachable in practice
                // but kept total (a row is a general `Map`).
                .map(|(k, _)| match k {
                    HKey::Str(s) => Value::Str(s.clone()),
                    HKey::Int(n) => Value::Str(n.to_string().into()),
                    HKey::Bool(b) => Value::Str(b.to_string().into()),
                })
                .collect();
            Ok(Value::List(Rc::new(names)))
        }
        _ => Err("Core.DatabaseModule.columnNames expects (Row)".into()),
    }
}

/// `row.isNull(column)` → `true` iff the column is SQL NULL; a DB error if the column is absent
/// (reusing `row_cell`, so a missing nested column is a strict error exactly like the accessors).
fn is_null_inner(args: &[Value]) -> Result<Value, String> {
    let (v, _k) = row_cell(args, "isNull")?;
    Ok(Value::Bool(matches!(v, Value::Null)))
}

// --- Public natives: each wraps its inner body so a DB error becomes `Result.Failure` (a value the
// prelude throws on), never a hard fault. `_out` (the stdout buffer) is unused — DB ops have no stdout. ---

macro_rules! db_native {
    ($name:ident, $inner:ident) => {
        fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap($inner(args)))
        }
    };
}
db_native!(db_open, open_inner);
/// `dsnWithPassword` returns a plain `string` (the authenticated DSN), NOT a `DatabaseResult` — it is a pure
/// string transform with no DB error, so it does not go through `wrap`.
fn db_dsn_with_password(args: &[Value], _out: &mut String) -> Result<Value, String> {
    dsn_with_password_inner(args)
}
db_native!(db_prepare, prepare_inner);
db_native!(db_stream_next, stream_next_inner);
db_native!(db_bind, bind_inner);
db_native!(db_bind_named, bind_named_inner);
db_native!(db_bind_list, bind_list_inner);
db_native!(db_last_insert_id, last_insert_id_inner);
db_native!(db_timeout, timeout_inner);
db_native!(db_on_query, on_query_inner);
db_native!(db_begin, begin_inner);
db_native!(db_commit, commit_inner);
db_native!(db_rollback, rollback_inner);
db_native!(db_close, close_inner);
db_native!(row_get_int, get_int_inner);
db_native!(row_get_string, get_string_inner);
db_native!(row_get_float, get_float_inner);
db_native!(row_get_bool, get_bool_inner);
db_native!(row_get_int_or_null, get_int_or_null_inner);
db_native!(row_get_string_or_null, get_string_or_null_inner);
db_native!(row_get_float_or_null, get_float_or_null_inner);
db_native!(row_get_bool_or_null, get_bool_or_null_inner);
db_native!(row_get_decimal, get_decimal_inner);
db_native!(row_get_decimal_or_null, get_decimal_or_null_inner);
db_native!(row_get_int_list, get_int_list_inner);
db_native!(row_get_string_list, get_string_list_inner);
db_native!(row_get_float_list, get_float_list_inner);
db_native!(row_get_bool_list, get_bool_list_inner);
db_native!(row_get_int_list_or_null, get_int_list_or_null_inner);
db_native!(row_get_string_list_or_null, get_string_list_or_null_inner);
db_native!(row_get_float_list_or_null, get_float_list_or_null_inner);
db_native!(row_get_bool_list_or_null, get_bool_list_or_null_inner);
db_native!(row_column_names, column_names_inner);
db_native!(row_is_null, is_null_inner);

// HigherOrder natives (DEC-208 slice D): the statement-executing paths route through `with_hook` so
// they can fire the `onQuery` closure and apply the timeout classification. `wrap` still turns a DB
// error into a catchable `DatabaseResult.Err`; the `Result<_, String>` a HigherOrder body returns is used
// ONLY for the hook-invoke propagation (a hard fault / throw sentinel), never for a DB error.
fn db_query(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, query_inner)
}
fn db_exec(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, exec_inner)
}
fn db_stream(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, stream_inner)
}
fn db_execute_many(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, execute_many_inner)
}
fn db_exec_returning_id(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    with_hook(args, invoke, exec_returning_id_inner)
}

/// `db.transaction(fn)` (DEC-208 slice C, the closure form — unblocked by DEC-222 throwing closures) —
/// ONE transactional attempt: `BEGIN`, run the closure, `COMMIT` on a normal return (returning the
/// closure's value), and auto-`ROLLBACK` + **re-propagate the ORIGINAL thrown value** on a throw. A
/// NESTED call opens a `SAVEPOINT` (via the shared `tx_depth`), so it composes into partial rollback.
/// The RETRY loop lives in the prelude, not here: retry must inspect the TYPED error to decide whether
/// it is transient (`SerializationFailureError`), and that thrown value sits in the backend's `pending_throw`
/// — invisible to a native. So this native is a single attempt; the prelude's `catch (SerializationFailureError)`
/// loop drives the retries.
///
/// **Throw preservation** (the load-bearing part): a closure `throw` reaches the invoker as
/// `Err(THROW_SENTINEL)` with the thrown `Value` stashed in the backend's `pending_throw`.
/// [`rollback_inner`] runs pure `rusqlite` SQL ([`control`] → `execute_batch`) and NEVER re-enters the
/// backend, so `pending_throw` stays intact; returning the SAME `Err(e)` unchanged lets the outer
/// backend arm (interpreter `call.rs` / VM `exec.rs`, both keyed on the sentinel) rebuild the ORIGINAL
/// typed `DatabaseError` — the caller catches the exact error the closure threw, never a generic one.
fn db_transaction(args: &[Value], invoke: &mut ClosureInvoker) -> Result<Value, String> {
    let (db, fnv) = match args {
        [db, fnv] => (db, fnv),
        _ => return Err("Core.DatabaseModule.__transaction expects (Database, fn)".into()),
    };
    // BEGIN. A DB error opening the (save)point is a catchable `DatabaseResult.Err`, never a hard fault.
    if let Err(msg) = begin_inner(std::slice::from_ref(db)) {
        return Ok(failure(msg));
    }
    match invoke(fnv, Vec::new()) {
        // Normal return: COMMIT and hand back the closure's value. If the COMMIT itself fails, roll
        // back best-effort (to reset the shared `tx_depth`) and surface the commit error as a
        // catchable `DatabaseResult.Err` — the closure's work is not returned.
        Ok(v) => match commit_inner(std::slice::from_ref(db)) {
            Ok(_) => Ok(success(v)),
            Err(msg) => {
                let _ = rollback_inner(std::slice::from_ref(db));
                Ok(failure(msg))
            }
        },
        // The closure threw (sentinel + `pending_throw`) or hard-faulted: roll back best-effort — a
        // rollback error must NEVER mask the original — then re-propagate the SAME `Err` unchanged, so
        // the backend reconstructs the ORIGINAL typed throw (`pending_throw` is untouched by rollback).
        Err(e) => {
            let _ = rollback_inner(std::slice::from_ref(db));
            Err(e)
        }
    }
}

/// The `Core.Native.Database` registry entries — the INTERNAL natives the phorj-source `Core.DatabaseModule` prelude wraps.
/// They live under the `DbSys` qualifier (NOT `Database`) so a prelude `class Database` calling `DbSys.open(..)`
/// never collides with the class. Every opaque connection / statement / row handle is typed `DatabaseHandle`
/// (a reserved opaque type backed by `Value::Db`/`Value::Map` — the prelude threads it, never inspects
/// it). Every native is `pure: false` (opens/uses a real DB resource) so any `import Core.DatabaseModule` program is
/// auto-quarantined from the byte-identity differential, and every native returns `Result<T, string>`
/// (Success | Failure) — never a hard fault on a DB error (the prelude throws a catchable `DatabaseError`).
/// The `php` emitters map to PDO (DEC-208 LADDER case 1); finalized in the transpile slice.
pub fn db_natives() -> Vec<NativeFn> {
    let handle = || Ty::Named("DatabaseHandle".into(), vec![]);
    let res = |t: Ty| Ty::Named("DatabaseResult".into(), vec![t]);
    // A bindable scalar. Built via `Ty::union_of` so members are in the checker's CANONICAL (sorted-by-
    // Display) order — load-bearing for the `List<bindable>` params (`bindList`/`executeMany`): a list
    // literal is contextually typed to `List<canonical-union>`, and generics are invariant, so a native
    // param whose union order differed would reject the well-typed argument.
    let bindable = || Ty::union_of(vec![Ty::String, Ty::Int, Ty::Float, Ty::Bool]);
    vec![
        NativeFn {
            module: "Core.Native.Database",
            name: "connect",
            params: vec![Ty::String],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_open),
            php: |a| format!("new \\PDO({})", a.first().map_or("''", |s| s)),
        },
        // Credential-Secret DSN builder (DEC-208 slice G): inject a `Core.Secret` password into a
        // `postgres://` DSN (`Database.withPassword`). Returns a plain `string` (the authenticated DSN),
        // consumed immediately by `new Database(...)`; the driver parses the password out and retains only a
        // redacted DSN. A non-postgres DSN is returned unchanged. `pure:false` keeps the module
        // spine-quarantined (a program using it also connects). PHP: no faithful analog (quarantined).
        NativeFn {
            module: "Core.Native.Database",
            name: "dsnWithPassword",
            params: vec![Ty::String, Ty::String],
            ret: Ty::String,
            pure: false,
            eval: NativeEval::Pure(db_dsn_with_password),
            php: |a| a.first().cloned().unwrap_or_else(|| "''".to_string()),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "prepare",
            params: vec![handle(), Ty::String],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_prepare),
            php: |a| format!("{}->prepare({})", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "bind",
            params: vec![handle(), bindable()],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_bind),
            // Positional binds are collected and passed to execute() in the transpile slice; the
            // receiver PHP is threaded through for now (finalized there).
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "bindNamed",
            params: vec![handle(), Ty::String, bindable()],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_bind_named),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "query",
            params: vec![handle()],
            ret: res(Ty::List(Box::new(handle()))),
            pure: false,
            // HigherOrder (DEC-208 slice D): fires the `onQuery` hook + applies timeout classification.
            eval: NativeEval::HigherOrder(db_query),
            php: |a| {
                format!(
                    "{}->execute() /* fetchAll finalized in transpile slice */",
                    a[0]
                )
            },
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "exec",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::HigherOrder(db_exec),
            php: |a| format!("{}->execute()", a[0]),
        },
        // Streaming (DEC-208 item H): `stream` runs the query (HigherOrder — fires `onQuery` exactly
        // like `query`) and returns a cursor handle; `streamNext` pulls one row handle at a time
        // (`null` = exhausted). PHP emitters are placeholders like the rest of DbSys (Core.DatabaseModule is
        // E-TRANSPILE-DB native-only — pipeline ladder gate).
        NativeFn {
            module: "Core.Native.Database",
            name: "stream",
            params: vec![handle()],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::HigherOrder(db_stream),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "streamNext",
            params: vec![handle()],
            ret: res(Ty::Optional(Box::new(handle()))),
            pure: false,
            eval: NativeEval::Pure(db_stream_next),
            php: |a| a[0].clone(),
        },
        // --- Writes & robustness (DEC-208 slice D, spec §4/§7). `bindList` (IN-list) is Pure (records
        // a bind); `executeMany`/`execReturningId` are HigherOrder (they run SQL → fire `onQuery`);
        // `lastInsertId`/`timeout`/`onQuery` are connection-level Pure. All `pure:false` (real DB I/O →
        // byte-identity quarantine). PHP emitters are placeholders (Core.DatabaseModule transpile finalized later). ---
        // `bindList`/`executeMany` are GENERIC over the element type `T` (not `List<bindable>`): an
        // invariant `List<union>` param cannot accept a homogeneous list literal/variable (a `List<int>`
        // is not a `List<string | int | float | bool>`), so bindability is enforced at RUNTIME by
        // `to_sql` (a non-scalar element → a catchable `DatabaseError`) instead of at compile time. `T` is
        // inferred from the argument's element type (same as `List.firstOr<T>`).
        NativeFn {
            module: "Core.Native.Database",
            name: "bindList",
            params: vec![handle(), Ty::List(Box::new(Ty::Param("T".into())))],
            ret: res(handle()),
            pure: false,
            eval: NativeEval::Pure(db_bind_list),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "executeMany",
            params: vec![
                handle(),
                Ty::List(Box::new(Ty::List(Box::new(Ty::Param("T".into()))))),
            ],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::HigherOrder(db_execute_many),
            php: |a| {
                format!(
                    "{}->execute() /* executeMany finalized in transpile slice */",
                    a[0]
                )
            },
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "execReturningId",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::HigherOrder(db_exec_returning_id),
            php: |a| format!("{}->execute()", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "lastInsertId",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_last_insert_id),
            php: |a| format!("(int) {}->lastInsertId()", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "timeout",
            params: vec![handle(), Ty::Int],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_timeout),
            // PDO: ATTR_TIMEOUT (seconds); the receiver is threaded through for now.
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "onQuery",
            params: vec![
                handle(),
                Ty::Function(vec![Ty::String, Ty::Int], Box::new(Ty::Void), vec![]),
            ],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_on_query),
            // No faithful PDO analog (PDO has no per-query hook); placeholder, quarantined.
            php: |_a| "null".to_string(),
        },
        // --- Transaction control (DEC-208 slice C). Savepoint-aware via the connection's depth counter
        // (managed in the native, shared across handles). The `php` emitters map to PDO's transaction
        // methods (LADDER case 1); nested-savepoint PDO emission is finalized in the transpile slice. ---
        NativeFn {
            module: "Core.Native.Database",
            name: "begin",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_begin),
            php: |a| format!("{}->beginTransaction()", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "commit",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_commit),
            php: |a| format!("{}->commit()", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "rollback",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_rollback),
            php: |a| format!("{}->rollBack()", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "close",
            params: vec![handle()],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(db_close),
            // PDO closes when the last reference is unset; there is no explicit close() method.
            php: |_a| "null".to_string(),
        },
        // Closure-form transaction (DEC-208 slice C, unblocked by DEC-222). GENERIC over the closure's
        // return type `T` (so `db.transaction(fn)` returns the closure's value); the closure param is a
        // THROWING function type `() => T throws DatabaseError` — the `throws DatabaseError` set is REQUIRED so the
        // user's throwing closure is accepted (variance rejects a throwing fn into a non-throwing slot).
        // HigherOrder: it invokes the closure re-entrantly on the calling backend. PHP is a placeholder
        // (Core.DatabaseModule is spine-quarantined; nested-savepoint PDO emission is finalized in the transpile slice).
        NativeFn {
            module: "Core.Native.Database",
            name: "transaction",
            params: vec![
                handle(),
                Ty::Function(
                    vec![],
                    Box::new(Ty::Param("T".into())),
                    vec![Ty::Named("DatabaseError".into(), vec![])],
                ),
            ],
            ret: res(Ty::Param("T".into())),
            pure: false,
            eval: NativeEval::HigherOrder(db_transaction),
            php: |a| format!("/* db.transaction finalized in transpile slice */ {}", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getInt",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Int),
            pure: false,
            eval: NativeEval::Pure(row_get_int),
            php: |a| format!("(int) {}[{}]", a[0], a[1]),
        },
        // Typed ARRAY-column accessors (DEC-208 slice K): Postgres `int[]`/`text[]`/`float8[]`/
        // `bool[]` cells → typed `List<scalar>` (strict; NULL elements rejected; `OrNull` admits a
        // whole-array NULL). PHP emitters are placeholders (Core.DatabaseModule is E-TRANSPILE-DB native-only).
        NativeFn {
            module: "Core.Native.Database",
            name: "getIntList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::Int))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_list),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getStringList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_list),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloatList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::Float))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_list),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBoolList",
            params: vec![handle(), Ty::String],
            ret: res(Ty::List(Box::new(Ty::Bool))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_list),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getIntListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::Int))))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_list_or_null),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getStringListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::String))))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_list_or_null),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloatListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::Float))))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_list_or_null),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBoolListOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::List(Box::new(Ty::Bool))))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_list_or_null),
            php: |a| a[0].clone(),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getString",
            params: vec![handle(), Ty::String],
            ret: res(Ty::String),
            pure: false,
            eval: NativeEval::Pure(row_get_string),
            php: |a| format!("(string) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloat",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Float),
            pure: false,
            eval: NativeEval::Pure(row_get_float),
            php: |a| format!("(float) {}[{}]", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBool",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Bool),
            pure: false,
            eval: NativeEval::Pure(row_get_bool),
            php: |a| format!("(bool) {}[{}]", a[0], a[1]),
        },
        // Nullable accessors (DEC-208 S2): a NULL column yields `null`; a wrong non-null type is still
        // a DB error. `ret` is `DatabaseResult<T?>` so the prelude method types as `T?`.
        NativeFn {
            module: "Core.Native.Database",
            name: "getIntOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Int))),
            pure: false,
            eval: NativeEval::Pure(row_get_int_or_null),
            php: |a| format!("(({0}[{1}] === null) ? null : (int) {0}[{1}])", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getStringOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_get_string_or_null),
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : (string) {0}[{1}])",
                    a[0], a[1]
                )
            },
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getFloatOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Float))),
            pure: false,
            eval: NativeEval::Pure(row_get_float_or_null),
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : (float) {0}[{1}])",
                    a[0], a[1]
                )
            },
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getBoolOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Bool))),
            pure: false,
            eval: NativeEval::Pure(row_get_bool_or_null),
            php: |a| format!("(({0}[{1}] === null) ? null : (bool) {0}[{1}])", a[0], a[1]),
        },
        // Decimal accessor (DEC-208 slice E): a `decimal`-typed hydration field maps its column here
        // (exact money — TEXT parsed exactly, never through float). PHP emitters are placeholders
        // (Core.DatabaseModule is spine-quarantined; the transpile is finalized in a later slice).
        NativeFn {
            module: "Core.Native.Database",
            name: "getDecimal",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Decimal),
            pure: false,
            eval: NativeEval::Pure(row_get_decimal),
            php: |a| format!("__phorj_dec_of((string) {}[{}])", a[0], a[1]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "getDecimalOrNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Optional(Box::new(Ty::Decimal))),
            pure: false,
            eval: NativeEval::Pure(row_get_decimal_or_null),
            php: |a| {
                format!(
                    "(({0}[{1}] === null) ? null : __phorj_dec_of((string) {0}[{1}]))",
                    a[0], a[1]
                )
            },
        },
        // Column introspection (DEC-208 slice B). `columnNames` → ordered `List<string>`; `isNull` →
        // `bool`. Used by the `queryScalar`/`queryMap`/nested-hydration desugar; PHP emitters are
        // placeholders (Core.DatabaseModule is spine-quarantined, transpile finalized in a later slice).
        NativeFn {
            module: "Core.Native.Database",
            name: "columnNames",
            params: vec![handle()],
            ret: res(Ty::List(Box::new(Ty::String))),
            pure: false,
            eval: NativeEval::Pure(row_column_names),
            php: |a| format!("array_keys({})", a[0]),
        },
        NativeFn {
            module: "Core.Native.Database",
            name: "isNull",
            params: vec![handle(), Ty::String],
            ret: res(Ty::Bool),
            pure: false,
            eval: NativeEval::Pure(row_is_null),
            php: |a| format!("({0}[{1}] === null)", a[0], a[1]),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // Slice D flipped `db_query`/`db_exec` from `Pure` to `HigherOrder`; the in-module tests call them
    // directly, so these shims supply a no-op closure invoker (no hook registered → never invoked) and
    // keep the `(args, &mut out)` call ergonomics. Return shape is unchanged (`Ok(wrap(..))`).
    fn q(args: &[Value], _out: &mut String) -> Result<Value, String> {
        let mut noop = |_: &Value, _: Vec<Value>| Ok(Value::Null);
        db_query(args, &mut noop)
    }
    fn x(args: &[Value], _out: &mut String) -> Result<Value, String> {
        let mut noop = |_: &Value, _: Vec<Value>| Ok(Value::Null);
        db_exec(args, &mut noop)
    }

    /// Extract the payload of a `Result.Success(v)` value the natives now return; panic on `Failure`.
    fn ok_of(v: Value) -> Value {
        match v {
            Value::Enum(e) if e.variant.as_ref() == "Ok" => e.payload[0].clone(),
            other => panic!("expected DatabaseResult.Ok, got {other:?}"),
        }
    }

    /// Extract the message of a `Result.Failure(msg)` value; panic on `Success`.
    fn err_of(v: Value) -> String {
        match v {
            Value::Enum(e) if e.variant.as_ref() == "Err" => match &e.payload[0] {
                Value::Str(s) => s.as_str().to_string(),
                other => panic!("Failure payload not a string: {other:?}"),
            },
            other => panic!("expected DatabaseResult.Err, got {other:?}"),
        }
    }

    /// End-to-end runtime round-trip (in-process): open in-memory → DDL → insert (positional + named
    /// binds) → query back → Row accessors. Proves the rusqlite integration through the `Value` model
    /// and the `Result`-returning protocol, independent of the language surface (which lands next slice).
    #[test]
    fn db_runtime_round_trip() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str("sqlite::memory:".into())], &mut out).unwrap());

        // CREATE TABLE (no binds) via exec.
        let stmt = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("CREATE TABLE users(name TEXT, age INTEGER)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        assert!(ok_of(x(&[stmt], &mut out).unwrap()).eq_val(&Value::Int(0)));

        // INSERT with positional binds.
        let ins = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO users(name, age) VALUES(?, ?)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let ins = ok_of(db_bind(&[ins, Value::Str("Ada".into())], &mut out).unwrap());
        let ins = ok_of(db_bind(&[ins, Value::Int(36)], &mut out).unwrap());
        assert!(ok_of(x(&[ins], &mut out).unwrap()).eq_val(&Value::Int(1)));

        // INSERT with named binds.
        let ins2 = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO users(name, age) VALUES(:n, :a)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let ins2 = ok_of(
            db_bind_named(
                &[ins2, Value::Str("n".into()), Value::Str("Grace".into())],
                &mut out,
            )
            .unwrap(),
        );
        let ins2 = ok_of(
            db_bind_named(&[ins2, Value::Str("a".into()), Value::Int(45)], &mut out).unwrap(),
        );
        assert!(ok_of(x(&[ins2], &mut out).unwrap()).eq_val(&Value::Int(1)));

        // Query back, ordered, and read via Row accessors.
        let sel = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("SELECT name, age FROM users WHERE age > ? ORDER BY age".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let sel = ok_of(db_bind(&[sel, Value::Int(30)], &mut out).unwrap());
        let rows = ok_of(q(&[sel], &mut out).unwrap());
        let Value::List(rows) = rows else {
            panic!("query must return a list")
        };
        assert_eq!(rows.len(), 2);

        // Row 0 = Ada / 36.
        assert!(ok_of(
            row_get_string(&[rows[0].clone(), Value::Str("name".into())], &mut out).unwrap()
        )
        .eq_val(&Value::Str("Ada".into())));
        assert!(ok_of(
            row_get_int(&[rows[0].clone(), Value::Str("age".into())], &mut out).unwrap()
        )
        .eq_val(&Value::Int(36)));
        // Row 1 = Grace / 45.
        assert!(ok_of(
            row_get_string(&[rows[1].clone(), Value::Str("name".into())], &mut out).unwrap()
        )
        .eq_val(&Value::Str("Grace".into())));
    }

    #[test]
    fn mixing_bind_styles_is_a_failure() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(db_prepare(&[db, Value::Str("SELECT ?, :x".into())], &mut out).unwrap());
        let s = ok_of(db_bind(&[s, Value::Int(1)], &mut out).unwrap());
        // A DB usage error is a catchable Result.Failure, NOT a hard fault.
        let msg =
            err_of(db_bind_named(&[s, Value::Str("x".into()), Value::Int(2)], &mut out).unwrap());
        assert!(msg.contains("cannot mix"), "got: {msg}");
    }

    #[test]
    fn get_int_on_null_is_a_failure() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(db_prepare(&[db, Value::Str("SELECT NULL AS x".into())], &mut out).unwrap());
        let rows = ok_of(q(&[s], &mut out).unwrap());
        let Value::List(rows) = rows else { panic!() };
        let msg =
            err_of(row_get_int(&[rows[0].clone(), Value::Str("x".into())], &mut out).unwrap());
        assert!(msg.contains("NULL"), "got: {msg}");
    }

    // ── DEC-208 slice C: transactions, savepoints, taxonomy, close ─────────────────────────────

    /// Open an in-memory DB and run one `exec` statement, panicking on any failure.
    fn exec1(db: &Value, sql: &str, out: &mut String) {
        let s = ok_of(db_prepare(&[db.clone(), Value::Str(sql.into())], out).unwrap());
        ok_of(x(&[s], out).unwrap());
    }

    /// Read a single-int scalar from `sql`.
    fn scalar(db: &Value, sql: &str, col: &str, out: &mut String) -> i64 {
        let s = ok_of(db_prepare(&[db.clone(), Value::Str(sql.into())], out).unwrap());
        let rows = ok_of(q(&[s], out).unwrap());
        let Value::List(rows) = rows else {
            panic!("query returns a list")
        };
        let v = ok_of(row_get_int(&[rows[0].clone(), Value::Str(col.into())], out).unwrap());
        match v {
            Value::Int(n) => n,
            other => panic!("expected int, got {other:?}"),
        }
    }

    /// A UNIQUE / PRIMARY KEY collision maps (via the extended result code) to the `UniqueViolationError`
    /// marker the prelude classifier reads.
    #[test]
    fn unique_violation_is_tagged() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(id INTEGER PRIMARY KEY)", &mut out);
        exec1(&db, "INSERT INTO t(id) VALUES(1)", &mut out);
        let s = ok_of(
            db_prepare(
                &[db, Value::Str("INSERT INTO t(id) VALUES(1)".into())],
                &mut out,
            )
            .unwrap(),
        );
        let msg = err_of(x(&[s], &mut out).unwrap());
        assert!(msg.starts_with("<<UniqueViolationError>>"), "got: {msg}");
    }

    /// A malformed statement maps to the `SyntaxError` marker.
    #[test]
    fn syntax_error_is_tagged() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(db_prepare(&[db, Value::Str("SELCT oops".into())], &mut out).unwrap());
        let msg = err_of(q(&[s], &mut out).unwrap());
        assert!(msg.starts_with("<<SyntaxError>>"), "got: {msg}");
    }

    /// A committed transaction persists; a rolled-back one is discarded.
    #[test]
    fn commit_persists_rollback_discards() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);

        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap());
        exec1(&db, "INSERT INTO t(n) VALUES(1)", &mut out);
        ok_of(db_commit(std::slice::from_ref(&db), &mut out).unwrap());
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);

        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap());
        exec1(&db, "INSERT INTO t(n) VALUES(2)", &mut out);
        ok_of(db_rollback(std::slice::from_ref(&db), &mut out).unwrap());
        // Still one row — the second insert was rolled back.
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);
    }

    /// A nested `begin` is a SAVEPOINT: rolling it back leaves the outer transaction's work intact.
    #[test]
    fn savepoint_partial_rollback() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);

        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap()); // outer
        exec1(&db, "INSERT INTO t(n) VALUES(1)", &mut out);
        ok_of(db_begin(std::slice::from_ref(&db), &mut out).unwrap()); // savepoint
        exec1(&db, "INSERT INTO t(n) VALUES(2)", &mut out);
        ok_of(db_rollback(std::slice::from_ref(&db), &mut out).unwrap()); // roll back savepoint only
        ok_of(db_commit(std::slice::from_ref(&db), &mut out).unwrap()); // commit outer
                                                                        // Only the outer insert (n=1) survives.
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 1);
        assert_eq!(scalar(&db, "SELECT n FROM t", "n", &mut out), 1);
    }

    /// `close` is idempotent and invalidates every derived handle; a later op faults with the
    /// `ConnectionError` marker.
    #[test]
    fn close_invalidates_connection() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);
        ok_of(db_close(std::slice::from_ref(&db), &mut out).unwrap());
        // Idempotent: a second close is still Ok.
        ok_of(db_close(std::slice::from_ref(&db), &mut out).unwrap());
        let msg = err_of(db_prepare(&[db, Value::Str("SELECT 1".into())], &mut out).unwrap());
        assert!(msg.starts_with("<<ConnectionError>>"), "got: {msg}");
    }

    #[test]
    fn column_names_are_selection_ordered() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(
            db_prepare(
                &[db, Value::Str("SELECT 1 AS a, 2 AS b, 3 AS c".into())],
                &mut out,
            )
            .unwrap(),
        );
        let rows = ok_of(q(&[s], &mut out).unwrap());
        let Value::List(rows) = rows else { panic!() };
        let cols = ok_of(row_column_names(&[rows[0].clone()], &mut out).unwrap());
        let Value::List(cols) = cols else {
            panic!("columnNames must return a list")
        };
        let got: Vec<&str> = cols
            .iter()
            .map(|v| match v {
                Value::Str(s) => s.as_str(),
                _ => panic!("column name not a string"),
            })
            .collect();
        assert_eq!(got, vec!["a", "b", "c"]);
    }

    #[test]
    fn is_null_reports_null_and_present() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        let s = ok_of(
            db_prepare(
                &[db, Value::Str("SELECT NULL AS a, 7 AS b".into())],
                &mut out,
            )
            .unwrap(),
        );
        let rows = ok_of(q(&[s], &mut out).unwrap());
        let Value::List(rows) = rows else { panic!() };
        assert!(
            ok_of(row_is_null(&[rows[0].clone(), Value::Str("a".into())], &mut out).unwrap())
                .eq_val(&Value::Bool(true))
        );
        assert!(
            ok_of(row_is_null(&[rows[0].clone(), Value::Str("b".into())], &mut out).unwrap())
                .eq_val(&Value::Bool(false))
        );
        // A missing column is a strict DB error (reuses `row_cell`).
        let msg =
            err_of(row_is_null(&[rows[0].clone(), Value::Str("zzz".into())], &mut out).unwrap());
        assert!(msg.contains("no column"), "got: {msg}");
    }

    // ── DEC-208 slice D: writes + robustness ──────────────────────────────────────────────────

    /// `execute_many` inserts every row atomically and returns the total; a mid-batch failure rolls the
    /// WHOLE batch back (savepoint), leaving nothing behind.
    #[test]
    fn execute_many_atomic() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(
            &db,
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
            &mut out,
        );
        let s = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO t(id, v) VALUES(?, ?)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let rows = Value::List(Rc::new(vec![
            Value::List(Rc::new(vec![Value::Int(1), Value::Str("a".into())])),
            Value::List(Rc::new(vec![Value::Int(2), Value::Str("b".into())])),
        ]));
        // The `_inner` bodies return the RAW `Result` (the public HO natives `wrap` it); assert directly.
        let n = execute_many_inner(&[s, rows]).unwrap();
        assert!(n.eq_val(&Value::Int(2)));
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 2);

        // A duplicate PK mid-batch → the whole savepoint rolls back (still 2 rows, none of the batch).
        let s2 = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO t(id, v) VALUES(?, ?)".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let bad = Value::List(Rc::new(vec![
            Value::List(Rc::new(vec![Value::Int(3), Value::Str("c".into())])),
            Value::List(Rc::new(vec![Value::Int(1), Value::Str("dup".into())])),
        ]));
        let msg = execute_many_inner(&[s2, bad]).unwrap_err();
        assert!(msg.starts_with("<<UniqueViolationError>>"), "got: {msg}");
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 2);
    }

    /// `execReturningId` / `lastInsertId` report the auto-generated rowid.
    #[test]
    fn insert_id_helpers() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        exec1(
            &db,
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
            &mut out,
        );
        let ins = ok_of(
            db_prepare(
                &[
                    db.clone(),
                    Value::Str("INSERT INTO t(v) VALUES('a')".into()),
                ],
                &mut out,
            )
            .unwrap(),
        );
        let id = exec_returning_id_inner(&[ins]).unwrap();
        assert!(id.eq_val(&Value::Int(1)));
        exec1(&db, "INSERT INTO t(v) VALUES('b')", &mut out);
        let last = last_insert_id_inner(std::slice::from_ref(&db)).unwrap();
        assert!(last.eq_val(&Value::Int(2)));
    }

    /// `remap_timeout` reclassifies a transient `SerializationFailureError` as `TimeoutError` only when a timeout
    /// is armed (mapping unit — deterministic, no lock races).
    #[test]
    fn remap_timeout_only_when_armed() {
        let armed = remap_timeout(
            Err("<<SerializationFailureError>>Core.DatabaseModule: database is locked".into()),
            true,
        );
        assert!(
            matches!(&armed, Err(m) if m.starts_with("<<TimeoutError>>")),
            "got: {armed:?}"
        );
        let unarmed = remap_timeout(Err("<<SerializationFailureError>>x".into()), false);
        assert!(matches!(&unarmed, Err(m) if m.starts_with("<<SerializationFailureError>>")));
        // A non-busy error is never touched, armed or not.
        let other = remap_timeout(Err("<<SyntaxError>>x".into()), true);
        assert!(matches!(&other, Err(m) if m.starts_with("<<SyntaxError>>")));
    }

    /// End-to-end: with `db.timeout(ms)` armed, a genuine lock contention (a second connection blocked
    /// by a held write lock) surfaces as `TimeoutError` (not `SerializationFailureError`). Deterministic: the
    /// first connection holds the lock for the whole test, so the second always exhausts its busy wait.
    #[test]
    fn armed_timeout_maps_busy_to_timeout_end_to_end() {
        let mut out = String::new();
        let path = std::env::temp_dir().join(format!(
            "phorj_db_to_{}_{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dsn = format!("sqlite:{}", path.display());
        let c1 = ok_of(db_open(&[Value::Str(dsn.as_str().into())], &mut out).unwrap());
        exec1(&c1, "CREATE TABLE t(x INTEGER)", &mut out);
        // c1 opens a transaction and takes a write lock, holding it for the rest of the test.
        ok_of(db_begin(std::slice::from_ref(&c1), &mut out).unwrap());
        exec1(&c1, "INSERT INTO t(x) VALUES(1)", &mut out);
        // c2 arms a short busy timeout, then its write waits for and fails to get the lock → TimeoutError.
        let c2 = ok_of(db_open(&[Value::Str(dsn.as_str().into())], &mut out).unwrap());
        ok_of(db_timeout(&[c2.clone(), Value::Int(30)], &mut out).unwrap());
        let s = ok_of(
            db_prepare(
                &[c2, Value::Str("INSERT INTO t(x) VALUES(2)".into())],
                &mut out,
            )
            .unwrap(),
        );
        let msg = err_of(x(&[s], &mut out).unwrap());
        let _ = std::fs::remove_file(&path);
        assert!(msg.starts_with("<<TimeoutError>>"), "got: {msg}");
    }

    /// A hook registered via `onQuery` is stored and returned by the shared cell; a Pure store never
    /// fails. (Invocation with `(sql, ms)` is exercised end-to-end by `tests/db.rs`, which has a real
    /// backend to run the closure.)
    #[test]
    fn on_query_stores_the_hook() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str(":memory:".into())], &mut out).unwrap());
        // A non-closure stand-in is fine here: the native only stores the value; the checker enforces
        // the `(string, int) => void` shape at the call site.
        let sentinel = Value::Int(42);
        ok_of(db_on_query(&[db.clone(), sentinel], &mut out).unwrap());
        let conn = as_conn(&db).unwrap();
        assert!(conn.hook.borrow().is_some());
    }

    // ── DEC-208 slice I: multi-driver DSN dispatch + slice G credential injection ────────────────

    /// A `sqlite:`/`:memory:` DSN dispatches to the SQLite driver (unchanged behaviour): the open
    /// succeeds and a round-trip through the resulting connection works — the dispatch never misroutes a
    /// sqlite DSN. (The postgres branch is proven by `postgres_dsn_without_feature_is_a_clean_error`.)
    #[test]
    fn sqlite_dsn_dispatches_to_sqlite() {
        let mut out = String::new();
        let db = ok_of(db_open(&[Value::Str("sqlite::memory:".into())], &mut out).unwrap());
        assert!(as_conn(&db).is_ok());
        exec1(&db, "CREATE TABLE t(n INTEGER)", &mut out);
        assert_eq!(scalar(&db, "SELECT count(*) AS c FROM t", "c", &mut out), 0);
    }

    /// A `postgres://` DSN dispatches to the postgres driver — which, when `db-postgres` is OFF, is a
    /// clean feature-gated `ConnectionError`, NEVER a fall-through to the SQLite file path (which would
    /// silently create a file literally named `postgres://…`).
    #[cfg(not(feature = "db-postgres"))]
    #[test]
    fn postgres_dsn_without_feature_is_a_clean_error() {
        let mut out = String::new();
        let msg = err_of(db_open(&[Value::Str("postgres://u@h/db".into())], &mut out).unwrap());
        assert!(msg.starts_with("<<ConnectionError>>"), "got: {msg}");
        assert!(msg.contains("not compiled in"), "got: {msg}");
    }

    /// `dsnWithPassword` (slice G) percent-encodes and injects a password into a postgres DSN, replaces
    /// an existing one, and leaves a non-postgres DSN untouched.
    #[test]
    fn dsn_with_password_injects_for_postgres_only() {
        assert_eq!(
            inject_pg_password("postgres://user@host:5432/db", "p@ss:w/d"),
            "postgres://user:p%40ss%3Aw%2Fd@host:5432/db"
        );
        assert_eq!(
            inject_pg_password("postgres://user:old@host/db", "new"),
            "postgres://user:new@host/db"
        );
        assert_eq!(
            inject_pg_password("sqlite::memory:", "x"),
            "sqlite::memory:"
        );
        // Slice J: mysql/mariadb URL DSNs take the injected credential too (withPassword must never
        // silently no-op on a MySQL DSN).
        assert_eq!(
            inject_pg_password("mysql://user@host:3306/db", "pw"),
            "mysql://user:pw@host:3306/db"
        );
        assert_eq!(
            inject_pg_password("mariadb://u:old@h/db", "new"),
            "mariadb://u:new@h/db"
        );
    }

    // ── Typed array-accessor validation (DEC-208 slice K, server-free) ──────────────────────────

    #[test]
    fn list_from_cell_validates_elements_strictly() {
        let ok = Value::List(Rc::new(vec![Value::Int(1), Value::Int(2)]));
        assert!(matches!(
            list_from_cell(&ok, "c", "getIntList", "int", false, |v| matches!(
                v,
                Value::Int(_)
            )),
            Ok(Value::List(_))
        ));
        // A NULL element is rejected with the array_remove steering.
        let holed = Value::List(Rc::new(vec![Value::Int(1), Value::Null]));
        let err = list_from_cell(&holed, "c", "getIntList", "int", false, |v| {
            matches!(v, Value::Int(_))
        })
        .unwrap_err();
        assert!(err.contains("NULL element at [1]"), "{err}");
        // A wrong element type names the offender.
        let mixed = Value::List(Rc::new(vec![Value::Str("x".into())]));
        let err = list_from_cell(&mixed, "c", "getIntList", "int", false, |v| {
            matches!(v, Value::Int(_))
        })
        .unwrap_err();
        assert!(err.contains("element [0] is string, not int"), "{err}");
        // Whole-array NULL: OrNull admits, strict rejects; a scalar cell is "not an array".
        assert!(matches!(
            list_from_cell(&Value::Null, "c", "getIntListOrNull", "int", true, |_| true),
            Ok(Value::Null)
        ));
        assert!(list_from_cell(&Value::Null, "c", "getIntList", "int", false, |_| true).is_err());
        let err =
            list_from_cell(&Value::Int(3), "c", "getIntList", "int", false, |_| true).unwrap_err();
        assert!(err.contains("not an array"), "{err}");
    }
}
