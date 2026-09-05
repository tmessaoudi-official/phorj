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

// ── Lane R-5: defaults, `@`, interfaces, root-qualified parents, `1_000_000` ──────────────────────

#[test]
fn parameter_defaults_lift_on_functions_and_promoted_constructor_parameters() {
    let out = lift(
        "<?php\nfinal class S { public function __construct(public int $tier = 0) {} }\nfunction f(int $n = 3, string $s = \"x\"): int { return $n; }",
    );
    assert!(out.contains("int n = 3, string s = \"x\""), "{out}");
    assert!(out.contains("int tier = 0"), "{out}");
    assert_reparses(&out);
}

/// `@f(x)` → `f(x)`: phorj natives raise no PHP warnings, so the silence operator has nothing to
/// silence on the phorj side. The call itself must survive.
#[test]
fn the_silence_operator_is_dropped_and_the_call_survives() {
    let out = lift("<?php function f(int $a, int $b): int { return @intdiv($a, $b); }");
    assert!(!out.contains('@'), "{out}");
    assert!(out.contains("integerDivide(b)"), "{out}");
    let err = super::lifter::lift_source("<?php $x = `ls`;").expect_err("backticks");
    assert!(err.contains("backtick"), "{err}");
}

#[test]
fn interfaces_lift_with_docblock_typed_methods_and_root_qualified_parents() {
    let out = lift(
        "<?php\nnamespace App;\ninterface HttpClient extends \\App\\Contracts\\Base\n{\n    /** @param list<string> $headers */\n    public function get(string $url, array $headers): string;\n}",
    );
    assert!(out.contains("interface HttpClient extends Base {"), "{out}");
    assert!(
        out.contains("function get(string url, List<string> headers): string;"),
        "{out}"
    );
    assert!(out.contains("import App.Contracts.Base;"), "{out}");
    assert_reparses(&out);
    let err = super::lifter::lift_source("<?php interface I { const X = 1; }")
        .expect_err("interface const");
    assert!(err.contains("constant"), "{err}");
}

/// A NON-exception root-qualified parent — `\RuntimeException` would pass through the DEC-421 map
/// and prove nothing about the `extends` path.
#[test]
fn a_root_qualified_parent_class_becomes_an_implicit_import() {
    let out = lift("<?php\nnamespace App;\nfinal class E extends \\App\\Core\\Base implements \\App\\Core\\Marker {}");
    assert!(
        out.contains("class E extends Base implements Marker"),
        "{out}"
    );
    assert!(out.contains("import App.Core.Base;"), "{out}");
    assert!(out.contains("import App.Core.Marker;"), "{out}");
}

#[test]
fn numeric_literal_separators_are_accepted() {
    let out = lift("<?php function f(): int { return 1_000_000 + 2_5; }");
    assert!(out.contains("1000000 + 25"), "{out}");
}

// ── Lane R-6: the empty literal gets its type from the program's own declarations ────────────────

/// `/** @var list<int> $xs */ $xs = [];` — the docblock on the LOCAL (54 in scout).
#[test]
fn a_var_docblock_types_an_empty_local_literal() {
    let out = lift(
        "<?php\nfunction f(): int {\n    /** @var list<int> $xs */\n    $xs = [];\n    $xs[] = 1;\n    return count($xs);\n}",
    );
    assert!(out.contains("mutable var xs = new List<int>();"), "{out}");
    assert_reparses(&out);
}

/// `$out = []; …; return $out;` under `@return array<string, int>` — the builder idiom, typed from
/// the declared return. Both the function and the method site run the pass.
#[test]
fn a_returned_empty_literal_is_typed_from_the_declared_return() {
    let out = lift(
        "<?php\n/** @return array<string, int> */\nfunction f(): array {\n    $out = [];\n    $out['a'] = 1;\n    return $out;\n}\nfinal class C {\n    /** @return list<string> */\n    public function g(): array { $xs = []; return $xs; }\n}",
    );
    assert!(
        out.contains("mutable var out = new Map<string, int>();"),
        "{out}"
    );
    assert!(out.contains("function f(): Map<string, int>"), "{out}");
    assert!(
        out.contains("mutable var xs = new List<string>();"),
        "{out}"
    );
    assert_reparses(&out);
}

/// No declaration to read from → the literal is left exactly as written (the checker asks for the
/// type, as before); nothing is inferred from the elements.
#[test]
fn an_untyped_empty_literal_is_left_alone() {
    let out = lift("<?php function f(): int { $xs = []; $xs[] = 1; return count($xs); }");
    assert!(out.contains("mutable var xs = [];"), "{out}");
}

// ── Lane R-7: `self`, field initializers, `instanceof \A\B` ──────────────────────────────────────

/// `: self` (31 uses in 10 scout files) is the enclosing class, exactly; `static` stays refused.
#[test]
fn self_in_a_class_body_is_the_enclosing_class() {
    let out = lift(
        "<?php\nfinal class Money {\n    public function __construct(public int $cents) {}\n    public function plus(self $other): self { return new Money($this->cents + $other->cents); }\n    /** @return list<self> */\n    public function halves(): array { return [$this, $this]; }\n}",
    );
    assert!(out.contains("function plus(Money other): Money"), "{out}");
    assert!(out.contains("halves(): List<Money>"), "{out}");
    assert_reparses(&out);
    let err = super::lifter::lift_source(
        "<?php final class A { public function me(): static { return $this; } }",
    )
    .expect_err("static return");
    assert!(err.contains("static"), "{err}");
}

/// A property default is a phorj field initializer — the "needs constructor synthesis" refusal
/// predated `guide/field-init.phg`.
#[test]
fn a_property_default_lifts_to_a_field_initializer() {
    let out = lift("<?php final class Counter { private int $n = 0; public function bump(): int { $this->n++; return $this->n; } }");
    assert!(out.contains("private mutable int n = 0;"), "{out}");
    assert_reparses(&out);
}

#[test]
fn instanceof_accepts_a_root_qualified_class() {
    let out = lift("<?php\nnamespace App;\nfunction f(Base $o): bool { return $o instanceof \\App\\Core\\Money; }");
    assert!(out.contains("o instanceof Money"), "{out}");
    assert!(out.contains("import App.Core.Money;"), "{out}");
}

/// A root-qualified name pointing INTO the file's own namespace is not an import. `\App\Scorer`
/// inside `namespace App` is how PHP writes "the Scorer of this very namespace"; lifting it to
/// `import App.Scorer;` made the package import itself, and the draft failed to check with
/// `E-MODULE-NOT-FOUND` — a message that reads as a missing dependency rather than a self-reference.
#[test]
fn a_root_qualified_name_in_the_files_own_namespace_is_not_imported() {
    let out = lift(
        "<?php\nnamespace App;\ninterface Scorer {}\nfunction f(Scorer $s): bool { return $s instanceof \\App\\Scorer; }",
    );
    assert!(out.contains("s instanceof Scorer"), "{out}");
    assert!(
        !out.contains("import App.Scorer;"),
        "self-import emitted:\n{out}"
    );
    // A DIFFERENT namespace under the same root still imports — the check is the full prefix, not
    // the first segment.
    let other = lift(
        "<?php\nnamespace App;\nfunction f(Base $o): bool { return $o instanceof \\App\\Core\\Money; }",
    );
    assert!(other.contains("import App.Core.Money;"), "{other}");
}

/// Top-level statements share ONE already-declared set, because they all land in one synthesized
/// `main`. Until 2026-09-05 each statement got a fresh set, so a second assignment re-DECLARED the
/// variable (`mutable var x = x + 2;`) and the draft failed to check with `E-SHADOW-LOCAL`. The bug
/// could only bite at file scope — a function body always threaded one set — which is precisely
/// where a PHP script does most of its assigning.
#[test]
fn a_reassignment_at_file_scope_is_an_assignment_not_a_redeclaration() {
    let out = lift("<?php\n$x = 1;\n$x = $x + 2;\n$y = $x;\n$y = $y * 2;\n");
    assert_eq!(out.matches("mutable var x").count(), 1, "{out}");
    assert_eq!(out.matches("mutable var y").count(), 1, "{out}");
    assert!(out.contains("    x = x + 2;"), "{out}");
    assert!(out.contains("    y = y * 2;"), "{out}");
    assert_reparses(&out);
    // The same shape inside a function was always correct — it stays correct.
    let f = lift("<?php function f(): int { $x = 1; $x = $x + 2; return $x; }");
    assert_eq!(f.matches("mutable var x").count(), 1, "{f}");
}

/// The PHP static-factory idiom — `public static function at(…): self { return new self(…); }` —
/// is 18 `E-OPEN-STATIC` plus 17 `E-NEW-ON-NONCONSTRUCT` across scout, and both halves are one
/// shape. A static method is never `open` (phorj rejects the pair outright, and PHP's late static
/// binding is not method overriding), and `new self` names the enclosing class.
#[test]
fn a_static_factory_lifts_without_open_and_constructs_its_own_class() {
    let out = lift(
        "<?php\nfinal class ConfigError {\n    public function __construct(public string $msg) {}\n    public static function at(string $p, string $e): self { return new self($p . \": \" . $e); }\n    public function widen(): self { return new self($this->msg); }\n}",
    );
    assert!(out.contains("public static function at("), "{out}");
    assert!(
        !out.contains("open static"),
        "a static method must not be open:\n{out}"
    );
    assert_eq!(out.matches("new ConfigError(").count(), 2, "{out}");
    assert!(!out.contains("new self"), "{out}");
    // A non-static method keeps `open` — PHP methods are overridable by default.
    assert!(
        out.contains("public open function widen(): ConfigError"),
        "{out}"
    );
    assert_reparses(&out);
    // Late static binding stays refused: `static` is the RECEIVER's class, which the enclosing name
    // would narrow.
    let err = super::lifter::lift_source(
        "<?php final class A { public static function make(): A { return new static(); } }",
    )
    .expect_err("new static");
    assert!(err.contains("static"), "{err}");
}

/// The third `self` site: `self::CONST` and `self::method()`. 411 occurrences in scout's source —
/// the draft used to say `self::X`, and phorj reported `unknown identifier self`.
#[test]
fn self_in_static_access_position_is_the_enclosing_class() {
    let out = lift(
        "<?php\nfinal class Limits {\n    public const int MAX = 10;\n    public static function cap(): int { return self::MAX; }\n    public function twice(): int { return self::cap() * 2; }\n}",
    );
    assert!(out.contains("Limits::MAX"), "{out}");
    assert!(out.contains("Limits::cap()"), "{out}");
    assert!(!out.contains("self::"), "{out}");
    assert_reparses(&out);
    let err = super::lifter::lift_source(
        "<?php final class A { public const int N = 1; public static function n(): int { return static::N; } }",
    )
    .expect_err("static::");
    assert!(err.contains("late static binding"), "{err}");
}
