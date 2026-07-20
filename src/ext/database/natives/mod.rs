//! `Core.DatabaseModule` — the enhanced-PDO database primitive (DEC-208), a MULTI-DRIVER runtime behind a scheme-
//! dispatched [`driver::DriverConn`] trait (DEC-208 slice I): `sqlite:…` → [`sqlite`] (bundled `rusqlite`),
//! `postgres://…` → [`postgres`] (the sync `postgres` crate, `database-postgres` feature),
//! `mysql://…` → [`mysql`] (the sync `mysql` crate, `database-mysql` feature).
//!
//! Feature-gated (`database`) and native-only. This module owns the BACKEND-AGNOSTIC layer, decomposed
//! per Invariant 13 into sibling files:
//!
//! - [`driver`] — the [`driver::DriverConn`] trait, the DSN→backend dispatcher, and the pure DSN
//!   credential helpers (inject / redact / percent-encode).
//! - [`handles`] — the `DatabaseResult` value wrappers, the opaque connection / statement / cursor
//!   handles (`DbConn`/`DbStmt`/`DbCursor`, carried by `Value::Db` via `DbObject`), the bind
//!   accumulator (`Binds`/`PosBind`), the downcast helpers, and the closed-connection fault string.
//! - [`ops`] — the connection / statement / transaction operation bodies (`Ok(payload)` on success,
//!   `Err(db-error-message)` on a DB error).
//! - [`rows`] — the Row cell accessors (scalar / nullable / decimal / typed-array / introspection).
//! - [`wrappers`] — the public native wrappers that map an inner body onto the `DatabaseResult<T>`
//!   VALUE the prelude throws on (Pure via `wrap`/`wrap_unit`; statement-executing via `with_hook`).
//! - [`registry`] + [`registry_rows`] — the `Core.Native.Database` registry rows and the crate-facing
//!   [`database_natives`] assembler.
//!
//! Each concrete backend implements only its genuinely dialect-specific pieces (value mapping,
//! placeholder syntax, error-code taxonomy) in its own submodule. The public `Core.DatabaseModule`
//! SURFACE (`Database`/`Statement`/`Row` + `new Database(dsn)`) is the phorj-source `DB_PRELUDE`
//! (`src/ext/database/prelude.rs`) on top of these — the natives live under the `DbSys` qualifier so a
//! prelude `class Database` never collides with them.
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
//! across the drivers and PHP PDO). Correctness: the in-module unit tests + the `tests/database.rs` fixture.
//! `run ≡ runvm` holds unconditionally (both backends call these one shared `eval` bodies). The `php`
//! emitters (faithful PDO, DEC-208 LADDER case 1) are finalized in the DEC-208 transpile slice.

mod driver;
mod handles;
mod ops;
mod registry;
mod registry_rows;
mod rows;
mod wrappers;

#[cfg(feature = "database-mysql")]
mod mysql;
#[cfg(feature = "database-mysql")]
mod mysql_sql;
#[cfg(feature = "database-postgres")]
mod postgres;
#[cfg(feature = "database-postgres")]
mod postgres_sql;
mod sqlite;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_more;

pub use registry::database_natives;
