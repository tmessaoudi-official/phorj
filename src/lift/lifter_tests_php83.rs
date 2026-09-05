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

/// Lane R — arrow closures, the dominant closure shape in real code (`static fn` in 25 of scout's
/// 120 files). `static` is a no-op, the enclosing locals are captured by value on both sides so
/// nothing is written for it, types and the return type travel, and the draft reparses.
#[test]
fn arrow_closures_lift_to_lambdas() {
    let out = lift(
        "<?php\nfunction main(): void {\n    $k = 3;\n    $f = static fn (int $v): int => $v * $k;\n    $g = fn (string $s): bool => strlen($s) > $k;\n    echo $f(2);\n}",
    );
    assert!(out.contains("function(int v): int => v * k"), "{out}");
    assert!(out.contains("function(string s): bool =>"), "{out}");
    assert_reparses(&out);
}

/// A block-bodied `function (…) { … }` closure is still refused: its body is a statement list and
/// its `use (…)` list can capture by reference, neither of which has a faithful draft yet.
#[test]
fn a_block_closure_stays_tier_2() {
    let err = super::lifter::lift_source(
        "<?php function main(): void { $f = function (int $x): int { return $x; }; }",
    )
    .expect_err("block closures are refused in this slice");
    assert!(err.contains("Tier-2"), "{err}");
    assert!(err.contains("fn (…) => …"), "{err}");
}

// ── Lane R-3: the three walls after closures, in scout file-count order ──────────────────────────

/// `$xs[] = v` (38 of scout's 120 files) → `xs = List.append(xs, v)` with `Core.List` imported.
#[test]
fn array_append_lifts_to_list_append_with_the_import() {
    let out = lift(
        "<?php\nfunction main(): void {\n    $parts = [];\n    $parts[] = 1;\n    echo count($parts);\n}",
    );
    assert!(out.contains("parts = List.append(parts, 1);"), "{out}");
    assert!(out.contains("import Core.List;"), "{out}");
    assert_reparses(&out);
    let err = super::lifter::lift_source("<?php function main(): void { $n = $xs[]; }")
        .expect_err("an append slot as an rvalue");
    assert!(err.contains("target of `=`"), "{err}");
}

/// `(float) $x` (38 files) → `x as float`; `(array)` stays Tier-2 by name.
#[test]
fn primitive_casts_lift_to_as() {
    let out =
        lift("<?php function f(int $n, string $s): float { return (float) $n / 2 + (int) $s; }");
    assert!(out.contains("n as float"), "{out}");
    assert!(out.contains("s as int"), "{out}");
    assert_reparses(&out);
    let err = super::lifter::lift_source("<?php function f($a): int { return count((array) $a); }")
        .expect_err("an (array) cast");
    assert!(err.contains("(array)"), "{err}");
}

/// `\A\B\C::m()` inline (16 files) → `C.m()` plus an implicit `import A.B.C`; `\strtoupper` just
/// loses its root marker; an explicit `use` of the same path is not duplicated.
#[test]
fn root_qualified_names_become_implicit_imports() {
    let out = lift(
        "<?php\nnamespace App;\nuse Scout\\Rent\\Config\\Other;\nfunction f(string $c): string { return \\Scout\\Rent\\Config\\Criteria::communeKey($c) . \\strtoupper(Other::NAME); }",
    );
    assert!(out.contains("import Scout.Rent.Config.Criteria;"), "{out}");
    // DEC-207: the written `::` separator is kept on the phorj side.
    assert!(out.contains("Criteria::communeKey(c)"), "{out}");
    assert_eq!(
        out.matches("import Scout.Rent.Config.Other;").count(),
        1,
        "{out}"
    );
    assert!(!out.contains('\\'), "{out}");
    assert_reparses(&out);
}

#[test]
fn a_closure_typed_property_is_refused_by_name() {
    let err = super::lifter::lift_source("<?php final class A { private \\Closure $f; }")
        .expect_err("a bare Closure type");
    assert!(err.contains("Closure"), "{err}");
}

// ── Lane R-4: docblock generics type the bare `array` (60 + 42 of scout's 120 files) ────────────

#[test]
fn docblock_generics_type_a_bare_array_parameter_and_return() {
    let out = lift(
        "<?php\n/**\n * @param list<string> $xs\n * @return array<string, int>\n */\nfunction f(array $xs): array { return []; }",
    );
    assert!(
        out.contains("function f(List<string> xs): Map<string, int>"),
        "{out}"
    );
    assert_reparses(&out);
}

#[test]
fn docblock_generics_cover_methods_properties_and_nullable_returns() {
    let out = lift(
        "<?php\nfinal class A {\n    /** @var list<int> */\n    private array $xs;\n    /** @return list<int> */\n    public function all(): ?array { return $this->xs; }\n}",
    );
    assert!(out.contains("List<int> xs"), "{out}");
    assert!(out.contains("all(): List<int>?"), "{out}");
    assert_reparses(&out);
}

/// The substitution guard: only a declared `array` is replaced — a refinement on any other type is
/// left exactly as declared.
#[test]
fn a_docblock_refinement_on_a_non_array_type_is_left_alone() {
    let out = lift(
        "<?php\n/** @param non-empty-string $s */\nfunction f(string $s): string { return $s; }",
    );
    assert!(out.contains("f(string s): string"), "{out}");
}

#[test]
fn bare_array_and_array_shapes_stay_refused_by_name() {
    let err = super::lifter::lift_source("<?php function f(array $xs): int { return 0; }")
        .expect_err("a bare array");
    assert!(err.contains("@param"), "{err}");
    let err = super::lifter::lift_source(
        "<?php\n/** @return array{a: int} */\nfunction f(): array { return []; }",
    )
    .expect_err("an array shape");
    assert!(err.contains("shape"), "{err}");
}

#[test]
fn a_root_qualified_class_in_a_docblock_generic_is_imported() {
    let out = lift(
        "<?php\nnamespace App;\n/** @return list<\\Scout\\Rent\\Core\\RawListing> */\nfunction f(): array { return []; }",
    );
    assert!(out.contains("import Scout.Rent.Core.RawListing;"), "{out}");
    assert!(out.contains("f(): List<RawListing>"), "{out}");
}

/// PHP 8.0 named arguments were read only inside attributes; scout writes them in `new` (6 files).
#[test]
fn named_arguments_lift_in_calls_and_construction() {
    let out = lift(
        "<?php\nfinal class S { public function __construct(public int $tier, public string $mode) {} }\nfunction f(): S { return new S(tier: 0, mode: \"a\"); }",
    );
    assert!(out.contains("new S(tier: 0, mode: \"a\")"), "{out}");
    assert_reparses(&out);
}
