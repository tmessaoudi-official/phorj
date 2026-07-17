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
use crate::native::{parg, NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;

const EMARK: &str = "<<E>>";
const MSG_MALFORMED: &str = "The specified URI is malformed";
const MSG_PORT_RANGE: &str = "The port is out of range";
const MSG_BASE_NOT_ABS: &str = "The specified base URI must be absolute";

fn err_value(msg: &str) -> Value {
    Value::Str(format!("{EMARK}{msg}").into())
}

fn component_msg(component: &str) -> String {
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

fn opt_str(v: Option<String>) -> Value {
    match v {
        Some(s) => Value::Str(s.into()),
        None => Value::Null,
    }
}

// ── parse / render ──────────────────────────────────────────────────────────────────────────────

fn uri_parse(a: &[Value], _: &mut String) -> Result<Value, String> {
    let s = arg_str(a, "Uri.parse")?;
    Ok(match kernel::parse(s) {
        Ok(_) => Value::Str(s.into()),
        Err(UriErr::Malformed) => err_value(MSG_MALFORMED),
        Err(UriErr::PortRange) => err_value(MSG_PORT_RANGE),
    })
}

fn uri_to_string(a: &[Value], _: &mut String) -> Result<Value, String> {
    let p = stored(arg_str(a, "Uri.toString")?)?;
    Ok(Value::Str(kernel::to_string_normalized(&p).into()))
}

// ── getters (normalized + raw) ──────────────────────────────────────────────────────────────────

fn getter(a: &[Value], what: &str, f: impl Fn(&Parts) -> Value) -> Result<Value, String> {
    let p = stored(arg_str(a, what)?)?;
    Ok(f(&p))
}

/// Split a userinfo into `(username, password)` — password is `""` when no `:` (twin-probed),
/// and the getters return null only when the whole userinfo is absent.
fn split_userinfo(u: &str) -> (&str, &str) {
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
fn wither(
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

fn opt_arg(a: &[Value], i: usize) -> Option<&Value> {
    match a.get(i) {
        Some(Value::Null) | None => None,
        Some(v) => Some(v),
    }
}

fn with_string_component(
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

fn uri_resolve(a: &[Value], _: &mut String) -> Result<Value, String> {
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

fn uri_equals(a: &[Value], _: &mut String) -> Result<Value, String> {
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

// ── registry ────────────────────────────────────────────────────────────────────────────────────

/// PHP: rebuild the twin object from the stored raw form (always valid, so never throws).
fn php_obj(raw: &str) -> String {
    format!("__phorj_uri({raw})")
}

pub fn uri_natives() -> Vec<NativeFn> {
    let str_ty = || Ty::String;
    let mut v = vec![
        NativeFn {
            module: "Core.Native.Uri",
            name: "parse",
            params: vec![str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(uri_parse),
            php: |a| format!("__phorj_uri_parse({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "toText",
            params: vec![str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(uri_to_string),
            php: |a| format!("{}->toString()", php_obj(parg(a, 0))),
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "resolve",
            params: vec![str_ty(), str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(uri_resolve),
            php: |a| format!("__phorj_uri_resolve({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "equals",
            params: vec![str_ty(), str_ty(), Ty::Bool],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(uri_equals),
            php: |a| {
                format!(
                    "{}->equals({}, {} ? \\Uri\\UriComparisonMode::IncludeFragment : \\Uri\\UriComparisonMode::ExcludeFragment)",
                    php_obj(parg(a, 0)),
                    php_obj(parg(a, 1)),
                    parg(a, 2)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "port",
            params: vec![str_ty()],
            ret: Ty::Optional(Box::new(Ty::Int)),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                getter(a, "Uri.port", |p| match p.port.as_deref() {
                    None | Some("") => Value::Null,
                    Some(digits) => Value::Int(
                        kernel::norm_port(digits)
                            .parse::<i64>()
                            .expect("validated port fits i64"),
                    ),
                })
            }),
            php: |a| format!("{}->getPort()", php_obj(parg(a, 0))),
        },
    ];
    // The normalized + raw string?-getter pairs. `NativeEval::Pure` and `NativeFn.php` are plain
    // fn pointers (no captures), so each getter is minted as a real fn by the macro.
    macro_rules! uri_getter {
        ($name:literal, $php_getter:literal, |$p:ident| $extract:expr) => {{
            fn eval(a: &[Value], _: &mut String) -> Result<Value, String> {
                getter(a, concat!("Uri.", $name), |$p| $extract)
            }
            fn php(a: &[String]) -> String {
                format!(concat!("__phorj_uri({})->", $php_getter, "()"), parg(a, 0))
            }
            NativeFn {
                module: "Core.Native.Uri",
                name: $name,
                params: vec![Ty::String],
                ret: Ty::Optional(Box::new(Ty::String)),
                pure: true,
                eval: NativeEval::Pure(eval),
                php,
            }
        }};
    }
    v.push(uri_getter!("scheme", "getScheme", |p| opt_str(
        p.scheme.as_deref().map(str::to_ascii_lowercase)
    )));
    v.push(uri_getter!("rawScheme", "getRawScheme", |p| opt_str(
        p.scheme.clone()
    )));
    v.push(uri_getter!("userInfo", "getUserInfo", |p| opt_str(
        p.userinfo.as_deref().map(kernel::pct_normalize)
    )));
    v.push(uri_getter!("rawUserInfo", "getRawUserInfo", |p| opt_str(
        p.userinfo.clone()
    )));
    v.push(uri_getter!("username", "getUsername", |p| opt_str(
        p.userinfo
            .as_deref()
            .map(|u| kernel::pct_normalize(split_userinfo(u).0))
    )));
    v.push(uri_getter!("rawUsername", "getRawUsername", |p| opt_str(
        p.userinfo
            .as_deref()
            .map(|u| split_userinfo(u).0.to_string())
    )));
    v.push(uri_getter!("password", "getPassword", |p| opt_str(
        p.userinfo
            .as_deref()
            .map(|u| kernel::pct_normalize(split_userinfo(u).1))
    )));
    v.push(uri_getter!("rawPassword", "getRawPassword", |p| opt_str(
        p.userinfo
            .as_deref()
            .map(|u| split_userinfo(u).1.to_string())
    )));
    v.push(uri_getter!("host", "getHost", |p| opt_str(
        p.host.as_deref().map(kernel::norm_host_getter)
    )));
    v.push(uri_getter!("rawHost", "getRawHost", |p| opt_str(
        p.host.clone()
    )));
    v.push(uri_getter!("query", "getQuery", |p| opt_str(
        p.query.as_deref().map(kernel::pct_normalize)
    )));
    v.push(uri_getter!("rawQuery", "getRawQuery", |p| opt_str(
        p.query.clone()
    )));
    v.push(uri_getter!("fragment", "getFragment", |p| opt_str(
        p.fragment.as_deref().map(kernel::pct_normalize)
    )));
    v.push(uri_getter!("rawFragment", "getRawFragment", |p| opt_str(
        p.fragment.clone()
    )));
    // `path` is always present (possibly empty) — a plain string getter.
    v.push(NativeFn {
        module: "Core.Native.Uri",
        name: "path",
        params: vec![str_ty()],
        ret: str_ty(),
        pure: true,
        eval: NativeEval::Pure(|a, _| {
            getter(a, "Uri.path", |p| {
                Value::Str(kernel::normalize(p).path.into())
            })
        }),
        php: |a| format!("{}->getPath()", php_obj(parg(a, 0))),
    });
    v.push(NativeFn {
        module: "Core.Native.Uri",
        name: "rawPath",
        params: vec![str_ty()],
        ret: str_ty(),
        pure: true,
        eval: NativeEval::Pure(|a, _| {
            getter(a, "Uri.rawPath", |p| Value::Str(p.path.clone().into()))
        }),
        php: |a| format!("{}->getRawPath()", php_obj(parg(a, 0))),
    });
    v.extend(wither_rows());
    v
}

/// The seven component withers. Each takes `(raw, newValue?)` and returns the new raw form or a
/// `<<E>>` message; the PHP side calls the twin's wither inside `__phorj_uri_with`, which catches
/// `InvalidUriException` into the same sentinel.
fn wither_rows() -> Vec<NativeFn> {
    let str_ty = || Ty::String;
    let opt_str_ty = || Ty::Optional(Box::new(Ty::String));
    vec![
        NativeFn {
            module: "Core.Native.Uri",
            name: "withScheme",
            params: vec![str_ty(), opt_str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                with_string_component(
                    a,
                    "Uri.withScheme",
                    "scheme",
                    kernel::valid_scheme,
                    |p, v| {
                        p.scheme = v;
                    },
                )
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withScheme', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "withUserInfo",
            params: vec![str_ty(), opt_str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                with_string_component(
                    a,
                    "Uri.withUserInfo",
                    "userinfo",
                    kernel::valid_userinfo,
                    |p, v| p.userinfo = v,
                )
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withUserInfo', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "withHost",
            params: vec![str_ty(), opt_str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                with_string_component(a, "Uri.withHost", "host", kernel::valid_host, |p, v| {
                    // Clearing the host removes the whole authority (userinfo/port have nowhere
                    // to live) — [Inferred] from the twin's recomposition shape; probed for the
                    // plain case (`withHost(null)` → `http:/p`).
                    if v.is_none() {
                        p.userinfo = None;
                        p.port = None;
                    }
                    p.host = v;
                })
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withHost', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "withPort",
            params: vec![str_ty(), Ty::Optional(Box::new(Ty::Int))],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                wither(a, "Uri.withPort", "port", |p, args| {
                    match opt_arg(args, 1) {
                        None => p.port = None,
                        Some(Value::Int(n)) => {
                            if *n < 0 {
                                return Err(component_msg("port"));
                            }
                            p.port = Some(n.to_string());
                        }
                        Some(_) => return Err("Uri.withPort: expected an int".into()),
                    }
                    Ok(())
                })
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withPort', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "withPath",
            params: vec![str_ty(), str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                wither(a, "Uri.withPath", "path", |p, args| match args.get(1) {
                    Some(Value::Str(s)) => {
                        p.path = s.to_string();
                        Ok(())
                    }
                    _ => Err("Uri.withPath: expected a string".into()),
                })
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withPath', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "withQuery",
            params: vec![str_ty(), opt_str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                with_string_component(
                    a,
                    "Uri.withQuery",
                    "query",
                    kernel::valid_query_or_fragment,
                    |p, v| p.query = v,
                )
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withQuery', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Native.Uri",
            name: "withFragment",
            params: vec![str_ty(), opt_str_ty()],
            ret: str_ty(),
            pure: true,
            eval: NativeEval::Pure(|a, _| {
                with_string_component(
                    a,
                    "Uri.withFragment",
                    "fragment",
                    kernel::valid_query_or_fragment,
                    |p, v| p.fragment = v,
                )
            }),
            php: |a| {
                format!(
                    "__phorj_uri_with({}, 'withFragment', {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
    ]
}
