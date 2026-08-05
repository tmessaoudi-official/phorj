//! LIFT-ATTR tests — PHP 8 `#[…]` attributes → phorj `#[…]`.
//!
//! Why this slice exists: attributes were invisible to the lift. A bare `#` is a line comment in PHP,
//! so `#[ORM\Column]` was **silently eaten as a comment** — the single worst failure shape for a tool
//! whose contract is "refuse loudly, never guess", and the reason no Symfony/Laravel/Doctrine file
//! could be lifted with its meaning intact.
//!
//! The design pinned here is the NAME RESOLUTION. An attribute name is a CLASS name, so PHP resolves
//! it against the file's `use` map and `namespace` first; the lifted spelling is then the BARE leaf for
//! a class in this file's own package and the FULLY-QUALIFIED path for anything from elsewhere.
//!
//! Neither half is cosmetic. phorj recognizes a built-in attribute by segment-boundary SUFFIX match
//! (`attr_path_matches`), so a bare `#[Route]` lifted out of a Symfony controller would bind to phorj's
//! own `Core.Http.Route` and mean something completely different — the DEC-435 bug class, one layer up.
//! And a same-package class must NOT be qualified: a single-file compile keys classes bare, so
//! `#[App.Meta.Tag]` matches nothing there even though it is the "more precise" spelling.

use super::lifter::lift_source;

fn lift(php: &str) -> String {
    lift_source(php).expect("lift")
}

fn refusal(php: &str) -> String {
    lift_source(php).expect_err("this shape must be refused, not lifted")
}

/// Does the lifted draft actually pass `phg check`? Asserting on the emitted STRING alone has already
/// let two slices ship drafts that failed the very check they were supposed to pass (see the LIFT-NS
/// test header), so every "this lifts correctly" case below ends here.
fn checks_clean(phg: &str) {
    let prog =
        crate::cli::parse_program(phg).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{phg}"));
    crate::cli::check_and_expand(&prog, phg)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{phg}"));
}

/// A file declaring its own attribute class and using it — the shape every framework's *user* writes.
const DECLARE_AND_USE: &str = r#"<?php
#[\Attribute]
class Tag {
    public function __construct(public string $name) {}
}

#[Tag("cli")]
class Widget {
    public function label(): string { return "widget"; }
}

function main(): void { $w = new Widget(); echo $w->label(), "\n"; }
"#;

#[test]
fn the_php_attribute_marker_becomes_phorjs_canonical_path() {
    let out = lift(DECLARE_AND_USE);
    // The FULLY-QUALIFIED `Core.Runtime.Attribute`, not the bare `Attribute`: a bare injected type is
    // `E-INJECTED-TYPE-BARE` without a member import, while a dotted name is self-gating — so the
    // canonical form needs no synthesized import at all.
    assert!(out.contains("#[Core.Runtime.Attribute]"), "{out}");
    assert!(out.contains("#[Tag(\"cli\")]"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_bare_attribute_with_no_namespace_resolves_at_the_root() {
    // No `namespace`, no `use` → PHP resolves an unqualified name at the ROOT, so `#[Attribute]`
    // without the leading `\` is the same class as `#[\Attribute]`.
    let out = lift(
        DECLARE_AND_USE
            .replace("#[\\Attribute]", "#[Attribute]")
            .as_str(),
    );
    assert!(out.contains("#[Core.Runtime.Attribute]"), "{out}");
    checks_clean(&out);
}

/// A class in the file's OWN package keeps the bare leaf — and the reason is mechanical, not stylistic.
/// PHP resolves `#[Tag]` under `namespace App\Meta;` to `App\Meta\Tag`, but a single-file phorj compile
/// registers the class under the BARE key `Tag`, so the "more precise" `#[App.Meta.Tag]` matches nothing
/// and lands on `E-ATTR-TARGET`. The bare form matches both keyings (`attr_path_matches` accepts a
/// segment-boundary suffix), which is why it is the correct spelling rather than the lazy one.
#[test]
fn an_own_package_attribute_keeps_the_bare_leaf() {
    let php = format!(
        "<?php\nnamespace App\\Meta;\nuse Attribute;\n{}",
        &DECLARE_AND_USE[6..]
    );
    let out = lift(&php);
    assert!(out.contains("package App.Meta;"), "{out}");
    assert!(out.contains("#[Tag(\"cli\")]"), "{out}");
    assert!(
        !out.contains("#[App.Meta.Tag"),
        "a same-package attribute must not be qualified — it would not resolve:\n{out}"
    );
    // `use Attribute;` binds the ROOT `Attribute`, so the marker still maps onto phorj's own.
    assert!(out.contains("#[Core.Runtime.Attribute]"), "{out}");
    // …and the `use` it came from must NOT survive as an import. The unused-import probe matches on word
    // boundaries, and `.` is one — so it saw the `Attribute` inside `Core.Runtime.Attribute` and kept an
    // `import Attribute;` for a name the output no longer references at all. `phg check` did NOT catch
    // that (a one-segment import is accepted), so only this assertion does.
    assert!(
        !out.contains("import Attribute;"),
        "the `use` was consumed by the attribute remap — its import is dead:\n{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_use_alias_is_expanded_to_the_full_path() {
    // The Doctrine spelling. `ORM` is only an abbreviation for the namespace, so expanding it loses
    // nothing — and it is what keeps the leaf `Column` from being read as anything else.
    let php =
        "<?php\nnamespace App;\nuse Doctrine\\ORM\\Mapping as ORM;\n#[ORM\\Column]\nclass Row {}\n";
    let out = lift(php);
    assert!(out.contains("#[Doctrine.ORM.Mapping.Column]"), "{out}");
}

/// THE regression this slice's design exists for. A Symfony controller's `#[Route]` must NOT become
/// phorj's built-in `#[Route]`: they take different arguments and mean different things, and phorj's
/// suffix matching would accept the bare leaf silently.
#[test]
fn a_symfony_route_is_not_captured_by_phorjs_builtin_route() {
    let php = "<?php\nnamespace App;\nuse Symfony\\Component\\Routing\\Attribute\\Route;\n#[Route(\"/home\")]\nclass HomeController {}\n";
    let out = lift(php);
    assert!(
        out.contains("#[Symfony.Component.Routing.Attribute.Route(\"/home\")]"),
        "the attribute must keep its own identity:\n{out}"
    );
    assert!(
        !out.contains("#[Route("),
        "a bare `#[Route]` would bind to phorj's Core.Http.Route:\n{out}"
    );
}

/// The one case the qualified spelling cannot save: a NON-namespaced file whose attribute leaf is one
/// of phorj's built-ins. There is no longer path to emit, so the built-in would win the suffix match.
#[test]
fn a_root_level_name_colliding_with_a_builtin_attribute_is_refused() {
    let php = "<?php\n#[\\Attribute]\nclass Route { public function __construct(public string $p) {} }\n#[Route(\"/x\")]\nclass C {}\n";
    let err = refusal(php);
    assert!(
        err.contains("Route") && err.contains("built-in"),
        "the refusal must name the collision: {err}"
    );
}

#[test]
fn named_arguments_lift_one_to_one() {
    let php = r#"<?php
#[\Attribute]
class Tag {
    public function __construct(public string $name, public int $order) {}
}

#[Tag(order: 3, name: "late")]
class Widget {}

function main(): void { echo "ok\n"; }
"#;
    let out = lift(php);
    assert!(out.contains("#[Tag(order: 3, name: \"late\")]"), "{out}");
    // Order does not have to match the constructor — the checker normalizes named args into their
    // positional slots (DEC-297/DEC-435), which is exactly why the lifter must not reorder them.
    checks_clean(&out);
}

#[test]
fn a_function_attribute_lifts_too() {
    let php = r#"<?php
#[\Attribute]
class Audited { public function __construct(public bool $on) {} }

#[Audited(true)]
function work(): string { return "worked"; }

function main(): void { echo work(), "\n"; }
"#;
    let out = lift(php);
    assert!(out.contains("#[Audited(true)]"), "{out}");
    checks_clean(&out);
}

#[test]
fn several_attributes_in_one_group_all_survive() {
    // PHP allows `#[A, B]` in a single group; phorj writes one per line, so the group is flattened.
    let php = r#"<?php
#[\Attribute]
class A {}
#[\Attribute]
class B {}

#[A, B]
class Both {}

function main(): void { echo "ok\n"; }
"#;
    let out = lift(php);
    assert!(out.contains("#[A]") && out.contains("#[B]"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_trailing_comma_in_a_group_and_in_arguments_is_tolerated() {
    let php = r#"<?php
#[\Attribute]
class Tag { public function __construct(public string $name) {} }

#[Tag("x",),]
class Widget {}

function main(): void { echo "ok\n"; }
"#;
    let out = lift(php);
    assert!(out.contains("#[Tag(\"x\")]"), "{out}");
    checks_clean(&out);
}

/// A framework attribute's class lives in `vendor/`, so the lifted draft names something nothing
/// declares. The attribute is still emitted with its identity intact — and the draft SAYS so, instead of
/// leaving the reader to work it out from an `E-UNKNOWN-ATTRIBUTE` at the bottom of the file. Same
/// discipline `exceptions.rs` applies to unmapped exception classes.
#[test]
fn an_attribute_whose_class_is_not_in_the_file_gets_a_cannot_lift_note() {
    let php =
        "<?php\nnamespace App;\nuse Doctrine\\ORM\\Mapping as ORM;\n#[ORM\\Column]\nclass Row {}\n";
    let out = lift(php);
    assert!(
        out.contains("// CANNOT LIFT: attribute `#[Doctrine.ORM.Mapping.Column]`"),
        "the draft must say why it will not check:\n{out}"
    );
    // Named, not dropped: the attribute is still there.
    assert!(out.contains("#[Doctrine.ORM.Mapping.Column]"), "{out}");
}

#[test]
fn an_attribute_the_file_does_declare_gets_no_note() {
    // The note must not fire on the case that works, or it becomes noise nobody reads.
    let out = lift(DECLARE_AND_USE);
    assert!(!out.contains("CANNOT LIFT"), "{out}");
}

// ── refusals: positions phorj has no target for ─────────────────────────────────────────────────
//
// phorj accepts `#[…]` on a top-level `function` or `class` ONLY (`E-ATTR-TARGET`). Every other PHP
// position is refused with the position named, because DROPPING one is a silent semantic loss —
// `#[ORM\Column]` on a property *is* the meaning of that line.

#[test]
fn an_attribute_on_a_method_is_refused() {
    let err =
        refusal("<?php\nclass C {\n  #[Audited]\n  public function m(): int { return 1; }\n}\n");
    assert!(err.contains("class member"), "{err}");
}

#[test]
fn an_attribute_on_a_property_is_refused() {
    let err = refusal("<?php\nclass C {\n  #[ORM\\Column]\n  public int $id = 0;\n}\n");
    assert!(err.contains("class member"), "{err}");
}

#[test]
fn an_attribute_on_a_parameter_is_refused() {
    let err = refusal("<?php\nfunction f(#[Sensitive] string $secret): int { return 1; }\n");
    assert!(err.contains("a parameter"), "{err}");
}

#[test]
fn an_attribute_on_an_enum_is_refused() {
    let err = refusal("<?php\n#[Flagged]\nenum Color { case Red; }\n");
    assert!(err.contains("top-level `function` or `class`"), "{err}");
}

#[test]
fn an_attribute_on_an_enum_case_is_refused() {
    let err = refusal("<?php\nenum Color {\n  #[Flagged]\n  case Red;\n}\n");
    assert!(err.contains("enum case"), "{err}");
}

#[test]
fn an_attribute_on_a_statement_is_refused() {
    let err = refusal("<?php\n#[Weird]\n$x = 1;\n");
    assert!(err.contains("top-level `function` or `class`"), "{err}");
}

#[test]
fn a_non_ascii_attribute_segment_is_refused() {
    // Legal PHP; phorj's lexer rejects `é`, and a LEX error suppresses every other diagnostic in the
    // file — so emitting it would produce a draft whose real problems are invisible.
    let err = refusal("<?php\n#[Caf\u{e9}]\nclass C {}\n");
    assert!(err.contains("ASCII"), "{err}");
}

// ── interactions ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_phpdoc_above_an_attribute_still_reaches_the_declaration() {
    // DEC-419 keys the doc by the token index it precedes — which is now the `#[`, not the `class`.
    let php = "<?php\n/**\n * A widget.\n */\n#[\\Attribute]\nclass Tag {}\n";
    let out = lift(php);
    assert!(out.contains(" * A widget."), "the doc was dropped:\n{out}");
    assert!(out.contains("#[Core.Runtime.Attribute]"), "{out}");
}

#[test]
fn a_bare_hash_comment_is_still_a_comment() {
    // The lexer distinguishes `#[` from `#`; conflating them was the original bug in both directions.
    let out = lift("<?php\n# just a note\nfunction f(): int { return 1; }\n");
    assert!(!out.contains("just a note"), "{out}");
    assert!(out.contains("function f(): int"), "{out}");
}

#[test]
fn a_lifted_attribute_survives_into_the_php_transpile() {
    // Invariant 17 — transpile and lift move together, and since DEC-437 that is literal: a lifted
    // attribute goes back OUT into the PHP, so `PHP → phorj → PHP` keeps the metadata instead of
    // dropping it. (`tests/attribute_transpile.rs` closes the other direction against a real `php`.)
    let phg = lift(DECLARE_AND_USE);
    let prog = crate::cli::parse_program(&phg).expect("draft parses");
    let checked = crate::cli::check_and_expand(&prog, &phg).expect("draft checks");
    let php = crate::transpile::emit_with_source(&checked, Some(&phg)).expect("it transpiles");
    assert!(php.contains("class Widget"), "{php}");
    assert!(
        php.contains("#[\\Attribute]"),
        "the marker must survive:\n{php}"
    );
    assert!(
        php.contains("#[Tag('cli')]"),
        "the attribute use must survive:\n{php}"
    );
}
