//! `Core.Hash` — crc32 / md5 / sha1 / sha256 digests (native-stdlib wave, Tier A).
//!
//! Pure, deterministic, std-only (no crates). Each digest is hand-rolled from its public spec and is
//! byte-identical to a PHP **core** function available under `php -n`: `hash("crc32b", …)`, `md5`,
//! `sha1`, `hash("sha256", …)`. Inputs are `bytes`; outputs are lowercase hex `string`. These are
//! *checksums/digests*, not a password-hashing or MAC facility (that needs a constant-time, salted
//! KDF — out of scope and deliberately not hand-rolled). Parity is pinned by unit tests against real
//! `php` output and by the differential PHP oracle.

use super::*;
use crate::types::Ty;
use crate::value::Value;

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

/// CRC-32 (the ISO-HDLC / zip / PNG variant — PHP `hash("crc32b", …)` / `crc32()`).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

const MD5_K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];
const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

fn md5(msg: &[u8]) -> [u8; 16] {
    let (mut a0, mut b0, mut c0, mut d0) = (
        0x6745_2301u32,
        0xefcd_ab89u32,
        0x98ba_dcfeu32,
        0x1032_5476u32,
    );
    let mut m = msg.to_vec();
    let bitlen = (msg.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bitlen.to_le_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 16];
        for (i, word) in w.iter_mut().enumerate() {
            *word = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | (!b & d), i)
            } else if i < 32 {
                ((d & b) | (!d & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | !d), (7 * i) % 16)
            };
            let f = f.wrapping_add(a).wrapping_add(MD5_K[i]).wrapping_add(w[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(MD5_S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut m = msg.to_vec();
    let bitlen = (msg.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5A82_7999u32)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ED9_EBA1)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC)
            } else {
                (b ^ c ^ d, 0xCA62_C1D6)
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

const SHA256_K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

fn sha256(msg: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut m = msg.to_vec();
    let bitlen = (msg.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hash_bytes(args: &[Value], digest: fn(&[u8]) -> String, who: &str) -> Result<Value, String> {
    match args {
        [Value::Bytes(b)] => Ok(Value::Str(digest(b))),
        _ => Err(format!("Hash.{who} expects (bytes)")),
    }
}
fn crc32_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| format!("{:08x}", crc32(b)), "crc32")
}
fn md5_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| to_hex(&md5(b)), "md5")
}
fn sha1_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| to_hex(&sha1(b)), "sha1")
}
fn sha256_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    hash_bytes(a, |b| to_hex(&sha256(b)), "sha256")
}

// --- W3-4: HMAC / HKDF / PBKDF2 / timing-safe compare (all over the in-file SHA-256) ------------
// Security-critical, pure, std-only. Pinned by RFC known-answer vectors (hash_tests.rs) and by
// byte-identity vs real PHP (`hash_hmac`/`hash_hkdf`/`hash_pbkdf2`/`hash_equals`).

const SHA256_BLOCK: usize = 64;

/// HMAC-SHA256 (RFC 2104).
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    // Keys longer than the block are hashed first; shorter keys are zero-padded to the block.
    let mut k = [0u8; SHA256_BLOCK];
    if key.len() > SHA256_BLOCK {
        k[..32].copy_from_slice(&sha256(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; SHA256_BLOCK];
    let mut opad = [0x5cu8; SHA256_BLOCK];
    for i in 0..SHA256_BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Vec::with_capacity(SHA256_BLOCK + msg.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(msg);
    let inner_digest = sha256(&inner);
    let mut outer = Vec::with_capacity(SHA256_BLOCK + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_digest);
    sha256(&outer)
}

/// HKDF-SHA256 (RFC 5869): extract-then-expand. An empty `salt` is HMAC-equivalent to the RFC's
/// HashLen-zeros default (HMAC zero-pads the key to the block either way), matching PHP `hash_hkdf`.
fn hkdf_sha256(ikm: &[u8], salt: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
    let n = length.div_ceil(32);
    if length == 0 || n > 255 {
        return Err("Hash.hkdf: length must be 1..=8160".to_string());
    }
    let prk = hmac_sha256(salt, ikm); // extract
    let mut okm = Vec::with_capacity(n * 32);
    let mut t: Vec<u8> = Vec::new();
    for i in 1..=n {
        let mut input = Vec::with_capacity(t.len() + info.len() + 1);
        input.extend_from_slice(&t);
        input.extend_from_slice(info);
        input.push(i as u8);
        t = hmac_sha256(&prk, &input).to_vec();
        okm.extend_from_slice(&t);
    }
    okm.truncate(length);
    Ok(okm)
}

/// PBKDF2-HMAC-SHA256 (RFC 8018 §5.2).
fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32, length: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(length);
    let mut block_index: u32 = 1;
    while out.len() < length {
        let mut salted = Vec::with_capacity(salt.len() + 4);
        salted.extend_from_slice(salt);
        salted.extend_from_slice(&block_index.to_be_bytes());
        let mut u = hmac_sha256(password, &salted);
        let mut acc = u;
        for _ in 1..iterations {
            u = hmac_sha256(password, &u);
            for j in 0..32 {
                acc[j] ^= u[j];
            }
        }
        out.extend_from_slice(&acc);
        block_index += 1;
    }
    out.truncate(length);
    out
}

/// Timing-safe byte equality. Matches PHP `hash_equals`: a length mismatch returns `false`
/// immediately (that leak is intentional parity); equal-length inputs are compared in constant time.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn two_bytes<'a>(args: &'a [Value], who: &str) -> Result<(&'a [u8], &'a [u8]), String> {
    match args {
        [Value::Bytes(x), Value::Bytes(y)] => Ok((x, y)),
        _ => Err(format!("Hash.{who} expects (bytes, bytes)")),
    }
}

fn hmac_native(a: &[Value], _: &mut String) -> Result<Value, String> {
    let (key, data) = two_bytes(a, "hmac")?;
    Ok(Value::Str(to_hex(&hmac_sha256(key, data))))
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
            let dk = pbkdf2_sha256(pw, salt, *iters as u32, length);
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
pub(crate) fn hash_natives() -> Vec<NativeFn> {
    fn entry(
        name: &'static str,
        eval: fn(&[Value], &mut String) -> Result<Value, String>,
        php: fn(&[String]) -> String,
    ) -> NativeFn {
        NativeFn {
            module: "Core.Hash",
            name,
            params: vec![Ty::Bytes],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(eval),
            php,
        }
    }
    vec![
        entry("crc32", crc32_native, |a| {
            format!("hash('crc32b', {})", parg(a, 0))
        }),
        entry("md5", md5_native, |a| format!("md5({})", parg(a, 0))),
        entry("sha1", sha1_native, |a| format!("sha1({})", parg(a, 0))),
        entry("sha256", sha256_native, |a| {
            format!("hash('sha256', {})", parg(a, 0))
        }),
        // W3-4 MAC/KDF. `hmac(key, data)` — note PHP `hash_hmac(algo, data, key)` arg order.
        NativeFn {
            module: "Core.Hash",
            name: "hmac",
            params: vec![Ty::Bytes, Ty::Bytes],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(hmac_native),
            php: |a| format!("hash_hmac('sha256', {}, {})", parg(a, 1), parg(a, 0)),
        },
        NativeFn {
            module: "Core.Hash",
            name: "equals",
            params: vec![Ty::Bytes, Ty::Bytes],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(equals_native),
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

#[cfg(test)]
#[path = "hash_tests.rs"]
mod tests;
