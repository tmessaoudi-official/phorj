//! Tests: PEM/base64 decoding (S3.5). The property under test throughout is **fewer blocks, never a
//! wrong one** — every malformed shape must vanish rather than decode to something plausible, because
//! `tls::build` reads an empty result as a startup refusal and a wrong one as a working server.
use super::*;

#[test]
fn a_single_block_decodes_to_its_der() {
    // "hello" in base64. The point is the framing, not the payload.
    let doc = "-----BEGIN CERTIFICATE-----\naGVsbG8=\n-----END CERTIFICATE-----\n";
    let blocks = pem_blocks(doc);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "CERTIFICATE");
    assert_eq!(blocks[0].1, b"hello");
}

#[test]
fn a_chain_keeps_every_block_in_order() {
    // Certificate chains are ordered (leaf first); rustls relies on that, so the decoder must not
    // reorder or deduplicate.
    let doc = "-----BEGIN CERTIFICATE-----\naGVsbG8=\n-----END CERTIFICATE-----\n\
               -----BEGIN CERTIFICATE-----\nd29ybGQ=\n-----END CERTIFICATE-----\n";
    let blocks = pem_blocks(doc);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].1, b"hello");
    assert_eq!(blocks[1].1, b"world");
}

#[test]
fn payload_lines_are_joined_across_the_wrap() {
    // Real PEM wraps at 64 columns. A decoder that treats each line separately produces garbage that
    // still base64-decodes, which is the worst possible failure here.
    let doc = "-----BEGIN CERTIFICATE-----\naGVs\nbG8=\n-----END CERTIFICATE-----\n";
    assert_eq!(pem_blocks(doc)[0].1, b"hello");
}

#[test]
fn text_that_is_not_pem_yields_nothing() {
    for doc in [
        "this is not a certificate\n",
        "",
        "-----BEGIN CERTIFICATE-----\n", // truncated: no END
        "-----END CERTIFICATE-----\n",   // an END with no BEGIN
    ] {
        assert!(
            pem_blocks(doc).is_empty(),
            "must not read a block out of {doc:?}"
        );
    }
}

#[test]
fn an_end_marker_must_match_its_begin() {
    // A file whose END names a different label is truncated or spliced. Accepting it would hand
    // rustls a "certificate" assembled from two different objects.
    let doc = "-----BEGIN CERTIFICATE-----\naGVsbG8=\n-----END PRIVATE KEY-----\n";
    assert!(
        pem_blocks(doc).is_empty(),
        "mismatched labels must not close a block"
    );
}

#[test]
fn a_block_whose_payload_is_not_base64_is_dropped() {
    // The alternative — decoding what parses and ignoring the rest — is how a corrupted file becomes
    // a server with a subtly wrong key that fails only at handshake time.
    for payload in ["not base64!", "aGVsbG8", "aGV$bG8=", "===="] {
        let doc = format!("-----BEGIN CERTIFICATE-----\n{payload}\n-----END CERTIFICATE-----\n");
        assert!(
            pem_blocks(&doc).is_empty(),
            "payload {payload:?} must not decode"
        );
    }
}

#[test]
fn the_label_is_preserved_so_key_kinds_can_be_told_apart() {
    // PKCS#8, PKCS#1 and SEC1 keys are all valid and are wrapped differently; `tls::build` selects on
    // this label. Losing it would mean guessing at the encoding.
    for label in ["PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"] {
        let doc = format!("-----BEGIN {label}-----\naGVsbG8=\n-----END {label}-----\n");
        assert_eq!(pem_blocks(&doc)[0].0, label);
    }
}

#[test]
fn base64_padding_is_honoured_exactly() {
    assert_eq!(b64("aGVsbG8=").as_deref(), Some(&b"hello"[..]));
    assert_eq!(b64("aGVsbG9v").as_deref(), Some(&b"helloo"[..]));
    assert_eq!(b64("aGVsbA==").as_deref(), Some(&b"hell"[..]));
    // A length that is not a multiple of four cannot be valid base64, however plausible it looks.
    assert_eq!(b64("aGVsbG8"), None);
    assert_eq!(b64(""), None);
}
