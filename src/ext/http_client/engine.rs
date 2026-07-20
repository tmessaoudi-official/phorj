//! `Core.HttpClientModule` request engine: TLS-capable exchange, SSRF IP-blocking, redirect following.

use super::protocol::{
    classify_io, default_port, parse_url, read_response, resolve_location, RawResponse, Url,
};
use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

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
        .map_err(|e| {
            format!("<<ConnectionFailedError>>Core.HttpClientModule: resolve `{addr}`: {e}")
        })?
        .next()
        .ok_or_else(|| {
            format!(
                "<<ConnectionFailedError>>Core.HttpClientModule: `{addr}` resolved to no address"
            )
        })?;
    if !allow_private && is_blocked_ip(sock_addrs.ip()) {
        // Name the REQUESTED host, not the resolved IP — echoing the specific private address it
        // resolved to would be a minor internal-DNS resolution oracle for an attacker-supplied URL.
        return Err(format!(
            "<<BlockedAddressError>>Core.HttpClientModule: refusing to connect to `{addr}` — it resolves to a \
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
                    "<<TlsError>>Core.HttpClientModule: invalid TLS name `{}`",
                    url.host
                )
            })?;
        let conn = rustls::ClientConnection::new(Arc::new(config), name)
            .map_err(|e| format!("<<TlsError>>Core.HttpClientModule: {e}"))?;
        let mut tls = rustls::StreamOwned::new(conn, stream);
        write_request(&mut tls, method, url, headers, body)?;
        read_response(&mut tls).map_err(|e| {
            // rustls surfaces handshake failures on first I/O — retag them as TLS.
            if e.contains("Connection") && e.contains("Alert") {
                e.replace("<<ConnectionFailedError>>", "<<TlsError>>")
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
pub(super) fn headers_for_hop(
    from: &Url,
    to: &Url,
    headers: &[(String, String)],
) -> Vec<(String, String)> {
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
                "<<TooManyRedirectsError>>Core.HttpClientModule: exceeded {max_redirects} redirects"
            ));
        }
        let location = resp
            .headers
            .iter()
            .find(|(n, _)| n == "location")
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                format!(
                    "<<ProtocolError>>Core.HttpClientModule: {} redirect without a Location header",
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
