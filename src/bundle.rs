//! Self-contained-executable support (M2.5). Embed a `.phg` program as a `.phorge` section in the
//! phorge binary and detect+extract it at startup. std-only: the section reader is hand-rolled (no
//! `object`/`goblin` — that code runs *inside* the produced binary, so it must stay zero-dep).
//! Only the ELF arm is wired in Phase 1; PE/Mach-O are Phase 2. `unsafe` is forbidden crate-wide
//! (see `lib.rs`), so this module inherits that guarantee without restating it.

const MAGIC: [u8; 8] = *b"PHORGE\0\0";
const CONTAINER_VERSION: u16 = 1;
const HEADER_LEN: u16 = 32;
/// Section name carrying the payload container.
pub const SECTION_NAME: &str = ".phorge";

/// CRC-32 (IEEE 802.3, reflected, poly 0xEDB88320), bitwise — std-only, no static table.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Build the payload container for `source` (Phase 1: `payload_kind = 0`, source UTF-8).
/// Layout per design §3: magic | version | header_len | kind | comp | enc | flags | len |
/// payload_crc32 | header_crc32(over [0..28)) | payload.
pub fn encode_container(source: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN as usize + source.len());
    out.extend_from_slice(&MAGIC); // 0..8
    out.extend_from_slice(&CONTAINER_VERSION.to_le_bytes()); // 8..10
    out.extend_from_slice(&HEADER_LEN.to_le_bytes()); // 10..12
    out.push(0); // 12 payload_kind = source_utf8
    out.push(0); // 13 compression = none
    out.push(0); // 14 encryption = none
    out.push(0); // 15 flags
    out.extend_from_slice(&(source.len() as u64).to_le_bytes()); // 16..24
    out.extend_from_slice(&crc32(source).to_le_bytes()); // 24..28 payload_crc32
    let header_crc = crc32(&out[0..28]); // 28..32 header_crc32
    out.extend_from_slice(&header_crc.to_le_bytes());
    out.extend_from_slice(source); // 32..
    out
}

/// Validate + extract the source bytes from a container blob. Returns `None` on any malformed,
/// tampered, truncated, or unsupported-version/kind input — callers fall through to the CLI.
pub fn decode_container(blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < HEADER_LEN as usize || blob[0..8] != MAGIC {
        return None;
    }
    if u16::from_le_bytes([blob[8], blob[9]]) > CONTAINER_VERSION {
        return None; // artifact built for a newer phorge
    }
    let header_len = u16::from_le_bytes([blob[10], blob[11]]) as usize;
    if header_len < HEADER_LEN as usize || header_len > blob.len() {
        return None;
    }
    let header_crc = u32::from_le_bytes([blob[28], blob[29], blob[30], blob[31]]);
    if crc32(&blob[0..28]) != header_crc {
        return None; // can't trust payload_len from a corrupt header
    }
    if blob[12] != 0 {
        return None; // only source_utf8 in Phase 1
    }
    let payload_len = u64::from_le_bytes(blob[16..24].try_into().ok()?) as usize;
    let payload_crc = u32::from_le_bytes([blob[24], blob[25], blob[26], blob[27]]);
    let end = header_len.checked_add(payload_len)?;
    if end > blob.len() {
        return None;
    }
    let payload = &blob[header_len..end];
    if crc32(payload) != payload_crc {
        return None;
    }
    Some(payload.to_vec())
}

/// Find a named section's bytes in an ELF64 little-endian image (the only Phase-1 format). Returns
/// `None` on any malformed/unsupported input (too short, not ELF, 32-bit, big-endian, OOB offset).
/// Hand-rolled — no object-parsing crate may link into the produced binary.
fn elf_find_section<'a>(bytes: &'a [u8], name: &str) -> Option<&'a [u8]> {
    // e_ident: 0x7f 'E' 'L' 'F', EI_CLASS=2 (ELF64), EI_DATA=1 (little-endian).
    if bytes.len() < 64 || bytes[0..4] != *b"\x7fELF" || bytes[4] != 2 || bytes[5] != 1 {
        return None;
    }
    // ALL offset arithmetic is checked. `e_shoff` is read as a full u64 cast to usize, so a crafted
    // header can drive a derived offset to usize::MAX; a plain `+` would overflow-panic under the
    // debug/test profile (overflow-checks on). Adversarial input must return None, never panic (EV-7).
    let u16at = |o: usize| -> Option<u16> {
        Some(u16::from_le_bytes(
            bytes.get(o..o.checked_add(2)?)?.try_into().ok()?,
        ))
    };
    let u32at = |o: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(o..o.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let u64at = |o: usize| -> Option<u64> {
        Some(u64::from_le_bytes(
            bytes.get(o..o.checked_add(8)?)?.try_into().ok()?,
        ))
    };

    let e_shoff = u64at(0x28)? as usize; // section header table file offset
    let e_shentsize = u16at(0x3A)? as usize; // per-entry size (64 for ELF64)
    let e_shnum = u16at(0x3C)? as usize;
    let e_shstrndx = u16at(0x3E)? as usize;
    if e_shentsize < 64 {
        return None;
    }

    // Section-name string table (the section header at index e_shstrndx).
    let strtab_hdr = e_shoff.checked_add(e_shstrndx.checked_mul(e_shentsize)?)?;
    let strtab_off = u64at(strtab_hdr.checked_add(24)?)? as usize; // sh_offset
    let strtab_size = u64at(strtab_hdr.checked_add(32)?)? as usize; // sh_size
    let strtab = bytes.get(strtab_off..strtab_off.checked_add(strtab_size)?)?;

    for i in 0..e_shnum {
        let sh = e_shoff.checked_add(i.checked_mul(e_shentsize)?)?;
        let sh_name = u32at(sh)? as usize; // offset into strtab
        let rest = strtab.get(sh_name..)?;
        let nul = rest.iter().position(|&b| b == 0)?;
        if std::str::from_utf8(&rest[..nul]).ok()? == name {
            let off = u64at(sh.checked_add(24)?)? as usize; // sh_offset
            let sz = u64at(sh.checked_add(32)?)? as usize; // sh_size
            return bytes.get(off..off.checked_add(sz)?);
        }
    }
    None
}

/// If this executable carries an embedded `.phorge` payload, return its source. Any failure — no
/// payload, unreadable `current_exe`, malformed ELF, bad CRC — returns `None`, so the caller falls
/// through to normal CLI dispatch. Never panics.
pub fn embedded_source() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bytes = std::fs::read(exe).ok()?;
    let section = elf_find_section(&bytes, SECTION_NAME)?;
    let payload = decode_container(section)?;
    String::from_utf8(payload).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vector() {
        // Canonical CRC-32 of "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn container_round_trips() {
        let src = b"function main() { println(\"hi\"); }";
        let blob = encode_container(src);
        assert_eq!(decode_container(&blob).as_deref(), Some(&src[..]));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut blob = encode_container(b"x");
        blob[0] = b'Q';
        assert_eq!(decode_container(&blob), None);
    }

    #[test]
    fn rejects_tampered_payload() {
        let mut blob = encode_container(b"abcd");
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;
        assert_eq!(decode_container(&blob), None);
    }

    #[test]
    fn rejects_tampered_header() {
        let mut blob = encode_container(b"abcd");
        blob[16] ^= 0xFF; // corrupt payload_len -> header_crc mismatch
        assert_eq!(decode_container(&blob), None);
    }

    #[test]
    fn rejects_truncated() {
        let blob = encode_container(b"abcd");
        assert_eq!(decode_container(&blob[..20]), None);
        assert_eq!(decode_container(&[]), None);
    }

    #[test]
    fn rejects_future_version() {
        let mut blob = encode_container(b"abcd");
        blob[8] = 2; // container_version = 2
        blob[9] = 0;
        // header_crc now stale -> rejected (also future-version guard would catch it)
        assert_eq!(decode_container(&blob), None);
    }

    #[test]
    fn elf_reader_finds_added_section() {
        // Use the compiled test binary itself as a real ELF64 to objcopy into.
        let exe = std::env::current_exe().expect("current_exe");
        let tmp = std::env::temp_dir().join("phorge_bundle_reader_test");
        let payload = std::env::temp_dir().join("phorge_bundle_reader_payload");
        let src = b"function main() { println(\"x\"); }";
        std::fs::write(&payload, encode_container(src)).unwrap();
        let objcopy = std::env::var("PHORGE_OBJCOPY").unwrap_or_else(|_| "llvm-objcopy".into());
        let status = std::process::Command::new(&objcopy)
            .args([
                "--add-section",
                &format!(".phorge={}", payload.display()),
                "--set-section-flags",
                ".phorge=noload,readonly",
            ])
            .arg(&exe)
            .arg(&tmp)
            .status();
        let _ = std::fs::remove_file(&payload);
        match status {
            Ok(s) if s.success() => {}
            _ => {
                eprintln!("skipping: {objcopy} unavailable");
                let _ = std::fs::remove_file(&tmp);
                return;
            }
        }
        let bytes = std::fs::read(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        let section = elf_find_section(&bytes, SECTION_NAME).expect("section found");
        assert_eq!(decode_container(section).as_deref(), Some(&src[..]));
    }

    #[test]
    fn elf_reader_rejects_adversarial_offsets_without_panic() {
        // A 64-byte ELF64-LE header whose e_shoff drives derived section-header offsets to/near
        // usize::MAX. A plain `+` would overflow-panic under the debug profile (overflow-checks on,
        // the profile the quality gate runs); checked arithmetic must return None instead — the
        // function's "Never panics" contract and INVARIANTS EV-7 ("never SIGABRT/panic on input").
        fn header(e_shoff: u64, e_shentsize: u16, e_shnum: u16, e_shstrndx: u16) -> Vec<u8> {
            let mut h = vec![0u8; 64];
            h[0..4].copy_from_slice(b"\x7fELF");
            h[4] = 2; // EI_CLASS = ELF64
            h[5] = 1; // EI_DATA = little-endian
            h[0x28..0x30].copy_from_slice(&e_shoff.to_le_bytes());
            h[0x3A..0x3C].copy_from_slice(&e_shentsize.to_le_bytes());
            h[0x3C..0x3E].copy_from_slice(&e_shnum.to_le_bytes());
            h[0x3E..0x40].copy_from_slice(&e_shstrndx.to_le_bytes());
            h
        }
        // e_shoff = u64::MAX -> strtab_hdr = usize::MAX -> `strtab_hdr + 24` would overflow.
        assert_eq!(
            elf_find_section(&header(u64::MAX, 64, 0, 0), SECTION_NAME),
            None
        );
        // e_shoff = u64::MAX - 28 -> overflow surfaces inside the u64at closure (`o + 8`).
        assert_eq!(
            elf_find_section(&header(u64::MAX - 28, 64, 0, 0), SECTION_NAME),
            None
        );
        // e_shstrndx = 0xFFFF with a small base -> large in-range offset, OOB `.get()` -> None.
        assert_eq!(
            elf_find_section(&header(64, 64, 0, 0xFFFF), SECTION_NAME),
            None
        );
        // Sanity: a well-formed-looking but section-less header returns None, no panic.
        assert_eq!(elf_find_section(&header(64, 64, 0, 0), SECTION_NAME), None);
    }
}
