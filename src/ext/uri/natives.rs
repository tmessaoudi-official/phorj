//! `Core.Native.Uri` natives — the raw seam under the injected `Uri` class (DEC-240). All operate on
//! the Uri's stored RAW string (validated once at `parse`); fallible operations return either the
//! new raw string or a `<<E>>`-prefixed twin-exact error message (a raw URI can never start with
//! `<` — it is malformed everywhere — so the sentinel is collision-free). The prelude's
//! `UriError.fail` classifies the message into the typed taxonomy.
//!
//! PHP emissions wrap the twin directly (`Uri\Rfc3986\Uri` via the tiny `__phorj_uri*` helpers in
//! `runtime_php.rs`), so on the PHP leg the extension IS the implementation; the Rust kernel is
//! pinned to it by `kernel_tests.rs` + the probe record.

use super::kernel::{self, Parts, UriErr};
use crate::value::Value;

const EMARK: &str = "<<E>>";
const MSG_MALFORMED: &str = "The specified URI is malformed";
const MSG_PORT_RANGE: &str = "The port is out of range";
const MSG_BASE_NOT_ABS: &str = "The specified base URI must be absolute";

fn err_value(msg: &str) -> Value {
    Value::Str(format!("{EMARK}{msg}").into())
}

pub(super) fn component_msg(component: &str) -> String {
    format!("The specified {component} is malformed")
}

/// The single string argument of a getter/parse native.
fn arg_str<'a>(a: &'a [Value], what: &str) -> Result<&'a str, String> {
    match a.first() {
        Some(Value::Str(s)) => Ok(s),
        _ => Err(format!("{what}: expected a string argument")),
    }
}

/// Parse a STORED raw form — always valid by construction (only `parse`/withers mint them), so a
/// failure here is a phorj bug, surfaced loudly rather than masked.
fn stored(raw: &str) -> Result<Parts, String> {
    kernel::parse(raw).map_err(|e| {
        format!("Core.UriModule internal: stored raw re-parse failed ({e:?}) for {raw:?}")
    })
}

pub(super) fn opt_str(v: Option<String>) -> Value {
    match v {
        Some(s) => Value::Str(s.into()),
        None => Value::Null,
    }
}

// ── parse / render ──────────────────────────────────────────────────────────────────────────────

pub(super) fn uri_parse(a: &[Value], _: &mut String) -> Result<Value, String> {
    let s = arg_str(a, "Uri.parse")?;
    Ok(match kernel::parse(s) {
        Ok(_) => Value::Str(s.into()),
        Err(UriErr::Malformed) => err_value(MSG_MALFORMED),
        Err(UriErr::PortRange) => err_value(MSG_PORT_RANGE),
    })
}

pub(super) fn uri_to_string(a: &[Value], _: &mut String) -> Result<Value, String> {
    let p = stored(arg_str(a, "Uri.toString")?)?;
    Ok(Value::Str(kernel::to_string_normalized(&p).into()))
}

// ── getters (normalized + raw) ──────────────────────────────────────────────────────────────────

pub(super) fn getter(
    a: &[Value],
    what: &str,
    f: impl Fn(&Parts) -> Value,
) -> Result<Value, String> {
    let p = stored(arg_str(a, what)?)?;
    Ok(f(&p))
}

/// Split a userinfo into `(username, password)` — password is `""` when no `:` (twin-probed),
/// and the getters return null only when the whole userinfo is absent.
pub(super) fn split_userinfo(u: &str) -> (&str, &str) {
    match u.split_once(':') {
        Some((n, p)) => (n, p),
        None => (u, ""),
    }
}

// ── withers ─────────────────────────────────────────────────────────────────────────────────────

/// Apply a component swap: validate the new component (twin message on failure), swap, re-validate
/// the whole shape (a cross-component break — e.g. removing the scheme in front of a `a:b`-first
/// path segment — reports the same component message; the twin's exact choice here is unprobed, so
/// the component message is the [Inferred] mapping), and return the new raw form.
pub(super) fn wither(
    a: &[Value],
    what: &str,
    component: &str,
    apply: impl Fn(&mut Parts, &[Value]) -> Result<(), String>,
) -> Result<Value, String> {
    let mut p = stored(arg_str(a, what)?)?;
    if let Err(msg) = apply(&mut p, a) {
        return Ok(err_value(&msg));
    }
    Ok(match kernel::validate(&p) {
        Ok(()) => Value::Str(kernel::recompose(&p).into()),
        Err(UriErr::PortRange) => err_value(MSG_PORT_RANGE),
        Err(UriErr::Malformed) => err_value(&component_msg(component)),
    })
}

pub(super) fn opt_arg(a: &[Value], i: usize) -> Option<&Value> {
    match a.get(i) {
        Some(Value::Null) | None => None,
        Some(v) => Some(v),
    }
}

pub(super) fn with_string_component(
    a: &[Value],
    what: &str,
    component: &'static str,
    valid: impl Fn(&str) -> bool + Copy,
    set: impl Fn(&mut Parts, Option<String>) + Copy,
) -> Result<Value, String> {
    wither(a, what, component, |p, args| {
        match opt_arg(args, 1) {
            None => set(p, None),
            Some(Value::Str(s)) => {
                if !valid(s) {
                    return Err(component_msg(component));
                }
                set(p, Some(s.to_string()));
            }
            Some(_) => return Err(format!("{what}: expected a string argument")),
        }
        Ok(())
    })
}

// ── resolve / equals ────────────────────────────────────────────────────────────────────────────

pub(super) fn uri_resolve(a: &[Value], _: &mut String) -> Result<Value, String> {
    let base = stored(arg_str(a, "Uri.resolve")?)?;
    let r = match a.get(1) {
        Some(Value::Str(s)) => s,
        _ => return Err("Uri.resolve: expected a string reference".into()),
    };
    if base.scheme.is_none() {
        return Ok(err_value(MSG_BASE_NOT_ABS));
    }
    Ok(match kernel::parse(r) {
        Ok(rp) => Value::Str(kernel::recompose(&kernel::resolve(&base, &rp)).into()),
        Err(UriErr::Malformed) => err_value(MSG_MALFORMED),
        Err(UriErr::PortRange) => err_value(MSG_PORT_RANGE),
    })
}

pub(super) fn uri_equals(a: &[Value], _: &mut String) -> Result<Value, String> {
    let (x, y, include_fragment) = match a {
        [Value::Str(x), Value::Str(y), Value::Bool(f)] => (x, y, *f),
        _ => return Err("Uri.equals: expected (string, string, bool)".into()),
    };
    let mut px = stored(x)?;
    let mut py = stored(y)?;
    if !include_fragment {
        px.fragment = None;
        py.fragment = None;
    }
    Ok(Value::Bool(
        kernel::to_string_normalized(&px) == kernel::to_string_normalized(&py),
    ))
}
