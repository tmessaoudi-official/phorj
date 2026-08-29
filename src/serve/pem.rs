//! PEM decoding for `phg serve`'s inbound TLS (S3.5) — splitting a PEM document into its
//! `(label, DER)` blocks, and the strict base64 that backs it.
//!
//! **Hand-rolled rather than a fifteenth external crate.** PEM is a base64 payload between two marker
//! lines; this is under a hundred lines, and admitting a dependency for it would need a developer
//! ruling under the dependency policy for no capability gain. Split out of `tls.rs` so that file
//! stays inside Invariant 13's 300-line soft cap, and because encoding has nothing to do with TLS
//! policy — the two change for entirely different reasons.
//!
//! **The safety property: malformed input yields FEWER blocks, never a wrong one.** A block whose
//! base64 does not decode is dropped rather than guessed at, and an END marker only closes a BEGIN
//! with the same label. `tls::build` turns an empty result into a startup error, so "this file is not
//! really a certificate" surfaces when the server starts rather than on every request afterwards.

/// Split a PEM document into `(label, DER)` pairs. Hand-rolled rather than pulling in a PEM crate:
/// the format is a base64 payload between two marker lines, this is forty lines, and a fifteenth
/// external dependency would need a developer ruling under the dependency policy for no gain.
///
/// Malformed input yields FEWER blocks, never a wrong one — a block whose base64 does not decode is
/// dropped, and [`build`] turns an empty result into a startup error rather than a silent success.
pub(super) fn pem_blocks(text: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut label: Option<String> = None;
    let mut payload = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("-----BEGIN ") {
            label = rest.strip_suffix("-----").map(str::to_string);
            payload.clear();
        } else if let Some(rest) = line.strip_prefix("-----END ") {
            let closing = rest.strip_suffix("-----").unwrap_or_default();
            // Only a matching END closes a block: a truncated file whose END belongs to a different
            // label must not be read as a complete one.
            if let Some(open) = label.take() {
                if open == closing {
                    if let Some(der) = b64(&payload) {
                        out.push((open, der));
                    }
                }
            }
            payload.clear();
        } else if label.is_some() {
            payload.push_str(line);
        }
    }
    out
}

/// Strict base64 over the standard alphabet, whitespace already stripped by the caller. Returns
/// `None` on any character outside the alphabet or a length that cannot be a valid encoding — so a
/// text file that merely looks like PEM cannot decode to arbitrary bytes.
fn b64(s: &str) -> Option<Vec<u8>> {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = s.as_bytes();
    // A base64 group is four characters, so a length that is not a multiple of four cannot be a
    // valid encoding however plausible the characters look. This is the only length check needed:
    // with it, each of the three legal padding shapes leaves a body of a consistent length.
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }
    let body = match bytes.strip_suffix(b"==") {
        Some(b) => b,
        None => bytes.strip_suffix(b"=").unwrap_or(bytes),
    };
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for &c in body {
        // `?` on the alphabet lookup is what rejects everything else: a stray `=` in the MIDDLE of
        // the payload, whitespace the caller failed to strip, or any other character. There is no
        // lenient path — a text file that merely looks like PEM must not decode to arbitrary bytes.
        let v = u32::try_from(A.iter().position(|&a| a == c)?).ok()?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xFF).ok()?);
        }
    }
    // No trailing check: `bits < 8` is a loop invariant (it is reduced the moment it reaches 8), and
    // the padding is 0, 1 or 2 by construction above. A guard for either would be unreachable code
    // dressed as caution.
    Some(out)
}

#[cfg(test)]
#[path = "pem_tests.rs"]
mod pem_tests;
