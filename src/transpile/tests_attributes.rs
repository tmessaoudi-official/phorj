//! PHP transpiler tests — **attribute re-emission** (DEC-437).
//!
//! Two things are pinned here, and the second is the one that can break the byte-identity spine.
//!
//! 1. USER attributes and the `#[Attribute]` marker reach the PHP output, so PHP-side reflection can
//!    read a transpiled program's metadata (`ReflectionAttribute::newInstance()` needs the marker —
//!    without it PHP refuses with *"Attempting to use non-attribute class"*).
//! 2. Everything whose PHP form would change BEHAVIOUR stays out: phorj's built-ins (compile-time
//!    machinery consumed by a desugar), and — the sharp one — `#[Deprecated]`, because PHP 8.4's own
//!    `#[\Deprecated]` prints a runtime notice that neither phorj engine prints.

use super::emit;
use crate::parser::Parser;
use crate::tokenizer::lex;

/// Transpile a program through the FULL pipeline (`check_and_expand`, as `phg transpile` does), so the
/// desugars have run and the attributes under test are the ones a real user's output would carry.
fn php_checked(src: &str) -> String {
    let prog = crate::cli::parse_program(src).expect("parse");
    let checked = crate::cli::check_and_expand(&prog, src).expect("check");
    crate::transpile::emit(&checked).expect("emit")
}

/// Transpile a RAW parsed program (no desugars) — used where the point is the emitter's own filtering.
fn php_raw(src: &str) -> String {
    let tokens = lex(src).expect("lex");
    let prog = Parser::new(tokens).parse_program().expect("parse");
    emit(&prog).expect("emit")
}

/// Does the output carry an actual `#[Name…]` ATTRIBUTE line? A plain `contains` cannot answer that:
/// the not-re-emitted disclosure quotes the attribute in a comment, so it matches too.
fn emits_attribute(php: &str, name: &str) -> bool {
    php.lines()
        .any(|l| l.trim_start().starts_with(&format!("#[{name}")))
}

const HEAD: &str = "package Main;\nimport Core.Output;\nimport Core.Runtime.Attribute;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n";

fn program(body: &str) -> String {
    format!("{HEAD}\n{body}\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ Output.printLine(\"ok\"); }}\n")
}

#[test]
fn a_user_attribute_and_its_marker_both_reach_the_php() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Audited { constructor(public string reason) {} }\n\n#[Audited(\"billing\")]\nclass Invoice { function label(): string { return \"x\"; } }",
    ));
    // PHP's own marker, root-qualified so it resolves from inside a `namespace` block too.
    assert!(out.contains("#[\\Attribute]"), "{out}");
    assert!(out.contains("#[Audited('billing')]"), "{out}");
}

#[test]
fn a_function_attribute_reaches_the_php_too() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Audited { constructor(public bool on) {} }\n\n#[Audited(true)]\nfunction work(): string { return \"w\"; }",
    ));
    assert!(out.contains("#[Audited(true)]"), "{out}");
}

/// Built-ins are phorj COMPILE-TIME machinery — `#[Entry]` becomes the entry call, `#[Route]` an
/// `autoRouter` registration. Emitting them would put phorj-internal concepts in a user's PHP.
#[test]
fn builtin_attributes_are_not_emitted() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tag { constructor(public int n) {} }\n\n#[Tag(1)]\nclass C {}",
    ));
    assert!(
        out.contains("#[Tag(1)]"),
        "the user attribute must be there:\n{out}"
    );
    assert!(!out.contains("#[Entry"), "{out}");
    assert!(!out.contains("EntryKind"), "{out}");
}

/// THE byte-identity case. PHP 8.4's `#[\Deprecated]` prints
/// `Deprecated: Function greet() is deprecated, …` when the function is CALLED; phorj's `#[Deprecated]`
/// is compile-time only (DEC-417 — use-site warnings come from the reference pass, at check time).
/// Mapping them would make the PHP leg print a line the VM and interpreter do not.
#[test]
fn deprecated_is_never_mapped_onto_phps_runtime_deprecated() {
    let out = php_checked(&format!(
        "{HEAD}\nimport Core.Runtime.Deprecated;\n#[Deprecated(message: \"use shout\")]\nfunction greet(): string {{ return \"hi\"; }}\n\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ Output.printLine(\"ok\"); }}\n"
    ));
    assert!(
        !out.contains("Deprecated"),
        "PHP's #[\\Deprecated] has RUNTIME behaviour phorj's does not:\n{out}"
    );
}

// ── the argument gate ────────────────────────────────────────────────────────────────────────────

#[test]
fn literal_arguments_render_as_php_constants() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Many { constructor(public string s, public int i, public float f, public bool b) {} }\n\n#[Many(\"hi\", 3, 1.5, false)]\nclass C {}",
    ));
    assert!(out.contains("#[Many('hi', 3, 1.5, false)]"), "{out}");
}

#[test]
fn a_named_argument_keeps_its_name() {
    // PHP 8.0 spells a named argument identically, so nothing is reordered.
    let out = php_checked(&program(
        "#[Attribute]\nclass Route2 { constructor(public string path, public string name) {} }\n\n#[Route2(name: \"home\", path: \"/x\")]\nclass C {}",
    ));
    assert!(out.contains("#[Route2(name: 'home', path: '/x')]"), "{out}");
}

#[test]
fn a_single_quote_in_a_string_argument_is_escaped() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tag { constructor(public string s) {} }\n\n#[Tag(\"it's\")]\nclass C {}",
    ));
    assert!(out.contains(r"#[Tag('it\'s')]"), "{out}");
}

/// An enum-valued attribute argument. Three things had to be right for this, and my first version got
/// all three wrong in the same way — by writing the argument as `Colour.Red`, which Invariant 12 makes
/// invalid phorj EVERYWHERE (construction is `new`-mandatory: `E-NEW-REQUIRED`). Because the test used a
/// shape that cannot exist, it "passed" against an emitter arm that could never fire, and I recorded a
/// non-existent CHECKER bug in `KNOWN_ISSUES` on the strength of it.
///
/// What is actually true:
/// * the source spelling is `new Colour.Red()` and it type-checks clean in attribute position [Verified];
/// * `new Enum_Variant()` IS admissible in a PHP attribute argument — PHP 8.1 allows `new` there because
///   the argument is evaluated on REFLECTION, not at parse time [Verified under php-8.5.8];
/// * `Expr::New` still WRAPS the call at this point, because the `unwrap_new` desugar does not walk
///   attribute arguments [Verified] — so the gate unwraps it itself.
#[test]
fn an_enum_valued_argument_renders_as_a_variant_construction() {
    let out = php_checked(&format!(
        "{HEAD}\nenum Colour {{ Red, Green }}\n\n#[Attribute]\nclass Painted {{ constructor(public Colour c) {{}} }}\n\n#[Painted(new Colour.Red())]\nclass C {{}}\n\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ Output.printLine(\"ok\"); }}\n"
    ));
    assert!(out.contains("#[Painted(new Colour_Red())]"), "{out}");
}

/// A CLASS construction is admissible for the same reason, and its own arguments are gated recursively.
#[test]
fn a_class_valued_argument_renders_as_a_construction() {
    let out = php_checked(&program(
        "class Inner { constructor(public string s) {} }\n\n#[Attribute]\nclass Wrap { constructor(public Inner i) {} }\n\n#[Wrap(new Inner(\"x\"))]\nclass C {}",
    ));
    assert!(out.contains("#[Wrap(new Inner('x'))]"), "{out}");
}

/// A construction's own arguments are gated recursively — so a nested CALL still refuses the whole
/// attribute, all-or-nothing.
#[test]
fn a_construction_with_a_call_argument_is_still_disclosed() {
    let out = php_checked(&program(
        "function three(): int { return 3; }\n\nclass Inner { constructor(public int n) {} }\n\n#[Attribute]\nclass Wrap { constructor(public Inner i) {} }\n\n#[Wrap(new Inner(three()))]\nclass C {}",
    ));
    assert!(!emits_attribute(&out, "Wrap"), "{out}");
    assert!(
        out.contains("// phorj: `#[Wrap(…)]` not re-emitted"),
        "{out}"
    );
}

/// …and the fold reaches INSIDE a construction: `new Inner(1 + 2)` becomes `new Inner(3)`.
#[test]
fn arithmetic_inside_a_construction_folds() {
    let out = php_checked(&program(
        "class Inner { constructor(public int n) {} }\n\n#[Attribute]\nclass Wrap { constructor(public Inner i) {} }\n\n#[Wrap(new Inner(1 + 2))]\nclass C {}",
    ));
    assert!(out.contains("#[Wrap(new Inner(3))]"), "{out}");
}

#[test]
fn a_list_argument_becomes_a_php_array() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tags { constructor(public List<string> names) {} }\n\n#[Tags([\"a\", \"b\"])]\nclass C {}",
    ));
    assert!(out.contains("#[Tags(['a', 'b'])]"), "{out}");
}

/// The gate that keeps the PHP leg compiling at all. PHP parses attribute arguments as CONSTANT
/// expressions, so a function CALL is *"Fatal error: Constant expression contains invalid operations"* and
/// kills the whole file [Verified under php-8.5.8]. phorj accepts a call there [Verified:
/// `#[Tag(three())]` type-checks clean], so the shape is reachable — and unlike arithmetic it can never be
/// folded, because the value is not known until run time. Not emitted, and the omission is DISCLOSED in
/// the output (DEC-166).
#[test]
fn a_call_argument_is_disclosed_rather_than_emitted() {
    let out = php_checked(&program(
        "function three(): int { return 3; }\n\n#[Attribute]\nclass Tag { constructor(public int n) {} }\n\n#[Tag(three())]\nclass C {}",
    ));
    // Checked line-wise, NOT with a bare `contains`: the disclosure comment quotes `#[Tag(…)]` in its
    // own text, so a substring test passes vacuously. (It did — that caught a bug in this very test.)
    assert!(
        !emits_attribute(&out, "Tag"),
        "an attribute with a call argument must not be emitted:\n{out}"
    );
    assert!(
        out.contains("// phorj: `#[Tag(…)]` not re-emitted"),
        "the omission must be disclosed in the output:\n{out}"
    );
}

/// An attribute naming no declared attribute class emits nothing — the checker has already reported
/// `E-UNKNOWN-ATTRIBUTE`, so the transpiler must not invent a PHP name for it.
#[test]
fn an_unresolvable_attribute_name_emits_nothing() {
    // Raw (no check), because this program deliberately would not type-check.
    let out = php_raw("package Main;\n#[Nope.Missing]\nclass C {}\n");
    assert!(!out.contains("#["), "{out}");
    assert!(out.contains("class C"), "{out}");
}

/// Indentation: a class-level attribute sits at the class's own indent, not column 0, so the emitted
/// PHP stays readable inside a `namespace` block.
#[test]
fn an_attribute_is_emitted_at_the_declarations_indent() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tag { constructor(public int n) {} }\n\n#[Tag(1)]\nclass C {}",
    ));
    // At top level that means column 0 — asserted explicitly so a future namespace-mode change to the
    // indent is a visible test change rather than a silent reformat.
    assert!(out.contains("\n#[Tag(1)]\nfinal class C"), "{out}");
}

// ── constant folding (the narrow, attribute-argument-only fold) ───────────────────────────────────

/// `#[Tag(-5)]` was refused before the fold, which is the most surprising thing about the old gate: a
/// plain NEGATIVE NUMBER parses as `Unary { Neg, Int(5) }`, not a literal, so the commonest computed
/// shape in real code was the one that failed.
#[test]
fn a_negative_number_argument_folds() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tag { constructor(public int n) {} }\n\n#[Tag(-5)]\nclass C {}",
    ));
    assert!(out.contains("#[Tag(-5)]"), "{out}");
}

#[test]
fn arithmetic_folds_with_precedence_and_recursion() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tag { constructor(public int n) {} }\n\n#[Tag(1 + 2 * 3)]\nclass C {}",
    ));
    assert!(out.contains("#[Tag(7)]"), "{out}");
}

#[test]
fn float_and_string_arguments_fold() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Two { constructor(public float f, public string s) {} }\n\n#[Two(1.5 + 2.0, \"a\" + \"b\")]\nclass C {}",
    ));
    // phorj spells string concatenation `+`; folding it is exactly equivalent to PHP's `'a' . 'b'`.
    assert!(out.contains("#[Two(3.5, 'ab')]"), "{out}");
}

/// The fold uses the SINGLE-SOURCED checked kernel (`crate::value::int_add`, Invariant 4), so an
/// overflowing argument declines to fold and falls back to the disclosure — it is never wrapped, and it
/// never becomes a new compile error. This is what keeps the narrow fold free of the language question a
/// general folder would have to answer.
#[test]
fn an_overflowing_argument_declines_to_fold_rather_than_wrapping() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Over { constructor(public int n) {} }\n\n#[Over(9223372036854775807 + 1)]\nclass C {}",
    ));
    assert!(!emits_attribute(&out, "Over"), "{out}");
    assert!(
        out.contains("// phorj: `#[Over(…)]` not re-emitted"),
        "{out}"
    );
}

/// Division stays out. It faults on zero, and a folded quotient is where an exactness argument would
/// have to be made — so it is excluded on purpose rather than by oversight.
#[test]
fn division_is_deliberately_not_folded() {
    let out = php_checked(&program(
        "#[Attribute]\nclass Tag { constructor(public int n) {} }\n\n#[Tag(6 / 2)]\nclass C {}",
    ));
    assert!(!emits_attribute(&out, "Tag"), "{out}");
}

/// The fold must agree with what the ENGINES compute for the same expression — the fold is a second
/// implementation site of arithmetic, and Invariant 4 exists because those drift. Rather than trusting
/// the shared kernel by inspection, this runs the same expression through the interpreter and compares.
#[test]
fn a_folded_argument_equals_what_the_engines_compute() {
    let expr = "1 + 2 * 3 - 4";
    let out = php_checked(&program(&format!(
        "#[Attribute]\nclass Tag {{ constructor(public int n) {{}} }}\n\n#[Tag({expr})]\nclass C {{}}"
    )));
    let engine = crate::cli::cmd_treewalk(&format!(
        "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ Output.printLine(\"{{{expr}}}\"); }}\n"
    ))
    .expect("the interpreter evaluates it");
    let expected = format!("#[Tag({})]", engine.trim());
    assert!(
        out.contains(&expected),
        "the fold and the interpreter disagree — expected {expected}:\n{out}"
    );
}
