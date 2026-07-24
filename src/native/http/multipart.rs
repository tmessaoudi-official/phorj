//! `multipart/form-data` splitting (RFC 7578 shape) — byte-level, std-only; the PHP twin
//! `__phorj_http_parse_multipart` mirrors these exact acceptance rules. `None` = malformed
//! (the eager path turns that into `Request.parse → null` → the bridge's 400; slice-3 lazy
//! surfaces [`super::FAULT_MALFORMED_MULTIPART`]).
//!
//! Acceptance rules (deliberately strict — over-rejection is safe, silent misparse is not):
//!   * the body must OPEN with `--<boundary>`;
//!   * each part: `\r\n`, headers up to `\r\n\r\n`, content up to the next `\r\n--<boundary>`;
//!   * after a delimiter, `--` closes the stream (trailing bytes ignored per RFC);
//!   * every part needs a `Content-Disposition` with a quoted `name="…"` (quoted-pair escapes
//!     are NOT interpreted — value runs to the next `"`; documented simplification);
//!   * more than [`super::MULTIPART_MAX_PARTS`] parts = malformed (recorded cap).
//!
//! Each part becomes a hand-built [`Value::Instance`] of the INJECTED prelude class
//! `MultipartPart` — the field-name SET here must exactly equal that class's declared fields
//! (`content`, `contentType`, `fileName`, `name` — see `cli::http_request_prelude`); the checker
//! cannot catch a mismatch (it would surface as a runtime field-miss; the bag conformance golden
//! and `examples/web/rich_request.phg` are the gate). The Regex-carrier precedent (S1b).
use crate::value::{Instance, Value};
use std::rc::Rc;

/// Find `needle` in `haystack[from..]`, returning the absolute offset.
fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// One parsed part, pre-carrier.
struct Part {
    name: String,
    file_name: String,
    content_type: String,
    content: Vec<u8>,
}

fn part_value(p: Part) -> Value {
    let inst = Instance::new(
        "MultipartPart".into(),
        crate::value::ClassLayout::from_sorted_names(&[
            "content",
            "contentType",
            "fileName",
            "name",
        ]),
    );
    inst.set_field("content", Value::Bytes(Rc::new(p.content)));
    inst.set_field("contentType", Value::Str(p.content_type.into()));
    inst.set_field("fileName", Value::Str(p.file_name.into()));
    inst.set_field("name", Value::Str(p.name.into()));
    Value::Instance(Rc::new(inst))
}

/// Pull the quoted value of `key="…"` out of a Content-Disposition line (no escape handling).
/// The key must sit at a parameter boundary (start / space / `;` / tab) — otherwise looking up
/// `name` would match the tail of `filename` (the PHP twin uses the same `(?:^|[;\s])` guard).
fn quoted_param(header: &str, key: &str) -> Option<String> {
    let marker = format!("{key}=\"");
    let mut from = 0;
    while let Some(rel) = header[from..].find(&marker) {
        let abs = from + rel;
        if abs == 0 || matches!(header.as_bytes()[abs - 1], b' ' | b';' | b'\t') {
            let rest = &header[abs + marker.len()..];
            let end = rest.find('"')?;
            return Some(rest[..end].to_string());
        }
        from = abs + 1;
    }
    None
}

/// Parse the head of one part (everything before its `\r\n\r\n`). Header names are matched
/// case-insensitively (HTTP rule); a part without a Content-Disposition `name` is malformed.
fn parse_part_head(head: &str) -> Option<(String, String, String)> {
    let mut name = None;
    let mut file_name = String::new();
    let mut content_type = String::new();
    for line in head.split("\r\n") {
        let Some((h, v)) = line.split_once(':') else {
            continue;
        };
        let key = h.trim().to_ascii_lowercase();
        let v = v.trim();
        if key == "content-disposition" {
            name = quoted_param(v, "name");
            if let Some(f) = quoted_param(v, "filename") {
                file_name = f;
            }
        } else if key == "content-type" {
            content_type = v.to_string();
        }
    }
    name.map(|n| (n, file_name, content_type))
}

/// Split `body` on `boundary`. `None` = malformed per the module rules.
pub(crate) fn parse_multipart(body: &[u8], boundary: &str) -> Option<Vec<Value>> {
    if boundary.is_empty() {
        return None;
    }
    let open = format!("--{boundary}");
    let delim = format!("\r\n--{boundary}");
    if !body.starts_with(open.as_bytes()) {
        return None;
    }
    let mut parts: Vec<Value> = Vec::new();
    // Cursor sits just past a delimiter (the opening one, then each `\r\n--boundary`).
    let mut cur = open.len();
    loop {
        // Stream close: `--` right after a delimiter (trailing bytes ignored, RFC 7578 §4.1).
        if body[cur..].starts_with(b"--") {
            return Some(parts);
        }
        if !body[cur..].starts_with(b"\r\n") {
            return None; // neither a close nor a part start
        }
        let head_start = cur + 2;
        let head_end = find_from(body, b"\r\n\r\n", head_start)?;
        let content_start = head_end + 4;
        let content_end = find_from(body, delim.as_bytes(), content_start)?;
        let head = std::str::from_utf8(&body[head_start..head_end]).ok()?;
        let (name, file_name, content_type) = parse_part_head(head)?;
        parts.push(part_value(Part {
            name,
            file_name,
            content_type,
            content: body[content_start..content_end].to_vec(),
        }));
        if parts.len() > super::MULTIPART_MAX_PARTS {
            return None; // recorded cap: over-cap is DELIBERATELY classified malformed
        }
        cur = content_end + delim.len();
    }
}
