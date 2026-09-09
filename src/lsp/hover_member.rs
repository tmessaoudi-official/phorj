//! Hover for a NAMED-TUPLE FIELD (DEC-504) — the one member access whose type the editor can
//! resolve without a nominal declaration to jump to.
//!
//! Hover otherwise resolves the identifier under the cursor to a DECLARATION, and a tuple field is
//! not one: `t.bp` answered `null`, and where an unrelated local happened to share the field's
//! name it answered with THAT local's type instead — a wrong answer presented as a right one.
//! Keying the lookup on the RECEIVER is what rules the impostor out.

use crate::ast::Program;

/// `field: type` for a `<receiver>.<name>` whose receiver is a named tuple, else `None` (so hover
/// falls through to its ordinary declaration lookup).
pub(super) fn tuple_field_signature(
    text: &str,
    offset: usize,
    name: &str,
    program: &Program,
) -> Option<String> {
    let receiver = receiver_before_field(text, offset)?;
    let fields = super::scope::receiver_tuple_fields(program, offset, &receiver)?;
    let (field, ty) = fields.into_iter().find(|(f, _)| f == name)?;
    Some(format!("{field}: {ty}"))
}

/// The identifier that precedes the `.` in front of the identifier under `offset`.
///
/// Byte indexing is safe here because every byte it walks over is ASCII — an identifier character
/// or ASCII whitespace — so `k` and `end` always land on char boundaries.
fn receiver_before_field(text: &str, offset: usize) -> Option<String> {
    let b = text.as_bytes();
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = offset.min(b.len());
    while i > 0 && is_ident(b[i - 1]) {
        i -= 1;
    }
    let skip_ws = |b: &[u8], mut n: usize| {
        while n > 0 && b[n - 1].is_ascii_whitespace() {
            n -= 1;
        }
        n
    };
    let j = skip_ws(b, i);
    if j == 0 || b[j - 1] != b'.' {
        return None;
    }
    // `?.` is refused on a tuple (E-TUPLE-SAFE-FIELD), but it still READS as a field access, so the
    // optional `?` is stepped over and hovering one shows the type while the diagnostic explains the
    // refusal. Without this step the walk stops ON the `?`, finds no receiver, and falls through to
    // the declaration lookup — the same-named-local decoy this module exists to rule out.
    let mut k = skip_ws(b, j - 1);
    if k > 0 && b[k - 1] == b'?' {
        k = skip_ws(b, k - 1);
    }
    let end = k;
    while k > 0 && is_ident(b[k - 1]) {
        k -= 1;
    }
    (k < end).then(|| text[k..end].to_string())
}
