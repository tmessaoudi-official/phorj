//! `Core.HttpClientModule` wire protocol: absolute-URL parsing and HTTP/1.1 response reading.

use std::io::Read;

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
/// `<<InvalidUrlError>>`. IPv6 literals in brackets are supported.
pub(crate) fn parse_url(url: &str) -> Result<Url, String> {
    let bad = |m: &str| format!("<<InvalidUrlError>>Core.HttpClientModule: {m}: `{url}`");
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

pub(super) fn default_port(https: bool) -> u16 {
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
/// read-to-close body). TimeoutError/size violations are clean errors.
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
            return Err(
                "<<ProtocolError>>Core.HttpClientModule: response headers exceed 1 MB".into(),
            );
        }
        let n = r.read(&mut chunk).map_err(io)?;
        if n == 0 {
            return Err(
                "<<ConnectionFailedError>>Core.HttpClientModule: connection closed before headers completed"
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
            format!("<<ProtocolError>>Core.HttpClientModule: malformed status line `{status_line}`")
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
                    "<<ProtocolError>>Core.HttpClientModule: connection closed mid-chunked-body"
                        .into(),
                );
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body = decode_chunked(&body)?;
    } else if let Some(len) = header("content-length") {
        let len: usize = len.parse().map_err(|_| {
            "<<ProtocolError>>Core.HttpClientModule: malformed Content-Length".to_string()
        })?;
        if len > MAX_RESPONSE {
            return Err(size_err());
        }
        while body.len() < len {
            let n = r.read(&mut chunk).map_err(io)?;
            if n == 0 {
                return Err(
                    "<<ProtocolError>>Core.HttpClientModule: connection closed mid-body".into(),
                );
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
    "<<TooLargeError>>Core.HttpClientModule: response exceeds the 64 MB cap".into()
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
    let bad = || "<<ProtocolError>>Core.HttpClientModule: malformed chunked body".to_string();
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

pub(super) fn classify_io(e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    let kind = match e.kind() {
        K::TimedOut | K::WouldBlock => "TimeoutError",
        _ => "ConnectionFailedError",
    };
    format!("<<{kind}>>Core.HttpClientModule: {e}")
}
