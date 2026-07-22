//! PHP transpiler tests — enums, variant classes (DEC-329.3 scoping), classes, and `match`
//! lowering (M-Decomp split from `tests.rs`, Invariant 13).

use super::emit;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn php(src: &str) -> String {
    let tokens = lex(src).expect("lex");
    let prog = Parser::new(tokens).parse_program().expect("parse");
    emit(&prog).expect("emit")
}

fn parse_only(src: &str) -> crate::ast::Program {
    // Auto-prepend the reserved `package Main;` (line-preserving) unless declared — same helper
    // shape as `tests.rs`.
    let src = if src.trim_start().starts_with("package ") {
        src.to_string()
    } else {
        format!("package Main; import Core.Runtime.Entry; {src}")
    };
    let tokens = lex(&src).expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

const SHAPE: &str = "enum Shape { Circle(float radius), Rect(float w, float h), }";

#[test]
fn enum_emits_base_and_subclasses() {
    let out = php(SHAPE);
    assert!(out.contains("abstract class Shape {}"), "{out}");
    assert!(
        out.contains("final class Shape_Circle extends Shape {"),
        "{out}"
    );
    assert!(
        out.contains("public function __construct(public float $radius) {}"),
        "{out}"
    );
    assert!(
        out.contains("final class Shape_Rect extends Shape {"),
        "{out}"
    );
    assert!(
        out.contains("public function __construct(public float $w, public float $h) {}"),
        "{out}"
    );
}

// DEC-329.3: variant classes are enum-SCOPED (`Tok_Int`) — which also subsumes the old
// reserved-word mangle: PHP reserves int/float/bool/null (and string/true/false/…) as class names
// even inside a namespace, but a scoped name always carries the `Enum_` prefix and can never be a
// bare reserved word. Transpiler-only: interp/VM use the Phorj variant string, so stdout
// byte-identity is untouched.
const RESERVED_ENUM: &str =
    "enum Tok { Int(int v), Float(float f), Bool(bool b), Null(), Str(string s) }";

#[test]
fn reserved_value_type_variant_names_are_safe_via_enum_scoping() {
    let out = php(RESERVED_ENUM);
    assert!(out.contains("final class Tok_Int extends Tok {"), "{out}");
    assert!(out.contains("final class Tok_Float extends Tok {"), "{out}");
    assert!(out.contains("final class Tok_Bool extends Tok {"), "{out}");
    assert!(out.contains("final class Tok_Null extends Tok {"), "{out}");
    // A non-reserved variant name scopes identically (one uniform rule).
    assert!(out.contains("final class Tok_Str extends Tok {"), "{out}");
}

// The keyword-as-class-name words (`empty`/`echo`/`match`/…) and the always-present PHP builtin
// class names (`Exception`/`Closure`/`Generator`/…) are ALSO reserved as PHP class names — the
// historic F-m byte-identity break (`final class Empty` parse-errored while interp/VM succeeded).
// Enum scoping (DEC-329.3) covers them all uniformly.
const RESERVED_ENUM_KW: &str =
    "enum Kw { Empty(), Echo(), Match(), Exception(), Closure(), Generator(), Plain(int v) }";

#[test]
fn reserved_keyword_and_builtin_variant_names_are_safe_via_enum_scoping() {
    let out = php(RESERVED_ENUM_KW);
    assert!(out.contains("final class Kw_Empty extends Kw {"), "{out}");
    assert!(out.contains("final class Kw_Echo extends Kw {"), "{out}");
    assert!(out.contains("final class Kw_Match extends Kw {"), "{out}");
    assert!(
        out.contains("final class Kw_Exception extends Kw {"),
        "{out}"
    );
    assert!(out.contains("final class Kw_Closure extends Kw {"), "{out}");
    assert!(
        out.contains("final class Kw_Generator extends Kw {"),
        "{out}"
    );
    // A non-reserved variant name scopes identically (one uniform rule).
    assert!(out.contains("final class Kw_Plain extends Kw {"), "{out}");
}

#[test]
fn variant_construction_and_instanceof_reference_the_scoped_class() {
    let src = format!(
        "{RESERVED_ENUM} function f() -> Tok {{ return Int(5); }} \
         function g(Tok t) -> int {{ return match t {{ Int(n) => n, default => 0 }}; }}"
    );
    let out = php(&src);
    assert!(out.contains("new Tok_Int(5)"), "{out}");
    assert!(out.contains("instanceof Tok_Int"), "{out}");
}

#[test]
fn variant_construction_uses_new() {
    let out = php(&format!(
        "{SHAPE} function f() -> Shape {{ return Circle(2.0); }}"
    ));
    assert!(out.contains("return new Shape_Circle(2.0);"), "{out}");
}

#[test]
fn free_function_call_no_new() {
    let out = php("function inc(int n) -> int { return n + 1; } \
             function f() -> int { return inc(1); }");
    assert!(out.contains("return inc(1);"), "{out}");
}

#[test]
fn class_with_promotion_and_method() {
    let out = php("class Greeter { constructor(private string name) {} \
               function greet() -> string { return \"Hello {name}\"; } }");
    assert!(out.contains("class Greeter {"), "{out}");
    assert!(
        out.contains("function __construct(private string $name) {}"),
        "{out}"
    );
    assert!(out.contains("function greet(): string {"), "{out}");
    // bare field ref inside a method resolves to $this->name (coerced via __phorj_str — P0-3)
    assert!(
        out.contains(r#"return "Hello " . __phorj_str($this->name);"#),
        "{out}"
    );
}

#[test]
fn explicit_non_promoted_field_emitted() {
    // A plain field (not a ctor param) is emitted as a standalone property.
    let out = php("class C { private int count; constructor() {} }");
    assert!(out.contains("private int $count;"), "{out}");
}

#[test]
fn promoted_field_not_redeclared() {
    // Declared both explicitly AND via promotion: emit only the promotion (PHP forbids
    // redeclaring a promoted property as a separate one — caught by the round-trip test).
    let out = php("class C { private int total; constructor(private int total) {} }");
    assert!(
        out.contains("function __construct(private int $total) {}"),
        "{out}"
    );
    assert!(
        !out.contains("private int $total;"),
        "standalone redeclaration must be gone: {out}"
    );
}

#[test]
fn member_access_and_method_call() {
    let out = php(
        "import core.console; class Greeter { constructor(private string name) {} \
               function greet() -> string { return name; } } \
             function main() -> void { Greeter g = Greeter(\"Tak\"); Output.printLine(g.greet()); }",
    );
    assert!(out.contains(r#"$g = new Greeter("Tak");"#), "{out}");
    assert!(out.contains("$g->greet()"), "{out}");
}

#[test]
fn match_in_return_emits_instanceof_chain() {
    let out = php(&format!(
        "{SHAPE} function area(Shape s) -> float {{ \
               return match s {{ Circle(r) => 3.14159 * r * r, Rect(w, h) => w * h, }}; }}"
    ));
    assert!(out.contains(" = $s;"), "{out}"); // scrutinee bound ONCE (P0 audit fix)
    assert!(out.contains("instanceof Shape_Circle) {"), "{out}");
    assert!(out.contains("->radius;"), "{out}"); // positional: r <- field 0 (radius)
                                                 // P0-2: a compound operand keeps grouping parens (`3.14159 * r * r` is left-assoc Mul, so the
                                                 // left operand of the outer `*` is the inner product, conservatively parenthesized).
    assert!(out.contains("return (3.14159 * $r) * $r;"), "{out}");
    assert!(out.contains("instanceof Shape_Rect) {"), "{out}");
    assert!(out.contains("->w;") && out.contains("->h;"), "{out}");
    assert!(out.contains("throw new \\UnhandledMatchError();"), "{out}");
}

#[test]
fn match_in_var_decl_assigns_in_each_arm() {
    let out = php(&format!(
        "{SHAPE} function f(Shape s) -> float {{ \
               float a = match s {{ Circle(r) => r, Rect(w, h) => w, }}; return a; }}"
    ));
    // Scrutinee bound ONCE to a `$__mN` temp (P0 audit fix) — arms test/read the temp.
    assert!(out.contains(" = $s;"), "{out}");
    assert!(
        out.contains("instanceof Shape_Circle) { $r = $__m")
            && out.contains("->radius; $a = $r; }"),
        "{out}"
    );
    assert!(out.contains("instanceof Shape_Rect) {"), "{out}");
}

#[test]
fn wildcard_arm_has_no_trailing_throw() {
    let out = php(&format!(
        "{SHAPE} function area(Shape s) -> float {{ \
               return match s {{ Circle(r) => r, default => 0.0, }}; }}"
    ));
    assert!(!out.contains("UnhandledMatchError"), "{out}");
}

#[test]
fn match_as_call_argument_emits_match_true() {
    // T2: a variant `match` in expression position (here a call argument) lowers to a native PHP
    // `match (true) { <cond> => <body>, … }` expression, NOT an IIFE. PHP `if` is a statement and
    // `match` is an expression, so the if-chain stays for statement-position matches while
    // expression position uses `match` — mapping Phorj's match onto PHP's own statement/expression
    // duality. Payload bindings ride into the condition as `(($x = access) || true)` conjuncts (the
    // same proven technique the guarded if-chain uses), so no preceding statement is needed.
    let prog = parse_only(&format!(
        "{SHAPE} function f(Shape s) -> float {{ \
               float a = id(match s {{ Circle(r) => r, Rect(w, h) => w, }}); return a; }}"
    ));
    let out = emit(&prog).expect("expression-position match transpiles");
    assert!(out.contains("id((match (true) {"), "{out}");
    assert!(
        out.contains("$s instanceof Shape_Circle && (($r = $s->radius) || true) => $r,"),
        "{out}"
    );
    assert!(
        out.contains(
            "$s instanceof Shape_Rect && (($w = $s->w) || true) && (($h = $s->h) || true) => $w,"
        ),
        "{out}"
    );
    // No IIFE.
    assert!(!out.contains("function () use"), "{out}");
    assert!(!out.contains("function() use"), "{out}");
}
