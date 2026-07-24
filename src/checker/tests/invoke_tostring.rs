//! DEC-331 D9 — `#[Invoke]` / `#[ToString]` checker behaviour: positive resolution + every guard.

use super::support::*;

fn has(errs: &[Diagnostic], code: &str) -> bool {
    errs.iter().any(|d| d.code == Some(code))
}

const ADDER: &str = "class Adder { \
    constructor(public int bias) {} \
    #[Invoke] function add(int x): int { return x + this.bias; } \
    #[Invoke] function addPair(int x, int y): int { return x + y + this.bias; } \
    #[ToString] function describe(): string { return \"A\"; } }";

#[test]
fn invoke_and_tostring_check_clean() {
    let errs = errors_of(&format!(
        "import Core.Output; {ADDER} \
         function main(): void {{ Adder a = new Adder(1); \
           Output.printLine(\"{{a(5)}}\"); Output.printLine(\"{{a(1, 2)}}\"); \
           Output.printLine(\"{{a.add(5)}}\"); Output.printLine(\"{{a}}\"); }}"
    ));
    assert!(errs.is_empty(), "expected clean, got {errs:?}");
}

#[test]
fn invoke_result_is_an_arithmetic_operand() {
    // Invariant 7 (CTy-operand): the rewritten method call must type as the operand the checker proved.
    let errs = errors_of(&format!(
        "{ADDER} function main(): void {{ Adder a = new Adder(1); int n = a(5) + 1; }}"
    ));
    assert!(errs.is_empty(), "expected clean, got {errs:?}");
}

#[test]
fn invoke_on_free_function_is_attribute_target() {
    let errs = errors_of("#[Invoke] function f(int x): int { return x; }");
    assert!(has(&errs, "E-ATTRIBUTE-TARGET"), "{errs:?}");
}

#[test]
fn tostring_on_static_method_is_attribute_target() {
    let errs = errors_of("class C { #[ToString] static function s(): string { return \"\"; } }");
    assert!(has(&errs, "E-ATTRIBUTE-TARGET"), "{errs:?}");
}

#[test]
fn tostring_wrong_signature_is_rejected() {
    let params = errors_of("class C { #[ToString] function t(int x): string { return \"\"; } }");
    assert!(has(&params, "E-TOSTRING-SIGNATURE"), "params: {params:?}");
    let ret = errors_of("class C { #[ToString] function t(): int { return 0; } }");
    assert!(has(&ret, "E-TOSTRING-SIGNATURE"), "ret: {ret:?}");
}

#[test]
fn two_tostring_methods_is_duplicate() {
    let errs = errors_of(
        "class C { #[ToString] function a(): string { return \"\"; } \
                   #[ToString] function b(): string { return \"\"; } }",
    );
    assert!(has(&errs, "E-TOSTRING-DUPLICATE"), "{errs:?}");
}

#[test]
fn two_invoke_methods_same_signature_is_duplicate() {
    let errs = errors_of(
        "class C { #[Invoke] function a(int x): int { return x; } \
                   #[Invoke] function b(int y): int { return y; } }",
    );
    assert!(has(&errs, "E-INVOKE-DUPLICATE"), "{errs:?}");
}

#[test]
fn invoke_method_with_default_param_is_rejected() {
    // DEC-331 slice 1: `#[Invoke]` uses exact-arity resolution, so a default/variadic param is
    // rejected (no silent divergence from the direct call). Honoring defaults is slice 1b.
    let errs = errors_of("class C { #[Invoke] function m(int x, int y = 0): int { return x + y; } }");
    assert!(has(&errs, "E-INVOKE-DEFAULTS"), "{errs:?}");
}

#[test]
fn object_in_string_context_without_tostring_is_no_tostring() {
    let errs = errors_of(
        "import Core.Output; class C { constructor(public int n) {} } \
         function main(): void { C c = new C(1); Output.printLine(\"{c}\"); }",
    );
    assert!(has(&errs, "E-NO-TOSTRING"), "{errs:?}");
}

#[test]
fn conversion_tostring_without_tostring_is_no_tostring() {
    let errs = errors_of(
        "import Core.Conversion; class C { constructor(public int n) {} } \
         function main(): void { C c = new C(1); string s = Conversion.toString(c); }",
    );
    assert!(has(&errs, "E-NO-TOSTRING"), "{errs:?}");
}

#[test]
fn calling_a_non_invoke_instance_is_not_callable() {
    let errs = errors_of(
        "class C { constructor(public int n) {} } \
         function main(): void { C c = new C(1); int r = c(5); }",
    );
    assert!(has(&errs, "E-NOT-CALLABLE"), "{errs:?}");
}

#[test]
fn invoke_call_with_no_matching_overload_is_no_match() {
    let errs = errors_of(&format!(
        "{ADDER} function main(): void {{ Adder a = new Adder(1); string bad = \"x\"; int r = a(bad); }}"
    ));
    assert!(has(&errs, "E-OVERLOAD-NO-MATCH"), "{errs:?}");
}

#[test]
fn tostring_role_is_inherited() {
    // A subclass with no own `#[ToString]` stringifies through the inherited one (no E-NO-TOSTRING).
    let errs = errors_of(
        "import Core.Output; \
         open class Base { #[ToString] function d(): string { return \"b\"; } } \
         class Sub extends Base {} \
         function main(): void { Sub s = new Sub(); Output.printLine(\"{s}\"); }",
    );
    assert!(
        errs.is_empty(),
        "expected clean (inherited #[ToString]), got {errs:?}"
    );
}
