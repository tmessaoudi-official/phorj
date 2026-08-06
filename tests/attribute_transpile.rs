//! DEC-437 — phorj attributes re-emitted into the transpiled PHP, validated against a REAL `php`.
//!
//! The unit tests in `src/transpile/tests_attributes.rs` pin what the emitter writes. This file pins
//! the two things only a real interpreter can answer:
//!
//! 1. **Byte-identity survives** (Invariant 1). An emitted attribute must be inert: `phg run` ≡
//!    `phg run --tree-walker` ≡ the transpiled PHP. That is not a formality — PHP would *fatal* on an
//!    attribute argument that is not a constant expression, and a `#[\Deprecated]` mapping would print a
//!    runtime notice neither phorj engine prints.
//! 2. **PHP can actually READ the metadata.** This is the whole point of the ruling: erasing attributes
//!    was "correct" and useless. `ReflectionAttribute::newInstance()` is the acceptance test, and it
//!    requires the `#[Attribute]` marker on the attribute class — without it PHP refuses with
//!    *"Attempting to use non-attribute class"*.

use std::process::Command;

fn php_bin() -> Option<String> {
    let cand = std::env::var("PHORJ_PHP").unwrap_or_else(|_| "php".to_string());
    let ok = Command::new(&cand)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then_some(cand)
}

/// The php binary, or `None` with a loud SKIP — a FAILURE under `PHORJ_REQUIRE_PHP=1` (skip-loud, the
/// house rule: an unmeasurable check is never silently a pass).
fn php_or_gate(label: &str) -> Option<String> {
    if let Some(p) = php_bin() {
        return Some(p);
    }
    assert!(
        std::env::var("PHORJ_REQUIRE_PHP").as_deref() != Ok("1"),
        "{label}: php required (PHORJ_REQUIRE_PHP=1) but not found on PATH or $PHORJ_PHP"
    );
    eprintln!("SKIP {label}: php not found — set PHORJ_REQUIRE_PHP=1 to make this a failure");
    None
}

fn run_php(php: &str, src: &str, label: &str) -> String {
    // The pid keeps concurrent test binaries off one another's fixture (DEC-378's root cause).
    let path =
        std::env::temp_dir().join(format!("phorj_attr_{}_{}.php", label, std::process::id()));
    std::fs::write(&path, src).expect("write php fixture");
    let out = Command::new(php)
        .arg(&path)
        .output()
        .expect("run php")
        .stdout;
    let _ = std::fs::remove_file(&path);
    String::from_utf8(out).expect("php stdout is utf-8")
}

fn transpile(src: &str) -> String {
    let prog = phorj::cli::parse_program(src).expect("parse");
    let checked = phorj::cli::check_and_expand(&prog, src).expect("check");
    phorj::transpile::emit(&checked).expect("emit")
}

const PROGRAM: &str = r#"
package Main;
import Core.Output;
import Core.Runtime.Attribute;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;

#[Attribute]
class Audited {
    constructor(public string reason, public int level) {}
}

#[Audited("billing", 2)]
class Invoice {
    constructor(public string ref) {}
    function label(): string { return "invoice " + this.ref; }
}

#[Entry(kind: EntryKind.Cli)]
function main(): void {
    Invoice inv = new Invoice("A-1");
    Output.printLine(inv.label());
}
"#;

#[test]
fn emitted_attributes_do_not_change_program_output() {
    let interp = phorj::cli::cmd_treewalk(PROGRAM).expect("interpreter runs it");
    let vm = phorj::cli::cmd_run(PROGRAM).expect("VM runs it");
    assert_eq!(interp, vm, "interp ≡ VM");
    assert_eq!(interp, "invoice A-1\n");
    let Some(php) = php_or_gate("emitted_attributes_do_not_change_program_output") else {
        return;
    };
    let out = run_php(&php, &transpile(PROGRAM), "identity");
    assert_eq!(
        out, interp,
        "the PHP leg must agree with both phorj engines"
    );
}

#[test]
fn php_reflection_can_read_a_transpiled_attribute() {
    let Some(php) = php_or_gate("php_reflection_can_read_a_transpiled_attribute") else {
        return;
    };
    let emitted = transpile(PROGRAM);
    assert!(emitted.contains("#[\\Attribute]"), "{emitted}");
    assert!(emitted.contains("#[Audited('billing', 2)]"), "{emitted}");
    // Append a reflection probe. `newInstance()` CONSTRUCTS the attribute — the strongest available
    // evidence that the emitted metadata is real and not decoration.
    let probe = format!(
        "{emitted}\n$rc = new ReflectionClass('Invoice');\nforeach ($rc->getAttributes() as $a) {{\n    $i = $a->newInstance();\n    echo $a->getName(), '=', $i->reason, '/', $i->level, \"\\n\";\n}}\n"
    );
    let out = run_php(&php, &probe, "reflect");
    assert!(
        out.contains("Audited=billing/2"),
        "reflection could not read the attribute:\n{out}"
    );
}

/// The metadata survives a full `phorj → PHP → phorj` pass. The trailing top-level entry CALL the
/// transpiler emits (`main();`) is what stops a whole-file round trip — the lifter refuses a file with
/// both a `main()` and top-level code — so it is dropped here to isolate the ATTRIBUTE half. That
/// pre-existing limitation is tracked in `KNOWN_ISSUES`, not fixed by this slice.
#[test]
fn attributes_survive_a_phorj_to_php_to_phorj_round_trip() {
    let emitted = transpile(PROGRAM);
    let body: String = {
        let mut lines: Vec<&str> = emitted.lines().collect();
        while lines.last().is_some_and(|l| l.trim() != "main();") {
            lines.pop();
        }
        lines.pop();
        lines.join("\n")
    };
    let lifted = phorj::lift::lifter::lift_source(&body).expect("the transpiled PHP lifts back");
    assert!(
        lifted.contains("#[Core.Runtime.Attribute]"),
        "the marker did not survive:\n{lifted}"
    );
    assert!(
        lifted.contains("#[Audited(\"billing\", 2)]"),
        "the attribute use did not survive:\n{lifted}"
    );
}

/// An ENUM-VALUED attribute, end to end against a real `php`. This is the case my first build silently
/// dropped: the gate matched `Colour.Red` (a bare member access), which Invariant 12 makes invalid phorj
/// everywhere, so the arm could never fire and every enum-valued attribute fell through to
/// "no PHP constant form". The real source spelling is `new Colour.Red()`, and `Expr::New` still wraps it
/// here because the `unwrap_new` desugar does not walk attribute arguments.
///
/// Reflection reading the enum FIELD (`get_class($i->c)`) is the acceptance test — it proves PHP both
/// parsed the argument and constructed the variant object.
#[test]
fn php_reflection_can_read_an_enum_valued_attribute() {
    let src = r#"
package Main;
import Core.Output;
import Core.Runtime.Attribute;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;

enum Colour { Red, Green }

#[Attribute]
class Painted {
    constructor(public Colour c) {}
}

#[Painted(new Colour.Red())]
class Widget {}

#[Entry(kind: EntryKind.Cli)]
function main(): void {
    Output.printLine("ok");
}
"#;
    let interp = phorj::cli::cmd_treewalk(src).expect("interpreter runs it");
    let vm = phorj::cli::cmd_run(src).expect("VM runs it");
    assert_eq!(interp, vm, "interp ≡ VM");
    let emitted = transpile(src);
    assert!(
        emitted.contains("#[Painted(new Colour_Red())]"),
        "{emitted}"
    );
    let Some(php) = php_or_gate("php_reflection_can_read_an_enum_valued_attribute") else {
        return;
    };
    let probe = format!(
        "{emitted}\n$rc = new ReflectionClass('Widget');\nforeach ($rc->getAttributes() as $a) {{\n    echo $a->getName(), ' c=', get_class($a->newInstance()->c), \"\\n\";\n}}\n"
    );
    let out = run_php(&php, &probe, "enumarg");
    assert!(
        out.contains("Painted c=Colour_Red"),
        "reflection could not read the enum-valued attribute:\n{out}"
    );
    // …and the emitted attribute did not disturb program output.
    assert!(out.starts_with(&interp), "output changed:\n{out}");
}

/// RETRACTION PIN (DEC-449, retracting task #67). The backlog carried *"desugars must walk attribute
/// arguments — `Expr::New` reaches the transpiler, latent `unreachable!()` panic"*. Investigated: **no
/// panic is reachable**, and attribute arguments ARE expanded. This pins both halves so the retraction
/// cannot silently rot back into a real defect.
///
/// Why no panic: the only sugar that could survive to a backend is gated three ways. The interpreter and
/// VM never EVALUATE an attribute argument (it is metadata), and the transpiler's argument gate declines
/// anything without a PHP constant form and emits a disclosure comment instead. So an unexpanded node
/// yields a *degraded re-emission*, never a crash — a real Invariant-5 concern, but a far smaller one
/// than the backlog claimed.
#[test]
fn attribute_arguments_are_expanded_and_never_panic_a_backend() {
    // (a) A nested construction survives intact — this is the `Expr::New` the backlog named.
    let out = transpile(
        "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\n\
         import Core.Runtime.EntryKind;\nimport Core.Runtime.Attribute;\n\
         #[Attribute] class Inner { constructor(public int v) {} }\n\
         #[Attribute] class Tag { constructor(public Inner i) {} }\n\
         #[Tag(new Inner(7))] function f(): int { return 1; }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{f()}\"); }",
    );
    assert!(
        out.contains("#[Tag(new Inner(7))]"),
        "a nested construction must re-emit, not panic or degrade:\n{out}"
    );

    // (b) A type ALIAS in the attribute's own signature, plus a foldable argument: the alias is erased
    // and the arithmetic folds (DEC-438), so the expansion chain genuinely reaches attribute arguments.
    let out = transpile(
        "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\n\
         import Core.Runtime.EntryKind;\nimport Core.Runtime.Attribute;\n\
         type Count = int;\n\
         #[Attribute] class Tag { constructor(public Count n) {} }\n\
         #[Tag(41 + 1)] function f(): int { return 1; }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{f()}\"); }",
    );
    assert!(
        out.contains("#[Tag(42)]"),
        "the alias must erase and the argument must fold:\n{out}"
    );

    // (c) An `html\"…\"` argument — the shape that would hit
    // `unreachable!(\"html literal not resolved before transpilation\")` if the gate let it through.
    // It must be DECLINED with the disclosure, and the program must still transpile.
    let out = transpile(
        "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\n\
         import Core.Runtime.EntryKind;\nimport Core.Runtime.Attribute;\nimport Core.Html;\n\
         #[Attribute] class Banner { constructor(public Html markup) {} }\n\
         #[Banner(html\"<h1>hi</h1>\")] function f(): int { return 1; }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{f()}\"); }",
    );
    assert!(
        out.contains("not re-emitted"),
        "an html argument must be declined with the disclosure, never panic:\n{out}"
    );
    assert!(
        out.contains("function f(): int"),
        "the program must still transpile:\n{out}"
    );
}
