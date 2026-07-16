//! The RFC 3986 kernel for `Core.Uri` (DEC-240) — parse, per-component validation,
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

fn is_unreserved(b: u8) -> bool {
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

// ── normalization (RFC 3986 §6.2.2, twin-faithful) ──────────────────────────────────────────────

/// Percent-normalize: decode ASCII-unreserved escapes, uppercase the hex of what stays encoded.
pub(super) fn pct_normalize(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = (b[i + 1] as char).to_digit(16).unwrap_or(16);
            let lo = (b[i + 2] as char).to_digit(16).unwrap_or(16);
            if hi < 16 && lo < 16 {
                let v = (hi * 16 + lo) as u8;
                if is_unreserved(v) {
                    out.push(v as char);
                } else {
                    out.push('%');
                    out.push((b[i + 1] as char).to_ascii_uppercase());
                    out.push((b[i + 2] as char).to_ascii_uppercase());
                }
                i += 3;
                continue;
            }
            out.push('%');
            i += 1;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// Lowercase the PLAIN letters of `s`, leaving `%XX` escape hex untouched (it was already
/// uppercased by `pct_normalize`). For scheme/host normalization.
fn lowercase_plain(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push('%');
            out.push(b[i + 1] as char);
            out.push(b[i + 2] as char);
            i += 3;
        } else {
            out.push((b[i] as char).to_ascii_lowercase());
            i += 1;
        }
    }
    out
}

/// Normalized host for the GETTER surface: reg-name pct-normalized + lowercased; an IPv6 literal
/// lowercased AS WRITTEN (the twin does not re-compress); IPvFuture lowercased.
pub(super) fn norm_host_getter(h: &str) -> String {
    if h.starts_with('[') {
        return h.to_ascii_lowercase();
    }
    lowercase_plain(&pct_normalize(h))
}

/// Normalized host for RECOMPOSITION (`toString`): like the getter, except an IPv6 literal is
/// EXPANDED to eight 4-digit hextets (twin quirk — an IPv4-mixed tail becomes pure hex).
pub(super) fn norm_host_tostring(h: &str) -> String {
    if let Some(inner) = h.strip_prefix('[') {
        if let Some(inner) = inner.strip_suffix(']') {
            if let Some(groups) = parse_ipv6(inner) {
                let segs: Vec<String> = groups.iter().map(|g| format!("{g:04x}")).collect();
                return format!("[{}]", segs.join(":"));
            }
            // IPvFuture: lowercased as written.
            return h.to_ascii_lowercase();
        }
    }
    norm_host_getter(h)
}

/// Normalized port: leading zeros stripped (`0080`→`80`), a lone `0` and the empty port kept.
pub(super) fn norm_port(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    let t = p.trim_start_matches('0');
    if t.is_empty() {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// Remove dot segments — twin-faithful (the whole PATH corpus in the probe record reproduces):
/// - a non-trailing `.` segment is removed; a TRAILING `.` (or a trailing matched `..`) becomes a
///   trailing empty segment (the `/` it leaves behind — `a/.` → `a/`, `/a/..` → `/`);
/// - a matched `..` pops the previous segment (empty segments pop too, and survive as segments:
///   `a//b/../c` → `a//c`);
/// - an UNMATCHED `..` is kept verbatim on a scheme-less relative path (`keep_unmatched`:
///   `../g/./h` → `../g/h`, `a/../..` → `..`), and dropped otherwise (`mailto:../b` → `b`,
///   `/a/../../b` → `/b`);
/// - the exact path `./` is preserved verbatim (uriparser quirk — `s:./` keeps `./` while `./a`
///   drops the dot; probed).
pub(super) fn remove_dot_segments(path: &str, keep_unmatched: bool) -> String {
    if path == "./" {
        return "./".to_string();
    }
    let rooted = path.starts_with('/');
    let body = if rooted { &path[1..] } else { path };
    if body.is_empty() {
        return path.to_string();
    }
    let segs: Vec<&str> = body.split('/').collect();
    let mut out: Vec<&str> = Vec::new();
    let mut kept_dotdots = 0usize; // leading `..` kept verbatim (relative refs only)
    let last_idx = segs.len() - 1;
    for (i, seg) in segs.iter().enumerate() {
        let trailing = i == last_idx;
        match *seg {
            "." => {
                if trailing {
                    out.push(""); // the slash `.` leaves behind
                }
            }
            ".." => {
                if out.len() > kept_dotdots {
                    out.pop();
                    if trailing {
                        out.push(""); // the slash a matched trailing `..` leaves behind
                    }
                } else if !rooted && keep_unmatched {
                    out.push("..");
                    kept_dotdots += 1;
                }
                // else: unmatched on a rooted or scheme-ful path — dropped (RFC behavior)
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if rooted {
        format!("/{joined}")
    } else {
        joined
    }
}

/// The fully-normalized components (the getter surface + `toString` recomposition input).
pub(super) fn normalize(p: &Parts) -> Parts {
    Parts {
        scheme: p.scheme.as_deref().map(str::to_ascii_lowercase),
        userinfo: p.userinfo.as_deref().map(pct_normalize),
        host: p.host.as_deref().map(norm_host_getter),
        port: p.port.as_deref().map(norm_port),
        path: remove_dot_segments(&pct_normalize(&p.path), p.scheme.is_none()),
        query: p.query.as_deref().map(pct_normalize),
        fragment: p.fragment.as_deref().map(pct_normalize),
    }
}

// ── recomposition (RFC 3986 §5.3) ───────────────────────────────────────────────────────────────

/// Recompose raw parts verbatim (`toRawString`), or pass normalized parts for the getter forms.
pub(super) fn recompose(p: &Parts) -> String {
    let mut out = String::new();
    if let Some(s) = &p.scheme {
        out.push_str(s);
        out.push(':');
    }
    if let Some(h) = &p.host {
        out.push_str("//");
        if let Some(u) = &p.userinfo {
            out.push_str(u);
            out.push('@');
        }
        out.push_str(h);
        if let Some(port) = &p.port {
            out.push(':');
            out.push_str(port);
        }
    }
    out.push_str(&p.path);
    if let Some(q) = &p.query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = &p.fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

/// The normalized string form (`toString`): normalization + the IPv6-expansion host quirk.
pub(super) fn to_string_normalized(p: &Parts) -> String {
    let mut n = normalize(p);
    n.host = p.host.as_deref().map(norm_host_tostring);
    recompose(&n)
}

// ── resolution (RFC 3986 §5.2) ──────────────────────────────────────────────────────────────────

/// Resolve `r` against absolute `base` (strict, no backward-compat scheme trick). The caller has
/// verified `base.scheme.is_some()`. Output components are RAW (normalization happens at the
/// getter/toString layer, matching the twin).
pub(super) fn resolve(base: &Parts, r: &Parts) -> Parts {
    if r.scheme.is_some() {
        return Parts {
            path: remove_dot_segments(&r.path, false),
            ..r.clone()
        };
    }
    if r.host.is_some() {
        return Parts {
            scheme: base.scheme.clone(),
            path: remove_dot_segments(&r.path, false),
            ..r.clone()
        };
    }
    let (path, query) = if r.path.is_empty() {
        (
            base.path.clone(),
            r.query.clone().or_else(|| base.query.clone()),
        )
    } else if r.path.starts_with('/') {
        (remove_dot_segments(&r.path, false), r.query.clone())
    } else {
        (
            remove_dot_segments(&merge_paths(base, &r.path), false),
            r.query.clone(),
        )
    };
    Parts {
        scheme: base.scheme.clone(),
        userinfo: base.userinfo.clone(),
        host: base.host.clone(),
        port: base.port.clone(),
        path,
        query,
        fragment: r.fragment.clone(),
    }
}

/// RFC 3986 §5.2.3 — merge a relative-path reference with the base path.
fn merge_paths(base: &Parts, rpath: &str) -> String {
    if base.host.is_some() && base.path.is_empty() {
        return format!("/{rpath}");
    }
    match base.path.rfind('/') {
        Some(i) => format!("{}{}", &base.path[..=i], rpath),
        None => rpath.to_string(),
    }
}
