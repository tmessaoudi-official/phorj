//! `Core.HttpClient` (W3-2 — the TOP-20 #2 parity blocker): a SYNC HTTP/1.1 client, std
//! `TcpStream` + `rustls` for https. The `Core.Db`/`Core.Mail` architecture verbatim: natives under
//! the disjoint `Core.HttpClientSys` qualifier return the prelude-local `HcResult<T>`; the prelude
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

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{DbObject, EnumVal, Value};
use std::any::Any;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

// ── HcResult wrappers (the DbResult mechanism) ───────────────────────────────────────────────────────

fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "HcResult".into(),
        variant: "Ok".into(),
        payload: vec![v],
    }))
}

fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "HcResult".into(),
        variant: "Err".into(),
        payload: vec![Value::Str(msg.into())],
    }))
}

fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

// ── URL parsing (std-only, the subset a client needs) ────────────────────────────────────────────────

/// A parsed absolute http(s) URL: scheme, host, port, path+query (the request target).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Url {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub target: String,
}

/// Parse an absolute `http://`/`https://` URL. Rejects other schemes, userinfo (credential
/// smuggling — pass credentials via headers, never the URL), and empty hosts — each a clean
/// `<<InvalidUrl>>`. IPv6 literals in brackets are supported.
pub(crate) fn parse_url(url: &str) -> Result<Url, String> {
    let bad = |m: &str| format!("<<InvalidUrl>>Core.HttpClient: {m}: `{url}`");
    let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(bad("only http:// and https:// URLs are supported"));
    };
    let (authority, target) = match rest.find(['/', '?']) {
        Some(i) if rest.as_bytes()[i] == b'?' => (&rest[..i], format!("/{}", &rest[i..])),
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    if authority.contains('@') {
        return Err(bad(
            "URL userinfo (user:pass@) is not accepted — send credentials in a header",
        ));
    }
    let (host, port) = if let Some(h) = authority.strip_prefix('[') {
        // IPv6 literal: [::1]:8080
        let end = h.find(']').ok_or_else(|| bad("unclosed IPv6 literal"))?;
        let host = &h[..end];
        let port = match h[end + 1..].strip_prefix(':') {
            Some(p) => p.parse::<u16>().map_err(|_| bad("invalid port"))?,
            None if h[end + 1..].is_empty() => default_port(https),
            _ => return Err(bad("junk after IPv6 literal")),
        };
        (host.to_string(), port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (
                h.to_string(),
                p.parse::<u16>().map_err(|_| bad("invalid port"))?,
            ),
            None => (authority.to_string(), default_port(https)),
        }
    };
    if host.is_empty() {
        return Err(bad("empty host"));
    }
    Ok(Url {
        https,
        host,
        port,
        target,
    })
}

fn default_port(https: bool) -> u16 {
    if https {
        443
    } else {
        80
    }
}

/// Resolve a redirect `Location` against the current URL: absolute stands alone; `/path` keeps the
/// origin; a relative path resolves against the current target's directory.
pub(crate) fn resolve_location(cur: &Url, location: &str) -> Result<Url, String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return parse_url(location);
    }
    let mut next = cur.clone();
    if let Some(abs) = location.strip_prefix('/') {
        next.target = format!("/{abs}");
    } else {
        let base = match cur.target.rfind('/') {
            Some(i) => &cur.target[..=i],
            None => "/",
        };
        next.target = format!("{base}{location}");
    }
    Ok(next)
}

// ── Response reading (Content-Length + chunked) ──────────────────────────────────────────────────────

/// Response size cap: 64 MB — a DoS bound (PHP's curl has none by default; ours is explicit).
const MAX_RESPONSE: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct RawResponse {
    pub status: u16,
    /// Header (lowercased-name, value) pairs in wire order.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Read one HTTP/1.1 response from `r` (headers, then a `Content-Length`, chunked, or
/// read-to-close body). Timeout/size violations are clean errors.
pub(crate) fn read_response(r: &mut impl Read) -> Result<RawResponse, String> {
    let io = |e: std::io::Error| classify_io(&e);
    // Read until the header terminator.
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = find_crlfcrlf(&buf) {
            break i;
        }
        if buf.len() > 1024 * 1024 {
            return Err("<<ProtocolError>>Core.HttpClient: response headers exceed 1 MB".into());
        }
        let n = r.read(&mut chunk).map_err(io)?;
        if n == 0 {
            return Err(
                "<<ConnectionFailed>>Core.HttpClient: connection closed before headers completed"
                    .into(),
            );
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            format!("<<ProtocolError>>Core.HttpClient: malformed status line `{status_line}`")
        })?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((n, v)) = line.split_once(':') {
            headers.push((n.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let mut body: Vec<u8> = buf[head_end + 4..].to_vec();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    let chunked = header("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    if chunked {
        // Keep reading until the terminal 0-chunk is present, then decode.
        while !has_final_chunk(&body) {
            if body.len() > MAX_RESPONSE {
                return Err(size_err());
            }
            let n = r.read(&mut chunk).map_err(io)?;
            if n == 0 {
                return Err(
                    "<<ProtocolError>>Core.HttpClient: connection closed mid-chunked-body".into(),
                );
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body = decode_chunked(&body)?;
    } else if let Some(len) = header("content-length") {
        let len: usize = len.parse().map_err(|_| {
            "<<ProtocolError>>Core.HttpClient: malformed Content-Length".to_string()
        })?;
        if len > MAX_RESPONSE {
            return Err(size_err());
        }
        while body.len() < len {
            let n = r.read(&mut chunk).map_err(io)?;
            if n == 0 {
                return Err("<<ProtocolError>>Core.HttpClient: connection closed mid-body".into());
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(len);
    } else {
        // No length, not chunked: read to close (HTTP/1.0-style).
        loop {
            if body.len() > MAX_RESPONSE {
                return Err(size_err());
            }
            let n = match r.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => 0,
                Err(e) => return Err(io(e)),
            };
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
    }
    Ok(RawResponse {
        status,
        headers,
        body,
    })
}

fn size_err() -> String {
    "<<TooLarge>>Core.HttpClient: response exceeds the 64 MB cap".into()
}

fn find_crlfcrlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Does a raw chunked body already contain its terminal `0\r\n...\r\n\r\n`? A cheap containment
/// probe (decode validates properly afterwards).
fn has_final_chunk(buf: &[u8]) -> bool {
    // The terminal chunk is "0\r\n" followed (possibly after trailers) by "\r\n".
    buf.windows(5).any(|w| w == b"0\r\n\r\n") || buf.windows(7).any(|w| w == b"\r\n0\r\n\r\n")
}

/// Decode a complete chunked body (chunk-size lines in hex; trailers dropped).
pub(crate) fn decode_chunked(mut buf: &[u8]) -> Result<Vec<u8>, String> {
    let bad = || "<<ProtocolError>>Core.HttpClient: malformed chunked body".to_string();
    let mut out = Vec::with_capacity(buf.len());
    loop {
        let line_end = buf.windows(2).position(|w| w == b"\r\n").ok_or_else(bad)?;
        let size_str = std::str::from_utf8(&buf[..line_end]).map_err(|_| bad())?;
        let size_hex = size_str.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16).map_err(|_| bad())?;
        buf = &buf[line_end + 2..];
        if size == 0 {
            return Ok(out); // trailers (if any) dropped
        }
        if buf.len() < size + 2 {
            return Err(bad());
        }
        out.extend_from_slice(&buf[..size]);
        if &buf[size..size + 2] != b"\r\n" {
            return Err(bad());
        }
        buf = &buf[size + 2..];
        if out.len() > MAX_RESPONSE {
            return Err(size_err());
        }
    }
}

fn classify_io(e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    let kind = match e.kind() {
        K::TimedOut | K::WouldBlock => "Timeout",
        _ => "ConnectionFailed",
    };
    format!("<<{kind}>>Core.HttpClient: {e}")
}

// ── The request engine ───────────────────────────────────────────────────────────────────────────────

fn write_request(
    w: &mut impl Write,
    method: &str,
    url: &Url,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<(), String> {
    let mut req = format!("{method} {} HTTP/1.1\r\n", url.target);
    let has = |name: &str| headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    if !has("host") {
        let default = url.port == default_port(url.https);
        if default {
            req.push_str(&format!("Host: {}\r\n", url.host));
        } else {
            req.push_str(&format!("Host: {}:{}\r\n", url.host, url.port));
        }
    }
    req.push_str("Connection: close\r\nAccept-Encoding: identity\r\n");
    if (!body.is_empty() || matches!(method, "POST" | "PUT" | "PATCH")) && !has("content-length") {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    for (n, v) in headers {
        req.push_str(&format!("{n}: {v}\r\n"));
    }
    req.push_str("\r\n");
    w.write_all(req.as_bytes())
        .and_then(|()| w.write_all(body))
        .and_then(|()| w.flush())
        .map_err(|e| classify_io(&e))
}

/// One HTTP exchange (no redirect logic here). https rides rustls with webpki roots. `allow_private`
/// opts out of the DEC-270 SSRF guard for this connection.
fn exchange(
    url: &Url,
    method: &str,
    headers: &[(String, String)],
    body: &[u8],
    timeout_ms: u64,
    allow_private: bool,
) -> Result<RawResponse, String> {
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let addr = format!("{}:{}", url.host, url.port);
    // DNS-PIN + SSRF guard (DEC-270): resolve ONCE, and connect to THAT resolved IP — never re-resolve
    // (defeats DNS-rebinding, where a second lookup swaps in a private address). The chosen address is
    // checked against `is_blocked_ip` unless `allow_private`; blocking here (before connect) covers
    // every redirect hop too, since each hop re-enters `exchange` with its own freshly-resolved host.
    let sock_addrs = std::net::ToSocketAddrs::to_socket_addrs(&addr)
        .map_err(|e| format!("<<ConnectionFailed>>Core.HttpClient: resolve `{addr}`: {e}"))?
        .next()
        .ok_or_else(|| {
            format!("<<ConnectionFailed>>Core.HttpClient: `{addr}` resolved to no address")
        })?;
    if !allow_private && is_blocked_ip(sock_addrs.ip()) {
        // Name the REQUESTED host, not the resolved IP — echoing the specific private address it
        // resolved to would be a minor internal-DNS resolution oracle for an attacker-supplied URL.
        return Err(format!(
            "<<BlockedAddress>>Core.HttpClient: refusing to connect to `{addr}` — it resolves to a \
             private, link-local, or metadata address (SSRF guard); pass `.allowPrivateHosts(true)` to permit it"
        ));
    }
    let stream = TcpStream::connect_timeout(&sock_addrs, timeout).map_err(|e| classify_io(&e))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| classify_io(&e))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| classify_io(&e))?;
    if url.https {
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let name: rustls::pki_types::ServerName<'static> =
            url.host.clone().try_into().map_err(|_| {
                format!(
                    "<<TlsError>>Core.HttpClient: invalid TLS name `{}`",
                    url.host
                )
            })?;
        let conn = rustls::ClientConnection::new(Arc::new(config), name)
            .map_err(|e| format!("<<TlsError>>Core.HttpClient: {e}"))?;
        let mut tls = rustls::StreamOwned::new(conn, stream);
        write_request(&mut tls, method, url, headers, body)?;
        read_response(&mut tls).map_err(|e| {
            // rustls surfaces handshake failures on first I/O — retag them as TLS.
            if e.contains("Connection") && e.contains("Alert") {
                e.replace("<<ConnectionFailed>>", "<<TlsError>>")
            } else {
                e
            }
        })
    } else {
        let mut stream = stream;
        write_request(&mut stream, method, url, headers, body)?;
        read_response(&mut stream)
    }
}

/// SSRF guard (DEC-270, refined): is this resolved address one the client must REFUSE by default?
/// Blocked: RFC1918 private (10/8, 172.16/12, 192.168/16), CGNAT/shared 100.64/10 (RFC 6598 — holds
/// e.g. Alibaba metadata 100.100.100.200), IETF-assignments 192.0.0.0/24 (a documented metadata/SSRF
/// range), link-local 169.254/16 (INCLUDING the cloud-metadata endpoint 169.254.169.254), the
/// unspecified `0.0.0.0`/`::`, IPv4 broadcast, IPv6 ULA (fc00::/7) and IPv6 link-local (fe80::/10).
/// ALLOWED: loopback (127.0.0.0/8, `::1`) — overwhelmingly a legitimate target (local services,
/// sidecars, dev), unlike the metadata / internal-LAN destinations that are the real SSRF-exfiltration
/// targets. Every IPv6 form that EMBEDS an IPv4 address — IPv4-mapped `::ffff:a.b.c.d`, IPv4-compatible
/// `::a.b.c.d`, 6to4 `2002:AABB:CCDD::`, NAT64 `64:ff9b::a.b.c.d` — is decoded and re-checked as its
/// embedded v4, so a private target can't be smuggled through an IPv6 literal (the NAT64/DNS64 bypass).
/// The opt-in `HttpClient.allowPrivateHosts(true)` bypasses this deliberately.
pub(crate) fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => is_blocked_v4(v4),
        std::net::IpAddr::V6(v6) => match embedded_v4(v6) {
            Some(v4) => is_blocked_v4(v4),
            None => is_blocked_v6(v6),
        },
    }
}

/// The IPv4 address embedded in an IPv6 address, for every well-known embedding an SSRF check must see
/// through: IPv4-mapped (`::ffff:0:0/96`), IPv4-compatible (`::/96`, deprecated but cheap to cover),
/// 6to4 (`2002::/16` — bytes 2-3-4-5 are the v4), and NAT64 (`64:ff9b::/96`). `None` = not an
/// IPv4-embedding form (a genuine IPv6 address → `is_blocked_v6`).
fn embedded_v4(a: std::net::Ipv6Addr) -> Option<std::net::Ipv4Addr> {
    if let Some(m) = a.to_ipv4_mapped() {
        return Some(m); // ::ffff:a.b.c.d
    }
    let seg = a.segments();
    // 6to4 2002:AABB:CCDD::/48 — the v4 is segments 1..3.
    if seg[0] == 0x2002 {
        return Some(std::net::Ipv4Addr::new(
            (seg[1] >> 8) as u8,
            seg[1] as u8,
            (seg[2] >> 8) as u8,
            seg[2] as u8,
        ));
    }
    // NAT64 64:ff9b::/96 — the v4 is the low 32 bits (segments 6..8).
    if seg[0] == 0x0064 && seg[1] == 0xff9b && seg[2..6].iter().all(|&s| s == 0) {
        return Some(std::net::Ipv4Addr::new(
            (seg[6] >> 8) as u8,
            seg[6] as u8,
            (seg[7] >> 8) as u8,
            seg[7] as u8,
        ));
    }
    // IPv4-compatible ::a.b.c.d — high 96 bits zero, low 32 nonzero (`::` and `::1` are handled as
    // unspecified/loopback elsewhere; exclude them so they aren't re-read as 0.0.0.x).
    if seg[0..6].iter().all(|&s| s == 0) && (seg[6] != 0 || seg[7] > 1) {
        return Some(std::net::Ipv4Addr::new(
            (seg[6] >> 8) as u8,
            seg[6] as u8,
            (seg[7] >> 8) as u8,
            seg[7] as u8,
        ));
    }
    None
}

fn is_blocked_v4(a: std::net::Ipv4Addr) -> bool {
    if a.is_loopback() {
        return false; // 127.0.0.0/8 — allowed
    }
    let o = a.octets();
    let cgnat = o[0] == 100 && (64..=127).contains(&o[1]); // 100.64.0.0/10 (RFC 6598)
    let ietf_assign = o[0] == 192 && o[1] == 0 && o[2] == 0; // 192.0.0.0/24 (incl. 192.0.0.192)
    a.is_private()
        || a.is_link_local()
        || a.is_unspecified()
        || a.is_broadcast()
        || cgnat
        || ietf_assign
}

fn is_blocked_v6(a: std::net::Ipv6Addr) -> bool {
    if a.is_loopback() {
        return false; // ::1 — allowed
    }
    if a.is_unspecified() {
        return true; // ::
    }
    let seg = a.segments();
    let ula = (seg[0] >> 8) as u8 & 0xfe == 0xfc; // fc00::/7
    let link_local = seg[0] & 0xffc0 == 0xfe80; // fe80::/10
    ula || link_local
}

/// Two http(s) URLs share an ORIGIN iff scheme, host (ASCII-case-insensitive), and port all match.
/// A redirect that crosses origins — INCLUDING an https→http downgrade (a scheme change) — must not
/// carry credentials forward (DEC-264).
pub(crate) fn same_origin(a: &Url, b: &Url) -> bool {
    a.https == b.https && a.port == b.port && a.host.eq_ignore_ascii_case(&b.host)
}

/// Headers that must be STRIPPED before following a cross-origin redirect (DEC-264): sending them to a
/// different origin leaks credentials to a host the caller never authorized (the curl CVE-2022-27774
/// class). `Cookie` and `Authorization` are the obvious ones; `Proxy-Authorization` (proxy creds) and
/// `WWW-Authenticate` (a challenge echoed back) round out the set. Same-origin hops keep every header.
fn is_credential_header(name: &str) -> bool {
    const SENSITIVE: [&str; 4] = [
        "authorization",
        "cookie",
        "proxy-authorization",
        "www-authenticate",
    ];
    SENSITIVE.iter().any(|s| name.eq_ignore_ascii_case(s))
}

/// The header set to send to `to`, given the headers sent to `from`: identical on a same-origin hop,
/// credential-stripped on a cross-origin (or downgrading) one (DEC-264). Pure + unit-tested.
fn headers_for_hop(from: &Url, to: &Url, headers: &[(String, String)]) -> Vec<(String, String)> {
    if same_origin(from, to) {
        return headers.to_vec();
    }
    headers
        .iter()
        .filter(|(n, _)| !is_credential_header(n))
        .cloned()
        .collect()
}

/// The full request: redirects (301/302/303/307/308) followed up to `max_redirects`; a 303 (and,
/// per the historical browser contract, 301/302 on POST) downgrades to GET with an empty body.
/// Credentials are stripped on any cross-origin / downgrading hop (DEC-264).
pub(crate) fn run_request(
    method: &str,
    url_str: &str,
    headers: &[(String, String)],
    body: &[u8],
    timeout_ms: u64,
    max_redirects: u32,
    allow_private: bool,
) -> Result<RawResponse, String> {
    let mut url = parse_url(url_str)?;
    let mut method = method.to_string();
    let mut body = body.to_vec();
    // Working header set — narrowed (never widened) as redirects cross origins. Once a credential is
    // dropped at a cross-origin hop it stays dropped, even if a later hop returns to the first origin.
    let mut headers = headers.to_vec();
    let mut hops = 0u32;
    loop {
        let resp = exchange(&url, &method, &headers, &body, timeout_ms, allow_private)?;
        let redirect = matches!(resp.status, 301 | 302 | 303 | 307 | 308);
        if !redirect {
            return Ok(resp);
        }
        if hops >= max_redirects {
            return Err(format!(
                "<<TooManyRedirects>>Core.HttpClient: exceeded {max_redirects} redirects"
            ));
        }
        let location = resp
            .headers
            .iter()
            .find(|(n, _)| n == "location")
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                format!(
                    "<<ProtocolError>>Core.HttpClient: {} redirect without a Location header",
                    resp.status
                )
            })?;
        let next = resolve_location(&url, &location)?;
        headers = headers_for_hop(&url, &next, &headers);
        url = next;
        if resp.status == 303 || (matches!(resp.status, 301 | 302) && method == "POST") {
            method = "GET".into();
            body.clear();
        }
        hops += 1;
    }
}

// ── Natives ──────────────────────────────────────────────────────────────────────────────────────────

/// The response handle (`Value::Db`-opaque, the Core.Db pattern): inert data the typed accessor
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
            .ok_or_else(|| "Core.HttpClient: expected a response handle".to_string()),
        other => Err(format!(
            "Core.HttpClient: expected a response handle, got {}",
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
                "Core.HttpClient.__request expects (string, string, List, List, bytes|string, int, int, bool)"
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
            "<<InvalidUrl>>Core.HttpClient: unsupported method `{method}`"
        ));
    }
    if hn.len() != hv.len() {
        return Err("Core.HttpClient.__request: header name/value length mismatch".into());
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
                        "<<InvalidUrl>>Core.HttpClient: header `{n}` contains a forbidden character"
                    ));
                }
                headers.push((n.to_string(), v.to_string()));
            }
            _ => return Err("Core.HttpClient.__request: headers must be strings".into()),
        }
    }
    let body_bytes: Vec<u8> = match body {
        Value::Bytes(b) => (**b).clone(),
        Value::Str(s) => s.as_bytes().to_vec(),
        Value::Null => Vec::new(),
        other => {
            return Err(format!(
                "Core.HttpClient.__request: body must be string/bytes/null, got {}",
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
        _ => return Err("Core.HttpClient.__status expects (handle)".into()),
    };
    Ok(Value::Int(i64::from(r.resp.status)))
}

fn header_inner(args: &[Value]) -> Result<Value, String> {
    let (r, name) = match args {
        [h, Value::Str(n)] => (as_resp(h)?, n.as_str().to_ascii_lowercase()),
        _ => return Err("Core.HttpClient.__header expects (handle, string)".into()),
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
        _ => return Err("Core.HttpClient.__headerNames expects (handle)".into()),
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
        _ => return Err("Core.HttpClient.__bodyBytes expects (handle)".into()),
    };
    Ok(Value::Bytes(Rc::new(r.resp.body.clone())))
}

fn body_text_inner(args: &[Value]) -> Result<Value, String> {
    let r = match args {
        [h] => as_resp(h)?,
        _ => return Err("Core.HttpClient.__bodyText expects (handle)".into()),
    };
    match String::from_utf8(r.resp.body.clone()) {
        Ok(s) => Ok(Value::Str(s.into())),
        Err(_) => Err(
            "<<ProtocolError>>Core.HttpClient: response body is not UTF-8 — read bodyBytes()"
                .into(),
        ),
    }
}

macro_rules! hc_native {
    ($name:ident, $inner:ident) => {
        fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
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
    let handle = || Ty::Named("HcHandle".into(), vec![]);
    let res = |t: Ty| Ty::Named("HcResult".into(), vec![t]);
    let entry =
        |name: &'static str,
         params: Vec<Ty>,
         ret: Ty,
         eval: fn(&[Value], &mut String) -> Result<Value, String>| NativeFn {
            module: "Core.HttpClientSys",
            name,
            params,
            ret,
            pure: false,
            eval: NativeEval::Pure(eval),
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
#[cfg(test)]
#[path = "http_client_tests.rs"]
mod tests;
