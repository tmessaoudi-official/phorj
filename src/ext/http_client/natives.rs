//! `Core.HttpClientModule` (W3-2 — the TOP-20 #2 parity blocker): a SYNC HTTP/1.1 client, std
//! `TcpStream` + `rustls` for https. The `Core.DatabaseModule`/`Core.Mail` architecture verbatim: natives under
//! the disjoint `Core.Native.HttpClient` qualifier return the prelude-local `HcResult<T>`; the prelude
//! throws the typed `HttpClientError` taxonomy off `<<Kind>>` markers. Native-only
//! (`E-TRANSPILE-HTTPCLIENT`, pipeline ladder gate): live network I/O cannot be byte-identical —
//! a faithful PHP curl-mapping is a recorded future lift, never a silent one. All natives are
//! `pure:false` (spine-quarantined); correctness is `tests/http_client.rs` against an in-process
//! `std::net::TcpListener` fixture server (deterministic, no external network).
//!
//! Scope (v1, honest): HTTP/1.1 over http/https · request bodies · custom headers · redirect
//! following (≤ limit, GET/HEAD semantics preserved, 303 → GET) · `Content-Length` AND chunked
//! response bodies · connect+read timeouts · response size cap (64 MB) · `Accept-Encoding:
//! identity` (no compression — honest, no silent gunzip). NOT in v1 (documented): HTTP/2,
//! keep-alive pooling, proxies, cookies (userland or a later slice).

use super::engine::run_request;
use super::protocol::RawResponse;
use crate::native::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{DbObject, EnumVal, Value};
use std::any::Any;
use std::rc::Rc;

// ── HcResult wrappers (the DatabaseResult mechanism) ───────────────────────────────────────────────────────

fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "HcResult".into(),
        variant: "Ok".into(),
        payload: crate::value::Payload::One(v),
    }))
}

fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "HcResult".into(),
        variant: "Err".into(),
        payload: crate::value::Payload::One(Value::Str(msg.into())),
    }))
}

fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

// ── Natives ──────────────────────────────────────────────────────────────────────────────────────────

/// The response handle (`Value::Db`-opaque, the Core.DatabaseModule pattern): inert data the typed accessor
/// natives below read; the prelude wraps it in the `HttpResponse` class.
#[derive(Debug)]
struct HttpRespObj {
    resp: RawResponse,
}

impl DbObject for HttpRespObj {
    fn kind(&self) -> &'static str {
        "http-response"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn as_resp(v: &Value) -> Result<&HttpRespObj, String> {
    match v {
        Value::Db(h) => h
            .as_any()
            .downcast_ref::<HttpRespObj>()
            .ok_or_else(|| "Core.HttpClientModule: expected a response handle".to_string()),
        other => Err(format!(
            "Core.HttpClientModule: expected a response handle, got {}",
            other.type_name()
        )),
    }
}

/// `HttpClientSys.request(method, url, headerNames, headerValues, body, timeoutMs, maxRedirects,
/// allowPrivateHosts)` → an opaque response handle the typed accessors below read.
fn request_inner(args: &[Value]) -> Result<Value, String> {
    let (method, url, hn, hv, body, timeout_ms, max_redirects, allow_private) = match args {
        [Value::Str(m), Value::Str(u), Value::List(hn), Value::List(hv), body, Value::Int(t), Value::Int(r), Value::Bool(ap)] => {
            (m.as_str(), u.as_str(), hn, hv, body, *t, *r, *ap)
        }
        _ => {
            return Err(
                "Core.HttpClientModule.__request expects (string, string, List, List, bytes|string, int, int, bool)"
                    .into(),
            )
        }
    };
    let method_up = method.to_ascii_uppercase();
    if !matches!(
        method_up.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        return Err(format!(
            "<<InvalidUrlError>>Core.HttpClientModule: unsupported method `{method}`"
        ));
    }
    if hn.len() != hv.len() {
        return Err("Core.HttpClientModule.__request: header name/value length mismatch".into());
    }
    let mut headers = Vec::with_capacity(hn.len());
    for (n, v) in hn.iter().zip(hv.iter()) {
        match (n, v) {
            (Value::Str(n), Value::Str(v)) => {
                let (n, v) = (n.as_str(), v.as_str());
                // The injection gate: header names/values may not carry CR/LF (request smuggling).
                if n.chars().any(|c| c == '\r' || c == '\n' || c == ':')
                    || v.chars().any(|c| c == '\r' || c == '\n')
                {
                    return Err(format!(
                        "<<InvalidUrlError>>Core.HttpClientModule: header `{n}` contains a forbidden character"
                    ));
                }
                headers.push((n.to_string(), v.to_string()));
            }
            _ => return Err("Core.HttpClientModule.__request: headers must be strings".into()),
        }
    }
    let body_bytes: Vec<u8> = match body {
        Value::Bytes(b) => (**b).clone(),
        Value::Str(s) => s.as_bytes().to_vec(),
        Value::Null => Vec::new(),
        other => {
            return Err(format!(
                "Core.HttpClientModule.__request: body must be string/bytes/null, got {}",
                other.type_name()
            ))
        }
    };
    let timeout_ms = u64::try_from(timeout_ms.max(1)).unwrap_or(30_000);
    let max_redirects = u32::try_from(max_redirects.max(0)).unwrap_or(0);
    let resp = run_request(
        &method_up,
        url,
        &headers,
        &body_bytes,
        timeout_ms,
        max_redirects,
        allow_private,
    )?;
    Ok(Value::Db(Rc::new(HttpRespObj { resp })))
}

fn status_inner(args: &[Value]) -> Result<Value, String> {
    let r = match args {
        [h] => as_resp(h)?,
        _ => return Err("Core.HttpClientModule.__status expects (handle)".into()),
    };
    Ok(Value::Int(i64::from(r.resp.status)))
}

fn header_inner(args: &[Value]) -> Result<Value, String> {
    let (r, name) = match args {
        [h, Value::Str(n)] => (as_resp(h)?, n.as_str().to_ascii_lowercase()),
        _ => return Err("Core.HttpClientModule.__header expects (handle, string)".into()),
    };
    Ok(r.resp
        .headers
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| Value::Str(v.as_str().into()))
        .unwrap_or(Value::Null))
}

fn header_names_inner(args: &[Value]) -> Result<Value, String> {
    let r = match args {
        [h] => as_resp(h)?,
        _ => return Err("Core.HttpClientModule.__headerNames expects (handle)".into()),
    };
    Ok(Value::List(Rc::new(
        r.resp
            .headers
            .iter()
            .map(|(n, _)| Value::Str(n.as_str().into()))
            .collect(),
    )))
}

fn body_bytes_inner(args: &[Value]) -> Result<Value, String> {
    let r = match args {
        [h] => as_resp(h)?,
        _ => return Err("Core.HttpClientModule.__bodyBytes expects (handle)".into()),
    };
    Ok(Value::Bytes(Rc::new(r.resp.body.clone())))
}

fn body_text_inner(args: &[Value]) -> Result<Value, String> {
    let r = match args {
        [h] => as_resp(h)?,
        _ => return Err("Core.HttpClientModule.__bodyText expects (handle)".into()),
    };
    match String::from_utf8(r.resp.body.clone()) {
        Ok(s) => Ok(Value::Str(s.into())),
        Err(_) => Err(
            "<<ProtocolError>>Core.HttpClientModule: response body is not UTF-8 — read bodyBytes()"
                .into(),
        ),
    }
}

macro_rules! hc_native {
    ($name:ident, $inner:ident) => {
        pub(super) fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap($inner(args)))
        }
    };
}
hc_native!(hc_request, request_inner);
hc_native!(hc_status, status_inner);
hc_native!(hc_header, header_inner);
hc_native!(hc_header_names, header_names_inner);
hc_native!(hc_body_bytes, body_bytes_inner);
hc_native!(hc_body_text, body_text_inner);

pub fn http_client_natives() -> Vec<NativeFn> {
    let handle = || Ty::Named("HttpClientHandle".into(), vec![]);
    let res = |t: Ty| Ty::Named("HcResult".into(), vec![t]);
    let entry =
        |name: &'static str,
         params: Vec<Ty>,
         ret: Ty,
         eval: fn(&[Value], &mut String) -> Result<Value, String>| NativeFn {
            module: "Core.Native.HttpClient",
            name,
            params,
            ret,
            pure: false,
            eval: NativeEval::Pure(eval),
            lift_from: &[],
            php: |a| a.first().cloned().unwrap_or_else(|| "''".to_string()),
        };
    vec![
        entry(
            "request",
            vec![
                Ty::String,
                Ty::String,
                Ty::List(Box::new(Ty::String)),
                Ty::List(Box::new(Ty::String)),
                Ty::Bytes,
                Ty::Int,
                Ty::Int,
                Ty::Bool, // DEC-270: allowPrivateHosts — bypass the SSRF guard when true
            ],
            res(handle()),
            hc_request,
        ),
        entry("status", vec![handle()], res(Ty::Int), hc_status),
        entry(
            "header",
            vec![handle(), Ty::String],
            res(Ty::Optional(Box::new(Ty::String))),
            hc_header,
        ),
        entry(
            "headerNames",
            vec![handle()],
            res(Ty::List(Box::new(Ty::String))),
            hc_header_names,
        ),
        entry("bodyBytes", vec![handle()], res(Ty::Bytes), hc_body_bytes),
        entry("bodyText", vec![handle()], res(Ty::String), hc_body_text),
    ]
}

// Unit tests live in the sibling `http_client_tests.rs` (Invariant 13 sizing discipline), mounted
// as a child module for private visibility.
