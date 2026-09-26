//! PHP-lift parser — docblock array SHAPES `array{…}` (split from `doc_types.rs`, row 4e, 2026-09-26).
//!
//! A positional shape is a phorj tuple (DEC-288), a keyed one a named-field tuple (DEC-504) whose
//! field names come from the docblock and are never invented (DEC-166).

use super::doc_types::Cursor;
use super::*;

impl PParser {
    /// An array SHAPE, `{` already eaten. Two very different things wear one syntax:
    ///
    /// * **positional** — `array{int, string}`, or explicit ascending indices from zero,
    ///   `array{0: float, 1: float}` (the form PHPStan and Psalm emit). That is a phorj TUPLE.
    /// * **keyed** — `array{tenure: Tenure, source: string}`. That is a NAMED-FIELD tuple (DEC-504),
    ///   and the docblock's own keys become the field names. The lifter never invents a name
    ///   (DEC-166): a shape that is partly keyed, or whose key is not a legal phorj field name, is
    ///   refused rather than half-named or mangled.
    ///
    /// Indices that are not a dense ascending run from zero (`array{1: int, 0: string}`) are NOT
    /// silently reordered into a tuple — reordering would be the lifter guessing at intent, which
    /// DEC-166 forbids.
    pub(super) fn doc_shape(&mut self, c: &mut Cursor, name: &str) -> Result<PhpType, String> {
        let mut elems: Vec<PhpType> = Vec::new();
        let mut indices: Vec<Option<String>> = Vec::new();
        loop {
            c.skip_ws();
            if c.eat('}') {
                break;
            }
            // A leading `<ident>:`, `<digits>:` or `'quoted':` is a key; without one the element is
            // positional. The quoted form is read HERE rather than left to fail as a malformed type,
            // so `array{'total-cost': int}` is refused as an illegal FIELD NAME — which is the real
            // problem — instead of as a broken docblock.
            let save = c.i;
            c.skip_ws();
            let key = match c.peek() {
                Some(q @ ('\'' | '"')) => c.quoted(q),
                _ => c.ident(),
            };
            c.skip_ws();
            let key = if !key.is_empty() && c.eat(':') {
                Some(key)
            } else {
                c.i = save;
                None
            };
            indices.push(key);
            elems.push(self.doc_type(c)?);
            c.skip_ws();
            if !c.eat(',') {
                if !c.eat('}') {
                    return Err(self.err(&format!("`}}` to close the array shape `{name}{{…}}`")));
                }
                break;
            }
        }
        if elems.is_empty() {
            return Err(self.err(&format!("a non-empty array shape `{name}{{…}}`")));
        }
        let positional = indices.iter().enumerate().all(|(i, k)| match k {
            None => true,
            Some(k) => k.parse::<usize>() == Ok(i),
        });
        if positional {
            return Ok(PhpType::Tuple(elems, None));
        }
        // NUMERIC indices that are not a dense ascending run from zero (`array{1: int, 0: string}`)
        // are a POSITIONAL shape written wrong, not a named one. Refused as the reordering hazard it
        // is — a digit is never a field name, so falling through to the keyed path below would
        // report it as an illegal identifier and hide the real mistake.
        if indices
            .iter()
            .any(|k| k.as_deref().is_some_and(|k| k.parse::<usize>().is_ok()))
        {
            return Err(self.err(&format!(
                "the array shape `{name}{{…}}` to index its fields in ascending order from zero — \
                 out-of-order or sparse numeric indices are not silently reordered into a positional \
                 tuple, because reordering would be the lifter guessing at intent (DEC-166)"
            )));
        }
        // KEYED (DEC-504). Every element must carry a key — a half-keyed shape would leave the rest
        // to be named by position, which is the lifter inventing names (DEC-166).
        let mut names = Vec::with_capacity(indices.len());
        for k in &indices {
            let Some(k) = k else {
                return Err(self.err(&format!(
                    "every field of the KEYED array shape `{name}{{…}}` to have a key — a shape that \
                     names only some of its fields would leave the lifter to name the rest, which it \
                     never does (DEC-166); key every field, or make the shape positional"
                )));
            };
            if !is_field_name(k) {
                return Err(self.err(&format!(
                    "the array-shape key `{k}` to be a legal phorj field name (a letter or `_`, then \
                     letters, digits or `_`) — it is not, and the lifter renames nothing (DEC-166)"
                )));
            }
            names.push(k.clone());
        }
        if let Some(d) = first_duplicate(&names) {
            return Err(self.err(&format!(
                "the array shape `{name}{{…}}` not to repeat the key `{d}` — a tuple's field names \
                 are unique (E-TUPLE-DUP-FIELD)"
            )));
        }
        Ok(PhpType::Tuple(elems, Some(names)))
    }
}

/// Whether `s` is a legal phorj field name: a letter or `_`, then letters, digits or `_`.
///
/// PHP array keys are arbitrary strings (`'total-cost'`, `'2fa'`, `''`), phorj field names are not.
/// A key that does not fit is REFUSED rather than mangled into one — renaming it would make the
/// lifted code disagree with the PHP it came from at every read site (DEC-166).
fn is_field_name(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The first name that appears more than once, if any.
fn first_duplicate(names: &[String]) -> Option<&String> {
    names
        .iter()
        .enumerate()
        .find(|(i, n)| names[..*i].contains(n))
        .map(|(_, n)| n)
}
