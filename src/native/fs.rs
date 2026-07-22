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
//! `tests/fs.rs` on both backends against a scratch temp dir, the `tests/database.rs` pattern). Files are
//! UTF-8 for the `*Text` forms (a non-UTF-8 file is a clean typed error steering to `readBytes`).

use super::fs_bodies::*;
use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;

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
    // DEC-313: each `php` emitter wraps its `__phorj_fs_*` helper AT THE CALL SITE into the
    // `FileSystemResult` enum — `new FileSystemResult_Ok(...)`/`new FileSystemResult_Err(...)` must bind in the caller's namespace
    // context, never inside a global helper (the recorded R1 risk). The helper returns `[ok, payload]`.
    let entry = |name: &'static str,
                 params: Vec<Ty>,
                 ret: Ty,
                 eval: fn(&[Value], &mut String) -> Result<Value, String>,
                 lift_from: &'static [&'static str],
                 php: fn(&[String]) -> String| NativeFn {
        module: "Core.Native.FileSystem",
        name,
        params,
        ret,
        pure: false,
        eval: NativeEval::Pure(eval),
        lift_from,
        php,
    };
    // `wrapped!(helper, N)` emits the call-site FileSystemResult wrap around an N-arg helper call;
    // `pure_ok!(expr)` wraps an infallible PHP expression in `new FileSystemResult_Ok(...)` directly.
    macro_rules! wrapped {
        ($helper:literal, 1) => {
            |a: &[String]| {
                format!(
                    concat!(
                        "(($__fsr = ",
                        $helper,
                        "({}))[0] ? new FileSystemResult_Ok($__fsr[1]) : new FileSystemResult_Err($__fsr[1]))"
                    ),
                    a.first().map_or("''", |s| s)
                )
            }
        };
        ($helper:literal, 2) => {
            |a: &[String]| {
                format!(
                    concat!(
                        "(($__fsr = ",
                        $helper,
                        "({}, {}))[0] ? new FileSystemResult_Ok($__fsr[1]) : new FileSystemResult_Err($__fsr[1]))"
                    ),
                    a.first().map_or("''", |s| s),
                    a.get(1).map_or("''", |s| s)
                )
            }
        };
        ($helper:literal, put $append:literal $op:literal) => {
            |a: &[String]| {
                format!(
                    concat!(
                        "(($__fsr = ",
                        $helper,
                        "({}, {}, ",
                        $append,
                        ", '",
                        $op,
                        "'))[0] ? new FileSystemResult_Ok($__fsr[1]) : new FileSystemResult_Err($__fsr[1]))"
                    ),
                    a.first().map_or("''", |s| s),
                    a.get(1).map_or("''", |s| s)
                )
            }
        };
    }
    vec![
        entry(
            "readText",
            vec![Ty::String],
            res(Ty::String),
            fs_read_text,
            &[],
            wrapped!("__phorj_fs_read_text", 1),
        ),
        entry(
            "readBytes",
            vec![Ty::String],
            res(Ty::Bytes),
            fs_read_bytes,
            &[],
            wrapped!("__phorj_fs_read_bytes", 1),
        ),
        entry(
            "writeText",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_write_text,
            &[],
            wrapped!("__phorj_fs_put", put "false" "writeText"),
        ),
        entry(
            "writeBytes",
            vec![Ty::String, Ty::Bytes],
            res(opt_null()),
            fs_write_bytes,
            &[],
            wrapped!("__phorj_fs_put", put "false" "writeBytes"),
        ),
        entry(
            "appendText",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_append_text,
            &[],
            wrapped!("__phorj_fs_put", put "true" "appendText"),
        ),
        entry(
            "copy",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_copy,
            &[],
            wrapped!("__phorj_fs_copy", 2),
        ),
        entry(
            "move",
            vec![Ty::String, Ty::String],
            res(opt_null()),
            fs_move,
            &[],
            wrapped!("__phorj_fs_move", 2),
        ),
        entry(
            "delete",
            vec![Ty::String],
            res(opt_null()),
            fs_delete,
            &[],
            wrapped!("__phorj_fs_delete", 1),
        ),
        entry(
            "size",
            vec![Ty::String],
            res(Ty::Int),
            fs_size,
            &[],
            wrapped!("__phorj_fs_size", 1),
        ),
        entry(
            "exists",
            vec![Ty::String],
            res(Ty::Bool),
            fs_exists,
            &[],
            |a| {
                format!(
                    "new FileSystemResult_Ok(file_exists({}))",
                    a.first().map_or("''", |s| s)
                )
            },
        ),
        entry(
            "isFile",
            vec![Ty::String],
            res(Ty::Bool),
            fs_is_file,
            &[],
            |a| {
                format!(
                    "new FileSystemResult_Ok(is_file({}))",
                    a.first().map_or("''", |s| s)
                )
            },
        ),
        entry(
            "isDir",
            vec![Ty::String],
            res(Ty::Bool),
            fs_is_dir,
            &[],
            |a| {
                format!(
                    "new FileSystemResult_Ok(is_dir({}))",
                    a.first().map_or("''", |s| s)
                )
            },
        ),
        entry(
            "createDir",
            vec![Ty::String],
            res(opt_null()),
            fs_create_dir,
            &[],
            wrapped!("__phorj_fs_create_dir", 1),
        ),
        entry(
            "removeDir",
            vec![Ty::String],
            res(opt_null()),
            fs_remove_dir,
            &[],
            wrapped!("__phorj_fs_remove_dir", 1),
        ),
        entry(
            "removeDirAll",
            vec![Ty::String],
            res(opt_null()),
            fs_remove_dir_all,
            &[],
            wrapped!("__phorj_fs_remove_dir_all", 1),
        ),
        entry(
            "listDir",
            vec![Ty::String],
            res(Ty::List(Box::new(Ty::String))),
            fs_list_dir,
            &[],
            wrapped!("__phorj_fs_list_dir", 1),
        ),
        entry(
            "walk",
            vec![Ty::String],
            res(Ty::List(Box::new(Ty::String))),
            fs_walk,
            &[],
            wrapped!("__phorj_fs_walk", 1),
        ),
        entry("tempDir", vec![], res(Ty::String), fs_temp_dir, &[], |_| {
            "new FileSystemResult_Ok(sys_get_temp_dir())".to_string()
        }),
    ]
}
