//! The RFC 3986 kernel for `Core.UriModule` (DEC-240) — parse, per-component validation,
//! normalization, recomposition, and reference resolution. Pure, deterministic, std-only.
//!
//! Every behavior here is pinned to PHP 8.5's `Uri\Rfc3986\Uri` (the transpile twin — built on
//! uriparser), probed live in `docs/research/2026-07-16-uri-twin-probes.md`; the quirks below are
//! FAITHFUL choices, not accidents:
//! - percent-normalization decodes **ASCII unreserved only** (`%41`→`A`, `%C3` stays) and
//!   uppercases the hex of what stays encoded (`%2f`→`%2F`);
//! - dot-segment removal preserves an UNMATCHED leading `..` on a scheme-less relative path
//!   (`../g/./h`→`../g/h`) but drops it when a scheme is present (`mailto:../b`→`b`) or the path
//!   is rooted (`/a/../../b`→`/b`);
//! - `getHost` lowercases an IPv6 literal **as written** (no re-compression), while `toString`
//!   EXPANDS it to eight 4-digit hextets (an IPv4-mixed tail becomes pure hex);
//! - ports have no 65535 cap (u64 digits; overflow is "out of range"), keep an empty `:`
//!   round-trippable, and normalize leading zeros.

/// The raw (as-written) components of a URI reference. `None` = component absent; `Some("")` =
/// present but empty (`http://h:` has `port: Some("")` — the distinction round-trips).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Parts {
    pub scheme: Option<String>,
    pub userinfo: Option<String>,
    pub host: Option<String>,
    pub port: Option<String>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

/// A parse/validation failure, mapped 1:1 onto the PHP twin's messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UriErr {
    /// "The specified URI is malformed"
    Malformed,
    /// "The port is out of range"
    PortRange,
}

// ── character classes (RFC 3986 §2) ─────────────────────────────────────────────────────────────

pub(super) fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn is_sub_delim(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}

fn is_pchar_plain(b: u8) -> bool {
    is_unreserved(b) || is_sub_delim(b) || matches!(b, b':' | b'@')
}

/// Validate `s` as a sequence of `plain`-allowed bytes and well-formed `%XX` escapes.
fn valid_seq(s: &str, plain: impl Fn(u8) -> bool) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
        } else if plain(b[i]) {
            i += 1;
        } else {
            return false;
        }
    }
    true
}

pub(super) fn valid_scheme(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_alphabetic()
        && b.iter()
            .all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
}

pub(super) fn valid_userinfo(s: &str) -> bool {
    valid_seq(s, |b| is_unreserved(b) || is_sub_delim(b) || b == b':')
}

pub(super) fn valid_port(s: &str) -> Result<(), UriErr> {
    if !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(UriErr::Malformed);
    }
    // No 65535 cap; the twin-probed limit is exactly i64::MAX (9223372036854775807 parses,
    // 9223372036854775808 is "The port is out of range").
    if !s.is_empty() && s.parse::<i64>().is_err() {
        return Err(UriErr::PortRange);
    }
    Ok(())
}

pub(super) fn valid_host(s: &str) -> bool {
    if let Some(inner) = s.strip_prefix('[') {
        let Some(inner) = inner.strip_suffix(']') else {
            return false;
        };
        return valid_ip_literal_inner(inner);
    }
    // reg-name (an IPv4 address is a valid reg-name; the twin does no IPv4 normalization).
    valid_seq(s, |b| is_unreserved(b) || is_sub_delim(b))
}

/// The bracket-inner of an IP-literal: IPv6address, or IPvFuture (`v` HEXDIG+ `.` chars).
fn valid_ip_literal_inner(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix('v').or_else(|| s.strip_prefix('V')) {
        // IPvFuture = "v" 1*HEXDIG "." 1*( unreserved / sub-delims / ":" )
        let Some(dot) = rest.find('.') else {
            return false;
        };
        let (hex, tail) = (&rest[..dot], &rest[dot + 1..]);
        return !hex.is_empty()
            && hex.bytes().all(|b| b.is_ascii_hexdigit())
            && !tail.is_empty()
            && tail
                .bytes()
                .all(|b| is_unreserved(b) || is_sub_delim(b) || b == b':');
    }
    parse_ipv6(s).is_some()
}

/// Parse an IPv6 address (with optional embedded IPv4 tail) into its eight u16 hextets.
pub(super) fn parse_ipv6(s: &str) -> Option<[u16; 8]> {
    let (head, tail) = match s.find("::") {
        Some(i) => (&s[..i], Some(&s[i + 2..])),
        None => (s, None),
    };
    // A second `::` is malformed.
    if let Some(t) = tail {
        if t.contains("::") {
            return None;
        }
    }
    let parse_groups = |part: &str| -> Option<Vec<u16>> {
        if part.is_empty() {
            return Some(Vec::new());
        }
        let mut out = Vec::new();
        let segs: Vec<&str> = part.split(':').collect();
        for (i, seg) in segs.iter().enumerate() {
            if seg.contains('.') {
                // Embedded IPv4 — only legal as the LAST group pair.
                if i != segs.len() - 1 {
                    return None;
                }
                let v4 = parse_ipv4_strict(seg)?;
                out.push(((v4 >> 16) & 0xffff) as u16);
                out.push((v4 & 0xffff) as u16);
            } else {
                if seg.is_empty() || seg.len() > 4 || !seg.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return None;
                }
                out.push(u16::from_str_radix(seg, 16).ok()?);
            }
        }
        Some(out)
    };
    let h = parse_groups(head)?;
    match tail {
        Some(t) => {
            let t = parse_groups(t)?;
            if h.len() + t.len() > 7 {
                return None; // `::` must compress at least one group
            }
            let mut out = [0u16; 8];
            out[..h.len()].copy_from_slice(&h);
            out[8 - t.len()..].copy_from_slice(&t);
            Some(out)
        }
        None => {
            if h.len() != 8 {
                return None;
            }
            let mut out = [0u16; 8];
            out.copy_from_slice(&h);
            Some(out)
        }
    }
}

/// Strict dotted-quad IPv4 (for the embedded-in-IPv6 tail): four 0-255 decimal octets, no
/// leading-zero padding beyond a lone `0`.
fn parse_ipv4_strict(s: &str) -> Option<u32> {
    let mut out: u32 = 0;
    let octets: Vec<&str> = s.split('.').collect();
    if octets.len() != 4 {
        return None;
    }
    for o in octets {
        if o.is_empty() || o.len() > 3 || !o.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if o.len() > 1 && o.starts_with('0') {
            return None;
        }
        let v: u32 = o.parse().ok()?;
        if v > 255 {
            return None;
        }
        out = (out << 8) | v;
    }
    Some(out)
}

pub(super) fn valid_path(s: &str, has_authority: bool, has_scheme: bool) -> bool {
    if !valid_seq(s, |b| is_pchar_plain(b) || b == b'/') {
        return false;
    }
    if has_authority && !s.is_empty() && !s.starts_with('/') {
        return false; // with an authority the path must be empty or absolute
    }
    if !has_authority && !has_scheme {
        // relative reference: the FIRST segment may not contain `:` (it would read as a scheme)
        let first = s.split('/').next().unwrap_or("");
        if first.contains(':') {
            return false;
        }
    }
    true
}

pub(super) fn valid_query_or_fragment(s: &str) -> bool {
    valid_seq(s, |b| is_pchar_plain(b) || matches!(b, b'/' | b'?'))
}

// ── parsing (RFC 3986 §3, strict) ───────────────────────────────────────────────────────────────

pub(super) fn parse(input: &str) -> Result<Parts, UriErr> {
    if !input.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        // Raw spaces, controls, and non-ASCII are malformed everywhere (must be pct-encoded).
        return Err(UriErr::Malformed);
    }
    let (before_frag, fragment) = match input.split_once('#') {
        Some((a, f)) => (a, Some(f.to_string())),
        None => (input, None),
    };
    let (before_query, query) = match before_frag.split_once('?') {
        Some((a, q)) => (a, Some(q.to_string())),
        None => (before_frag, None),
    };
    // Scheme: a `:` before any `/` ends a mandatory-valid scheme.
    let (scheme, rest) = match before_query.find([':', '/']) {
        Some(i) if before_query.as_bytes()[i] == b':' => {
            let cand = &before_query[..i];
            if !valid_scheme(cand) {
                return Err(UriErr::Malformed);
            }
            (Some(cand.to_string()), &before_query[i + 1..])
        }
        _ => (None, before_query),
    };
    // Authority: `//` then up to the next `/` (or end).
    let (userinfo, host, port, path) = if let Some(auth_rest) = rest.strip_prefix("//") {
        let (auth, path) = match auth_rest.find('/') {
            Some(i) => (&auth_rest[..i], &auth_rest[i..]),
            None => (auth_rest, ""),
        };
        let (userinfo, hostport) = match auth.split_once('@') {
            Some((u, h)) => (Some(u.to_string()), h),
            None => (None, auth),
        };
        let (host, port) = if hostport.starts_with('[') {
            match hostport.find(']') {
                Some(i) => {
                    let host = &hostport[..=i];
                    match &hostport[i + 1..] {
                        "" => (host.to_string(), None),
                        p => match p.strip_prefix(':') {
                            Some(digits) => (host.to_string(), Some(digits.to_string())),
                            None => return Err(UriErr::Malformed),
                        },
                    }
                }
                None => return Err(UriErr::Malformed),
            }
        } else {
            match hostport.rsplit_once(':') {
                Some((h, p)) => (h.to_string(), Some(p.to_string())),
                None => (hostport.to_string(), None),
            }
        };
        (userinfo, Some(host), port, path.to_string())
    } else {
        (None, None, None, rest.to_string())
    };
    let parts = Parts {
        scheme,
        userinfo,
        host,
        port,
        path,
        query,
        fragment,
    };
    validate(&parts)?;
    Ok(parts)
}

/// Validate every present component of `parts` (used by `parse` and by wither recomposition).
pub(super) fn validate(p: &Parts) -> Result<(), UriErr> {
    if let Some(s) = &p.scheme {
        if !valid_scheme(s) {
            return Err(UriErr::Malformed);
        }
    }
    if let Some(u) = &p.userinfo {
        if !valid_userinfo(u) {
            return Err(UriErr::Malformed);
        }
    }
    if let Some(h) = &p.host {
        if !valid_host(h) {
            return Err(UriErr::Malformed);
        }
    }
    if let Some(port) = &p.port {
        valid_port(port)?;
    }
    if !valid_path(&p.path, p.host.is_some(), p.scheme.is_some()) {
        return Err(UriErr::Malformed);
    }
    if let Some(q) = &p.query {
        if !valid_query_or_fragment(q) {
            return Err(UriErr::Malformed);
        }
    }
    if let Some(f) = &p.fragment {
        if !valid_query_or_fragment(f) {
            return Err(UriErr::Malformed);
        }
    }
    Ok(())
}

// ── re-exports (normalization/recomposition/resolution live in `kernel_norm.rs`; file-size cap,
//    Invariant 13) — re-exported so `kernel::…` call-sites (natives, registry, kernel_tests) keep
//    their paths. ────────────────────────────────────────────────────────────────────────────────
pub(super) use super::kernel_norm::{
    norm_host_getter, norm_port, normalize, pct_normalize, recompose, resolve, to_string_normalized,
};
