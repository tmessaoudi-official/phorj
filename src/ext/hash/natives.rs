//! `Core.Hash` — crc32 / md5 / sha1 / sha256 digests + MAC/KDF registry (native-stdlib wave, Tier A).
//!
//! Pure, deterministic, std-only (no crates). The digest kernels live in `digests`, the MAC/KDF
//! facility in `mac`; this module wraps them as `NativeFn` registry entries. Each digest is
//! byte-identical to a PHP **core** function available under `php -n`: `hash("crc32b", …)`, `md5`,
//! `sha1`, `hash("sha256", …)`. Inputs are `bytes`; digest outputs are lowercase hex `string`.
//! Parity is pinned by unit tests against real `php` output and by the differential PHP oracle.

use super::digests::*;
use super::mac::*;
use crate::native::*;
use crate::types::Ty;
use crate::value::Value;

fn hash_bytes(args: &[Value], digest: fn(&[u8]) -> String, who: &str) -> Result<Value, String> {
    match args {
        [Value::Bytes(b)] => Ok(Value::Str(digest(b).into())),
        _ => Err(format!("Hash.{who} expects (bytes)")),
    }
}
pub(super) fn crc32_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| format!("{:08x}", crc32(b)), "crc32")
}
pub(super) fn md5_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| to_hex(&md5(b)), "md5")
}
pub(super) fn sha1_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| to_hex(&sha1(b)), "sha1")
}
pub(super) fn sha256_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| to_hex(&sha256(b)), "sha256")
}

fn two_bytes<'a>(args: &'a [Value], who: &str) -> Result<(&'a [u8], &'a [u8]), String> {
    match args {
        [Value::Bytes(x), Value::Bytes(y)] => Ok((x, y)),
        _ => Err(format!("Hash.{who} expects (bytes, bytes)")),
    }
}

fn hmac_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    let (key, data) = two_bytes(a, "hmac")?;
    // Returns raw bytes (UA-1.4): hex is a transport concern, not a MAC's type — this makes
    // hmac/hkdf/pbkdf2 uniformly `bytes`. Callers hex-encode for display via Encoding.hexEncode.
    Ok(Value::Bytes(std::rc::Rc::new(
        hmac_sha256(key, data).to_vec(),
    )))
}

fn equals_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    let (x, y) = two_bytes(a, "equals")?;
    Ok(Value::Bool(constant_time_eq(x, y)))
}

fn nonneg_len(v: &Value, who: &str) -> Result<usize, String> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(*n as usize),
        _ => Err(format!("Hash.{who}: length must be a non-negative int")),
    }
}

fn hkdf_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    match a {
        [Value::Bytes(ikm), Value::Bytes(salt), Value::Bytes(info), len] => {
            let length = nonneg_len(len, "hkdf")?;
            let okm = hkdf_sha256(ikm, salt, info, length)?;
            Ok(Value::Bytes(std::rc::Rc::new(okm)))
        }
        _ => Err("Hash.hkdf expects (bytes ikm, bytes salt, bytes info, int length)".to_string()),
    }
}

fn pbkdf2_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    match a {
        [Value::Bytes(pw), Value::Bytes(salt), Value::Int(iters), len] if *iters > 0 => {
            let length = nonneg_len(len, "pbkdf2")?;
            let dk = pbkdf2_sha256(pw, salt, *iters as u64, length);
            Ok(Value::Bytes(std::rc::Rc::new(dk)))
        }
        _ => Err(
            "Hash.pbkdf2 expects (bytes password, bytes salt, int iterations>0, int length)"
                .to_string(),
        ),
    }
}

/// The `Core.Hash` registry entries. The plain digests are `(bytes) -> string` (lowercase hex), 1:1
/// with a PHP core digest function; W3-4 adds the MAC/KDF facility (hmac/equals/hkdf/pbkdf2).
pub fn hash_natives() -> Vec<NativeFn> {
    fn entry(
        name: &'static str,
        eval: fn(&[Value], &mut String) -> Result<Value, String>,
        lift_from: &'static [&'static str],
        php: fn(&[String]) -> String,
    ) -> NativeFn {
        NativeFn {
            module: "Core.Hash",
            name,
            params: vec![Ty::Bytes],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(eval),
            lift_from,
            php,
        }
    }
    vec![
        // NOT lift-registered: PHP's `crc32()` builtin returns an INT; this native is the
        // `hash('crc32b', …)` HEX-string form — inverting would change semantics (DEC-312 rule:
        // no wrong guesses).
        entry("crc32", crc32_native, &[], |a| {
            format!("hash('crc32b', {})", parg(a, 0))
        }),
        entry("md5", md5_native, &["md5"], |a| {
            format!("md5({})", parg(a, 0))
        }),
        entry("sha1", sha1_native, &["sha1"], |a| {
            format!("sha1({})", parg(a, 0))
        }),
        entry("sha256", sha256_native, &[], |a| {
            format!("hash('sha256', {})", parg(a, 0))
        }),
        // W3-4 MAC/KDF. `hmac(key, data)` — note PHP `hash_hmac(algo, data, key)` arg order.
        NativeFn {
            module: "Core.Hash",
            name: "hmac",
            params: vec![Ty::Bytes, Ty::Bytes],
            ret: Ty::Bytes,
            pure: true,
            eval: NativeEval::Pure(hmac_native),
            // `true` = raw binary output (UA-1.4: hmac returns bytes, matching hkdf/pbkdf2).
            lift_from: &[],
            php: |a| format!("hash_hmac('sha256', {}, {}, true)", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.Hash",
            name: "equals",
            params: vec![Ty::Bytes, Ty::Bytes],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(equals_native),
            lift_from: &["hash_equals"],
            php: |a| format!("hash_equals({}, {})", parg(a, 0), parg(a, 1)),
        },
        // `hkdf(ikm, salt, info, length)` → PHP `hash_hkdf(algo, ikm, length, info, salt)` (raw bytes).
        NativeFn {
            module: "Core.Hash",
            name: "hkdf",
            params: vec![Ty::Bytes, Ty::Bytes, Ty::Bytes, Ty::Int],
            ret: Ty::Bytes,
            pure: true,
            eval: NativeEval::Pure(hkdf_native),
            lift_from: &[],
            php: |a| {
                format!(
                    "hash_hkdf('sha256', {}, {}, {}, {})",
                    parg(a, 0),
                    parg(a, 3),
                    parg(a, 2),
                    parg(a, 1)
                )
            },
        },
        // `pbkdf2(password, salt, iterations, length)` → PHP `hash_pbkdf2(..., raw_output=true)`.
        NativeFn {
            module: "Core.Hash",
            name: "pbkdf2",
            params: vec![Ty::Bytes, Ty::Bytes, Ty::Int, Ty::Int],
            ret: Ty::Bytes,
            pure: true,
            eval: NativeEval::Pure(pbkdf2_native),
            lift_from: &[],
            php: |a| {
                format!(
                    "hash_pbkdf2('sha256', {}, {}, {}, {}, true)",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2),
                    parg(a, 3)
                )
            },
        },
    ]
}
