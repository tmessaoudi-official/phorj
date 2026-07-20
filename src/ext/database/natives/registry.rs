//! The `Core.Native.Database` registry — the INTERNAL natives the phorj-source `Core.DatabaseModule`
//! prelude wraps. This file holds the shared `Ty` helpers, the crate-facing [`database_natives`]
//! assembler, and the connection / statement natives; the Row-accessor natives live in
//! [`super::registry_rows`]. They live under the `DbSys` qualifier (NOT `Database`) so a prelude
//! `class Database` calling `DbSys.open(..)` never collides with the class.

use super::wrappers::*;
use crate::native::{NativeEval, NativeFn};
use crate::types::Ty;

/// The opaque `DatabaseHandle` type (a reserved opaque type backed by `Value::Db`/`Value::Map`).
pub(super) fn handle() -> Ty {
    Ty::Named("DatabaseHandle".into(), vec![])
}

/// The prelude-local `DatabaseResult<T>` carrier every native returns (Success | Failure).
pub(super) fn res(t: Ty) -> Ty {
    Ty::Named("DatabaseResult".into(), vec![t])
}

/// A bindable scalar. Built via `Ty::union_of` so members are in the checker's CANONICAL (sorted-by-
/// Display) order — load-bearing for the `List<bindable>` params (`bindList`/`executeMany`): a list
/// literal is contextually typed to `List<canonical-union>`, and generics are invariant, so a native
/// param whose union order differed would reject the well-typed argument.
pub(super) fn bindable() -> Ty {
    Ty::union_of(vec![Ty::String, Ty::Int, Ty::Float, Ty::Bool])
}

/// The `Core.Native.Database` registry entries — the INTERNAL natives the phorj-source `Core.DatabaseModule` prelude wraps.
/// They live under the `DbSys` qualifier (NOT `Database`) so a prelude `class Database` calling `DbSys.open(..)`
/// never collides with the class. Every opaque connection / statement / row handle is typed `DatabaseHandle`
/// (a reserved opaque type backed by `Value::Db`/`Value::Map` — the prelude threads it, never inspects
/// it). Every native is `pure: false` (opens/uses a real DB resource) so any `import Core.DatabaseModule` program is
/// auto-quarantined from the byte-identity differential, and every native returns `Result<T, string>`
/// (Success | Failure) — never a hard fault on a DB error (the prelude throws a catchable `DatabaseError`).
/// The `php` emitters map to PDO (DEC-208 LADDER case 1); finalized in the transpile slice.
pub fn database_natives() -> Vec<NativeFn> {
    let mut v = conn_stmt_natives();
    v.extend(super::registry_rows::row_natives());
    v
}

/// The connection- and statement-level natives (DEC-208 slices C/D/G/H).
fn conn_stmt_natives() -> Vec<NativeFn> {
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
    ]
}
