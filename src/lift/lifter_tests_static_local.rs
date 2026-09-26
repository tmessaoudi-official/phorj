//! Row 5w — DEC-539. A PHP static local (`static $n = 0;`) persists across calls; phorj has none. It
//! is refused BY NAME, with its two rewrites (a private static field, or dropping the memo when the
//! value it caches is pure), instead of the generic `` `static` is not supported in Tier-1``. The
//! other spellings of `static` — a static property, `static fn` — take other paths and are untouched.

use super::lifter::lift_source;

fn refused(src: &str) -> String {
    match lift_source(src) {
        Err(e) => e,
        Ok(out) => panic!("expected a refusal, lifted:\n{out}"),
    }
}

#[test]
fn a_static_local_is_refused_by_name() {
    for src in [
        "<?php function counter(): int { static $n = 0; $n++; return $n; }",
        "<?php function f(): int { static $a, $b = 2; return 1; }",
        // Scout's one site (`TenureClassifier.php`) is a memo inside a METHOD body.
        "<?php class K { public static function keys(): int { static $keys = null; return 1; } }",
    ] {
        let e = refused(src);
        assert!(
            e.contains("static local")
                && e.contains("DEC-539")
                && e.contains("private static field"),
            "{src}\n{e}"
        );
    }
}

#[test]
fn a_static_property_and_a_static_closure_are_not_static_locals() {
    for src in [
        "<?php class C { private static int $n = 0; public function f(): int { return 1; } }",
        "<?php function g(): int { $f = static fn(int $x): int => $x; return $f(1); }",
    ] {
        if let Err(e) = lift_source(src) {
            panic!("{src}\nexpected to lift, refused: {e}");
        }
    }
}
