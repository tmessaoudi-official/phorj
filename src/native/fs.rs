//! `Core.FileSystemModule` (W3 — the FS/streams parity blocker, TOP-20 #5): the TYPED filesystem module, built on
//! the `Core.DatabaseModule`/`Mail`/`HttpClient` architecture (prelude-wrapper `FileSystemResult<T>` + a `<<Kind>>`
//! marker → typed catchable `FileSystemError` taxonomy). It SUPERSEDES the older `Core.File` ERGONOMICS
//! (whose write/delete failures are uncatchable hard faults and whose read maps every failure to
//! `null` — the pre-taxonomy era); `Core.File` stays untouched (additive — its deprecation is a
//! queued developer adjudication, never a silent break).
//!
//! Determinism: listings are SORTED (Invariant 10 — no directory-order leakage); `walk` yields
//! relative paths sorted lexicographically. All natives are `pure:false` EXCEPT nothing — the whole
//! module reads ambient filesystem state, so importing programs are spine-quarantined (validated by
//! `tests/fs.rs` on both backends against a scratch temp dir, the `tests/db.rs` pattern). Files are
//! UTF-8 for the `*Text` forms (a non-UTF-8 file is a clean typed error steering to `readBytes`).

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{EnumVal, Payload, Value};
use std::rc::Rc;

fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "FileSystemResult".into(),
        variant: "Ok".into(),
        payload: Payload::One(v),
    }))
}

fn failure(msg: String) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "FileSystemResult".into(),
        variant: "Err".into(),
        payload: Payload::One(Value::Str(msg.into())),
    }))
}

fn wrap(inner: Result<Value, String>) -> Value {
    match inner {
        Ok(v) => success(v),
        Err(msg) => failure(msg),
    }
}

/// Classify a std::io error into the `FileSystemError` taxonomy marker.
fn classify(op: &str, path: &str, e: &std::io::Error) -> String {
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

fn one_path<'a>(args: &'a [Value], who: &str) -> Result<&'a str, String> {
    match args {
        [Value::Str(p)] => Ok(p.as_str()),
        _ => Err(format!(
            "Core.FileSystemModule.__{who} expects (string path)"
        )),
    }
}

fn two_paths<'a>(args: &'a [Value], who: &str) -> Result<(&'a str, &'a str), String> {
    match args {
        [Value::Str(a), Value::Str(b)] => Ok((a.as_str(), b.as_str())),
        _ => Err(format!(
            "Core.FileSystemModule.__{who} expects (string, string)"
        )),
    }
}

// ── File bodies ──────────────────────────────────────────────────────────────────────────────────────

fn read_text_inner(args: &[Value]) -> Result<Value, String> {
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

fn read_bytes_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "readBytes")?;
    std::fs::read(p)
        .map(|b| Value::Bytes(Rc::new(b)))
        .map_err(|e| classify("readBytes", p, &e))
}

fn write_text_inner(args: &[Value]) -> Result<Value, String> {
    let (p, contents) = match args {
        [Value::Str(p), Value::Str(c)] => (p.as_str(), c.as_str()),
        _ => return Err("Core.FileSystemModule.__writeText expects (string, string)".into()),
    };
    std::fs::write(p, contents)
        .map(|()| Value::Null)
        .map_err(|e| classify("writeText", p, &e))
}

fn write_bytes_inner(args: &[Value]) -> Result<Value, String> {
    let (p, contents) = match args {
        [Value::Str(p), Value::Bytes(b)] => (p.as_str(), b),
        _ => return Err("Core.FileSystemModule.__writeBytes expects (string, bytes)".into()),
    };
    std::fs::write(p, &**contents)
        .map(|()| Value::Null)
        .map_err(|e| classify("writeBytes", p, &e))
}

fn append_text_inner(args: &[Value]) -> Result<Value, String> {
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

fn copy_inner(args: &[Value]) -> Result<Value, String> {
    let (from, to) = two_paths(args, "copy")?;
    std::fs::copy(from, to)
        .map(|_| Value::Null)
        .map_err(|e| classify("copy", from, &e))
}

fn move_inner(args: &[Value]) -> Result<Value, String> {
    let (from, to) = two_paths(args, "move")?;
    std::fs::rename(from, to)
        .map(|()| Value::Null)
        .map_err(|e| classify("move", from, &e))
}

fn delete_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "delete")?;
    std::fs::remove_file(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("delete", p, &e))
}

fn size_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "size")?;
    std::fs::metadata(p)
        .map(|m| Value::Int(i64::try_from(m.len()).unwrap_or(i64::MAX)))
        .map_err(|e| classify("size", p, &e))
}

fn exists_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "exists")?;
    Ok(Value::Bool(std::path::Path::new(p).exists()))
}

fn is_file_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "isFile")?;
    Ok(Value::Bool(std::path::Path::new(p).is_file()))
}

fn is_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "isDir")?;
    Ok(Value::Bool(std::path::Path::new(p).is_dir()))
}

// ── Directory bodies ─────────────────────────────────────────────────────────────────────────────────

fn create_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "createDir")?;
    std::fs::create_dir_all(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("createDir", p, &e))
}

fn remove_dir_inner(args: &[Value]) -> Result<Value, String> {
    let p = one_path(args, "removeDir")?;
    std::fs::remove_dir(p)
        .map(|()| Value::Null)
        .map_err(|e| classify("removeDir", p, &e))
}

/// `removeDirAll` — the RECURSIVE delete. Deliberately named loudly (never `removeDir`'s behavior);
/// refuses `/`, `.` and `..` outright (a cheap footgun net; the OS permission model is the real gate).
fn remove_dir_all_inner(args: &[Value]) -> Result<Value, String> {
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
fn list_dir_inner(args: &[Value]) -> Result<Value, String> {
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
fn walk_inner(args: &[Value]) -> Result<Value, String> {
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

fn temp_dir_inner(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err("Core.FileSystemModule.__tempDir expects no arguments".into());
    }
    Ok(Value::Str(
        std::env::temp_dir().to_string_lossy().into_owned().into(),
    ))
}

macro_rules! fs_native {
    ($name:ident, $inner:ident) => {
        fn $name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            Ok(wrap($inner(args)))
        }
    };
}
fs_native!(fs_read_text, read_text_inner);
fs_native!(fs_read_bytes, read_bytes_inner);
fs_native!(fs_write_text, write_text_inner);
fs_native!(fs_write_bytes, write_bytes_inner);
fs_native!(fs_append_text, append_text_inner);
fs_native!(fs_copy, copy_inner);
fs_native!(fs_move, move_inner);
fs_native!(fs_delete, delete_inner);
fs_native!(fs_size, size_inner);
fs_native!(fs_exists, exists_inner);
fs_native!(fs_is_file, is_file_inner);
fs_native!(fs_is_dir, is_dir_inner);
fs_native!(fs_create_dir, create_dir_inner);
fs_native!(fs_remove_dir, remove_dir_inner);
fs_native!(fs_remove_dir_all, remove_dir_all_inner);
fs_native!(fs_list_dir, list_dir_inner);
fs_native!(fs_walk, walk_inner);
fs_native!(fs_temp_dir, temp_dir_inner);

/// The `Core.Native.FileSystem` registry entries (std-only — no new dependency; always compiled, no feature).
pub fn fs_natives() -> Vec<NativeFn> {
    let res = |t: Ty| Ty::Named("FileSystemResult".into(), vec![t]);
    let opt_null = || Ty::Optional(Box::new(Ty::String));
    let entry =
        |name: &'static str,
         params: Vec<Ty>,
         ret: Ty,
         eval: fn(&[Value], &mut String) -> Result<Value, String>| NativeFn {
            module: "Core.Native.FileSystem",
            name,
            params,
            ret,
            pure: false,
            eval: NativeEval::Pure(eval),
            php: |a| a.first().cloned().unwrap_or_else(|| "''".to_string()),
        };
    vec![
        entry("readText", vec![Ty::String], res(Ty::String), fs_read_text),
        entry("readBytes", vec![Ty::String], res(Ty::Bytes), fs_read_bytes),
        entry(
            "writeText",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_write_text,
        ),
        entry(
            "writeBytes",
            vec![Ty::String, Ty::Bytes],
            res(opt_null()),
            fs_write_bytes,
        ),
        entry(
            "appendText",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_append_text,
        ),
        entry(
            "copy",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_copy,
        ),
        entry(
            "move",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_move,
        ),
        entry("delete", vec![Ty::String], res(opt_null()), fs_delete),
        entry("size", vec![Ty::String], res(Ty::Int), fs_size),
        entry("exists", vec![Ty::String], res(Ty::Bool), fs_exists),
        entry("isFile", vec![Ty::String], res(Ty::Bool), fs_is_file),
        entry("isDir", vec![Ty::String], res(Ty::Bool), fs_is_dir),
        entry(
            "createDir",
            vec![Ty::String],
            res(opt_null()),
            fs_create_dir,
        ),
        entry(
            "removeDir",
            vec![Ty::String],
            res(opt_null()),
            fs_remove_dir,
        ),
        entry(
            "removeDirAll",
            vec![Ty::String],
            res(opt_null()),
            fs_remove_dir_all,
        ),
        entry(
            "listDir",
            vec![Ty::String],
            res(Ty::List(Box::new(Ty::String))),
            fs_list_dir,
        ),
        entry(
            "walk",
            vec![Ty::String],
            res(Ty::List(Box::new(Ty::String))),
            fs_walk,
        ),
        entry("tempDir", vec![], res(Ty::String), fs_temp_dir),
    ]
}
