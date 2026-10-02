//! DEC-557 — what `==` / `!=` transpile TO, asserted on the emitted PHP text.
//!
//! `examples/guide/string-equality.phg` proves the legs AGREE; it cannot tell which emission path
//! produced the agreement, and a regression in `emit_eq`'s decision order (or in one arm of
//! `kind_has_decimal`) hides behind a green example until someone writes the one shape that needs it
//! (the enum-with-a-decimal-payload regression did exactly that). Each case below pins one arm and is
//! sabotaged: removing the arm reddens it.
//!
//! The rule under test: a kind that CONTAINS a decimal keeps PHP's loose `==` (a decimal is a PHP string
//! carrier and Phorj's decimal equality is numeric); a `string`/scalar operand emits `===`; anything the
//! transpiler cannot pin goes through `__phorj_eq`.

use phorj::cli;

const DECLS: &str = r#"
enum Price { Exact(decimal amount), Label(string text) }
interface Shape { function area(): decimal; }
class Sq implements Shape {
    constructor(public decimal side) {}
    function area(): decimal { return this.side; }
}
open class Base { constructor() {} }
class Sub extends Base { constructor(public decimal v) {} }
class Money { constructor(public decimal amount) {} }
class Plain { constructor(public int n) {} }
class Box<T> { constructor(public T v) {} }
"#;

/// Transpile `function cmp(<sig>): bool { return <expr>; }` next to the shared declarations.
fn emit(sig: &str, expr: &str) -> String {
    let src = format!(
        "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n{DECLS}\n\
         function cmp({sig}): bool {{ return {expr}; }}\n\
         #[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ Output.printLine(\"x\"); }}\n"
    );
    cli::cmd_transpile(&src).unwrap_or_else(|e| panic!("transpile failed: {e:?}\n{src}"))
}

/// The `return …;` line of the emitted `cmp` function.
fn cmp_return(php: &str) -> &str {
    let start = php.find("function cmp(").expect("cmp emitted");
    let rest = &php[start..];
    let line = rest
        .lines()
        .find(|l| l.contains("return "))
        .expect("a return line");
    line.trim()
}

fn assert_loose(sig: &str, expr: &str) {
    let php = emit(sig, expr);
    let ret = cmp_return(&php);
    assert!(
        ret.contains(" == ") && !ret.contains("===") && !ret.contains("__phorj_eq"),
        "`{sig}` / `{expr}` must keep LOOSE == (a decimal is inside); got: {ret}"
    );
}

#[test]
fn string_operands_emit_strict_equality() {
    let php = emit("string a, string b", "a == b");
    let ret = cmp_return(&php);
    assert!(
        ret.contains("===") && !ret.contains("__phorj_eq"),
        "got: {ret}"
    );
    let php = emit("string a, string b", "a != b");
    assert!(
        cmp_return(&php).contains("!=="),
        "got: {}",
        cmp_return(&php)
    );
}

#[test]
fn scalar_operands_emit_strict_equality() {
    let ret = emit("int a, int b", "a == b");
    assert!(
        cmp_return(&ret).contains("==="),
        "got: {}",
        cmp_return(&ret)
    );
}

#[test]
fn a_string_operand_is_cast_so_a_php_coerced_map_key_still_equals_its_string() {
    let php = emit("string a, string b", "a == b");
    assert!(
        cmp_return(&php).contains("(string)$a === (string)$b"),
        "got: {}",
        cmp_return(&php)
    );
}

#[test]
fn an_unresolvable_operand_goes_through_the_structural_helper() {
    let src = "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n\
               function cmp<T>(T a, T b): bool { return a == b; }\n\
               #[Entry(kind: EntryKind.Cli)]\nfunction main(): void { Output.printLine(\"x\"); }\n";
    let php = cli::cmd_transpile(src).expect("transpile");
    assert!(
        php.contains("return __phorj_eq($a, $b);"),
        "erased generic must use the helper:\n{php}"
    );
    assert!(
        php.contains("function __phorj_eq("),
        "helper must be emitted once:\n{php}"
    );
}

#[test]
fn a_plain_class_without_a_decimal_uses_the_helper() {
    let php = emit("Plain a, Plain b", "a == b");
    assert!(
        cmp_return(&php).contains("__phorj_eq("),
        "got: {}",
        cmp_return(&php)
    );
}

#[test]
fn decimal_keeps_loose_equality() {
    assert_loose("decimal a, decimal b", "a == b");
}

#[test]
fn an_optional_decimal_keeps_loose_equality() {
    assert_loose("decimal? a, decimal? b", "a == b");
}

#[test]
fn a_nullable_class_holding_a_decimal_keeps_loose_equality() {
    assert_loose("Money? a, Money? b", "a == b");
}

#[test]
fn an_enum_with_a_decimal_payload_keeps_loose_equality() {
    assert_loose("Price a, Price b", "a == b");
}

#[test]
fn a_list_of_decimal_bearing_enums_keeps_loose_equality() {
    assert_loose("List<Price> a, List<Price> b", "a == b");
}

#[test]
fn an_interface_typed_operand_sees_its_implementers_decimal_field() {
    assert_loose("Shape a, Shape b", "a == b");
}

#[test]
fn a_base_typed_operand_sees_its_subclasses_decimal_field() {
    assert_loose("Base a, Base b", "a == b");
}

#[test]
fn a_generic_instantiated_at_decimal_keeps_loose_equality() {
    assert_loose("Box<decimal> a, Box<decimal> b", "a == b");
}

#[test]
fn a_directly_constructed_variant_is_typed_as_its_enum() {
    assert_loose("int x", "new Price.Exact(1.50d) == new Price.Exact(1.5d)");
}
