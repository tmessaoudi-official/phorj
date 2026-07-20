//! MAC / KDF facility for `Core.Hash` — HMAC / HKDF / PBKDF2 / timing-safe compare over SHA-256.
//!
//! Security-critical, pure, std-only (all over the in-file SHA-256). Pinned by RFC known-answer
//! vectors (hash_tests.rs) and by byte-identity vs real PHP
//! (`hash_hmac`/`hash_hkdf`/`hash_pbkdf2`/`hash_equals`).

use super::digests::sha256;

const SHA256_BLOCK: usize = 64;

/// HMAC-SHA256 (RFC 2104).
pub(super) fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
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
pub(super) fn hkdf_sha256(
    ikm: &[u8],
    salt: &[u8],
    info: &[u8],
    length: usize,
) -> Result<Vec<u8>, String> {
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

/// PBKDF2-HMAC-SHA256 (RFC 8018 §5.2). `iterations` is `u64` (not `u32`): PHP's `hash_pbkdf2`
/// takes the count as a native `int` (i64), so a `u32` cap would silently truncate a large
/// iteration count and diverge from the PHP leg (UA-1.3). The RFC block counter stays `u32`.
pub(super) fn pbkdf2_sha256(
    password: &[u8],
    salt: &[u8],
    iterations: u64,
    length: usize,
) -> Vec<u8> {
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
pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}
