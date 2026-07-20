//! Normalization, recomposition, and RFC 3986 §5.2 reference resolution for `Core.UriModule` —
//! the second half of the URI kernel (parse + per-component validation live in `kernel.rs`, which
//! re-exports the functions below so `kernel::…` call-sites keep their paths). Pure, deterministic.

use super::kernel::{is_unreserved, parse_ipv6, Parts};

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
