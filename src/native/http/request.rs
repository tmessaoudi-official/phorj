//! `Core.Native.Http.parseRequest` (DEC-338) — the whole wire→`Request` parse, nativized to flip
//! the `queryparse` 0.10× perf loss (the phorj `Request.parse` was interpreter-bound, one full
//! bag-graph build per parse). This Rust twin and the PHP twin `__phorj_http_parse_request`
//! (`src/transpile/runtime_php_http.rs`) build the IDENTICAL object graph — change BOTH or neither.
//!
//! It is a line-for-line port of the former phorj `Request.parse` body + its private helpers
//! (`headerPairs`/`cookiePairs`/`multipartFields`/`boundaryOf`) in `cli::http_request_prelude`, so
//! every phorj op is mirrored by the exact Rust twin of that op's native: `Bytes.find`→windows,
//! `Bytes.toString ?? ""`→`from_utf8().unwrap_or("")`, `Bytes.slice`→byte range, `String.indexOf`
//! →`str::find` (byte offset), `String.substring`→byte slicing at the ASCII delimiter, `String.trim`
//! →`str::trim` (Unicode White_Space, matching `__phorj_text_trim`), `String.lowerCase`
//! →`to_ascii_lowercase`. The bags are hand-built [`Value::Instance`]s of the (kept) prelude classes;
//! field access is runtime-slot-resolved (`value::types` doc) so `from_sorted_names` layouts are
//! method-dispatchable identically to phorj-constructed ones (the multipart-carrier precedent).
//!
//! `null` (→ `Value::Null`) = malformed/oversize (the eager D8a contract — NEVER a fault, so the
//! serve bridge 400s it); a spill-store IO failure is the sole genuine `Err` (an ambient error).
use super::query::{decode_path, parse_query_pairs};
use super::{pairs_to_map, parse_multipart, stash_decision};
use crate::value::{ClassLayout, Instance, Value};
use std::rc::Rc;

// Per-CLASS layout cache. A `ClassLayout` is a sorted `Vec<String>` plus a name→slot map, and it
// depends only on the class's field set — yet `inst` rebuilt one for EVERY instance, so a single
// `Request.parse` allocated a fresh string vector, sorted it, and built a fresh hash map once per bag
// in the graph. Measured on `queryparse` (callgrind): malloc/free was ~38% of all instructions
// retired, with `HashMap::insert` and `Rc<ClassLayout>::drop_slow` right behind it. Caching it took
// the bench from 1839 ms to 1177 ms (-36%).
//
// A `Vec` with a linear scan, NOT a `HashMap`: there are under a dozen classes here, and the first
// version of this cache used a `std::collections::HashMap` whose SipHash of the class name promptly
// showed up as 3% of the profile — more than the lookup it was replacing. A short `memcmp` scan is
// cheaper than hashing at this size.
//
// Thread-local: `Value`/`Rc` are single-threaded by construction here.
thread_local! {
    static LAYOUTS: std::cell::RefCell<Vec<(&'static str, Rc<ClassLayout>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Hand-build a `Value::Instance` of a (kept) prelude class from `(field, value)` pairs.
///
/// Field order at the CALL SITE is irrelevant — the values are placed into their layout slots here, so
/// the instance is born fully populated. That is also why this does not use `set_field`: that takes a
/// fresh `RefCell` borrow per field, which is a dozen borrows per bag for an object nobody else can
/// see yet.
fn inst(class: &'static str, fields: Vec<(&str, Value)>) -> Value {
    let layout = LAYOUTS.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some((_, l)) = cache.iter().find(|(k, _)| *k == class) {
            return Rc::clone(l);
        }
        let names: Vec<&str> = fields.iter().map(|(n, _)| *n).collect();
        let l = ClassLayout::from_sorted_names(&names);
        cache.push((class, Rc::clone(&l)));
        l
    });
    let mut slots: Vec<Option<Value>> = vec![None; layout.len()];
    for (n, v) in fields {
        // A name with no slot means the cached layout and this call disagree on the field set — only
        // possible if a class is built two different ways, which these fixed prelude classes never
        // are. Fail loudly in debug rather than silently dropping the value.
        match layout.slot(n) {
            Some(i) => slots[i] = Some(v),
            None => debug_assert!(false, "class `{class}` has no slot for field `{n}`"),
        }
    }
    Value::Instance(Rc::new(Instance::from_slots(class.into(), layout, slots)))
}

/// `b""` when spilled (`handle >= 0`), else the raw bytes inline — the phorj `if (stash >= 0) { b"" }`.
fn inline_bytes(handle: i64, bytes: &[u8]) -> Value {
    Value::Bytes(Rc::new(if handle >= 0 {
        Vec::new()
    } else {
        bytes.to_vec()
    }))
}

/// Append `val` under `key` preserving FIRST-occurrence key order (D8b first-wins) — the phorj
/// `out[key] = List.concat(Map.get(out, key) ?? [], [val])` accumulation.
fn push_pair(out: &mut Vec<(String, Vec<String>)>, key: String, val: String) {
    match out.iter_mut().find(|(k, _)| *k == key) {
        Some((_, vs)) => vs.push(val),
        None => out.push((key, vec![val])),
    }
}

/// Read a `Value::Str` field of a hand-built `MultipartPart` (always present — parse_multipart set it).
fn part_str(part: &Value, field: &str) -> String {
    match part {
        Value::Instance(i) => match i.get_field(field) {
            Some(Value::Str(s)) => s.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Read the `content` `Value::Bytes` field of a hand-built `MultipartPart`.
fn part_content(part: &Value) -> Vec<u8> {
    match part {
        Value::Instance(i) => match i.get_field("content") {
            Some(Value::Bytes(b)) => b.as_ref().clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// `Request.headerPairs` — lowercased-key bag over the header lines (first-wins order, values trimmed).
fn header_pairs(lines: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            push_pair(
                &mut out,
                k.trim().to_ascii_lowercase(),
                v.trim().to_string(),
            );
        }
    }
    out
}

/// `Request.cookiePairs` — every `cookie` header, pieces split on `;`, FIRST `=` only, names
/// case-SENSITIVE, values verbatim (first-wins key order).
fn cookie_pairs(header_map: &[(String, Vec<String>)]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let lines = header_map.iter().find(|(k, _)| k == "cookie");
    if let Some((_, values)) = lines {
        for line in values {
            for piece in line.split(';') {
                let p = piece.trim();
                if p.is_empty() {
                    continue;
                }
                match p.find('=') {
                    Some(eq) => push_pair(&mut out, p[..eq].to_string(), p[eq + 1..].to_string()),
                    None => push_pair(&mut out, p.to_string(), String::new()),
                }
            }
        }
    }
    out
}

/// `Request.multipartFields` — non-file parts fold into the form bag (values verbatim, NOT urlencoded).
fn multipart_fields(parts: &[Value]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for p in parts {
        if part_str(p, "fileName").is_empty() {
            let content = part_content(p);
            let val = std::str::from_utf8(&content).unwrap_or("").to_string();
            push_pair(&mut out, part_str(p, "name"), val);
        }
    }
    out
}

/// `Request.boundaryOf` — the `boundary=` parameter of a multipart content-type (`""` when absent).
fn boundary_of(content_type: &str) -> String {
    let Some(b) = content_type.find("boundary=") else {
        return String::new();
    };
    let rest = &content_type[b + "boundary=".len()..];
    if let Some(inner) = rest.strip_prefix('"') {
        return match inner.find('"') {
            Some(q) => inner[..q].to_string(),
            None => String::new(),
        };
    }
    match rest.find(';') {
        Some(semi) => rest[..semi].trim().to_string(),
        None => rest.trim().to_string(),
    }
}

/// The `parseRequest(bytes) -> Request?` native (interpreter AND VM share this one body).
pub(super) fn native_parse_request(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Bytes(raw)] => parse_request(raw),
        _ => Err("Http.parseRequest expects (bytes)".into()),
    }
}

fn parse_request(raw: &[u8]) -> Result<Value, String> {
    // Head/body split on the first CRLFCRLF; absent → malformed.
    let Some(sep) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Ok(Value::Null);
    };
    let body_bytes = &raw[sep + 4..];
    let head = std::str::from_utf8(&raw[..sep]).unwrap_or("");
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let rl: Vec<&str> = request_line.split(' ').collect();
    if rl.len() < 2 {
        return Ok(Value::Null);
    }
    let method = rl[0];
    let target = rl[1];

    // Body stash: -2 oversize (malformed), -1 inline, else a spill handle.
    let stash = stash_decision(body_bytes)?;
    if stash == -2 {
        return Ok(Value::Null);
    }
    let body = inst(
        "RequestBody",
        vec![
            ("inline", inline_bytes(stash, body_bytes)),
            ("spillHandle", Value::Int(stash)),
            ("cachedJson", Value::Null),
            ("jsonParsed", Value::Bool(false)),
        ],
    );

    let header_lines: Vec<&str> = lines.collect();
    let header_map = header_pairs(&header_lines);
    let headers = inst(
        "HeaderBag",
        vec![("data", pairs_to_map(header_map.clone()))],
    );

    // Target → decoded path + query bag.
    let (path, query_string) = match target.find('?') {
        Some(q) => (&target[..q], &target[q + 1..]),
        None => (target, ""),
    };
    let query = inst(
        "ParamBag",
        vec![("data", pairs_to_map(parse_query_pairs(query_string)))],
    );
    let cookies = inst(
        "ParamBag",
        vec![("data", pairs_to_map(cookie_pairs(&header_map)))],
    );

    let content_type = header_map
        .iter()
        .find(|(k, _)| k == "content-type")
        .and_then(|(_, vs)| vs.first())
        .map_or("", |s| s.as_str());

    // Form + files by content type (urlencoded + multipart, D8c/D8d).
    let mut form_map: Vec<(String, Vec<String>)> = Vec::new();
    let mut file_items: Vec<Value> = Vec::new();
    let mut file_fields: Vec<Value> = Vec::new();
    if content_type.starts_with("application/x-www-form-urlencoded") {
        form_map = parse_query_pairs(std::str::from_utf8(body_bytes).unwrap_or(""));
    }
    if content_type.starts_with("multipart/form-data") && !body_bytes.is_empty() {
        let boundary = boundary_of(content_type);
        if boundary.is_empty() {
            return Ok(Value::Null);
        }
        match parse_multipart(body_bytes, &boundary) {
            Some(parts) => {
                form_map = multipart_fields(&parts);
                for p in &parts {
                    let file_name = part_str(p, "fileName");
                    if file_name.is_empty() {
                        continue;
                    }
                    let content = part_content(p);
                    let fh = stash_decision(&content)?;
                    if fh == -2 {
                        return Ok(Value::Null);
                    }
                    file_items.push(inst(
                        "UploadedFile",
                        vec![
                            ("name", Value::Str(file_name.into())),
                            ("size", Value::Int(content.len() as i64)),
                            ("contentType", Value::Str(part_str(p, "contentType").into())),
                            ("inline", inline_bytes(fh, &content)),
                            ("spillHandle", Value::Int(fh)),
                        ],
                    ));
                    file_fields.push(Value::Str(part_str(p, "name").into()));
                }
            }
            None => return Ok(Value::Null),
        }
    }

    let raw_header_lines: Vec<Value> = header_lines
        .iter()
        .map(|l| Value::Str((*l).into()))
        .collect();
    Ok(inst(
        "Request",
        vec![
            ("method", Value::Str(method.into())),
            ("path", Value::Str(decode_path(path).into())),
            ("query", query),
            ("headers", headers),
            ("cookies", cookies),
            (
                "form",
                inst("ParamBag", vec![("data", pairs_to_map(form_map))]),
            ),
            (
                "files",
                inst(
                    "FileBag",
                    vec![
                        ("items", Value::List(Rc::new(file_items))),
                        ("fieldNames", Value::List(Rc::new(file_fields))),
                    ],
                ),
            ),
            ("body", body),
            (
                "attributes",
                inst("AttrBag", vec![("data", Value::Map(Rc::new(Vec::new())))]),
            ),
            ("rawTarget", Value::Str(target.into())),
            ("rawHeaderLines", Value::List(Rc::new(raw_header_lines))),
            ("rawBody", Value::Bytes(Rc::new(body_bytes.to_vec()))),
        ],
    ))
}
