//! Charset transcoding kernel — DEC-468's surface, DEC-494's implementation strategy.
//!
//! A top-level shared leaf module like `phstr`/`json`, because THREE consumers read it: the
//! interpreter and the VM through `ext::encoding`'s natives, and `transpile::charset_php`, which
//! formats the tables below straight into the emitted `__phorj_cs_*` helper — one source, two legs,
//! no way for them to drift. It cannot live under `ext::encoding` (the transpiler is always
//! compiled and that module is not — `--no-default-features` caught exactly that), and it is not a
//! value kernel, so `src/value/` is the wrong shelf too.
//!
//! Six encodings: UTF-8, UTF-16 in both byte orders, ISO-8859-1 (Latin-1), ISO-8859-15 (Latin-9),
//! Windows-1252 and ASCII. Both directions are **total functions into an optional**: `None` when
//! the bytes are not valid in the source charset, or when a character has no representation in the
//! target. Nothing is replaced with U+FFFD or `?` — the loss is reported, never absorbed
//! (developer-ruled 2026-09-04, matching this module's own `base64Decode`/`hexDecode` convention).
//!
//! **DEC-494 — no crate, and no ini extension.** DEC-468 named `encoding_rs`; it was ruled out
//! because the PHP leg has no legal move with it: `mb_convert_encoding` and `iconv` are both shared
//! extensions, both absent under the oracle's `php -n`, and both rejected by the default-deny
//! `transpiled_examples_use_only_tier1_php_functions` guard. A native-only ladder tier would have
//! parked an exclusion at the exact moment DEC-493 forbade parks at the finish line. So both legs
//! are hand-rolled — and the tables below are the SINGLE SOURCE for both: `transpile::runtime_php`
//! formats these same consts into the emitted PHP helper, so the two legs cannot drift apart the
//! way two transcribed copies would.
//!
//! The tables are small because the encodings are near-identical: Latin-1 *is* the identity map
//! (code point == byte, by definition of ISO-8859-1), Latin-9 differs from it in eight positions,
//! and Windows-1252 differs from it only inside 0x80..=0x9F.

/// ISO-8859-15 (Latin-9) as a delta against Latin-1: exactly eight positions differ. Every other
/// byte decodes to itself, as in Latin-1.
pub(crate) const LATIN9_DIFF: &[(u8, u32)] = &[
    (0xA4, 0x20AC), // € EURO SIGN            (Latin-1: ¤ CURRENCY SIGN)
    (0xA6, 0x0160), // Š S WITH CARON         (Latin-1: ¦ BROKEN BAR)
    (0xA8, 0x0161), // š s with caron         (Latin-1: ¨ DIAERESIS)
    (0xB4, 0x017D), // Ž Z WITH CARON         (Latin-1: ´ ACUTE ACCENT)
    (0xB8, 0x017E), // ž z with caron         (Latin-1: ¸ CEDILLA)
    (0xBC, 0x0152), // Œ LIGATURE OE          (Latin-1: ¼ ONE QUARTER)
    (0xBD, 0x0153), // œ ligature oe          (Latin-1: ½ ONE HALF)
    (0xBE, 0x0178), // Ÿ Y WITH DIAERESIS     (Latin-1: ¾ THREE QUARTERS)
];

/// Windows-1252 as a delta against Latin-1: the C1 block 0x80..=0x9F carries printable characters
/// instead of control codes. Twenty-seven of the thirty-two are assigned; `0` marks the five that
/// the standard leaves **undefined** (0x81, 0x8D, 0x8F, 0x90, 0x9D), which decode to `None` rather
/// than to a substitute. Indexed by `byte - 0x80`.
pub(crate) const CP1252_C1: [u32; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, // 0x80..0x87
    0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017D, 0, // 0x88..0x8F
    0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, // 0x90..0x97
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178, // 0x98..0x9F
];

/// The six ruled encodings. Mirrors the injected `enum Charset` one variant for one variant; the
/// checker types the argument as `Charset`, so at runtime it is always a `Value::Enum`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Charset {
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
    Latin9,
    Windows1252,
    Ascii,
}

impl Charset {
    /// Project a `Charset` enum variant name onto the Rust codec. The variant set is closed by the
    /// injected prelude, so an unknown name is a bug in the injection, not user input.
    pub(crate) fn from_variant(v: &str) -> Option<Self> {
        Some(match v {
            "Utf8" => Self::Utf8,
            "Utf16Le" => Self::Utf16Le,
            "Utf16Be" => Self::Utf16Be,
            "Latin1" => Self::Latin1,
            "Latin9" => Self::Latin9,
            "Windows1252" => Self::Windows1252,
            "Ascii" => Self::Ascii,
            _ => return None,
        })
    }
}

/// One byte of a single-byte charset → its code point, or `None` where the standard leaves the
/// byte undefined. Only the four single-byte charsets reach here.
fn single_byte_cp(cs: Charset, b: u8) -> Option<u32> {
    match cs {
        Charset::Ascii => (b < 0x80).then_some(u32::from(b)),
        Charset::Latin1 => Some(u32::from(b)),
        Charset::Latin9 => Some(
            LATIN9_DIFF
                .iter()
                .find(|(k, _)| *k == b)
                .map_or(u32::from(b), |(_, cp)| *cp),
        ),
        Charset::Windows1252 => {
            if (0x80..=0x9F).contains(&b) {
                let cp = CP1252_C1[usize::from(b) - 0x80];
                (cp != 0).then_some(cp)
            } else {
                Some(u32::from(b))
            }
        }
        Charset::Utf8 | Charset::Utf16Le | Charset::Utf16Be => None,
    }
}

/// A code point → its byte in a single-byte charset, or `None` when the charset cannot represent
/// it. The inverse of [`single_byte_cp`], and deliberately written as a search over the same tables
/// rather than as a second hand-built table: one table, two directions, no way to disagree.
fn single_byte_byte(cs: Charset, cp: u32) -> Option<u8> {
    match cs {
        Charset::Ascii => (cp < 0x80).then_some(cp as u8),
        Charset::Latin1 => (cp < 0x100).then_some(cp as u8),
        Charset::Latin9 => {
            if let Some((b, _)) = LATIN9_DIFF.iter().find(|(_, c)| *c == cp) {
                return Some(*b);
            }
            // A byte whose Latin-9 meaning was reassigned is no longer reachable by its Latin-1
            // code point — `¤` (U+00A4) cannot be encoded in Latin-9 at all.
            if cp < 0x100 && !LATIN9_DIFF.iter().any(|(b, _)| u32::from(*b) == cp) {
                return Some(cp as u8);
            }
            None
        }
        Charset::Windows1252 => {
            if let Some(i) = CP1252_C1.iter().position(|c| *c == cp && cp != 0) {
                return Some(0x80 + i as u8);
            }
            // 0x80..=0x9F are the C1 controls in Latin-1; in Windows-1252 those code points are
            // simply not encodable, so they must not fall through to the identity map.
            if cp < 0x100 && !(0x80..=0x9F).contains(&cp) {
                return Some(cp as u8);
            }
            None
        }
        Charset::Utf8 | Charset::Utf16Le | Charset::Utf16Be => None,
    }
}

/// Decode a UTF-16 code-unit sequence, honouring surrogate pairs. `None` on an odd byte count, an
/// unpaired surrogate, or a low surrogate in lead position.
fn utf16_decode(bytes: &[u8], little: bool) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|p| {
            if little {
                u16::from_le_bytes([p[0], p[1]])
            } else {
                u16::from_be_bytes([p[0], p[1]])
            }
        })
        .collect();
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .ok()
}

/// Bytes in `cs` → a UTF-8 `string`, or `None` when they are not valid in that charset.
pub(crate) fn decode(bytes: &[u8], cs: Charset) -> Option<String> {
    match cs {
        // Already UTF-8: the only question is validity, and phorj's `string` is UTF-8 by
        // construction, so a successful conversion IS the decode.
        Charset::Utf8 => std::str::from_utf8(bytes).ok().map(str::to_owned),
        Charset::Utf16Le => utf16_decode(bytes, true),
        Charset::Utf16Be => utf16_decode(bytes, false),
        _ => {
            let mut out = String::with_capacity(bytes.len());
            for &b in bytes {
                out.push(char::from_u32(single_byte_cp(cs, b)?)?);
            }
            Some(out)
        }
    }
}

/// A UTF-8 `string` → bytes in `cs`, or `None` when some character has no representation there.
pub(crate) fn encode(s: &str, cs: Charset) -> Option<Vec<u8>> {
    match cs {
        Charset::Utf8 => Some(s.as_bytes().to_vec()),
        Charset::Utf16Le | Charset::Utf16Be => {
            let little = cs == Charset::Utf16Le;
            let mut out = Vec::with_capacity(s.len() * 2);
            for u in s.encode_utf16() {
                out.extend_from_slice(&if little {
                    u.to_le_bytes()
                } else {
                    u.to_be_bytes()
                });
            }
            Some(out)
        }
        _ => {
            let mut out = Vec::with_capacity(s.len());
            for c in s.chars() {
                out.push(single_byte_byte(cs, u32::from(c))?);
            }
            Some(out)
        }
    }
}
