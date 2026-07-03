use super::*;
use crate::value::Value;
use std::rc::Rc;

fn h(f: fn(&[Value], &mut String) -> Result<Value, String>, s: &str) -> String {
    match f(
        &[Value::Bytes(Rc::new(s.as_bytes().to_vec()))],
        &mut String::new(),
    )
    .unwrap()
    {
        Value::Str(t) => t,
        other => panic!("expected string, got {other:?}"),
    }
}

// All reference values captured from real `php -n` (hash("crc32b"/"sha256"), md5, sha1).

#[test]
fn crc32_matches_php() {
    assert_eq!(h(crc32_native, ""), "00000000");
    assert_eq!(h(crc32_native, "hi"), "d8932aac");
    assert_eq!(h(crc32_native, "Hello, Phorj!"), "692703ce");
    assert_eq!(h(crc32_native, "The quick brown fox"), "b74574de");
}

#[test]
fn md5_matches_php() {
    assert_eq!(h(md5_native, ""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(h(md5_native, "hi"), "49f68a5c8493ec2c0bf489821c21fc3b");
    assert_eq!(
        h(md5_native, "Hello, Phorj!"),
        "05da00417faae4d5650c36786cd0f580"
    );
    assert_eq!(
        h(md5_native, "The quick brown fox"),
        "a2004f37730b9445670a738fa0fc9ee5"
    );
}

#[test]
fn sha1_matches_php() {
    assert_eq!(
        h(sha1_native, ""),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
    assert_eq!(
        h(sha1_native, "hi"),
        "c22b5f9178342609428d6f51b2c5af4c0bde6a42"
    );
    assert_eq!(
        h(sha1_native, "The quick brown fox"),
        "c519c1a06cdbeb2bc499e22137fb48683858b345"
    );
}

#[test]
fn sha256_matches_php() {
    assert_eq!(
        h(sha256_native, ""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        h(sha256_native, "hi"),
        "8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4"
    );
    assert_eq!(
        h(sha256_native, "Hello, Phorj!"),
        "3a5635b8b6bdc8097413f3e9d075a7422e893fcd4e42d29ab807eecadd914d25"
    );
}

#[test]
fn digests_handle_multiblock_input() {
    // > 64 bytes exercises the multi-chunk padding path; pinned to php -n.
    let long = "a".repeat(100);
    // php -n: md5(str_repeat('a',100)), sha256(...) — computed below at test authoring time.
    assert_eq!(h(md5_native, &long), "36a92cc94a9e0fa21f625f8bfb007adf");
    assert_eq!(
        h(sha256_native, &long),
        "2816597888e4a0d3a36b82b83316ab32680eb8f00f8cd3b904d681246d285a0e"
    );
}

// --- W3-4 MAC/KDF: RFC known-answer vectors (independent of the PHP oracle) --------------------

#[test]
fn hmac_sha256_rfc4231() {
    // RFC 4231 Test Case 1: key = 0x0b×20, data = "Hi There".
    let tc1 = hmac_sha256(&[0x0b; 20], b"Hi There");
    assert_eq!(
        to_hex(&tc1),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
    // RFC 4231 Test Case 2: key = "Jefe".
    let tc2 = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
    assert_eq!(
        to_hex(&tc2),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn hkdf_sha256_rfc5869_tc1() {
    let ikm = [0x0b; 22];
    let salt: Vec<u8> = (0x00..=0x0c).collect();
    let info: Vec<u8> = (0xf0..=0xf9).collect();
    let okm = hkdf_sha256(&ikm, &salt, &info, 42).unwrap();
    assert_eq!(
        to_hex(&okm),
        "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
    );
}

#[test]
fn pbkdf2_sha256_known_vectors() {
    // password="password", salt="salt", dkLen=32; iteration counts 1 and 2 (published KATs).
    assert_eq!(
        to_hex(&pbkdf2_sha256(b"password", b"salt", 1, 32)),
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
    );
    assert_eq!(
        to_hex(&pbkdf2_sha256(b"password", b"salt", 2, 32)),
        "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
    );
}

#[test]
fn constant_time_eq_matches_php_hash_equals_semantics() {
    assert!(constant_time_eq(b"abc", b"abc"));
    assert!(!constant_time_eq(b"abc", b"abd"));
    assert!(!constant_time_eq(b"abc", b"abcd")); // length mismatch → false (PHP parity)
    assert!(constant_time_eq(b"", b""));
}
