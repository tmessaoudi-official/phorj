use super::natives::*;
use crate::value::Value;
use std::rc::Rc;

fn bytes(s: &str) -> Value {
    Value::Bytes(Rc::new(s.as_bytes().to_vec()))
}
fn enc(f: fn(&[Value], &mut String) -> Result<Value, String>, v: Value) -> Value {
    f(&[v], &mut String::new()).unwrap()
}

#[test]
fn base64_encode_matches_php() {
    // Pinned to real `php -n` output (base64_encode).
    assert!(matches!(enc(base64_encode_native, bytes("hi")), Value::Str(s) if s == "aGk="));
    assert!(
        matches!(enc(base64_encode_native, bytes("Hello, Phorj!")), Value::Str(s) if s == "SGVsbG8sIFBob3JqIQ==")
    );
    assert!(matches!(enc(base64_encode_native, bytes("")), Value::Str(s) if s.is_empty()));
    // padding variants: 1 and 2 leftover bytes.
    assert!(matches!(enc(base64_encode_native, bytes("a")), Value::Str(s) if s == "YQ=="));
    assert!(matches!(enc(base64_encode_native, bytes("ab")), Value::Str(s) if s == "YWI="));
}

#[test]
fn hex_encode_matches_php() {
    // Pinned to real `php -n` output (bin2hex) — lowercase.
    assert!(matches!(enc(hex_encode_native, bytes("hi")), Value::Str(s) if s == "6869"));
    assert!(matches!(enc(hex_encode_native, bytes("Phorj")), Value::Str(s) if s == "50686f726a"));
    assert!(matches!(enc(hex_encode_native, bytes("")), Value::Str(s) if s.is_empty()));
}

fn decoded_bytes(v: Value) -> Vec<u8> {
    match v {
        Value::Bytes(b) => (*b).clone(),
        other => panic!("expected bytes, got {other:?}"),
    }
}

#[test]
fn base64_roundtrip() {
    let raw = "The quick brown fox \u{1f98a}".as_bytes().to_vec();
    let Value::Str(b64) = enc(base64_encode_native, Value::Bytes(Rc::new(raw.clone()))) else {
        panic!("encode");
    };
    assert_eq!(
        decoded_bytes(enc(base64_decode_native, Value::Str(b64))),
        raw
    );
}

#[test]
fn hex_roundtrip() {
    let raw = b"\x00\x01\xfePhorj".to_vec();
    let Value::Str(hex) = enc(hex_encode_native, Value::Bytes(Rc::new(raw.clone()))) else {
        panic!("encode");
    };
    assert_eq!(decoded_bytes(enc(hex_decode_native, Value::Str(hex))), raw);
}

#[test]
fn decode_invalid_is_null() {
    // base64: a character outside the alphabet (strict) → null.
    assert!(matches!(
        enc(base64_decode_native, Value::Str("not base64!".into())),
        Value::Null
    ));
    // hex: odd length / non-hex digit → null.
    assert!(matches!(
        enc(hex_decode_native, Value::Str("abc".into())),
        Value::Null
    ));
    assert!(matches!(
        enc(hex_decode_native, Value::Str("zz".into())),
        Value::Null
    ));
}

// ── Charset transcoding (DEC-468 surface, DEC-494 strategy) ────────────────────────────────────
//
// The reason these tests exist in this shape: DEC-494 rejected `encoding_rs`, so the tables here
// are hand-written and are NOT validated by any external corpus. The differential pins the Rust leg
// against the PHP leg, but both are emitted from the SAME const — so if a table row is wrong, both
// legs are wrong together and stay green. The fixtures below are therefore checked against the
// PUBLISHED code points, which is the only thing that can catch a transcription error.

use crate::charset::{decode, encode, Charset};

/// Every one of the 256 byte values, under each single-byte charset, against the standard.
///
/// Latin-1 is the identity map by definition of ISO-8859-1; Latin-9 is Latin-1 with eight named
/// substitutions; Windows-1252 is Latin-1 with the C1 block replaced, five of its slots undefined.
/// Stating those three rules independently of the tables and then checking all 256 values against
/// them is what makes this a check of the data rather than a check of itself.
#[test]
fn every_byte_decodes_to_the_published_code_point() {
    // ISO-8859-1: code point == byte, all 256 defined.
    for b in 0u8..=255 {
        let got = decode(&[b], Charset::Latin1).expect("latin-1 is total");
        assert_eq!(
            got.chars().next().unwrap() as u32,
            u32::from(b),
            "Latin-1 byte {b:#04x}"
        );
    }
    // US-ASCII: defined below 0x80, undefined at or above it.
    for b in 0u8..=255 {
        let got = decode(&[b], Charset::Ascii);
        if b < 0x80 {
            assert_eq!(got.unwrap().chars().next().unwrap() as u32, u32::from(b));
        } else {
            assert_eq!(got, None, "ASCII must refuse byte {b:#04x}");
        }
    }
    // ISO-8859-15: the eight substitutions, stated here from the standard rather than read from
    // the table under test.
    let l9: [(u8, u32); 8] = [
        (0xA4, 0x20AC),
        (0xA6, 0x0160),
        (0xA8, 0x0161),
        (0xB4, 0x017D),
        (0xB8, 0x017E),
        (0xBC, 0x0152),
        (0xBD, 0x0153),
        (0xBE, 0x0178),
    ];
    for b in 0u8..=255 {
        let want = l9
            .iter()
            .find(|(k, _)| *k == b)
            .map_or(u32::from(b), |(_, cp)| *cp);
        let got = decode(&[b], Charset::Latin9).expect("latin-9 is total");
        assert_eq!(
            got.chars().next().unwrap() as u32,
            want,
            "Latin-9 byte {b:#04x}"
        );
    }
    // Windows-1252: outside 0x80..=0x9F it agrees with Latin-1; inside, the published C1 block with
    // five undefined slots.
    let c1: [u32; 32] = [
        0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
        0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
        0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
    ];
    let mut undefined = 0;
    for b in 0u8..=255 {
        let got = decode(&[b], Charset::Windows1252);
        if (0x80..=0x9F).contains(&b) {
            let want = c1[usize::from(b) - 0x80];
            if want == 0 {
                undefined += 1;
                assert_eq!(got, None, "cp1252 {b:#04x} is undefined and must refuse");
            } else {
                assert_eq!(got.unwrap().chars().next().unwrap() as u32, want);
            }
        } else {
            assert_eq!(got.unwrap().chars().next().unwrap() as u32, u32::from(b));
        }
    }
    assert_eq!(undefined, 5, "cp1252 has exactly five undefined C1 slots");
}

/// `encode` is the exact inverse of `decode` wherever `decode` is defined. This is what stops the
/// two direction tables from disagreeing — they are searches over one table, and this proves it.
#[test]
fn encode_inverts_decode_for_every_defined_byte() {
    for cs in [
        Charset::Ascii,
        Charset::Latin1,
        Charset::Latin9,
        Charset::Windows1252,
    ] {
        for b in 0u8..=255 {
            let Some(s) = decode(&[b], cs) else { continue };
            assert_eq!(
                encode(&s, cs).as_deref(),
                Some(&[b][..]),
                "{cs:?} round-trip of byte {b:#04x}"
            );
        }
    }
}

/// A code point that a single-byte charset cannot represent yields `None`, never a substitute.
#[test]
fn unrepresentable_characters_are_refused_not_replaced() {
    assert_eq!(encode("é", Charset::Ascii), None);
    assert_eq!(encode("€", Charset::Latin1), None);
    // `¤` (U+00A4) is Latin-1's currency sign; Latin-9 reassigned that byte to `€`, so the currency
    // sign is genuinely unencodable there — the case a naive identity map would get wrong.
    assert_eq!(encode("¤", Charset::Latin9), None);
    // U+0081 is a C1 control: reachable in Latin-1, not encodable in Windows-1252.
    assert_eq!(encode("\u{81}", Charset::Windows1252), None);
    assert_eq!(encode("\u{81}", Charset::Latin1), Some(vec![0x81]));
}

#[test]
fn utf16_handles_both_byte_orders_and_surrogate_pairs() {
    assert_eq!(
        decode(&[0x48, 0x00], Charset::Utf16Le).as_deref(),
        Some("H")
    );
    assert_eq!(
        decode(&[0x00, 0x48], Charset::Utf16Be).as_deref(),
        Some("H")
    );
    // U+1F600 = D83D DE00.
    assert_eq!(
        decode(&[0x3D, 0xD8, 0x00, 0xDE], Charset::Utf16Le).as_deref(),
        Some("😀")
    );
    assert_eq!(
        decode(&[0xD8, 0x3D, 0xDE, 0x00], Charset::Utf16Be).as_deref(),
        Some("😀")
    );
    assert_eq!(
        encode("😀", Charset::Utf16Le).as_deref(),
        Some(&[0x3D, 0xD8, 0x00, 0xDE][..])
    );
    // Malformed: odd length, unpaired high surrogate, lone low surrogate.
    assert_eq!(decode(&[0x48], Charset::Utf16Le), None);
    assert_eq!(decode(&[0x3D, 0xD8], Charset::Utf16Le), None);
    assert_eq!(decode(&[0x00, 0xDC], Charset::Utf16Le), None);
    // A high surrogate followed by a non-surrogate is also invalid.
    assert_eq!(decode(&[0x3D, 0xD8, 0x48, 0x00], Charset::Utf16Le), None);
}

#[test]
fn utf8_decode_is_validation_and_refuses_malformed_input() {
    assert_eq!(
        decode("héllo".as_bytes(), Charset::Utf8).as_deref(),
        Some("héllo")
    );
    assert_eq!(decode(&[0xFF], Charset::Utf8), None);
    // A truncated two-byte sequence.
    assert_eq!(decode(&[0xC3], Charset::Utf8), None);
    assert_eq!(
        encode("héllo", Charset::Utf8).as_deref(),
        Some("héllo".as_bytes())
    );
}

#[test]
fn empty_input_round_trips_in_every_charset() {
    for cs in [
        Charset::Utf8,
        Charset::Utf16Le,
        Charset::Utf16Be,
        Charset::Latin1,
        Charset::Latin9,
        Charset::Windows1252,
        Charset::Ascii,
    ] {
        assert_eq!(decode(&[], cs).as_deref(), Some(""), "{cs:?} empty decode");
        assert_eq!(
            encode("", cs).as_deref(),
            Some(&[][..]),
            "{cs:?} empty encode"
        );
    }
}

/// The variant names in the injected prelude and the Rust codec must stay in lockstep — a rename on
/// one side alone would make `Charset.X` type-check and then fail at runtime.
#[test]
fn every_prelude_variant_maps_to_a_codec() {
    let prelude = crate::cli::preludes::CHARSET_PRELUDE;
    for v in [
        "Utf8",
        "Utf16Le",
        "Utf16Be",
        "Latin1",
        "Latin9",
        "Windows1252",
        "Ascii",
    ] {
        assert!(
            prelude.contains(&format!("{v}()")),
            "`{v}` is a codec variant but is not declared in the injected prelude"
        );
        assert!(Charset::from_variant(v).is_some());
    }
    // And nothing else is declared: count the variants in the prelude source.
    assert_eq!(
        prelude.matches("(),").count() + 1,
        7,
        "the prelude declares a variant the codec does not know: {prelude}"
    );
}
