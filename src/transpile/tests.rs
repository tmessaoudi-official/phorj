use super::emit;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn php(src: &str) -> String {
    let tokens = lex(src).expect("lex");
    let prog = Parser::new(tokens).parse_program().expect("parse");
    emit(&prog).expect("emit")
}

fn parse_only(src: &str) -> crate::ast::Program {
    // Auto-prepend the reserved `package Main;` (M5 S1, line-preserving) unless declared, so
    // transpiler tests need no per-case edit. The transpiler ignores the package in S1 (flat
    // emission); brace-namespaces for non-`main` packages land in S2c.
    let src = if src.trim_start().starts_with("package ") {
        src.to_string()
    } else {
        format!("package Main; {src}")
    };
    let tokens = lex(&src).expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

#[test]
fn empty_program_emits_php_open_tag() {
    assert_eq!(php(""), "<?php\n");
}

#[test]
fn free_function_with_params_and_arithmetic() {
    let out = php("function add(int a, int b) -> int { int c = a + b; return c; }");
    assert!(out.contains("function add(int $a, int $b): int {"), "{out}");
    // DEC-255: both operands are statically `int`, so `+` emits `__phorj_checked_add` (throws on
    // overflow, matching phorj — bare `$a + $b` would silently promote to float). The unknown-kind
    // `__phorj_add` fallback is a distinct helper and is NOT used here.
    assert!(out.contains("$c = __phorj_checked_add($a, $b);"), "{out}");
    assert!(!out.contains("__phorj_add("), "{out}");
    assert!(out.contains("return $c;"), "{out}");
}

#[test]
fn t6d_index_native_call_and_const_reads_specialize() {
    // T6d: a list-index result is an `int` operand → native `intdiv`, no `__phorj_div`. (DEC-255: the
    // read is wrapped in `__phorj_index` — still an int operand, so the `intdiv` specialization holds.)
    let idx = php("function f(List<int> xs, int i) -> int { return xs[i] / 2; }");
    assert!(idx.contains("intdiv(__phorj_index($xs, $i), 2)"), "{idx}");
    assert!(!idx.contains("__phorj_div"), "{idx}");

    // T6d: a native-call result carries its declared return type — `String.upper` → string, so the
    // interpolation hole concatenates directly (no `__phorj_str`).
    let nat = php(
        "import Core.Output; import Core.String; function main() -> void { Output.printLine(\"got {String.upperCase(\\\"hi\\\")}\"); }",
    );
    assert!(nat.contains("strtoupper(\"hi\")"), "{nat}");
    assert!(!nat.contains("__phorj_str(strtoupper"), "{nat}");

    // T6d: a const read `Limits.MAX` is an `int` → `(string)` cast in interpolation, no `__phorj_str`.
    let c = php(
        "import Core.Output; class Limits { const int MAX = 9; } function main() -> void { Output.printLine(\"max={Limits.MAX}\"); }",
    );
    assert!(c.contains("(string)Limits::MAX"), "{c}");
    assert!(!c.contains("__phorj_str(Limits::MAX)"), "{c}");
}

#[test]
fn force_unwrap_uses_native_throw_expression_not_helper() {
    // `opt!` lowers to PHP 8.0's null-coalescing throw expression `($v ?? throw new …)` — `??`
    // throws iff the value is null and evaluates the receiver once, exactly the old `__phorj_unwrap`
    // helper. No runtime helper function is emitted.
    let out =
        php("function f(int? o) -> int { return o!; } function main() -> void { int x = f(5); }");
    assert!(
        out.contains("($o ?? throw new \\RuntimeException(\"force-unwrap of null\"))"),
        "{out}"
    );
    assert!(!out.contains("__phorj_unwrap"), "{out}");
}

#[test]
fn clone_with_lowers_to_native_php85_two_arg_clone() {
    // T4: the transpile floor is PHP 8.5, where `clone($o, [...])` is native (clone + property
    // overrides, constructor bypassed, `__clone` honored) — exactly what `obj with { f = e }` means.
    // It replaces the old `__phorj_clone_with` runtime helper (which existed only for the prior 8.4
    // floor). An empty override list is still a one-arg `clone($o)`.
    let out = php("class P { constructor(public int x, public int y) {} } \
             function main() -> void { P a = P(1, 2); P b = a with { x = 9 }; }");
    assert!(
        out.contains("clone($a, ['x' => 9])"),
        "clone-with uses native two-arg clone:\n{out}"
    );
    assert!(
        !out.contains("__phorj_clone_with"),
        "the 8.4 helper is gone (call site and definition):\n{out}"
    );
}

#[test]
fn error_cause_routed_to_php_previous_chain() {
    // M-faults 2c: a conventional `cause` field of optional-`Error` type on an `Error` subtype is
    // routed into PHP's native exception chain via `parent::__construct($message, 0, $cause)`, so
    // the transpiled PHP reports an idiomatic "caused by" through `getPrevious()` too. The cause
    // property is typed `?\Throwable` (PHP's `$previous` type) — NOT the unrelated engine `Error`
    // class (which `Error` would otherwise resolve to) nor a lossy `mixed`.
    let out = php(
        "class IoError implements Error { constructor(public string message) {} } \
             class ConfigError implements Error { \
               constructor(public string message, public Error? cause) {} }",
    );
    assert!(
        out.contains("parent::__construct($message, 0, $cause);"),
        "cause routed to native previous chain:\n{out}"
    );
    assert!(
        out.contains("?\\Throwable $cause"),
        "cause typed as ?\\Throwable (not engine Error / mixed):\n{out}"
    );
}

#[test]
fn no_return_type_is_void() {
    let out = php("function f() -> void { return; }");
    assert!(out.contains("function f(): void {"), "{out}");
}

#[test]
fn explicit_void_return_emits_php_void() {
    let out = php("function f() -> void { return; }");
    assert!(out.contains("function f(): void {"), "{out}");
}

#[test]
fn empty_return_emits_no_php_hint() {
    // `empty` must NOT emit `: void`/`: mixed`/`: null` — PHP would reject a fall-off or a bare
    // `return;`. No hint → PHP infers a capturable `null`.
    let out = php("function f() -> empty { } function main() -> void { empty x = f(); }");
    assert!(
        out.contains("function f() {"),
        "expected no return hint:\n{out}"
    );
    assert!(
        !out.contains("function f():"),
        "must not have a colon hint:\n{out}"
    );
}

#[test]
fn if_and_for_and_unary() {
    // Phorj is immutable (no reassignment) — use fresh var decls inside branches.
    let out = php("function f(int n) -> int { \
               List<int> xs = [1, 2]; \
               for (int x in xs) { if (x > 0) { int a = -x; } else { bool b = !true; } } \
               return n; }");
    assert!(out.contains("foreach ($xs as $x) {"), "{out}");
    assert!(out.contains("if ($x > 0) {"), "{out}");
    assert!(out.contains("} else {"), "{out}");
    // DEC-255: negating an `int` routes through `__phorj_checked_neg` (i64::MIN overflow faults);
    // the boolean `!` is unchanged.
    assert!(
        out.contains("$a = __phorj_checked_neg($x);") && out.contains("$b = !true;"),
        "{out}"
    );
    assert!(out.contains("[1, 2]"), "{out}");
}

#[test]
fn checked_int_natives_wrap_overflow_promoting_builtins() {
    // DEC-255: `Math.abs`/`Math.integerPower`/`List.sum` fault on int overflow in phorj, but the
    // underlying PHP builtins (`abs`/`pow`/`array_sum`) silently promote to float. Each emit is
    // wrapped in `__phorj_checked_int`, which throws on the promotion — matching phorj's fault.
    let a = php("import Core.Math; function f(int n) -> int { return Math.abs(n); }");
    assert!(a.contains("__phorj_checked_int(abs($n))"), "{a}");

    let p = php(
        "import Core.Math; function f(int b, int e) -> int { return Math.integerPower(b, e); }",
    );
    assert!(p.contains("__phorj_checked_int(pow($b, $e))"), "{p}");

    let s = php("import Core.List; function f(List<int> xs) -> int { return List.sum(xs); }");
    assert!(s.contains("__phorj_checked_int(array_sum($xs))"), "{s}");

    // The helper is emitted exactly once when used, and absent when no such native appears.
    assert_eq!(a.matches("function __phorj_checked_int(").count(), 1, "{a}");
    let none = php("import Core.Output; function main() -> void { Output.printLine(\"hi\"); }");
    assert!(!none.contains("__phorj_checked_int"), "{none}");
}

#[test]
fn indexing_emits_php_subscript() {
    // DEC-255: a READ index routes through `__phorj_index` (throws on OOB/missing, matching phorj's
    // fault — bare `$xs[$i]` would silently return null). An ASSIGNMENT target stays a bare `$xs[$i]`
    // lvalue (a function call can't be assigned to).
    let read = php("function at(List<int> xs, int i) -> int { return xs[i]; }");
    assert!(read.contains("__phorj_index($xs, $i)"), "{read}");
    assert!(read.contains("function __phorj_index"), "{read}");
    let write = php("function set() -> void { mutable List<int> xs = [1, 2, 3]; xs[1] = 9; }");
    assert!(
        write.contains("$xs[1] = 9"),
        "assignment target stays bare: {write}"
    );
}

#[test]
fn ranges_emit_php_range() {
    // Ranges route through `__phorj_range` (QW-13): the helper yields `[]` for an empty/reversed
    // range, where PHP's bare `range()` would descend. The `inclusive` flag is the third arg.
    let out = php(r#"import Core.Output;
function main() -> void { for (int i in 0..3) { Output.printLine("{i}"); } }"#);
    assert!(out.contains("__phorj_range(0, 3, false)"), "{out}");
    assert!(out.contains("function __phorj_range"), "{out}");
    let inc = php(r#"import Core.Output;
function main() -> void { for (int i in 1..=3) { Output.printLine("{i}"); } }"#);
    assert!(inc.contains("__phorj_range(1, 3, true)"), "{inc}");
}

#[test]
fn expression_if_emits_ternary() {
    let out = php("function pick(bool b) -> int { return if (b) { 1 } else { 2 }; }");
    assert!(out.contains("($b ? 1 : 2)"), "{out}");
}

#[test]
fn interpolation_string_hole_emits_native_php_interpolation() {
    // B-1: a variable-rooted `string`/`int` hole embeds as PHP `{$…}` interpolation (not concat).
    let out = php("function greet(string name) -> string { return \"Hello {name}\"; }");
    assert!(out.contains(r#"return "Hello {$name}";"#), "{out}");
    assert!(
        !out.contains(". $name"),
        "no concat for a simple hole: {out}"
    );
    assert!(!out.contains("__phorj_str"), "{out}");
}

#[test]
fn interpolation_int_hole_embeds() {
    // B-1: an `int` hole interpolates byte-identically (PHP stringifies int like `(string)`).
    let out = php("function f(int n) -> string { return \"n={n}!\"; }");
    assert!(out.contains(r#"return "n={$n}!";"#), "{out}");
    assert!(
        !out.contains("(string)"),
        "no cast needed inside interpolation: {out}"
    );
}

#[test]
fn interpolation_member_and_method_embed() {
    // B-1: `$`-rooted access chains (member, method-call) embed as `{$o->p}` / `{$o->m()}`.
    let out = php(
        "class C { public int x; function get() -> int { return this.x; } } \
         function f(C c) -> string { return \"{c.x} {c.get()}\"; }",
    );
    assert!(out.contains(r#"{$c->x}"#), "member embeds: {out}");
    assert!(out.contains(r#"{$c->get()}"#), "method-call embeds: {out}");
}

#[test]
fn interpolation_operator_hole_falls_back_to_concat() {
    // B-1: a top-level operator hole is NOT PHP-interpolatable (`{$a + $b}` is a parse error) → concat.
    let out = php("function f(int a, int b) -> string { return \"sum={a + b}\"; }");
    // DEC-255: the int `+` in the hole is `__phorj_checked_add`, concatenated (not PHP-embedded).
    assert!(
        out.contains("__phorj_checked_add($a, $b)"),
        "operator hole concatenates: {out}"
    );
    assert!(!out.contains("{$"), "operator hole is NOT embedded: {out}");
}

#[test]
fn println_newline_uses_echo_comma() {
    // B-2: `Output.printLine` lowers to `echo X, "\n"` (comma list), not `echo X . "\n"`.
    let out =
        php("import Core.Output; #[Entry] function main() -> void { Output.printLine(\"hi\"); }");
    assert!(out.contains(r#"echo "hi", "\n""#), "comma list: {out}");
    assert!(!out.contains(r#". "\n""#), "no concat newline: {out}");
}

#[test]
fn string_literal_dollar_minimal_escape() {
    // B-9: escape `$` in a literal only when it would interpolate. `$5` stays bare; `$xyz` is escaped.
    let bare = php("function f() -> string { return \"cost $5 each\"; }");
    assert!(
        bare.contains(r#"return "cost $5 each";"#),
        "digit-after-$ not escaped: {bare}"
    );
    let esc = php("function f() -> string { return \"$xyz var\"; }");
    assert!(
        esc.contains(r#"return "\$xyz var";"#),
        "ident-after-$ escaped: {esc}"
    );
}

#[test]
fn float_interpolation_emits_phorj_float_helper() {
    // T6: a statically-`float` interpolation hole emits `__phorj_float` directly (bypassing the
    // `__phorj_str` dispatch). `__phorj_float` reproduces Rust's shortest-round-trip positional
    // `f64` Display (no PHP precision-14 / scientific divergence) and is irreducible.
    let out = php("function f(float x) -> string { return \"v={x}\"; }");
    assert!(
        out.contains(r#"return "v=" . __phorj_float($x);"#),
        "float hole emits __phorj_float directly: {out}"
    );
    assert!(
        !out.contains("__phorj_str"),
        "no str dispatch needed: {out}"
    );
    assert!(
        out.contains("function __phorj_float($v) {")
            && out.contains(r#"$cand = sprintf("%.{$p}e", $a);"#),
        "__phorj_float helper is defined with the shortest-round-trip loop: {out}"
    );
    // Only tier-1 PHP functions — must stay correct under `php -n` (extension policy).
    for forbidden in ["mb_", "ctype_", "iconv", "bcadd"] {
        assert!(
            !out.contains(forbidden),
            "__phorj_float must use tier-1 functions only, found `{forbidden}`: {out}"
        );
    }
}

#[test]
fn string_format_helper_uses_tier1_functions_only() {
    // Regression guard (CI hermetic-php break): `String.format`'s `__phorj_format` helper must use
    // ONLY tier-1 core functions — it ran under a `php -n` build with `ctype` as a shared (unloaded)
    // extension and fataled on `ctype_digit` (the `__phorj_float` guard above never exercised THIS
    // helper). The digit scan now uses `strpos('0123456789', …)`, like the flag scan.
    let out = php(
        "function f() -> string { return String.format(\"%5d %-3s %.2f\", [42, \"x\", 3.5]); }",
    );
    assert!(
        out.contains("function __phorj_format($spec, $args) {"),
        "String.format emits the __phorj_format helper: {out}"
    );
    for forbidden in ["ctype_", "mb_", "iconv", "bcadd"] {
        assert!(
            !out.contains(forbidden),
            "__phorj_format must use tier-1 functions only (hermetic `php -n`), found `{forbidden}`: {out}"
        );
    }
}

#[test]
fn pure_string_literal_no_concat() {
    let out = php("function f() -> string { return \"hi\"; }");
    assert!(out.contains(r#"return "hi";"#), "{out}");
}

#[test]
fn literal_match_with_binding_emits_native_match() {
    // T1: a value `match` of literals + a bare-binding catch-all lowers to a native PHP `match`
    // expression (PHP `match` is strict `===`, agreeing with Phorj literal patterns). The binding
    // is assigned *inside* the subject (`match ($x = $n)`) so the `default` arm can reference it —
    // single evaluation, no `if/elseif` chain, no IIFE.
    let out = php(
            "function sign(int n) -> string { string s = match n { 0 => \"z\", 1 => \"one\", x => \"other\" }; return s; }",
        );
    assert!(out.contains("$s = match ($x = $n) {"), "{out}");
    assert!(out.contains("0 => \"z\","), "{out}");
    assert!(out.contains("1 => \"one\","), "{out}");
    assert!(out.contains("default => \"other\","), "{out}");
    // No legacy if-chain or stranded defensive throw.
    assert!(!out.contains("elseif ($n === 1)"), "{out}");
}

#[test]
fn literal_match_with_wildcard_emits_native_match() {
    // A wildcard `_` catch-all needs no binding, so the subject is the bare scrutinee.
    let out = php(
            "function classify(int code) -> string { return match code { 0 => \"zero\", 1 => \"one\", default => \"other\" }; }",
        );
    assert!(out.contains("return match ($code) {"), "{out}");
    assert!(out.contains("0 => \"zero\","), "{out}");
    assert!(out.contains("default => \"other\","), "{out}");
    assert!(!out.contains("if ($code === 0)"), "{out}");
}

#[test]
fn expression_position_literal_match_emits_native_match() {
    // T1: a literal value `match` in expression position is a native PHP `match` expression
    // (parenthesized so it composes), NOT an IIFE. The binding catch-all still works in expression
    // position via the assignment-as-subject trick (`match ($x = $n)`).
    let out = php(
            "function f(int n) -> int { int base = 5; int r = (match n { 0 => 10, x => x }) + base; return r; }",
        );
    assert!(out.contains("(match ($x = $n) {"), "{out}");
    assert!(out.contains("0 => 10,"), "{out}");
    assert!(out.contains("default => $x,"), "{out}");
    // No IIFE wrapper for a pure literal match.
    assert!(!out.contains("function() use"), "{out}");
}

#[test]
fn println_becomes_echo() {
    let out =
        php("import Core.Output; #[Entry] function main() -> void { Output.printLine(\"hi\"); }");
    assert!(out.contains(r#"echo "hi", "\n";"#), "{out}");
}

#[test]
fn main_is_invoked_when_present() {
    let out =
        php("import Core.Output; #[Entry] function main() -> void { Output.printLine(\"hi\"); }");
    assert!(out.trim_end().ends_with("main();"), "{out}");
    // no main -> no call
    let no_main = php("function helper() -> int { return 1; }");
    assert!(!no_main.contains("main();"), "{no_main}");
}

const SHAPE: &str = "enum Shape { Circle(float radius), Rect(float w, float h), }";

#[test]
fn enum_emits_base_and_subclasses() {
    let out = php(SHAPE);
    assert!(out.contains("abstract class Shape {}"), "{out}");
    assert!(out.contains("final class Circle extends Shape {"), "{out}");
    assert!(
        out.contains("public function __construct(public float $radius) {}"),
        "{out}"
    );
    assert!(out.contains("final class Rect extends Shape {"), "{out}");
    assert!(
        out.contains("public function __construct(public float $w, public float $h) {}"),
        "{out}"
    );
}

// Slice A: PHP reserves int/float/bool/null (and string/true/false/…) as class names, even inside a
// namespace. Enum variants transpile to `final class <V> extends <Enum>`, so a variant named after a
// reserved word must be mangled (trailing `_`) or the PHP is a parse error. Transpiler-only: run/runvm
// use the Phorj variant string, so stdout byte-identity is untouched.
const RESERVED_ENUM: &str =
    "enum Tok { Int(int v), Float(float f), Bool(bool b), Null(), Str(string s) }";

#[test]
fn reserved_enum_variant_names_are_mangled_in_php() {
    let out = php(RESERVED_ENUM);
    assert!(out.contains("final class Int_ extends Tok {"), "{out}");
    assert!(out.contains("final class Float_ extends Tok {"), "{out}");
    assert!(out.contains("final class Bool_ extends Tok {"), "{out}");
    assert!(out.contains("final class Null_ extends Tok {"), "{out}");
    // A non-reserved variant name is left untouched.
    assert!(out.contains("final class Str extends Tok {"), "{out}");
}

// Slice B (F-m byte-identity fix): the keyword-as-class-name words (`empty`/`echo`/`match`/…) and the
// always-present PHP builtin class names (`Exception`/`Closure`/`Generator`/…) are ALSO reserved as PHP
// class names, so a variant named after one must be mangled identically. Previously only the value-type
// words were covered, so `enum { Empty }` transpiled to `final class Empty extends …` — a PHP parse
// error — while `run`/`run --tree-walker` succeeded: an Invariant-1 (byte-identity spine) break.
const RESERVED_ENUM_KW: &str =
    "enum Kw { Empty(), Echo(), Match(), Exception(), Closure(), Generator(), Plain(int v) }";

#[test]
fn reserved_keyword_and_builtin_variant_names_are_mangled_in_php() {
    let out = php(RESERVED_ENUM_KW);
    assert!(out.contains("final class Empty_ extends Kw {"), "{out}");
    assert!(out.contains("final class Echo_ extends Kw {"), "{out}");
    assert!(out.contains("final class Match_ extends Kw {"), "{out}");
    assert!(out.contains("final class Exception_ extends Kw {"), "{out}");
    assert!(out.contains("final class Closure_ extends Kw {"), "{out}");
    assert!(out.contains("final class Generator_ extends Kw {"), "{out}");
    // A non-reserved variant name is left untouched.
    assert!(out.contains("final class Plain extends Kw {"), "{out}");
}

#[test]
fn reserved_variant_construction_and_instanceof_are_mangled() {
    let src = format!(
        "{RESERVED_ENUM} function f() -> Tok {{ return Int(5); }} \
         function g(Tok t) -> int {{ return match t {{ Int(n) => n, default => 0 }}; }}"
    );
    let out = php(&src);
    assert!(out.contains("new Int_(5)"), "{out}");
    assert!(out.contains("instanceof Int_"), "{out}");
}

#[test]
fn variant_construction_uses_new() {
    let out = php(&format!(
        "{SHAPE} function f() -> Shape {{ return Circle(2.0); }}"
    ));
    assert!(out.contains("return new Circle(2.0);"), "{out}");
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
    assert!(out.contains("if ($s instanceof Circle) {"), "{out}");
    assert!(out.contains("$r = $s->radius;"), "{out}"); // positional: r <- field 0 (radius)
                                                        // P0-2: a compound operand keeps grouping parens (`3.14159 * r * r` is left-assoc Mul, so the
                                                        // left operand of the outer `*` is the inner product, conservatively parenthesized).
    assert!(out.contains("return (3.14159 * $r) * $r;"), "{out}");
    assert!(out.contains("if ($s instanceof Rect) {"), "{out}");
    assert!(
        out.contains("$w = $s->w;") && out.contains("$h = $s->h;"),
        "{out}"
    );
    assert!(out.contains("throw new \\UnhandledMatchError();"), "{out}");
}

#[test]
fn match_in_var_decl_assigns_in_each_arm() {
    let out = php(&format!(
        "{SHAPE} function f(Shape s) -> float {{ \
               float a = match s {{ Circle(r) => r, Rect(w, h) => w, }}; return a; }}"
    ));
    assert!(
        out.contains("if ($s instanceof Circle) { $r = $s->radius; $a = $r; }"),
        "{out}"
    );
    assert!(out.contains("if ($s instanceof Rect) {"), "{out}");
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
        out.contains("$s instanceof Circle && (($r = $s->radius) || true) => $r,"),
        "{out}"
    );
    assert!(
        out.contains(
            "$s instanceof Rect && (($w = $s->w) || true) && (($h = $s->h) || true) => $w,"
        ),
        "{out}"
    );
    // No IIFE.
    assert!(!out.contains("function () use"), "{out}");
    assert!(!out.contains("function() use"), "{out}");
}

// ── M3 S3 Task 5: expression lambdas + named-fn references ──────────────

#[test]
fn transpiles_expression_lambda_to_arrow_fn() {
    let php_out = php("package Main; import Core.Output; function main()-> void { var d = function(int x) => x*2; Output.printLine(\"{d(5)}\"); }");
    // DEC-255: `x*2` (both int) is `__phorj_checked_mul` inside the arrow fn.
    assert!(
        php_out.contains("fn($x) => __phorj_checked_mul($x, 2)"),
        "{php_out}"
    );
}

#[test]
fn transpiles_named_fn_reference() {
    let php_out = php("package Main; function inc(int x)->int{return x+1;} function apply(int x,(int)->int f)->int{return f(x);} function main()-> void { apply(1, inc); }");
    assert!(
        php_out.contains("inc(...)"),
        "first-class callable: {php_out}"
    );
}

#[test]
fn truncate_round_out_of_range_php_helper_throws() {
    // Fault-parity pass 2026-07-05: `Conversion.truncate`/`round` emit throwing helpers (NOT a raw
    // `(int)` cast, which silently diverged — Rust saturates, PHP wraps on out-of-range). The emitted
    // PHP must define `__phorj_trunc`/`__phorj_round` and each must `throw` on the out-of-range branch,
    // so the PHP leg faults in lockstep with the Rust backends (byte-identity on faults).
    let t = php("package Main; import Core.Output; import Core.Conversion; function main() -> void { int n = Conversion.truncate(1.0e30); Output.printLine(\"{n}\"); }");
    assert!(t.contains("function __phorj_trunc("), "{t}");
    assert!(
        t.contains("__phorj_trunc(1.0E+30)") || t.contains("__phorj_trunc("),
        "{t}"
    );
    assert!(
        t.contains(
            "throw new \\RuntimeException(\"Conversion.truncate: float is out of int range\")"
        ),
        "truncate helper must throw on out-of-range: {t}"
    );
    let r = php("package Main; import Core.Output; import Core.Conversion; function main() -> void { int n = Conversion.round(1.0e30); Output.printLine(\"{n}\"); }");
    assert!(r.contains("function __phorj_round("), "{r}");
    assert!(
        r.contains("throw new \\RuntimeException(\"Conversion.round: float is out of int range\")"),
        "round helper must throw on out-of-range: {r}"
    );
}

#[test]
fn statement_position_match_lowers_to_an_if_chain() {
    // A statement-position `match` whose arms print (void bodies) must NOT emit a native
    // `match (true) { … => echo …, }` — `echo` is a PHP STATEMENT and that form is a parse error
    // (a real transpile-fatal caught while building the DEC-253 example). The if-chain hosts the
    // statement bodies; exactly one arm runs.
    let src = "package Main; import Core.Output;\n\
         function main(): void { int e = 1; match (e) { 1 => Output.printLine(\"x\"), default => Output.printLine(\"y\") }; }";
    let out = php(src);
    assert!(
        !out.contains("=> echo"),
        "statement match must not put echo in a match-expression arm:\n{out}"
    );
    assert!(
        out.contains("if (") && out.contains("else {"),
        "expected an if-chain lowering:\n{out}"
    );
}
