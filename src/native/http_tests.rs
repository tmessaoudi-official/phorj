//! Unit tests for the `Core.Native.Http` wire natives (DEC-331 slice 2) — decode edge cases,
//! multipart acceptance (small / spill / over-cap / malformed), and the spill store.
use super::{parse_multipart, parse_query_pairs, MULTIPART_MAX_PARTS, SPILL_THRESHOLD};
use crate::value::Value;

fn pairs(s: &str) -> Vec<(String, Vec<String>)> {
    parse_query_pairs(s)
}

#[test]
fn query_first_wins_order_and_dup_accumulation() {
    let p = pairs("b=1&a=2&b=3&c&a=4");
    assert_eq!(
        p,
        vec![
            ("b".into(), vec!["1".into(), "3".into()]),
            ("a".into(), vec!["2".into(), "4".into()]),
            ("c".into(), vec![String::new()]),
        ]
    );
}

#[test]
fn query_form_decoding_edges() {
    // '+' → space; %XX; value keeps everything after the FIRST '='.
    assert_eq!(
        pairs("q=a+b%21&eq=x=y"),
        vec![
            ("q".into(), vec!["a b!".into()]),
            ("eq".into(), vec!["x=y".into()]),
        ]
    );
    // Invalid/truncated escapes stay literal; %00 decodes to NUL (data, not an error).
    assert_eq!(pairs("k=%zz%2"), vec![("k".into(), vec!["%zz%2".into()])]);
    assert_eq!(pairs("n=%00x"), vec![("n".into(), vec!["\u{0}x".into()])]);
    // A component whose decoded bytes are invalid UTF-8 falls back to the UNDECODED original.
    assert_eq!(pairs("k=%ff"), vec![("k".into(), vec!["%ff".into()])]);
    // Dots and spaces in KEYS survive (the parse_str-mangling this parser exists to avoid).
    assert_eq!(
        pairs("a.b=1&c+d=2"),
        vec![
            ("a.b".into(), vec!["1".into()]),
            ("c d".into(), vec!["2".into()]),
        ]
    );
}

fn field(v: &Value, name: &str) -> Value {
    match v {
        Value::Instance(i) => i.get_field(name).expect("field present"),
        other => panic!("expected MultipartPart instance, got {}", other.type_name()),
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.as_str().to_string(),
        Value::Bytes(b) => String::from_utf8(b.as_ref().clone()).expect("utf8 fixture"),
        other => panic!("unexpected {}", other.type_name()),
    }
}

const SMALL: &[u8] = b"--B\r\ncontent-disposition: form-data; name=\"note\"\r\n\r\nhello\r\n--B\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n\x01\x02\r\n--B--";

#[test]
fn multipart_small_inline_parts() {
    let parts = parse_multipart(SMALL, "B").expect("well-formed");
    assert_eq!(parts.len(), 2);
    assert_eq!(text(&field(&parts[0], "name")), "note");
    assert_eq!(text(&field(&parts[0], "fileName")), "");
    assert_eq!(text(&field(&parts[0], "content")), "hello");
    assert_eq!(text(&field(&parts[1], "name")), "avatar");
    assert_eq!(text(&field(&parts[1], "fileName")), "a.bin");
    assert_eq!(
        text(&field(&parts[1], "contentType")),
        "application/octet-stream"
    );
    match field(&parts[1], "content") {
        Value::Bytes(b) => assert_eq!(b.as_ref(), &vec![1u8, 2u8]),
        other => panic!("bytes expected, got {}", other.type_name()),
    }
}

#[test]
fn multipart_filename_is_never_misread_as_name() {
    // `filename="…"` contains the substring `name="…"` — the boundary guard in quoted_param keeps
    // a name-less part malformed instead of adopting its filename as the field name.
    let body = b"--B\r\nContent-Disposition: form-data; filename=\"evil.bin\"\r\n\r\nx\r\n--B--";
    assert!(parse_multipart(body, "B").is_none());
}

#[test]
fn multipart_malformed_shapes_are_none() {
    assert!(parse_multipart(b"no boundary here", "B").is_none());
    assert!(parse_multipart(
        b"--B\r\nContent-Disposition: form-data\r\n\r\nx\r\n--B--",
        "B"
    )
    .is_none()); // no name=
    assert!(parse_multipart(
        b"--B\r\nContent-Disposition: form-data; name=\"n\"\r\n\r\nunterminated",
        "B"
    )
    .is_none());
    assert!(parse_multipart(SMALL, "").is_none());
}

#[test]
fn multipart_part_count_over_cap_is_malformed() {
    let mut body = Vec::new();
    for i in 0..=MULTIPART_MAX_PARTS {
        body.extend_from_slice(
            format!("--B\r\nContent-Disposition: form-data; name=\"f{i}\"\r\n\r\nv\r\n").as_bytes(),
        );
    }
    body.extend_from_slice(b"--B--");
    assert!(parse_multipart(&body, "B").is_none());
    // Exactly at the cap stays well-formed.
    let mut ok = Vec::new();
    for i in 0..MULTIPART_MAX_PARTS {
        ok.extend_from_slice(
            format!("--B\r\nContent-Disposition: form-data; name=\"f{i}\"\r\n\r\nv\r\n").as_bytes(),
        );
    }
    ok.extend_from_slice(b"--B--");
    assert_eq!(
        parse_multipart(&ok, "B").expect("at cap").len(),
        MULTIPART_MAX_PARTS
    );
}

#[test]
fn stash_body_thresholds_and_spill_round_trip() {
    // -1 inline at/below the threshold; a real handle above it; -2 over the body cap.
    assert_eq!(
        super::native_stash_for_tests(&vec![0u8; SPILL_THRESHOLD]).expect("ok"),
        -1
    );
    let big = vec![0xABu8; SPILL_THRESHOLD + 1];
    let h1 = super::native_stash_for_tests(&big).expect("spill ok");
    let h2 = super::native_stash_for_tests(&vec![1u8; SPILL_THRESHOLD + 2]).expect("ok");
    assert!(h1 >= 0);
    assert_eq!(h2, h1 + 1, "handles are sequential per thread");
    assert_eq!(super::native_read_spill_for_tests(h1).expect("read"), big);
    assert!(super::native_read_spill_for_tests(9999).is_err());
    // The over-cap probe would allocate >8MiB — cheap enough, and it pins the -2 contract.
    assert_eq!(
        super::native_stash_for_tests(&vec![0u8; super::DEFAULT_MAX_BODY_SIZE + 1]).expect("ok"),
        -2
    );
}

#[test]
fn canonical_fault_strings_are_pinned_to_the_spec() {
    // Spec §5 fixes these exact strings at build (Invariant 4 single-sourcing). They become
    // runtime-reachable in slice 3's lazy mode; this pin keeps them from drifting until then.
    assert_eq!(
        super::FAULT_BODY_TOO_LARGE,
        "request body exceeds maxBodySize"
    );
    assert_eq!(super::FAULT_MALFORMED_MULTIPART, "malformed multipart body");
}

#[test]
fn rich_request_example_fixture_never_spills() {
    // Panel guardrail (Inv 10): the differential example must stay under the spill threshold so
    // the 3-leg run never touches the filesystem. Guarded here against fixture growth.
    let src = std::fs::read("examples/web/rich_request.phg").expect("example exists");
    assert!(
        src.len() < SPILL_THRESHOLD,
        "examples/web/rich_request.phg must stay below SPILL_THRESHOLD ({SPILL_THRESHOLD}B) so its \
         inline bodies can never spill"
    );
}
