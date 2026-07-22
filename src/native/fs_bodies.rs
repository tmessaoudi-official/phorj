//! `Core.FileSystemModule` — the native bodies (M-Decomp split from `fs.rs`, Invariant 13): the
//! `FileSystemResult` wrap helpers, the io-error → `FileSystemError` taxonomy classifier, and the
//! per-native `*_inner` implementations the `fs.rs` registry rows call.

use crate::value::{EnumVal, Payload, Value};
use std::rc::Rc;

pub(super) fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "FileSystemResult".into(),
        variant: "Ok".into(),
        payload: Payload::One(v),
    }))
}

pub(super) fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "FileSystemResult".into(),
        variant: "Err".into(),
        payload: Payload::One(Value::Str(msg.into())),
    }))
}

pub(super) fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

/// Classify a std::io error into the `FileSystemError` taxonomy marker.
pub(super) fn classify(op: &str, path: &str, e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    let kind = match e.kind() {
        K::NotFound => "NotFound",
        K::PermissionDenied => "PermissionDenied",
        K::AlreadyExists => "AlreadyExists",
        K::NotADirectory => "NotADirectory",
        K::IsADirectory => "IsADirectory",
        K::DirectoryNotEmpty => "DirNotEmpty",
        _ => "FileSystemIoError",
    };
    format!("<<{kind}>>Core.FileSystemModule.{op}: `{path}`: {e}")
}

pub(super) fn one_path<'a>(args: &'a [Value], who: &str) -> Result<&'a str, String> {
    match args {
        [Value::Str(p)] => Ok(p.as_str()),
        _ => Err(format!(
            "Core.FileSystemModule.__{who} expects (string path)"
        )),
    }
}

pub(super) fn two_paths<'a>(args: &'a [Value], who: &str) -> Result<(&'a str, &'a str), String> {
    match args {
        [Value::Str(a), Value::Str(b)] => Ok((a.as_str(), b.as_str())),
        _ => Err(format!(
            "Core.FileSystemModule.__{who} expects (string, string)"
        )),
    }
}

// ── File bodies ──────────────────────────────────────────────────────────────────────────────────────

pub(super) fn read_text_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "readText")?;
    match std::fs::read(p) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Ok(Value::Str(s.into())),
            Err(_) => Err(format!(
                "<<FileSystemIoError>>Core.FileSystemModule.readText: `{p}` is not UTF-8 — use readBytes"
            )),
        },
        Err(e) => Err(classify("readText", p, &e)),
    }
}

pub(super) fn read_bytes_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "readBytes")?;
    std::fs::read(p)
        .map(|b| Value::Bytes(Rc::new(b)))
        .map_err(|e| classify("readBytes", p, &e))
}

pub(super) fn write_text_inner(args: &[Value]) -> Result<Value, String> {
    let (p, contents) = match args {
        [Value::Str(p), Value::Str(c)] => (p.as_str(), c.as_str()),
        _ => return Err("Core.FileSystemModule.__writeText expects (string, string)".into()),
    };
    std::fs::write(p, contents)
        .map(|()| Value::Null)
        .map_err(|e| classify("writeText", p, &e))
}

pub(super) fn write_bytes_inner(args: &[Value]) -> Result<Value, String> {
    let (p, contents) = match args {
        [Value::Str(p), Value::Bytes(b)] => (p.as_str(), b),
        _ => return Err("Core.FileSystemModule.__writeBytes expects (string, bytes)".into()),
    };
    std::fs::write(p, &**contents)
        .map(|()| Value::Null)
        .map_err(|e| classify("writeBytes", p, &e))
}

pub(super) fn append_text_inner(args: &[Value]) -> Result<Value, String> {
    use std::io::Write as _;
    let (p, contents) = match args {
        [Value::Str(p), Value::Str(c)] => (p.as_str(), c.as_str()),
        _ => return Err("Core.FileSystemModule.__appendText expects (string, string)".into()),
    };
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .map(|()| Value::Null)
        .map_err(|e| classify("appendText", p, &e))
}

pub(super) fn copy_inner(args: &[Value]) -> Result<Value, String> {
    let (from, to) = two_paths(args, "copy")?;
    std::fs::copy(from, to)
        .map(|_| Value::Null)
        .map_err(|e| classify("copy", from, &e))
}

pub(super) fn move_inner(args: &[Value]) -> Result<Value, String> {
    let (from, to) = two_paths(args, "move")?;
    std::fs::rename(from, to)
        .map(|()| Value::Null)
        .map_err(|e| classify("move", from, &e))
}

pub(super) fn delete_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "delete")?;
    std::fs::remove_file(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("delete", p, &e))
}

pub(super) fn size_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "size")?;
    std::fs::metadata(p)
        .map(|m| Value::Int(i64::try_from(m.len()).unwrap_or(i64::MAX)))
        .map_err(|e| classify("size", p, &e))
}

pub(super) fn exists_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "exists")?;
    Ok(Value::Bool(std::path::Path::new(p).exists()))
}

pub(super) fn is_file_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "isFile")?;
    Ok(Value::Bool(std::path::Path::new(p).is_file()))
}

pub(super) fn is_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "isDir")?;
    Ok(Value::Bool(std::path::Path::new(p).is_dir()))
}

// ── Directory bodies ─────────────────────────────────────────────────────────────────────────────────

pub(super) fn create_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "createDir")?;
    std::fs::create_dir_all(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("createDir", p, &e))
}

pub(super) fn remove_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "removeDir")?;
    std::fs::remove_dir(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("removeDir", p, &e))
}

/// `removeDirAll` — the RECURSIVE delete. Deliberately named loudly (never `removeDir`'s behavior);
/// refuses `/`, `.` and `..` outright (a cheap footgun net; the OS permission model is the real gate).
pub(super) fn remove_dir_all_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "removeDirAll")?;
    if matches!(p, "/" | "." | "..") || p.is_empty() {
        return Err(format!(
            "<<PermissionDenied>>Core.FileSystemModule.removeDirAll: refusing `{p}` (protect-the-obvious net)"
        ));
    }
    std::fs::remove_dir_all(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("removeDirAll", p, &e))
}

/// `listDir` — the ENTRY NAMES of one directory, SORTED (determinism: directory iteration order is
/// OS-dependent; a sorted listing is reproducible).
pub(super) fn list_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "listDir")?;
    let rd = std::fs::read_dir(p).map_err(|e| classify("listDir", p, &e))?;
    let mut names: Vec<String> = Vec::new();
    for entry in rd {
        let entry = entry.map_err(|e| classify("listDir", p, &e))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(Value::List(Rc::new(
        names.into_iter().map(|n| Value::Str(n.into())).collect(),
    )))
}

/// `walk` — every FILE under a root, recursive, as `/`-joined paths RELATIVE to the root, sorted.
pub(super) fn walk_inner(args: &[Value]) -> Result<Value, String> {
    let root = one_path(args, "walk")?;
    let mut out: Vec<String> = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, String)> =
        vec![(std::path::PathBuf::from(root), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let rd = std::fs::read_dir(&dir).map_err(|e| classify("walk", root, &e))?;
        for entry in rd {
            let entry = entry.map_err(|e| classify("walk", root, &e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let path = entry.path();
            if path.is_dir() {
                stack.push((path, child_rel));
            } else {
                out.push(child_rel);
            }
        }
    }
    out.sort();
    Ok(Value::List(Rc::new(
        out.into_iter().map(|n| Value::Str(n.into())).collect(),
    )))
}

pub(super) fn temp_dir_inner(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("Core.FileSystemModule.__tempDir expects no arguments".into());
    }
    Ok(Value::Str(
        std::env::temp_dir().to_string_lossy().into_owned().into(),
    ))
}
