//! Lane L1 of the scout forcing function (2026-09-07, DEC-509) — PHP **enums**: a case is not a
//! class constant, `self` inside an enum body is the enclosing enum, and a method lowers to a free
//! function reachable by UFCS. Split from `lifter_tests_php83.rs` under Invariant 13.

use super::lifter_tests::{assert_reparses, lift};

/// `parse_enum` never set `current_class` and never ran `resolve_self`, so every `self::CASE` in an
/// enum method refused with *"`self` outside a class body"* — a message that reads like the file is
/// malformed rather than like a lifter gap. scout's `Rent/Core/Tenure.php:70` is the shape: an
/// `isExcluded()` whose `match` arms are all `self::CASE`, and it blocks the tenure-classifier
/// depth target. Three scout files stop here.
#[test]
fn self_inside_an_enum_body_is_the_enclosing_enum() {
    let out = lift(
        "<?php\nenum Tenure: string {\n    case LLI = 'LLI';\n    case PLAI = 'PLAI';\n    case UNKNOWN = 'UNKNOWN';\n    public function isExcluded(): bool {\n        return match ($this) {\n            self::PLAI => true,\n            default => false,\n        };\n    }\n}",
    );
    // `self::PLAI` in pattern position is the VARIANT pattern `PLAI()` — phorj's zero-payload
    // variant form. What matters is that `self` resolved and that the arm is a pattern, not the
    // Tier-2 refusal ("a `match` arm with a non-literal condition") it used to be.
    assert!(out.contains("PLAI() => true"), "{out}");
    assert!(
        !out.contains("self::"),
        "self survived into the draft:\n{out}"
    );
    assert_reparses(&out);
}

/// The type positions too — a `self` return or parameter on an enum method is the enum, exactly the
/// rule `selfref.rs` already applied to classes. Without this the enum half of R-7 stays missing
/// while the class half looks complete.
#[test]
fn self_in_an_enum_method_signature_is_the_enclosing_enum() {
    let out = lift(
        "<?php\nenum Tenure: string {\n    case LLI = 'LLI';\n    public function orSelf(self $other): self { return $other; }\n}",
    );
    // The lowering (DEC-509) prepends the receiver, so the signature reads
    // `orSelf(Tenure tenure, Tenure other)` — what this pins is that BOTH `self` positions became
    // `Tenure`, which is the R-7 rule reaching enum bodies at last.
    assert!(
        out.contains("orSelf(Tenure tenure, Tenure other): Tenure"),
        "{out}"
    );
    assert_reparses(&out);
}

/// `static` stays refused inside an enum for the same reason it is refused inside a class: late
/// static binding means the receiver's class, and the enclosing name would narrow it. A silent
/// narrowing is what Invariant 14 forbids.
#[test]
fn static_binding_inside_an_enum_stays_refused() {
    let err = super::lifter::lift_source(
        "<?php enum E: int { case A = 1; public static function first(): int { return static::A->value; } }",
    )
    .expect_err("static:: inside an enum");
    assert!(err.contains("late static binding"), "{err}");
}

/// `Tenure::PLAI` is an ENUM CASE, not a class constant, and the two are spelled identically in
/// PHP. The lifter emitted `Tenure::PLAI` — which is not phorj syntax at all, so the draft lifted
/// "successfully" and then failed `phg check` with `E-UNKNOWN-IDENT`. A payload-less variant is
/// constructed, like everything else in phorj (Invariant 12, mandatory `new`).
#[test]
fn an_enum_case_reference_becomes_a_variant_construction() {
    let out = lift(
        "<?php\nenum Tenure: string {\n    case LLI = 'LLI';\n    case PLAI = 'PLAI';\n}\nfinal class Uses {\n    public function pick(): Tenure { return Tenure::PLAI; }\n    public function isBad(Tenure $t): bool { return $t === Tenure::PLAI; }\n}",
    );
    assert!(out.contains("new PLAI()"), "{out}");
    assert!(
        !out.contains("Tenure::PLAI"),
        "an enum case still lifted as a class constant:\n{out}"
    );
    assert_reparses(&out);
}

/// A CLASS constant keeps `::` — the fix must not turn every static access into a construction.
#[test]
fn a_class_constant_still_lifts_as_static_access() {
    let out = lift(
        "<?php\nfinal class Limits {\n    public const int MAX = 10;\n    public static function cap(): int { return Limits::MAX; }\n}",
    );
    assert!(out.contains("Limits::MAX"), "{out}");
    assert_reparses(&out);
}

/// DEC-509 — a PHP enum METHOD has no phorj form (enums carry no methods). It lowers to a free
/// function taking the enum as its first parameter, and UFCS resolves the call sites unchanged, so
/// `$t->isExcluded()` reads as `t.isExcluded()` exactly as it did in PHP. scout's
/// `Rent/Core/Tenure.php` is the shape and it gates the tenure-classifier depth target.
#[test]
fn an_enum_method_lowers_to_a_free_function_reachable_by_ufcs() {
    let out = lift(
        "<?php\nenum Tenure: string {\n    case LLI = 'LLI';\n    case PLAI = 'PLAI';\n    public function isExcluded(): bool {\n        return match ($this) {\n            self::PLAI => true,\n            default => false,\n        };\n    }\n}",
    );
    assert!(
        out.contains("function isExcluded(Tenure "),
        "no free function was emitted:\n{out}"
    );
    assert!(!out.contains("self::"), "{out}");
    assert_reparses(&out);
}

/// The receiver: `$this` inside the lowered body is the new first parameter, not a dangling name.
#[test]
fn this_inside_a_lowered_enum_method_becomes_the_receiver_parameter() {
    let out = lift(
        "<?php\nenum Suit: string {\n    case H = 'H';\n    public function label(): string { return $this->value; }\n}",
    );
    assert!(out.contains("function label(Suit "), "{out}");
    assert!(
        !out.contains("this."),
        "`$this` survived into a free function:\n{out}"
    );
    assert_reparses(&out);
}

/// The collision DEC-509 names: two enums each with a `label()` would become two free functions of
/// one name in one package. That is refused loudly rather than mangled — the lifter never invents
/// a name (DEC-166).
#[test]
fn two_enum_methods_of_the_same_name_are_refused_not_mangled() {
    let err = super::lifter::lift_source(
        "<?php\nenum A: string { case X = 'x'; public function label(): string { return $this->value; } }\nenum B: string { case Y = 'y'; public function label(): string { return $this->value; } }",
    )
    .expect_err("colliding lowered enum methods");
    assert!(err.contains("label"), "{err}");
}

/// A STATIC enum method lowers to a free function, so its CALL SITE must lower too:
/// `Tenure::fallback()` is a call to that function, not a static access on the enum. Found by
/// RUNNING the Invariant-9 example rather than reading it — the draft lifted, checked and then
/// died on `Tenure::fallback()`, a name that does not exist.
#[test]
fn a_static_enum_method_call_lowers_to_a_plain_call() {
    let out = lift(
        "<?php\nenum Tenure: string {\n    case UNKNOWN = 'UNKNOWN';\n    public static function fallback(): self { return self::UNKNOWN; }\n}\nfunction pick(): Tenure { return Tenure::fallback(); }",
    );
    assert!(out.contains("return fallback()"), "{out}");
    assert!(
        !out.contains("Tenure::fallback"),
        "the call site still names the enum:\n{out}"
    );
    assert_reparses(&out);
}

/// An INSTANCE enum method call keeps its receiver and reads as UFCS — `$t->isExcluded()` is
/// `t.isExcluded()`, which resolves to the lowered free function's first parameter.
#[test]
fn an_instance_enum_method_call_reads_as_ufcs() {
    let out = lift(
        "<?php\nenum Tenure: string {\n    case PLAI = 'PLAI';\n    public function isExcluded(): bool { return $this === self::PLAI; }\n}\nfunction bad(Tenure $t): bool { return $t->isExcluded(); }",
    );
    assert!(out.contains("t.isExcluded()"), "{out}");
    assert_reparses(&out);
}

/// A STATIC method on an ordinary CLASS is untouched — the lowering must not turn every `::` call
/// into a free-function call.
#[test]
fn a_static_class_method_call_still_uses_static_access() {
    let out = lift(
        "<?php\nfinal class Limits {\n    public static function cap(): int { return 10; }\n}\nfunction c(): int { return Limits::cap(); }",
    );
    assert!(out.contains("Limits::cap()"), "{out}");
    assert_reparses(&out);
}
