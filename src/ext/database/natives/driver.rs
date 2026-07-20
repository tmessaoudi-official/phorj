//! The multi-driver seam for `Core.DatabaseModule` (DEC-208 slice I): the scheme-dispatched
//! [`DriverConn`] trait, the DSN->backend dispatcher ([`open_driver`] + the feature-gated
//! `open_mysql`/`open_postgres` stubs), and the pure DSN credential helpers (password injection /
//! redaction / percent-encoding). No backend detail lives here -- each concrete driver
//! ([`super::sqlite`] / [`super::postgres`] / [`super::mysql`]) implements the trait.

use super::handles::Binds;
use crate::value::Value;

/// A live database connection behind one backend (SQLite / Postgres / …). This is the multi-driver seam
/// (DEC-208 slice I): the generic layer holds a `Box<dyn DriverConn>` (carried by [`super::handles::DbConn`]
/// / [`super::handles::DbStmt`] and set to `None` on `close()`) and threads the accumulated
/// [`Binds`] + portable transaction-control SQL through it, never touching a dialect detail. Methods take
/// `&self` (a backend needing `&mut` — like the `postgres` client — wraps it in a `RefCell` internally); a
/// DB error is a plain `Err(String)` with an optional leading `<<Kind>>` taxonomy marker (the generic
/// layer wraps it into a `DatabaseResult.Err`).
pub(super) trait DriverConn: std::fmt::Debug {
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
/// [`super::postgres`] (feature `database-postgres`; a clear feature-gated `ConnectionError` when it is
/// off — never a fall-through to the SQLite file path); everything else (`sqlite:`, `:memory:`,
/// `sqlite::memory:`, or a bare path) → [`super::sqlite`], unchanged from the shipped runtime.
pub(super) fn open_driver(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    if dsn.starts_with("postgres://") || dsn.starts_with("postgresql://") {
        return open_postgres(dsn);
    }
    if dsn.starts_with("mysql://") || dsn.starts_with("mariadb://") {
        return open_mysql(dsn);
    }
    super::sqlite::open(dsn)
}

#[cfg(feature = "database-mysql")]
fn open_mysql(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    super::mysql::open(dsn)
}

#[cfg(not(feature = "database-mysql"))]
fn open_mysql(_dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    Err(
        "<<ConnectionError>>Core.DatabaseModule: the mysql driver is not compiled in \
         (build with --features database-mysql)"
            .to_string(),
    )
}

#[cfg(feature = "database-postgres")]
fn open_postgres(dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    super::postgres::open(dsn)
}

#[cfg(not(feature = "database-postgres"))]
fn open_postgres(_dsn: &str) -> Result<Box<dyn DriverConn>, String> {
    Err(
        "<<ConnectionError>>Core.DatabaseModule: the postgres driver is not compiled in \
         (build with --features database-postgres)"
            .to_string(),
    )
}

/// Inject a password into a `postgres://` DSN's authority (DEC-208 slice G — the `Database.withPassword`
/// factory). The password is percent-encoded and placed as the userinfo password
/// (`postgres://user:PW@host/…`); an existing DSN password is replaced. This is a PURE string transform
/// (no `postgres` dep), so the `Database.withPassword` surface type-checks under the plain `database` feature; the
/// resulting DSN is consumed IMMEDIATELY by `new Database(...)` inside the factory (never surfaced to user
/// code), and the driver parses the password back OUT into its config and stores only a redacted DSN, so
/// nothing retains the plaintext. A non-postgres DSN is returned unchanged (SQLite has no password).
pub(super) fn inject_pg_password(dsn: &str, pw: &str) -> String {
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
/// handle (it lives only transiently in the `Config` during connect), so this scrubs the one place a
/// raw DSN could still surface — the connect-time error path.
#[cfg_attr(
    not(any(feature = "database-postgres", feature = "database-mysql")),
    allow(dead_code)
)]
pub(super) fn redact_dsn_password(dsn: &str) -> String {
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
