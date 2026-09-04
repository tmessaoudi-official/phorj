//! `String.soundex` — the phonetic key PHP's `soundex()` produces, byte-for-byte.
//!
//! A faithful port rather than a fresh implementation, because "soundex" names a FAMILY of
//! algorithms that disagree: the original Russell/Odell rule collapses letters separated by `h`/`w`
//! (giving `Ashcraft` → `A261`), and PHP's does not (`A226`). Matching PHP is the whole point here —
//! this is a parity row, so the reference is `soundex()` and not the textbook.
//!
//! Byte-oriented like PHP's, which matters for two behaviours the fixtures pin: a non-ASCII byte is
//! simply skipped (`éclair` → `C460`, keying on `clair`), and an input with no ASCII letter at all
//! yields `"0000"` rather than an empty string — including for the empty string itself.
//!
//! No UTF-8 hazard: every output character is ASCII by construction, so unlike `wordWrap` this needs
//! no codepoint adaptation and transpiles straight to core `soundex()` (ladder case 1).

use crate::native::*;
use crate::types::Ty;
use crate::value::Value;

/// PHP's table, indexed by `letter - 'A'`; `0` means "carries no code" (vowels, H, W, Y).
const SOUNDEX_TABLE: [u8; 26] = [
    0, b'1', b'2', b'3', 0, b'1', b'2', 0, 0, b'2', b'2', b'4', b'5', b'5', 0, b'1', b'2', b'6',
    b'2', b'3', 0, b'1', 0, b'2', 0, b'2',
];

pub(crate) fn soundex(s: &str) -> String {
    let mut out = String::with_capacity(4);
    let mut last: Option<u8> = None;
    for &b in s.as_bytes() {
        let c = b.to_ascii_uppercase();
        if !c.is_ascii_uppercase() {
            continue;
        }
        let code = SOUNDEX_TABLE[usize::from(c - b'A')];
        if out.is_empty() {
            // The first LETTER is kept verbatim; its code seeds the run-suppression state, which is
            // why `Lloyd` is `L300` and not `L430` — the second `l` repeats the first's code.
            out.push(c as char);
            last = Some(code);
        } else if Some(code) != last {
            if code != 0 {
                out.push(code as char);
            }
            // `last` updates even for a zero code, so a vowel SEPARATES two same-coded consonants
            // (`Tymczak` → `T522`); without this the pair would collapse.
            last = Some(code);
        }
        if out.len() == 4 {
            break;
        }
    }
    while out.len() < 4 {
        out.push('0');
    }
    out
}

fn text_soundex(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(soundex(s).into())),
        _ => Err("String.soundex expects (string)".into()),
    }
}

pub(super) fn soundex_natives() -> Vec<NativeFn> {
    vec![NativeFn {
        module: "Core.String",
        name: "soundex",
        params: vec![Ty::String],
        ret: Ty::String,
        pure: true,
        eval: NativeEval::Pure(text_soundex),
        lift_from: &["soundex"],
        php: |a| format!("soundex({})", parg(a, 0)),
    }]
}

#[cfg(test)]
mod tests {
    use super::soundex;

    /// Pinned to real `php -n` output (php-8.5.9), captured BEFORE this port was written — so the
    /// test compares against PHP, not against my own reading of the algorithm.
    #[test]
    fn matches_php_byte_for_byte() {
        for (input, want) in [
            // Classic pairs that must key alike — the property soundex exists for.
            ("Robert", "R163"),
            ("Rupert", "R163"),
            ("Ashcraft", "A226"),
            ("Ashcroft", "A226"),
            ("Euler", "E460"),
            ("Ellery", "E460"),
            ("Gauss", "G200"),
            ("Ghosh", "G200"),
            ("Hilbert", "H416"),
            ("Heilbronn", "H416"),
            ("Knuth", "K530"),
            ("Kant", "K530"),
            ("Lloyd", "L300"),
            ("Ladd", "L300"),
            ("Lukasiewicz", "L222"),
            ("Lissajous", "L222"),
            // Behaviours that separate PHP's variant from the textbook one.
            ("Tymczak", "T522"),
            ("Pfister", "P236"),
            ("Honeyman", "H555"),
            // Degenerate and non-ASCII input.
            ("", "0000"),
            ("123", "0000"),
            ("x", "X000"),
            ("  hello  ", "H400"),
            ("MacDonald", "M235"),
            ("van der Berg", "V536"),
            ("éclair", "C460"),
            ("O'Brien", "O165"),
        ] {
            assert_eq!(soundex(input), want, "soundex({input:?})");
        }
    }

    /// `Ashcraft` is the case that distinguishes PHP's soundex from the original Russell/Odell rule,
    /// which collapses letters separated by `h`/`w` and yields `A261`. Pinned so nobody "corrects"
    /// this into the textbook algorithm and silently breaks parity with PHP.
    #[test]
    fn php_does_not_apply_the_h_w_separator_rule() {
        assert_eq!(soundex("Ashcraft"), "A226");
        assert_ne!(soundex("Ashcraft"), "A261", "that is the OTHER soundex");
    }

    /// Structural guarantees every caller can rely on: always four characters, always ASCII, always
    /// a letter followed by three digits unless there was no letter at all.
    #[test]
    fn every_key_is_four_ascii_characters() {
        for s in [
            "Robert",
            "",
            "123",
            "éclair",
            "x",
            "aaaaaaaaaaaaaaaaaaaa",
            "Zzzzz",
        ] {
            let k = soundex(s);
            assert_eq!(k.len(), 4, "soundex({s:?}) = {k:?}");
            assert!(k.is_ascii(), "soundex({s:?}) = {k:?} is not ASCII");
            let rest_are_digits = k[1..].chars().all(|c| c.is_ascii_digit());
            assert!(rest_are_digits, "soundex({s:?}) = {k:?}");
        }
    }
}
