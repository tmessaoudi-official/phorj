//! Lane R (2026-09-05) — the PHP 8.2/8.3 forms that refused EVERY real module of the scout app at
//! the lift parser: `readonly class` (69 of its 120 files) and typed class constants (33 files).
use super::lifter_tests::{assert_reparses, lift};

#[test]
fn a_readonly_class_lifts_with_immutable_fields_and_promoted_params() {
    let out = lift(
        "<?php\nfinal readonly class Money {\n    public int $cents;\n    public function __construct(public string $currency, private int $scale) { $this->cents = 0; }\n}\nfunction main(): void { }",
    );
    assert!(out.contains("class Money"), "{out}");
    // phorj fields are immutable by default: a readonly class writes NO `mutable` anywhere.
    assert!(
        !out.contains("mutable"),
        "a readonly class lifted a `mutable`:\n{out}"
    );
    assert!(out.contains("public string currency"), "{out}");
    assert_reparses(&out);
}

#[test]
fn a_readonly_promoted_parameter_is_retained_on_an_ordinary_class() {
    let out = lift(
        "<?php\nclass P {\n    public function __construct(public readonly int $id, public int $n) {}\n}\nfunction main(): void { }",
    );
    assert!(out.contains("public int id"), "{out}");
    assert!(out.contains("public mutable int n"), "{out}");
    assert!(!out.contains("mutable int id"), "{out}");
}

#[test]
fn a_typed_class_constant_lifts_with_its_declared_type() {
    let out = lift(
        "<?php\nfinal class Text {\n    private const string FOLD_LOWER = \"abc\";\n    public const int MAX = 3;\n    const UNTYPED = 1.5;\n}\nfunction main(): void { }",
    );
    assert!(
        out.contains("private const string FOLD_LOWER = \"abc\""),
        "{out}"
    );
    assert!(out.contains("public const int MAX = 3"), "{out}");
    // Untyped keeps inferring from the literal.
    assert!(out.contains("const float UNTYPED = 1.5"), "{out}");
    assert_reparses(&out);
}
