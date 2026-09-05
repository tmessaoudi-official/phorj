//! JSON-RPC plumbing for the server — response envelopes, id/number/URI readers and the JSON
//! string escaper. Split from `mod.rs` per Invariant 13; every child module reaches them as
//! `super::…`, which resolves through the parent's `use`.

use super::Out;
use crate::json::Json;

/// Build a JSON-RPC success response for request `id` with a raw `result` JSON fragment.
pub(super) fn response(id: Option<&Json>, result: &str) -> Out {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{result}}}",
        id_json(id)
    )
}

/// Build a JSON-RPC error response.
pub(super) fn error_response(id: &Json, code: i64, message: &str) -> Out {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{code},\"message\":\"{}\"}}}}",
        id_json(Some(id)),
        escape(message)
    )
}

/// Render an `id` (a number or string) back into the response; `null` if absent.
pub(super) fn id_json(id: Option<&Json>) -> String {
    match id {
        Some(Json::Num(n)) => format!("{}", *n as i64),
        Some(Json::Str(s)) => format!("\"{}\"", escape(s)),
        _ => "null".to_string(),
    }
}

/// Read a JSON number as a `u32` (LSP positions are non-negative integers).
pub(super) fn num(j: Option<&Json>) -> Option<u32> {
    match j {
        Some(Json::Num(n)) if *n >= 0.0 => Some(*n as u32),
        _ => None,
    }
}

/// The document URI of a `textDocument/...` notification (`params.textDocument.uri`).
pub(crate) fn uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri));
    p.is_file().then_some(p)
}

pub(super) fn doc_uri(msg: &Json) -> Option<String> {
    msg.get("params")
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(Json::as_str)
        .map(str::to_string)
}

/// Minimal JSON string escaping for outbound message bodies (a local copy — `diagnostic::json_escape`
/// is private). Covers the control + quote/backslash set LSP message text needs.
pub(super) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
