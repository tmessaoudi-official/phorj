//! `Core.Debug` (DEC-238) — the beautiful value dumper (the Symfony-VarDumper niche, shipped in
//! core): `Debug.dump(x)` renders ANY value deeply, prints it, and returns a `Dumped<T>` carrying
//! BOTH the pass-through value (`.value()`) and the rendering (`.text()`); `Debug.dd(x)` dumps and
//! exits 1 (slice 2); `Runtime.exit(code)` is the clean-termination sibling (slice 2).
//!
//! THE FORMAT (v1, deterministic, versioned here — the PHP twin `__phorj_debug_render` in
//! `transpile/runtime_php.rs` must render byte-identically):
//! - scalars via the canonical display kernel (`Value::as_display` — the single-sourced renderer
//!   interpolation uses), EXCEPT strings, which are QUOTED with `\\ \" \n \r \t` escapes;
//! - `null` / `void` for Null/Unit; bytes as `b"<hex>"` truncated at 32 bytes (`… +N`);
//! - lists `[1, 2, 3]`, maps `{"k" => v}`, sets `Set {1, 2}` — inline when the rendering fits
//!   60 columns and has no newline, else one element per line at 4-space indents;
//! - instances `Class { field: value }` in LAYOUT order (the sorted field order `ClassLayout`
//!   carries — the same order eq/reflect iterate; the PHP twin sorts property names to match);
//! - enums `Ty.Variant` / `Ty.Variant(payload)`; closures `<function>`; opaque handles `<kind>`;
//! - CYCLES render as `*RECURSION*` (the var_dump convention), detected by container pointer
//!   identity — a DAG that shares a node renders it twice (only true cycles cut).
//!
//! No colors in v1 (byte-identity everywhere; a TTY-colorized mode is a recorded nicety). The
//! renderer is PURE (same value → same string), but `dump` PRINTS, so the natives are registered
//! under the import-gated `Core.DebugSys` and the prelude does the printing via `Core.Output`.

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{HKey, Value};

/// Inline-vs-multiline threshold for containers.
const INLINE_MAX: usize = 60;
const BYTES_MAX: usize = 32;

/// Render any value (the format above). `seen` = container identity stack for cycle cutting.
fn render(v: &Value, indent: usize, seen: &mut Vec<usize>) -> String {
    match v {
        Value::Str(s) => quote(s.as_str()),
        Value::Null => "null".into(),
        Value::Unit => "void".into(),
        Value::Bytes(b) => {
            let hex: String = b
                .iter()
                .take(BYTES_MAX)
                .map(|x| format!("{x:02x}"))
                .collect();
            if b.len() > BYTES_MAX {
                format!("b\"{hex}\" (+{} more)", b.len() - BYTES_MAX)
            } else {
                format!("b\"{hex}\"")
            }
        }
        Value::List(items) => {
            let addr = std::rc::Rc::as_ptr(items) as usize;
            if seen.contains(&addr) {
                return "*RECURSION*".into();
            }
            seen.push(addr);
            let parts: Vec<String> = items.iter().map(|e| render(e, indent + 1, seen)).collect();
            seen.pop();
            wrap_container("[", "]", &parts, indent)
        }
        Value::Map(pairs) => {
            let addr = std::rc::Rc::as_ptr(pairs) as usize;
            if seen.contains(&addr) {
                return "*RECURSION*".into();
            }
            seen.push(addr);
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, e)| format!("{} => {}", render_key(k), render(e, indent + 1, seen)))
                .collect();
            seen.pop();
            wrap_container("{", "}", &parts, indent)
        }
        Value::Set(items) => {
            let parts: Vec<String> = items.iter().map(render_key).collect();
            wrap_container("Set {", "}", &parts, indent)
        }
        Value::Instance(inst) => {
            let addr = std::rc::Rc::as_ptr(inst) as usize;
            if seen.contains(&addr) {
                return "*RECURSION*".into();
            }
            seen.push(addr);
            let mut parts = Vec::new();
            for name in inst.layout.names() {
                let field = match inst.get_field(name) {
                    Some(fv) => render(&fv, indent + 1, seen),
                    None => "<unset>".into(),
                };
                parts.push(format!("{name}: {field}"));
            }
            seen.pop();
            if parts.is_empty() {
                format!("{} {{}}", inst.class)
            } else {
                wrap_container(&format!("{} {{", inst.class), "}", &parts, indent)
            }
        }
        Value::Enum(e) => {
            let addr = std::rc::Rc::as_ptr(e) as usize;
            if seen.contains(&addr) {
                return "*RECURSION*".into();
            }
            seen.push(addr);
            let out = if e.payload.is_empty() {
                format!("{}.{}", e.ty, e.variant)
            } else {
                let parts: Vec<String> = e
                    .payload
                    .iter()
                    .map(|p| render(p, indent + 1, seen))
                    .collect();
                format!("{}.{}({})", e.ty, e.variant, parts.join(", "))
            };
            seen.pop();
            out
        }
        Value::Closure(_) => "<function>".into(),
        Value::Channel(..) => "<channel>".into(),
        Value::Task(_) => "<task>".into(),
        Value::Db(h) => format!("<{}>", h.kind()),
        // Everything displayable (int/float/bool/decimal/byte) rides the canonical kernel — the
        // SAME renderer interpolation uses, so a dumped scalar always matches its printed form.
        other => other
            .as_display()
            .unwrap_or_else(|| format!("<{}>", other.type_name())),
    }
}

fn render_key(k: &HKey) -> String {
    match k {
        HKey::Str(s) => quote(s.as_str()),
        other => other.to_value().as_display().unwrap_or_default(),
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Join container parts inline when short, else one per line at the next indent level.
fn wrap_container(open: &str, close: &str, parts: &[String], indent: usize) -> String {
    if parts.is_empty() {
        return format!("{open}{close}");
    }
    let inline = format!(
        "{open}{}{}{close}",
        if open.ends_with('{') { " " } else { "" },
        parts.join(", ")
    );
    let inline = if open.ends_with('{') {
        format!("{} {close}", inline.trim_end_matches(close).trim_end())
    } else {
        inline
    };
    if inline.len() <= INLINE_MAX && !inline.contains('\n') {
        return inline;
    }
    let pad = "    ".repeat(indent + 1);
    let end_pad = "    ".repeat(indent);
    let body: Vec<String> = parts.iter().map(|p| format!("{pad}{p}")).collect();
    format!("{open}\n{}\n{end_pad}{close}", body.join("\n"))
}

/// `DebugSys.render(v)` → the deterministic rendering (PURE — same value, same string).
fn debug_render(args: &[Value], _out: &mut String) -> Result<Value, String> {
    match args {
        [v] => Ok(Value::Str(render(v, 0, &mut Vec::new()).into())),
        _ => Err("Core.Debug.__render expects (value)".into()),
    }
}

pub fn debug_natives() -> Vec<NativeFn> {
    vec![NativeFn {
        module: "Core.DebugSys",
        name: "render",
        params: vec![Ty::Param("T".into())],
        ret: Ty::String,
        pure: true,
        eval: NativeEval::Pure(debug_render),
        php: |a| {
            format!(
                "__phorj_debug_render({})",
                a.first().cloned().unwrap_or_else(|| "null".to_string())
            )
        },
    }]
}

// Unit tests live in the sibling `debug_tests.rs` (Invariant 13 discipline).
#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
