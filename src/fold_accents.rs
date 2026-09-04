//! Accent folding — the pure table behind `Core.String.foldAccents` (DEC-468, shape ruled
//! 2026-09-04).
//!
//! Folds every accented Latin letter in **U+00C0–U+017F** (Latin-1 Supplement + Latin Extended-A)
//! to its unaccented ASCII base: `Crème Brûlée` → `Creme Brulee`, `Łódź` → `Lodz`, `Člověk` →
//! `Clovek`. That range is exactly the alphabet phorj's own six charsets can produce
//! ([`crate::charset`]), which is why the two ship together.
//!
//! **Output length may differ from input length.** Characters with no single-letter base EXPAND
//! (developer-ruled 2026-09-04, over a strip-only alternative that left `Straße` unchanged):
//! `ß`→`ss`, `æ`→`ae`, `œ`→`oe`, `Ĳ`→`IJ`, `þ`→`th`, plus the single-letter reassignments `ø`→`o`,
//! `ł`→`l`, `đ`→`d`, `ħ`→`h`, `ŋ`→`n`, `ŧ`→`t`, `ĸ`→`k`, `ŀ`→`l`, `ſ`→`s`. So this is NOT a
//! per-character map, and index arithmetic across a fold is wrong. Case is preserved — folding is
//! not lowercasing.
//!
//! **Provenance.** Every row whose character has a canonical decomposition was GENERATED from
//! Unicode NFD (decompose, drop the combining marks), not typed by hand; the rest are the expansion
//! list above, which is stated per character because no decomposition defines it. `tests` re-checks
//! a published sample plus the structural guarantees (sorted, unique, in range, ASCII output).
//!
//! NFD/full ICU normalisation stays out of scope by DEC-468 — that is `Core.Intl` (DEC-271).
//!
//! Read by BOTH legs: the native below and `transpile::fold_php`, which formats this same table
//! into the emitted `__phorj_fold_accents` helper, so they cannot drift.

/// `(character, its ASCII fold)` — sorted by character, unique keys, every value ASCII.
pub(crate) const FOLD: &[(char, &str)] = &[
    ('\u{00C0}', "A"),  // LATIN CAPITAL LETTER A WITH GRAVE → A
    ('\u{00C1}', "A"),  // LATIN CAPITAL LETTER A WITH ACUTE → A
    ('\u{00C2}', "A"),  // LATIN CAPITAL LETTER A WITH CIRCUMFLEX → A
    ('\u{00C3}', "A"),  // LATIN CAPITAL LETTER A WITH TILDE → A
    ('\u{00C4}', "A"),  // LATIN CAPITAL LETTER A WITH DIAERESIS → A
    ('\u{00C5}', "A"),  // LATIN CAPITAL LETTER A WITH RING ABOVE → A
    ('\u{00C6}', "AE"), // LATIN CAPITAL LETTER AE → AE
    ('\u{00C7}', "C"),  // LATIN CAPITAL LETTER C WITH CEDILLA → C
    ('\u{00C8}', "E"),  // LATIN CAPITAL LETTER E WITH GRAVE → E
    ('\u{00C9}', "E"),  // LATIN CAPITAL LETTER E WITH ACUTE → E
    ('\u{00CA}', "E"),  // LATIN CAPITAL LETTER E WITH CIRCUMFLEX → E
    ('\u{00CB}', "E"),  // LATIN CAPITAL LETTER E WITH DIAERESIS → E
    ('\u{00CC}', "I"),  // LATIN CAPITAL LETTER I WITH GRAVE → I
    ('\u{00CD}', "I"),  // LATIN CAPITAL LETTER I WITH ACUTE → I
    ('\u{00CE}', "I"),  // LATIN CAPITAL LETTER I WITH CIRCUMFLEX → I
    ('\u{00CF}', "I"),  // LATIN CAPITAL LETTER I WITH DIAERESIS → I
    ('\u{00D0}', "D"),  // LATIN CAPITAL LETTER ETH → D
    ('\u{00D1}', "N"),  // LATIN CAPITAL LETTER N WITH TILDE → N
    ('\u{00D2}', "O"),  // LATIN CAPITAL LETTER O WITH GRAVE → O
    ('\u{00D3}', "O"),  // LATIN CAPITAL LETTER O WITH ACUTE → O
    ('\u{00D4}', "O"),  // LATIN CAPITAL LETTER O WITH CIRCUMFLEX → O
    ('\u{00D5}', "O"),  // LATIN CAPITAL LETTER O WITH TILDE → O
    ('\u{00D6}', "O"),  // LATIN CAPITAL LETTER O WITH DIAERESIS → O
    ('\u{00D8}', "O"),  // LATIN CAPITAL LETTER O WITH STROKE → O
    ('\u{00D9}', "U"),  // LATIN CAPITAL LETTER U WITH GRAVE → U
    ('\u{00DA}', "U"),  // LATIN CAPITAL LETTER U WITH ACUTE → U
    ('\u{00DB}', "U"),  // LATIN CAPITAL LETTER U WITH CIRCUMFLEX → U
    ('\u{00DC}', "U"),  // LATIN CAPITAL LETTER U WITH DIAERESIS → U
    ('\u{00DD}', "Y"),  // LATIN CAPITAL LETTER Y WITH ACUTE → Y
    ('\u{00DE}', "Th"), // LATIN CAPITAL LETTER THORN → Th
    ('\u{00DF}', "ss"), // LATIN SMALL LETTER SHARP S → ss
    ('\u{00E0}', "a"),  // LATIN SMALL LETTER A WITH GRAVE → a
    ('\u{00E1}', "a"),  // LATIN SMALL LETTER A WITH ACUTE → a
    ('\u{00E2}', "a"),  // LATIN SMALL LETTER A WITH CIRCUMFLEX → a
    ('\u{00E3}', "a"),  // LATIN SMALL LETTER A WITH TILDE → a
    ('\u{00E4}', "a"),  // LATIN SMALL LETTER A WITH DIAERESIS → a
    ('\u{00E5}', "a"),  // LATIN SMALL LETTER A WITH RING ABOVE → a
    ('\u{00E6}', "ae"), // LATIN SMALL LETTER AE → ae
    ('\u{00E7}', "c"),  // LATIN SMALL LETTER C WITH CEDILLA → c
    ('\u{00E8}', "e"),  // LATIN SMALL LETTER E WITH GRAVE → e
    ('\u{00E9}', "e"),  // LATIN SMALL LETTER E WITH ACUTE → e
    ('\u{00EA}', "e"),  // LATIN SMALL LETTER E WITH CIRCUMFLEX → e
    ('\u{00EB}', "e"),  // LATIN SMALL LETTER E WITH DIAERESIS → e
    ('\u{00EC}', "i"),  // LATIN SMALL LETTER I WITH GRAVE → i
    ('\u{00ED}', "i"),  // LATIN SMALL LETTER I WITH ACUTE → i
    ('\u{00EE}', "i"),  // LATIN SMALL LETTER I WITH CIRCUMFLEX → i
    ('\u{00EF}', "i"),  // LATIN SMALL LETTER I WITH DIAERESIS → i
    ('\u{00F0}', "d"),  // LATIN SMALL LETTER ETH → d
    ('\u{00F1}', "n"),  // LATIN SMALL LETTER N WITH TILDE → n
    ('\u{00F2}', "o"),  // LATIN SMALL LETTER O WITH GRAVE → o
    ('\u{00F3}', "o"),  // LATIN SMALL LETTER O WITH ACUTE → o
    ('\u{00F4}', "o"),  // LATIN SMALL LETTER O WITH CIRCUMFLEX → o
    ('\u{00F5}', "o"),  // LATIN SMALL LETTER O WITH TILDE → o
    ('\u{00F6}', "o"),  // LATIN SMALL LETTER O WITH DIAERESIS → o
    ('\u{00F8}', "o"),  // LATIN SMALL LETTER O WITH STROKE → o
    ('\u{00F9}', "u"),  // LATIN SMALL LETTER U WITH GRAVE → u
    ('\u{00FA}', "u"),  // LATIN SMALL LETTER U WITH ACUTE → u
    ('\u{00FB}', "u"),  // LATIN SMALL LETTER U WITH CIRCUMFLEX → u
    ('\u{00FC}', "u"),  // LATIN SMALL LETTER U WITH DIAERESIS → u
    ('\u{00FD}', "y"),  // LATIN SMALL LETTER Y WITH ACUTE → y
    ('\u{00FE}', "th"), // LATIN SMALL LETTER THORN → th
    ('\u{00FF}', "y"),  // LATIN SMALL LETTER Y WITH DIAERESIS → y
    ('\u{0100}', "A"),  // LATIN CAPITAL LETTER A WITH MACRON → A
    ('\u{0101}', "a"),  // LATIN SMALL LETTER A WITH MACRON → a
    ('\u{0102}', "A"),  // LATIN CAPITAL LETTER A WITH BREVE → A
    ('\u{0103}', "a"),  // LATIN SMALL LETTER A WITH BREVE → a
    ('\u{0104}', "A"),  // LATIN CAPITAL LETTER A WITH OGONEK → A
    ('\u{0105}', "a"),  // LATIN SMALL LETTER A WITH OGONEK → a
    ('\u{0106}', "C"),  // LATIN CAPITAL LETTER C WITH ACUTE → C
    ('\u{0107}', "c"),  // LATIN SMALL LETTER C WITH ACUTE → c
    ('\u{0108}', "C"),  // LATIN CAPITAL LETTER C WITH CIRCUMFLEX → C
    ('\u{0109}', "c"),  // LATIN SMALL LETTER C WITH CIRCUMFLEX → c
    ('\u{010A}', "C"),  // LATIN CAPITAL LETTER C WITH DOT ABOVE → C
    ('\u{010B}', "c"),  // LATIN SMALL LETTER C WITH DOT ABOVE → c
    ('\u{010C}', "C"),  // LATIN CAPITAL LETTER C WITH CARON → C
    ('\u{010D}', "c"),  // LATIN SMALL LETTER C WITH CARON → c
    ('\u{010E}', "D"),  // LATIN CAPITAL LETTER D WITH CARON → D
    ('\u{010F}', "d"),  // LATIN SMALL LETTER D WITH CARON → d
    ('\u{0110}', "D"),  // LATIN CAPITAL LETTER D WITH STROKE → D
    ('\u{0111}', "d"),  // LATIN SMALL LETTER D WITH STROKE → d
    ('\u{0112}', "E"),  // LATIN CAPITAL LETTER E WITH MACRON → E
    ('\u{0113}', "e"),  // LATIN SMALL LETTER E WITH MACRON → e
    ('\u{0114}', "E"),  // LATIN CAPITAL LETTER E WITH BREVE → E
    ('\u{0115}', "e"),  // LATIN SMALL LETTER E WITH BREVE → e
    ('\u{0116}', "E"),  // LATIN CAPITAL LETTER E WITH DOT ABOVE → E
    ('\u{0117}', "e"),  // LATIN SMALL LETTER E WITH DOT ABOVE → e
    ('\u{0118}', "E"),  // LATIN CAPITAL LETTER E WITH OGONEK → E
    ('\u{0119}', "e"),  // LATIN SMALL LETTER E WITH OGONEK → e
    ('\u{011A}', "E"),  // LATIN CAPITAL LETTER E WITH CARON → E
    ('\u{011B}', "e"),  // LATIN SMALL LETTER E WITH CARON → e
    ('\u{011C}', "G"),  // LATIN CAPITAL LETTER G WITH CIRCUMFLEX → G
    ('\u{011D}', "g"),  // LATIN SMALL LETTER G WITH CIRCUMFLEX → g
    ('\u{011E}', "G"),  // LATIN CAPITAL LETTER G WITH BREVE → G
    ('\u{011F}', "g"),  // LATIN SMALL LETTER G WITH BREVE → g
    ('\u{0120}', "G"),  // LATIN CAPITAL LETTER G WITH DOT ABOVE → G
    ('\u{0121}', "g"),  // LATIN SMALL LETTER G WITH DOT ABOVE → g
    ('\u{0122}', "G"),  // LATIN CAPITAL LETTER G WITH CEDILLA → G
    ('\u{0123}', "g"),  // LATIN SMALL LETTER G WITH CEDILLA → g
    ('\u{0124}', "H"),  // LATIN CAPITAL LETTER H WITH CIRCUMFLEX → H
    ('\u{0125}', "h"),  // LATIN SMALL LETTER H WITH CIRCUMFLEX → h
    ('\u{0126}', "H"),  // LATIN CAPITAL LETTER H WITH STROKE → H
    ('\u{0127}', "h"),  // LATIN SMALL LETTER H WITH STROKE → h
    ('\u{0128}', "I"),  // LATIN CAPITAL LETTER I WITH TILDE → I
    ('\u{0129}', "i"),  // LATIN SMALL LETTER I WITH TILDE → i
    ('\u{012A}', "I"),  // LATIN CAPITAL LETTER I WITH MACRON → I
    ('\u{012B}', "i"),  // LATIN SMALL LETTER I WITH MACRON → i
    ('\u{012C}', "I"),  // LATIN CAPITAL LETTER I WITH BREVE → I
    ('\u{012D}', "i"),  // LATIN SMALL LETTER I WITH BREVE → i
    ('\u{012E}', "I"),  // LATIN CAPITAL LETTER I WITH OGONEK → I
    ('\u{012F}', "i"),  // LATIN SMALL LETTER I WITH OGONEK → i
    ('\u{0130}', "I"),  // LATIN CAPITAL LETTER I WITH DOT ABOVE → I
    ('\u{0131}', "i"),  // LATIN SMALL LETTER DOTLESS I → i
    ('\u{0132}', "IJ"), // LATIN CAPITAL LIGATURE IJ → IJ
    ('\u{0133}', "ij"), // LATIN SMALL LIGATURE IJ → ij
    ('\u{0134}', "J"),  // LATIN CAPITAL LETTER J WITH CIRCUMFLEX → J
    ('\u{0135}', "j"),  // LATIN SMALL LETTER J WITH CIRCUMFLEX → j
    ('\u{0136}', "K"),  // LATIN CAPITAL LETTER K WITH CEDILLA → K
    ('\u{0137}', "k"),  // LATIN SMALL LETTER K WITH CEDILLA → k
    ('\u{0138}', "k"),  // LATIN SMALL LETTER KRA → k
    ('\u{0139}', "L"),  // LATIN CAPITAL LETTER L WITH ACUTE → L
    ('\u{013A}', "l"),  // LATIN SMALL LETTER L WITH ACUTE → l
    ('\u{013B}', "L"),  // LATIN CAPITAL LETTER L WITH CEDILLA → L
    ('\u{013C}', "l"),  // LATIN SMALL LETTER L WITH CEDILLA → l
    ('\u{013D}', "L"),  // LATIN CAPITAL LETTER L WITH CARON → L
    ('\u{013E}', "l"),  // LATIN SMALL LETTER L WITH CARON → l
    ('\u{013F}', "L"),  // LATIN CAPITAL LETTER L WITH MIDDLE DOT → L
    ('\u{0140}', "l"),  // LATIN SMALL LETTER L WITH MIDDLE DOT → l
    ('\u{0141}', "L"),  // LATIN CAPITAL LETTER L WITH STROKE → L
    ('\u{0142}', "l"),  // LATIN SMALL LETTER L WITH STROKE → l
    ('\u{0143}', "N"),  // LATIN CAPITAL LETTER N WITH ACUTE → N
    ('\u{0144}', "n"),  // LATIN SMALL LETTER N WITH ACUTE → n
    ('\u{0145}', "N"),  // LATIN CAPITAL LETTER N WITH CEDILLA → N
    ('\u{0146}', "n"),  // LATIN SMALL LETTER N WITH CEDILLA → n
    ('\u{0147}', "N"),  // LATIN CAPITAL LETTER N WITH CARON → N
    ('\u{0148}', "n"),  // LATIN SMALL LETTER N WITH CARON → n
    ('\u{0149}', "'n"), // LATIN SMALL LETTER N PRECEDED BY APOSTROPHE → 'n
    ('\u{014A}', "N"),  // LATIN CAPITAL LETTER ENG → N
    ('\u{014B}', "n"),  // LATIN SMALL LETTER ENG → n
    ('\u{014C}', "O"),  // LATIN CAPITAL LETTER O WITH MACRON → O
    ('\u{014D}', "o"),  // LATIN SMALL LETTER O WITH MACRON → o
    ('\u{014E}', "O"),  // LATIN CAPITAL LETTER O WITH BREVE → O
    ('\u{014F}', "o"),  // LATIN SMALL LETTER O WITH BREVE → o
    ('\u{0150}', "O"),  // LATIN CAPITAL LETTER O WITH DOUBLE ACUTE → O
    ('\u{0151}', "o"),  // LATIN SMALL LETTER O WITH DOUBLE ACUTE → o
    ('\u{0152}', "OE"), // LATIN CAPITAL LIGATURE OE → OE
    ('\u{0153}', "oe"), // LATIN SMALL LIGATURE OE → oe
    ('\u{0154}', "R"),  // LATIN CAPITAL LETTER R WITH ACUTE → R
    ('\u{0155}', "r"),  // LATIN SMALL LETTER R WITH ACUTE → r
    ('\u{0156}', "R"),  // LATIN CAPITAL LETTER R WITH CEDILLA → R
    ('\u{0157}', "r"),  // LATIN SMALL LETTER R WITH CEDILLA → r
    ('\u{0158}', "R"),  // LATIN CAPITAL LETTER R WITH CARON → R
    ('\u{0159}', "r"),  // LATIN SMALL LETTER R WITH CARON → r
    ('\u{015A}', "S"),  // LATIN CAPITAL LETTER S WITH ACUTE → S
    ('\u{015B}', "s"),  // LATIN SMALL LETTER S WITH ACUTE → s
    ('\u{015C}', "S"),  // LATIN CAPITAL LETTER S WITH CIRCUMFLEX → S
    ('\u{015D}', "s"),  // LATIN SMALL LETTER S WITH CIRCUMFLEX → s
    ('\u{015E}', "S"),  // LATIN CAPITAL LETTER S WITH CEDILLA → S
    ('\u{015F}', "s"),  // LATIN SMALL LETTER S WITH CEDILLA → s
    ('\u{0160}', "S"),  // LATIN CAPITAL LETTER S WITH CARON → S
    ('\u{0161}', "s"),  // LATIN SMALL LETTER S WITH CARON → s
    ('\u{0162}', "T"),  // LATIN CAPITAL LETTER T WITH CEDILLA → T
    ('\u{0163}', "t"),  // LATIN SMALL LETTER T WITH CEDILLA → t
    ('\u{0164}', "T"),  // LATIN CAPITAL LETTER T WITH CARON → T
    ('\u{0165}', "t"),  // LATIN SMALL LETTER T WITH CARON → t
    ('\u{0166}', "T"),  // LATIN CAPITAL LETTER T WITH STROKE → T
    ('\u{0167}', "t"),  // LATIN SMALL LETTER T WITH STROKE → t
    ('\u{0168}', "U"),  // LATIN CAPITAL LETTER U WITH TILDE → U
    ('\u{0169}', "u"),  // LATIN SMALL LETTER U WITH TILDE → u
    ('\u{016A}', "U"),  // LATIN CAPITAL LETTER U WITH MACRON → U
    ('\u{016B}', "u"),  // LATIN SMALL LETTER U WITH MACRON → u
    ('\u{016C}', "U"),  // LATIN CAPITAL LETTER U WITH BREVE → U
    ('\u{016D}', "u"),  // LATIN SMALL LETTER U WITH BREVE → u
    ('\u{016E}', "U"),  // LATIN CAPITAL LETTER U WITH RING ABOVE → U
    ('\u{016F}', "u"),  // LATIN SMALL LETTER U WITH RING ABOVE → u
    ('\u{0170}', "U"),  // LATIN CAPITAL LETTER U WITH DOUBLE ACUTE → U
    ('\u{0171}', "u"),  // LATIN SMALL LETTER U WITH DOUBLE ACUTE → u
    ('\u{0172}', "U"),  // LATIN CAPITAL LETTER U WITH OGONEK → U
    ('\u{0173}', "u"),  // LATIN SMALL LETTER U WITH OGONEK → u
    ('\u{0174}', "W"),  // LATIN CAPITAL LETTER W WITH CIRCUMFLEX → W
    ('\u{0175}', "w"),  // LATIN SMALL LETTER W WITH CIRCUMFLEX → w
    ('\u{0176}', "Y"),  // LATIN CAPITAL LETTER Y WITH CIRCUMFLEX → Y
    ('\u{0177}', "y"),  // LATIN SMALL LETTER Y WITH CIRCUMFLEX → y
    ('\u{0178}', "Y"),  // LATIN CAPITAL LETTER Y WITH DIAERESIS → Y
    ('\u{0179}', "Z"),  // LATIN CAPITAL LETTER Z WITH ACUTE → Z
    ('\u{017A}', "z"),  // LATIN SMALL LETTER Z WITH ACUTE → z
    ('\u{017B}', "Z"),  // LATIN CAPITAL LETTER Z WITH DOT ABOVE → Z
    ('\u{017C}', "z"),  // LATIN SMALL LETTER Z WITH DOT ABOVE → z
    ('\u{017D}', "Z"),  // LATIN CAPITAL LETTER Z WITH CARON → Z
    ('\u{017E}', "z"),  // LATIN SMALL LETTER Z WITH CARON → z
    ('\u{017F}', "s"),  // LATIN SMALL LETTER LONG S → s
];

/// Fold every accented Latin letter in `s` to its ASCII base, leaving everything else untouched.
/// Total: there is no input this rejects.
pub fn fold_accents(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match FOLD.binary_search_by(|(k, _)| k.cmp(&c)) {
            Ok(i) => out.push_str(FOLD[i].1),
            Err(_) => out.push(c),
        }
    }
    out
}
