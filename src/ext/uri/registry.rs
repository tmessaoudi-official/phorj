//! `Core.Native.Uri` registry — the `NativeFn` rows (`uri_natives`) and the seven component-wither
//! rows (`wither_rows`), plus the `php_obj` twin-rebuild helper. Split out of `natives.rs` (file-size
//! cap, Invariant 13); the native eval/php bodies it wires up live there (`use super::natives::*`).

use super::kernel;
use super::natives::*;
use crate::native::{parg, NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;

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
