//! `Core.String.foldAccents` — the registry row for the accent-folding table in
//! [`crate::fold_accents`] (DEC-468; shape ruled 2026-09-04).
//!
//! **Why this row lives here rather than in `text_registry.rs` with the rest of `Core.String`.**
//! That file is 600 lines — twice Invariant 13's soft cap — and is frozen at its own line count in
//! `scripts/size-baseline.txt`, so the ratchet rejects any growth: "split it, do not grow it". The
//! honest split of `text_registry.rs` is real owed work and is tracked as such; adding one more row
//! to it in the meantime is exactly the regrowth the ratchet exists to stop. `registry_modules.rs`
//! is the un-grandfathered seam, so the row hangs off it as a child module.

use crate::native::*;
use crate::types::Ty;
use crate::value::Value;

fn fold_accents_native(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(crate::fold_accents::fold_accents(s).into())),
        _ => Err("String.foldAccents expects (string)".into()),
    }
}

pub(super) fn fold_natives() -> Vec<NativeFn> {
    vec![NativeFn {
        module: "Core.String",
        name: "foldAccents",
        params: vec![Ty::String],
        ret: Ty::String,
        pure: true,
        eval: NativeEval::Pure(fold_accents_native),
        // No PHP function to lift FROM: `iconv('UTF-8','ASCII//TRANSLIT',$s)` is the closest and it
        // is both an ini extension and locale-dependent (its output differs across glibc/musl), so
        // it is neither tier-1 nor byte-identical. DEC-468 names the `__phorj_fold_accents` helper
        // for exactly that reason.
        lift_from: &[],
        php: |a| format!("__phorj_fold_accents({})", parg(a, 0)),
    }]
}

#[cfg(test)]
mod tests {
    use crate::fold_accents::{fold_accents, FOLD};

    /// The structural guarantees the codec and the emitted PHP both rely on. `fold_accents` uses a
    /// BINARY SEARCH, so "sorted" is not cosmetic — an unsorted table silently returns wrong folds
    /// for some characters and correct ones for others.
    #[test]
    fn the_table_is_sorted_unique_in_range_and_ascii_valued() {
        assert_eq!(FOLD.len(), 190, "the ruled range is U+00C0..=U+017F");
        for w in FOLD.windows(2) {
            assert!(
                w[0].0 < w[1].0,
                "table must be sorted and unique for binary_search: {:?} !< {:?}",
                w[0].0,
                w[1].0
            );
        }
        for (k, v) in FOLD {
            assert!(
                ('\u{00C0}'..='\u{017F}').contains(k),
                "{k:?} is outside the ruled range"
            );
            assert!(
                !v.is_empty(),
                "{k:?} folds to nothing — that would DELETE input"
            );
            assert!(v.is_ascii(), "{k:?} folds to {v:?}, which is not ASCII");
        }
    }

    /// A published sample, written out by hand. The bulk of the table was generated from Unicode
    /// NFD, so this is the check that the generation itself did the right thing.
    #[test]
    fn published_folds_are_correct() {
        for (input, want) in [
            ("Crème Brûlée", "Creme Brulee"),
            ("ÉCOLE Élève", "ECOLE Eleve"),
            ("Łódź", "Lodz"),
            ("Člověk žije", "Clovek zije"),
            ("Ångström", "Angstrom"),
            ("piñata", "pinata"),
            ("Français garçon", "Francais garcon"),
            ("Gdańsk", "Gdansk"),
            ("İstanbul", "Istanbul"),
            ("Ćevapčići", "Cevapcici"),
        ] {
            assert_eq!(fold_accents(input), want, "folding {input:?}");
        }
    }

    /// The expansions are the user-visible half of the 2026-09-04 ruling: a character with no
    /// single-letter base becomes MORE than one character, and case is preserved rather than
    /// title-cased.
    #[test]
    fn characters_with_no_single_letter_base_expand_preserving_case() {
        for (input, want) in [
            ("Straße", "Strasse"),
            ("ÆON æon", "AEON aeon"),
            ("Œuvre œuvre", "OEuvre oeuvre"),
            ("Þór þór", "Thor thor"),
            ("Ĳsland ĳs", "IJsland ijs"),
            ("Øystein Ærø", "Oystein AEro"),
            ("ſtraße", "strasse"),
        ] {
            assert_eq!(fold_accents(input), want, "expanding {input:?}");
        }
        // The length change is the contract's sharp edge — pinned so nobody "optimises" the fold
        // into a per-character map later.
        assert_eq!("Straße".chars().count(), 6);
        assert_eq!(fold_accents("Straße").chars().count(), 7);
    }

    /// Out-of-range text must survive untouched: folding is not a general normaliser, and mangling
    /// Greek or CJK would be a silent data loss in exactly the slug/search-key use it exists for.
    #[test]
    fn everything_outside_the_range_passes_through_unchanged() {
        for s in [
            "plain ASCII 123",
            "Ωμέγα 東京",
            "",
            "\u{1F600}",
            "already-folded",
        ] {
            assert_eq!(fold_accents(s), s, "{s:?} must be untouched");
        }
    }

    /// Folding is idempotent — its own output contains nothing left to fold. This is what makes it
    /// safe as a search-key or slug function, where the same value is folded repeatedly.
    #[test]
    fn folding_is_idempotent() {
        for s in ["Crème Brûlée", "Straße", "Łódź", "Ĳsland", "Þór"] {
            let once = fold_accents(s);
            assert_eq!(fold_accents(&once), once, "second fold of {s:?} changed it");
        }
        // Stronger: every fold output is already ASCII, so nothing in it can be in the table.
        for (k, _) in FOLD {
            assert!(fold_accents(&k.to_string()).is_ascii());
        }
    }

    /// The native wrapper itself — arity and value shape, not just the kernel.
    #[test]
    fn the_native_wraps_the_kernel() {
        use crate::value::Value;
        let out = super::fold_accents_native(&[Value::Str("Crème".into())], &mut String::new());
        assert!(matches!(out, Ok(Value::Str(s)) if s == "Creme"));
        assert!(super::fold_accents_native(&[], &mut String::new()).is_err());
    }
}
