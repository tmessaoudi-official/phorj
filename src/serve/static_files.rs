//! DEC-282 — the dev server's docroot static-file layer (`phg serve <DIR>` mode).
//!
//! An exact-path match under `public/` (a real, non-`.phg` file) is served with a standard MIME
//! table + `ETag`/`Last-Modified` conditional caching; EVERYTHING else falls through to the
//! program's `index.phg` entry. The guard list is the point (the docroot exists so source can
//! never leak): the resolved path is canonicalized and prefix-checked (no `../`, no symlink
//! escape), `*.phg` source bytes are NEVER served, dot-files are invisible, and there is no
//! directory listing/auto-index. GET/HEAD only — any other method goes to the program.
//!
//! Deliberately OUT (later, on demand): Range requests, compression, Cache-Control config,
//! custom error pages, TLS — the correctness + security core only.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// MIME by extension — the ~20 everyday types; anything else is `application/octet-stream`.
fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// RFC 1123 HTTP-date (`Tue, 15 Nov 1994 08:12:31 GMT`) from a `SystemTime`, no external crates —
/// civil-from-days per Howard Hinnant's algorithm.
fn http_date(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let weekday = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][(days.rem_euclid(7)) as usize];
    // Civil date from day count (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m - 1) as usize];
    format!("{weekday}, {d:02} {month} {y} {hh:02}:{mm:02}:{ss:02} GMT")
}

/// The header value of `name` in a raw request head (case-insensitive name match), if present.
fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// Try to serve `raw` (one framed HTTP request) statically from `docroot`. `Some(response bytes)`
/// when the request maps to a real static file (or a guard 404/304); `None` ⇒ fall through to the
/// program entry. See the module docs for the guard list.
pub fn try_static(docroot: &Path, raw: &[u8]) -> Option<Vec<u8>> {
    let head_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .unwrap_or(raw.len());
    let head = std::str::from_utf8(&raw[..head_end]).ok()?;
    let mut parts = head.lines().next()?.split_whitespace();
    let method = parts.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }
    let target = parts.next()?;
    // Strip the query string; decode nothing (an encoded traversal never canonicalizes into the
    // docroot below, and asset paths are plain ASCII in practice).
    let path = target.split('?').next().unwrap_or(target);
    if !path.starts_with('/') || path.contains('\0') {
        return None;
    }
    let rel = path.trim_start_matches('/');
    if rel.is_empty() {
        return None; // `/` is the program's front page
    }
    // Guard: dot-files/dot-dirs are invisible; `.phg` source is NEVER served (the entry executes,
    // everything else 404s so the response never leaks whether source exists).
    let candidate = docroot.join(rel);
    if Path::new(rel)
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return None;
    }
    if candidate
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("phg"))
    {
        return Some(plain_response(404, "not found"));
    }
    // Guard: canonicalize + prefix-check — `..` segments and symlink escapes both fail here.
    let Ok(real) = candidate.canonicalize() else {
        return None; // no such file → the program's router decides
    };
    let Ok(root) = docroot.canonicalize() else {
        return None;
    };
    if !real.starts_with(&root) || !real.is_file() {
        return Some(plain_response(404, "not found"));
    }
    let meta = std::fs::metadata(&real).ok()?;
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let last_modified = http_date(modified);
    let etag = format!(
        "\"{:x}-{:x}\"",
        meta.len(),
        modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    // Conditional caching: ETag first (RFC 7232 precedence), else the exact Last-Modified echo.
    let unchanged = header_value(head, "If-None-Match").is_some_and(|v| v == etag)
        || header_value(head, "If-Modified-Since").is_some_and(|v| v == last_modified);
    if unchanged {
        return Some(
            format!(
                "HTTP/1.1 304 Not Modified\r\nETag: {etag}\r\nLast-Modified: {last_modified}\r\nContent-Length: 0\r\n\r\n"
            )
            .into_bytes(),
        );
    }
    let body = std::fs::read(&real).ok()?;
    let mut resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nETag: {etag}\r\nLast-Modified: {last_modified}\r\n\r\n",
        mime_of(&real),
        body.len()
    )
    .into_bytes();
    if method != "HEAD" {
        resp.extend_from_slice(&body);
    }
    Some(resp)
}

fn plain_response(status: u16, reason: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{reason}",
        match status {
            404 => "Not Found",
            _ => "Error",
        },
        reason.len()
    )
    .into_bytes()
}

/// Resolve `phg serve <DIR>`'s conventional layout: docroot = `DIR/public`, entry =
/// `DIR/public/index.phg`. A clear startup error when the convention isn't met.
pub fn resolve_site_dir(dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let docroot = dir.join("public");
    let entry = docroot.join("index.phg");
    if !docroot.is_dir() {
        return Err(format!(
            "phg serve: `{}` has no `public/` directory — a site directory serves public/ as its \
             docroot (code lives outside it; see phg serve --help)",
            dir.display()
        ));
    }
    if !entry.is_file() {
        return Err(format!(
            "phg serve: `{}` has no `public/index.phg` — the one web entry (an #[Entry] \
             function taking a Request) must live there",
            dir.display()
        ));
    }
    // Loud hygiene warning: any OTHER .phg inside the docroot is a misplacement (it would never be
    // served — source bytes are guarded — but it doesn't belong in the public surface either).
    if let Ok(entries) = crate::loader::discover_phg(&docroot) {
        for f in entries {
            if f != entry {
                eprintln!(
                    "warning: `{}` is inside the docroot — .phg source is never served, but code \
                     belongs OUTSIDE public/ (move it to src/) [W-PHG-IN-DOCROOT]",
                    f.display()
                );
            }
        }
    }
    Ok((docroot, entry))
}
