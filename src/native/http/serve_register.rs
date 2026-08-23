//! `Core.Native.Http.registerServe` — the DEC-331 D5 registration native (slice S3.3a).
//!
//! `Http.serve(cfg, handler)` REGISTERS and RETURNS; the accept loop stays exactly where it already
//! is (`serve::serve`). This native is the whole of the Rust side of that registration: the phorj
//! prelude has already wrapped the user's typed `(Request) => Response` into the runtime's raw
//! `(bytes) => bytes` contract, so all that is left is to put the closure somewhere the serve
//! runtime can pick it up.
//!
//! TWO SLOTS, DELIBERATELY DIFFERENT KINDS — this is the whole safety argument, so it is stated
//! rather than left to be re-derived:
//!
//! * The HANDLER is a `Value::Closure`, i.e. `Rc`-bearing. It must NEVER reach a process global,
//!   because a global is reachable from every worker thread and `Rc` is not `Send`. It goes into a
//!   **thread-local**, is written by the web entry running on that thread, and is taken by that same
//!   thread's factory. Nothing `Rc`-bearing ever crosses a thread boundary — the same property
//!   `HandlerFactory` was built to preserve.
//! * The CONFIG is plain Rust scalars ([`ServeCfg`]), which are `Send`. It goes into a **process
//!   global**, because the parent thread needs `workers`/`host`/`port` to decide how many workers to
//!   spawn and where to bind — a decision that necessarily happens outside any worker.
//!
//! The config half is written but STILL not consumed, and that is not an oversight of S3.3c: `phg
//! serve` takes its host/port from CLI flags, and making the registered config win instead requires
//! the flag-vs-config conflict to HARD-ERROR rather than silently pick a winner. D1 ruled the
//! precedence ORDERING; the machinery that implements it is the pending S3.2 Part C ruling, so
//! wiring it now would mean inventing a precedence rule the developer has not made. Setting `port`
//! on a `ServeConfig` therefore does not yet move the socket. It is stored and covered by
//! `config_round_trips_through_the_global` so the slice that reads it inherits a tested path.

use super::{parg, NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;
use std::cell::RefCell;
use std::sync::Mutex;

/// The registered serve configuration, flattened to `Send` scalars (D4's field set).
///
/// `requestParsing` is deliberately absent: it is a phorj enum consumed inside the handler, not by
/// the loop that binds the socket, so flattening it here would create a second representation of a
/// value the program already holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeCfg {
    pub host: String,
    pub port: i64,
    /// `0` = AUTO (one worker per core) — the sentinel is resolved by the serve loop, never baked
    /// into the class default, so the value stays machine-independent (Invariant 10).
    pub workers: i64,
    /// Seconds; `0` = no timeout.
    pub timeout: i64,
    pub cert: Option<String>,
    pub key: Option<String>,
    pub server_name: Option<String>,
    pub max_body_size: i64,
    pub tls_min_version: String,
}

thread_local! {
    /// This thread's registered raw handler. See the module doc for why this is thread-local and the
    /// config is not.
    static HANDLER: RefCell<Option<Value>> = const { RefCell::new(None) };
}

/// The last registered configuration. A `Mutex` rather than a `OnceLock`: `phg serve` may build
/// several factories over one process lifetime (the test harness does), and a `OnceLock` would make
/// the first program's config outlive it.
static CONFIG: Mutex<Option<ServeCfg>> = Mutex::new(None);

/// Clear this thread's registration slot. Called before running a web entry so that a stale handler
/// from an earlier factory build on the same thread can never be mistaken for this one's — the
/// factory would otherwise silently serve the previous program.
pub fn reset() {
    HANDLER.with(|h| *h.borrow_mut() = None);
}

/// Take this thread's registered handler, leaving the slot empty. `None` means the web entry ran
/// without calling `Http.serve` — the caller turns that into a startup diagnostic, never a panic.
#[must_use]
pub fn take_handler() -> Option<Value> {
    HANDLER.with(|h| h.borrow_mut().take())
}

/// Read the last registered configuration.
///
/// Its reader arrived in S3.2 Part C (DEC-455.14): `cli::serve_program` calls this after the
/// factory's startup run has populated it, and hands it to `serve::settings::resolve` to decide what
/// the socket binds. The `#[expect(dead_code)]` this carried through S3.3a is therefore gone — which
/// is exactly what `expect` (rather than `allow`) was chosen to force.
#[must_use]
pub fn config() -> Option<ServeCfg> {
    CONFIG.lock().ok().and_then(|g| g.clone())
}

/// Read a `string`/`string?` field off the `ServeConfig` instance.
fn str_field(v: &Value, name: &str) -> Option<String> {
    match v {
        Value::Instance(i) => match i.get_field(name) {
            Some(Value::Str(s)) => Some(s.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Read an `int` field off the `ServeConfig` instance, falling back to the D4 default when the field
/// is absent or not an int. A `ServeConfig` is a promoted constructor with defaults, so every field
/// is always populated in practice; the fallback exists so a malformed instance degrades to the
/// documented default rather than faulting inside a registration call.
fn int_field(v: &Value, name: &str, default: i64) -> i64 {
    match v {
        Value::Instance(i) => match i.get_field(name) {
            Some(Value::Int(n)) => n,
            _ => default,
        },
        _ => default,
    }
}

fn cfg_from_value(v: &Value) -> ServeCfg {
    ServeCfg {
        host: str_field(v, "host").unwrap_or_else(|| "127.0.0.1".to_string()),
        port: int_field(v, "port", 8080),
        workers: int_field(v, "workers", 0),
        timeout: int_field(v, "timeout", 0),
        cert: str_field(v, "cert"),
        key: str_field(v, "key"),
        server_name: str_field(v, "serverName"),
        max_body_size: int_field(v, "maxBodySize", super::DEFAULT_MAX_BODY_SIZE as i64),
        tls_min_version: str_field(v, "tlsMinVersion").unwrap_or_else(|| "1.2".to_string()),
    }
}

/// `registerServe(ServeConfig, (bytes) => bytes) -> void`.
///
/// Registering TWICE in one entry is an ERROR, not a last-one-wins overwrite. Two `Http.serve` calls
/// in one web entry describe two servers, and silently running the second is the class of behaviour
/// this repo refuses on principle: the program would work while doing something its author did not
/// write. An error can be relaxed later without breaking anyone; a silent pick cannot.
fn native_register_serve(args: &[Value], _out: &mut String) -> Result<Value, String> {
    match args {
        [cfg, handler @ Value::Closure(_)] => {
            let already = HANDLER.with(|h| h.borrow().is_some());
            if already {
                return Err(
                    "Http.serve was called twice in one web entry — a web entry registers \
                            exactly one handler"
                        .into(),
                );
            }
            if let Ok(mut g) = CONFIG.lock() {
                *g = Some(cfg_from_value(cfg));
            }
            HANDLER.with(|h| *h.borrow_mut() = Some(handler.clone()));
            Ok(Value::Null)
        }
        _ => Err("Core.Native.Http.registerServe expects (ServeConfig, (bytes) => bytes)".into()),
    }
}

/// The `Core.Native.Http.registerServe` registry row.
///
/// `pure: false` — it mutates registration state. The `php:` mapping is UNREACHABLE by construction:
/// a program that calls `Http.serve` is refused by `phg transpile` with `E-TRANSPILE-SERVE`
/// (Invariant 14 tier 2), so the transpiler never reaches this row through a serve program. It is a
/// parse-safe stub rather than a real helper precisely so that it cannot quietly become a third
/// implementation of the serve loop.
pub(super) fn row() -> NativeFn {
    NativeFn {
        module: "Core.Native.Http",
        name: "registerServe",
        params: vec![
            Ty::Named("ServeConfig".into(), vec![]),
            Ty::Function(vec![Ty::Bytes], Box::new(Ty::Bytes), Vec::new()),
        ],
        ret: Ty::Void,
        pure: false,
        eval: NativeEval::Pure(native_register_serve),
        lift_from: &[],
        php: |a| format!("__phorj_http_register_serve({})", parg(a, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A REAL `ServeConfig` instance — built by running phorj source through the interpreter rather
    /// than hand-rolling an `Instance`, so the slot layout and the D4 defaults are the shipped ones.
    /// A hand-built instance would test this module against my own idea of the class.
    fn cfg_instance() -> Value {
        let prog = crate::cli::parse_checked_program(
            "package Main;\n\
             import Core.Http.ServeConfig;\n\
             function mk(): ServeConfig { return new ServeConfig(); }\n",
        )
        .expect("ServeConfig program checks");
        let (v, _) = crate::interpreter::call_named(&prog, "mk", vec![]).expect("mk runs");
        v
    }

    /// A real `(bytes) => bytes` closure value, likewise built by running phorj.
    fn raw_handler() -> Value {
        let prog = crate::cli::parse_checked_program(
            "package Main;\n\
             function mk(): (bytes) => bytes { return function(bytes b): bytes { return b; }; }\n",
        )
        .expect("closure program checks");
        let (v, _) = crate::interpreter::call_named(&prog, "mk", vec![]).expect("mk runs");
        v
    }

    /// The config half of the registration is WRITTEN but not yet READ by `phg serve` (increment 2
    /// wires it). This pins the whole round trip NOW — through the native, into the process global,
    /// back out of [`config`] — so the increment that consumes it inherits a tested path. An untested
    /// store is how a field silently arrives as its default.
    #[test]
    fn config_round_trips_through_the_global() {
        reset();
        let mut out = String::new();
        native_register_serve(&[cfg_instance(), raw_handler()], &mut out)
            .expect("registration succeeds");
        let cfg = config().expect("a config was stored");
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.workers, 0, "0 is the AUTO sentinel, not a real count");
        assert_eq!(
            cfg.max_body_size,
            super::super::DEFAULT_MAX_BODY_SIZE as i64
        );
        assert_eq!(cfg.tls_min_version, "1.2");
        assert_eq!(cfg.cert, None);
        assert_eq!(cfg.key, None);
        assert!(take_handler().is_some(), "the handler was stored too");
        assert!(
            take_handler().is_none(),
            "taking it must EMPTY the slot — a second take would otherwise resurrect a stale handler"
        );
    }

    /// Registering twice in one entry is an ERROR, not last-one-wins. Two `Http.serve` calls describe
    /// two servers; silently running the second would make the program do something its author did
    /// not write.
    #[test]
    fn a_second_registration_in_one_entry_is_an_error() {
        reset();
        let mut out = String::new();
        native_register_serve(&[cfg_instance(), raw_handler()], &mut out).expect("first succeeds");
        let err = native_register_serve(&[cfg_instance(), raw_handler()], &mut out)
            .expect_err("the second must be refused");
        assert!(err.contains("twice"), "wrong message: {err}");
        reset();
    }

    /// A non-`ServeConfig` first argument degrades to the documented D4 defaults rather than
    /// faulting — the fallback the field readers exist for.
    #[test]
    fn a_non_instance_config_degrades_to_the_documented_defaults() {
        let cfg = cfg_from_value(&Value::Int(7));
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 8080);
    }
}
