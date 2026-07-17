use phorj::checker::check;
use phorj::checker::enforce_injected_discipline;
use phorj::parser::Parser;
use phorj::tokenizer::lex;

/// The complete sample program from the design spec (§6), verbatim.
const SAMPLE: &str = r#"package Main;
import Core.Runtime.Entry;
import Core.Output;

enum Shape {
    Circle(float radius),
    Rect(float w, float h),
}

function area(Shape s) -> float {
    return match s {
        Circle(r)  => 3.14159 * r * r,
        Rect(w, h) => w * h,
    };
}

class Greeter {
    private string name;

    constructor(private string name) {}

    function greet() -> string {
        return "Hello {this.name}";
    }
}

#[Entry] function main() -> void {
    Greeter g = new Greeter("Tak");
    Output.printLine(g.greet());

    List<Shape> shapes = [new Circle(2.0), new Rect(3.0, 4.0)];
    for (Shape s in shapes) {
        Output.printLine("area = {area(s)}");
    }
}
"#;

fn check_src(src: &str) -> Result<(), Vec<phorj::diagnostic::Diagnostic>> {
    let tokens = lex(src).expect("lex ok");
    let prog = Parser::new(tokens).parse_program().expect("parse ok");
    // `check` now returns the non-fatal warnings on success (M3 S2.5); this harness only cares
    // about the error/clean contract, so collapse `Ok(warnings)` to `Ok(())`.
    check(&prog).map(|_warnings| ())
}

/// Run the full pre-check expansion chain (incl. `desugar_di`), returning the rendered error string on
/// failure. DI errors originate in `desugar_di` (inside `check_and_expand`), not the raw `check`.
fn expand(src: &str) -> Result<(), String> {
    let tokens = lex(src).expect("lex ok");
    let prog = Parser::new(tokens).parse_program().expect("parse ok");
    phorj::cli::check_and_expand(&prog, src).map(|_| ())
}

/// Member-imports that bring the bare DI surface (`#[Injectable]` + `inject`) into scope — §7 import
/// discipline. Prepended to the clean-graph tests so they exercise the bare form.
const DI_IMPORTS: &str =
    "import Core.DependencyInjection.Injectable;\nimport Core.DependencyInjection.inject;\nimport Core.Output;\n";

#[test]
fn di_injectable_graph_expands_and_checks_clean() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class Db {{ constructor() {{}} }}\n\
        #[Injectable] class Svc {{ constructor(private Db db) {{}} }}\n\
        #[Entry] function main(): void {{ Svc s = inject<Svc>(); Output.printLine(\"ok\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected clean DI expansion, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_non_injectable_target_is_missing() {
    let src =
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.DependencyInjection.inject;\n\
        class Bare { constructor() {} }\n\
        #[Entry] function main(): void { Bare b = inject<Bare>(); }\n";
    let e = expand(src).unwrap_err();
    assert!(e.contains("E-DI-MISSING"), "{e}");
}

#[test]
fn di_multi_impl_interface_is_ambiguous() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        interface I {{ function f(): int; }}\n\
        #[Injectable] class A implements I {{ constructor() {{}} function f(): int {{ return 1; }} }}\n\
        #[Injectable] class B implements I {{ constructor() {{}} function f(): int {{ return 2; }} }}\n\
        #[Entry] function main(): void {{ I x = inject<I>(); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-DI-AMBIGUOUS"), "{e}");
}

#[test]
fn di_dependency_cycle_is_rejected() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class A {{ constructor(private B b) {{}} }}\n\
        #[Injectable] class B {{ constructor(private A a) {{}} }}\n\
        #[Entry] function main(): void {{ A x = inject<A>(); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-DI-CYCLE"), "{e}");
}

#[test]
fn di_bare_inject_without_annotation_is_rejected() {
    // `var` binding = no annotation source → E-INJECT-NO-TYPE (the imports are present, so this is the
    // no-target error, not the no-import one).
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class A {{ constructor() {{}} }}\n\
        #[Entry] function main(): void {{ var x = inject(); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-INJECT-NO-TYPE"), "{e}");
}

// --- §7 import discipline: nothing in the wind ---------------------------------------------------

#[test]
fn di_injectable_attribute_bare_without_import_is_rejected() {
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        #[Injectable] class A { constructor() {} }\n\
        #[Entry] function main(): void {}\n";
    let e = expand(src).unwrap_err();
    assert!(e.contains("E-INJECTED-TYPE-BARE"), "{e}");
}

#[test]
fn di_inject_verb_bare_without_member_import_is_rejected() {
    // `import Core.DependencyInjection;` binds the qualifier (so `#[DI.Injectable]` is fine) but NOT the bare `inject`
    // verb — a bare `inject<A>()` here is E-DI-NO-IMPORT (needs `import Core.DependencyInjection.inject;`).
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.DependencyInjection;\n\
        #[DI.Injectable] class A { constructor() {} }\n\
        #[Entry] function main(): void { A a = inject<A>(); }\n";
    let e = expand(src).unwrap_err();
    assert!(e.contains("E-DI-NO-IMPORT"), "{e}");
}

#[test]
fn di_qualified_surface_checks_clean() {
    // `import Core.DependencyInjection;` → `#[DependencyInjection.Injectable]` +
    // `DependencyInjection.inject<T>()` / `DependencyInjection.inject()`.
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.DependencyInjection;\nimport Core.Output;\n\
        #[DependencyInjection.Injectable] class A { constructor() {} function n(): int { return 1; } }\n\
        function build(): A { return DependencyInjection.inject(); }\n\
        #[Entry] function main(): void { A a = DependencyInjection.inject<A>(); Output.printLine(\"{a.n()}\"); Output.printLine(\"{build().n()}\"); }\n";
    assert!(
        expand(src).is_ok(),
        "expected clean qualified DI expansion, got: {:?}",
        expand(src)
    );
}

#[test]
fn di_inject_is_a_free_identifier_without_import() {
    // With no `Core.DependencyInjection` import, `inject` is an ordinary user function — no DI machinery, no error.
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\n\
        function inject(): int { return 7; }\n\
        #[Entry] function main(): void { Output.printLine(\"{inject()}\"); }\n";
    assert!(
        expand(src).is_ok(),
        "expected `inject` usable as a plain function, got: {:?}",
        expand(src)
    );
}

// --- slice 2: annotation-driven `inject()` -------------------------------------------------------

#[test]
fn di_annotation_from_var_decl_checks_clean() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class Db {{ constructor() {{}} }}\n\
        #[Injectable] class Svc {{ constructor(private Db db) {{}} }}\n\
        #[Entry] function main(): void {{ Svc s = inject(); Output.printLine(\"ok\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected clean annotation-driven DI, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_annotation_from_return_checks_clean() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class A {{ constructor() {{}} function n(): int {{ return 1; }} }}\n\
        function build(): A {{ return inject(); }}\n\
        #[Entry] function main(): void {{ A a = build(); Output.printLine(\"{{a.n()}}\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected clean return-position annotation DI, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_annotation_single_impl_interface_checks_clean() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        interface Greeter {{ function greet(): string; }}\n\
        #[Injectable] class En implements Greeter {{ constructor() {{}} function greet(): string {{ return \"hi\"; }} }}\n\
        #[Entry] function main(): void {{ Greeter g = inject(); Output.printLine(g.greet()); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected single-impl interface annotation DI, got: {:?}",
        expand(&src)
    );
}

// --- slice 3: field injection (synthesized-ctor) -------------------------------------------------

#[test]
fn di_field_injection_checks_clean() {
    // `Logger` field-injects `Clock`; `App` field-injects `Logger` and ctor-injects `Clock`. The field
    // has no initializer + an injectable type → folded to a promoted ctor param, resolved by the graph.
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class Clock {{ constructor() {{}} function n(): int {{ return 1; }} }}\n\
        #[Injectable] class Logger {{ private Clock clock; constructor() {{}} function m(): int {{ return this.clock.n(); }} }}\n\
        #[Injectable] class App {{ private Logger logger; constructor(private Clock clock) {{}} function go(): int {{ return this.logger.m(); }} }}\n\
        #[Entry] function main(): void {{ App a = inject<App>(); Output.printLine(\"{{a.go()}}\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected clean field-injection expansion, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_field_injection_cycle_is_rejected() {
    // A field-injects B, B field-injects A — the synthesized-ctor model makes field cycles ctor cycles,
    // so the existing cycle check catches them (a field-injection cycle is unbreakable in v1).
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class A {{ private B b; constructor() {{}} }}\n\
        #[Injectable] class B {{ private A a; constructor() {{}} }}\n\
        #[Entry] function main(): void {{ A x = inject<A>(); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-DI-CYCLE"), "{e}");
}

#[test]
fn di_field_injection_leaves_initialized_field_alone() {
    // A field WITH an initializer is user-provided — NOT folded into the constructor. `App` therefore has
    // no injected inputs, so `inject<App>()` builds `new App()` and the field initializes itself.
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class Clock {{ constructor() {{}} }}\n\
        #[Injectable] class App {{ private Clock clock = new Clock(); constructor() {{}} }}\n\
        #[Entry] function main(): void {{ App a = inject<App>(); Output.printLine(\"ok\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected an initialized field to be left alone, got: {:?}",
        expand(&src)
    );
}

// --- slice 4a: #[Provides] factories -------------------------------------------------------------

const DI_PROVIDES_IMPORTS: &str =
    "import Core.DependencyInjection.Injectable;\nimport Core.DependencyInjection.Provides;\nimport Core.DependencyInjection.inject;\nimport Core.Output;\n";

#[test]
fn di_provides_factory_checks_clean() {
    // `Db` needs a config value → not injectable; a `#[Provides]` factory constructs it, and `Repo` wires
    // `Db` through the factory (precedence over `new`).
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_PROVIDES_IMPORTS}\
        class Db {{ constructor(private string url) {{}} }}\n\
        class Providers {{ #[Provides] static function db(): Db {{ return new Db(\"x\"); }} }}\n\
        #[Injectable] class Repo {{ constructor(private Db db) {{}} }}\n\
        #[Entry] function main(): void {{ Repo r = inject<Repo>(); Output.printLine(\"ok\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected clean #[Provides] expansion, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_provides_disambiguates_multi_impl_interface() {
    // Two injectable impls would be E-DI-AMBIGUOUS — a `#[Provides]` returning the interface picks one and
    // resolves cleanly (the provider wins over the ambiguity).
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_PROVIDES_IMPORTS}\
        interface Logger {{ function log(): string; }}\n\
        #[Injectable] class FileLog implements Logger {{ constructor() {{}} function log(): string {{ return \"f\"; }} }}\n\
        #[Injectable] class NetLog implements Logger {{ constructor() {{}} function log(): string {{ return \"n\"; }} }}\n\
        class Bind {{ #[Provides] static function logger(): Logger {{ return new FileLog(); }} }}\n\
        #[Entry] function main(): void {{ Logger l = inject<Logger>(); Output.printLine(l.log()); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected #[Provides] to disambiguate, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_provides_wins_over_injectable_class() {
    // `Db` is BOTH `#[Injectable]` (a valid `new`-target) AND has a `#[Provides]` factory — the provider
    // must win, so this resolves without needing Db's own ctor deps.
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_PROVIDES_IMPORTS}\
        #[Injectable] class Db {{ constructor() {{}} }}\n\
        class Bind {{ #[Provides] static function db(): Db {{ return new Db(); }} }}\n\
        #[Injectable] class Repo {{ constructor(private Db db) {{}} }}\n\
        #[Entry] function main(): void {{ Repo r = inject<Repo>(); Output.printLine(\"ok\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected provider to win over the injectable class, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_provides_duplicate_is_ambiguous() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_PROVIDES_IMPORTS}\
        class Db {{ constructor(private string url) {{}} }}\n\
        class P1 {{ #[Provides] static function a(): Db {{ return new Db(\"1\"); }} }}\n\
        class P2 {{ #[Provides] static function b(): Db {{ return new Db(\"2\"); }} }}\n\
        #[Injectable] class Repo {{ constructor(private Db db) {{}} }}\n\
        #[Entry] function main(): void {{ Repo r = inject<Repo>(); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-DI-AMBIGUOUS"), "{e}");
}

#[test]
fn di_provides_on_non_static_method_is_rejected() {
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_PROVIDES_IMPORTS}\
        class Db {{ constructor() {{}} }}\n\
        class Providers {{ #[Provides] function db(): Db {{ return new Db(); }} }}\n\
        #[Entry] function main(): void {{ Output.printLine(\"x\"); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-PROVIDES-TARGET"), "{e}");
}

#[test]
fn di_provides_bare_without_import_is_rejected() {
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.Output;\n\
        class Db { constructor() {} }\n\
        class Providers { #[Provides] static function db(): Db { return new Db(); } }\n\
        #[Entry] function main(): void { Output.printLine(\"x\"); }\n";
    let e = expand(src).unwrap_err();
    assert!(e.contains("E-INJECTED-TYPE-BARE"), "{e}");
}

// --- slice 4b: #[Transient] lifetime -------------------------------------------------------------

#[test]
fn di_transient_checks_clean() {
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.DependencyInjection.Injectable;\nimport Core.DependencyInjection.Transient;\nimport Core.DependencyInjection.inject;\nimport Core.Output;\n\
        #[Injectable] class Db { constructor() {} }\n\
        #[Injectable] #[Transient] class Worker { constructor(private Db db) {} }\n\
        #[Injectable] class App { constructor(private Worker a, private Worker b) {} }\n\
        #[Entry] function main(): void { App x = inject<App>(); Output.printLine(\"ok\"); }\n";
    assert!(
        expand(src).is_ok(),
        "expected clean transient expansion, got: {:?}",
        expand(src)
    );
}

#[test]
fn di_transient_cycle_is_still_rejected() {
    // Transient does not skip cycle detection (the DFS path check is unchanged).
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.DependencyInjection.Injectable;\nimport Core.DependencyInjection.Transient;\nimport Core.DependencyInjection.inject;\nimport Core.Output;\n\
        #[Injectable] #[Transient] class A { constructor(private B b) {} }\n\
        #[Injectable] #[Transient] class B { constructor(private A a) {} }\n\
        #[Entry] function main(): void { A x = inject<A>(); }\n";
    let e = expand(src).unwrap_err();
    assert!(e.contains("E-DI-CYCLE"), "{e}");
}

#[test]
fn di_transient_bare_without_import_is_rejected() {
    let src =
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.DependencyInjection.Injectable;\nimport Core.DependencyInjection.inject;\nimport Core.Output;\n\
        #[Injectable] #[Transient] class W { constructor() {} }\n\
        #[Entry] function main(): void { W w = inject<W>(); Output.printLine(\"x\"); }\n";
    let e = expand(src).unwrap_err();
    assert!(e.contains("E-INJECTED-TYPE-BARE"), "{e}");
}

#[test]
fn di_field_injection_inherited_from_parent() {
    // An injectable subclass inherits its parent's injected (promoted) field — `ctor_plan` gathers
    // inherited promoted params, so `inject<Sub>()` wires the parent's `Clock`. (Injectable inheritance
    // is not a v1 focus, but it resolves correctly rather than silently dropping the field.)
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class Clock {{ constructor() {{}} function n(): int {{ return 1; }} }}\n\
        #[Injectable] open class Base {{ private Clock clock; constructor() {{}} function t(): int {{ return this.clock.n(); }} }}\n\
        #[Injectable] class Sub extends Base {{}}\n\
        #[Entry] function main(): void {{ Sub s = inject<Sub>(); Output.printLine(\"{{s.t()}}\"); }}\n"
    );
    assert!(
        expand(&src).is_ok(),
        "expected inherited field injection to resolve, got: {:?}",
        expand(&src)
    );
}

#[test]
fn di_annotation_in_lambda_inferred_return_is_rejected() {
    // A lambda with an inferred (no declared) return type is NOT an annotation source, even nested in a
    // function that returns an injectable — the lambda's `return inject()` must be E-INJECT-NO-TYPE (this
    // is the test whose result the `current_ret` save/restore determines: without the reset it would
    // wrongly inherit `App` and succeed).
    let src = format!(
        "package Main;\nimport Core.Runtime.Entry;\n{DI_IMPORTS}\
        #[Injectable] class App {{ constructor() {{}} }}\n\
        function make(): App {{ var f = function() => inject(); return inject(); }}\n\
        #[Entry] function main(): void {{ discard make(); }}\n"
    );
    let e = expand(&src).unwrap_err();
    assert!(e.contains("E-INJECT-NO-TYPE"), "{e}");
}

#[test]
fn sample_program_type_checks_clean() {
    let result = check_src(SAMPLE);
    assert!(result.is_ok(), "expected clean type-check, got: {result:?}");
}

#[test]
fn non_exhaustive_match_in_full_program_errors() {
    let broken = SAMPLE.replace("        Rect(w, h) => w * h,\n", "");
    let errs = check_src(&broken).expect_err("should be non-exhaustive");
    assert!(
        errs.iter().any(|e| e.message.contains("non-exhaustive")),
        "{errs:?}"
    );
}

#[test]
fn wrong_constructor_arg_in_full_program_errors() {
    let broken = SAMPLE.replace(r#"new Greeter("Tak")"#, "new Greeter(123)");
    let errs = check_src(&broken).expect_err("should be a type error");
    assert!(
        errs.iter().any(|e| e.message.contains("argument 1")),
        "{errs:?}"
    );
}

#[test]
fn loop_variable_type_mismatch_errors() {
    let broken = SAMPLE.replace("for (Shape s in shapes)", "for (int s in shapes)");
    let errs = check_src(&broken).expect_err("should be a type error");
    assert!(!errs.is_empty());
}

// ── M6 W2: `#[Route(...)]` attribute validation ───────────────────────────────────────────────

fn has_code(errs: &[phorj::diagnostic::Diagnostic], code: &str) -> bool {
    errs.iter().any(|e| e.code == Some(code))
}

// ── M-RT S2.2: method return-type overloading ─────────────────────────────────────────────────

#[test]
fn bare_return_overloaded_method_call_needs_selector() {
    // A bare return-overloaded method call has no type context to pick a member — C1 scope requires
    // a `<Type>` selector at the call site (same rule free functions have without a sink).
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        class C { constructor() {} function f()->int { return 1; } function f()->bool { return true; } }\n\
        #[Entry] function main() -> void { var c = new C(); discard c.f(); }\n";
    let errs = check_src(src).expect_err("bare return-overloaded method call");
    assert!(has_code(&errs, "E-OVERLOAD-NO-CONTEXT"), "{errs:?}");
}

#[test]
fn selector_picks_return_overloaded_method() {
    // The `<Type>` selector resolves the method overload by return type — clean check.
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        class C { constructor() {} function f()->int { return 1; } function f()->bool { return true; } }\n\
        #[Entry] function main() -> void { var c = new C(); int n = <int>c.f(); bool b = <bool>c.f(); }\n";
    assert!(check_src(src).is_ok(), "{:?}", check_src(src));
}

#[test]
fn static_methods_cannot_return_overload() {
    // Return-overloading is instance-only this slice: a `static` call `ClassName.m(args)` has no
    // `<Type>` selector path, so a static return-overload would mangle its def with no call rewrite.
    // Statics keep the classic shared-return rule → E-OVERLOAD-RETURN.
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        class C { static function f()->int { return 1; } static function f()->bool { return true; } }\n\
        #[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("static return-overload");
    assert!(has_code(&errs, "E-OVERLOAD-RETURN"), "{errs:?}");
}

#[test]
fn selector_unknown_return_on_method_is_rejected() {
    // A selector naming a return type no overload has is E-OVERLOAD-SELECT-UNKNOWN.
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        class C { constructor() {} function f()->int { return 1; } function f()->bool { return true; } }\n\
        #[Entry] function main() -> void { var c = new C(); string s = <string>c.f(); }\n";
    let errs = check_src(src).expect_err("unknown return selector");
    assert!(has_code(&errs, "E-OVERLOAD-SELECT-UNKNOWN"), "{errs:?}");
}

#[test]
fn route_attribute_well_formed_checks_clean() {
    // The raw `check` path does not inject the Core.Http prelude (that is `cli::check_and_expand`),
    // so this asserts only the attribute validation itself: a well-formed `#[Route]` (two string
    // literals, good path, one-param + return handler shape) produces no attribute diagnostics. The
    // end-to-end `Request`/`Response` typing is covered by the conformance + differential gates.
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Route(\"GET\", \"/health\")]\nfunction h(int x) -> int { return x; }\n#[Entry] function main() -> void {}\n";
    assert!(check_src(src).is_ok(), "{:?}", check_src(src));
}

#[test]
fn unknown_attribute_is_rejected() {
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Cache(60)]\nfunction f() -> void {}\n#[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("unknown attribute");
    assert!(has_code(&errs, "E-UNKNOWN-ATTRIBUTE"), "{errs:?}");
}

#[test]
fn route_with_wrong_arg_count_is_rejected() {
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.Http;\n#[Route(\"GET\")]\nfunction f(Request req) -> Response { return Response.text(200, \"x\"); }\n#[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("bad route args");
    assert!(has_code(&errs, "E-ROUTE-ARGS"), "{errs:?}");
}

#[test]
fn route_with_bad_path_is_rejected() {
    let src = "package Main;\nimport Core.Runtime.Entry;\nimport Core.Http;\n#[Route(\"GET\", \"health\")]\nfunction f(Request req) -> Response { return Response.text(200, \"x\"); }\n#[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("bad route spec");
    assert!(has_code(&errs, "E-ROUTE-SPEC"), "{errs:?}");
}

#[test]
fn route_handler_wrong_shape_is_rejected() {
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Route(\"GET\", \"/\")]\nfunction f(int a, int b) -> int { return a + b; }\n#[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("bad handler shape");
    assert!(has_code(&errs, "E-ROUTE-HANDLER"), "{errs:?}");
}

#[test]
fn route_on_static_method_checks_clean() {
    // `#[Route]` on a static method is valid; Request/Response need not resolve in the raw `check`
    // path (no Core.Http injection here), so use a non-Http handler shape — the attribute + static
    // checks are what this exercises.
    let src = "package Main;\nimport Core.Runtime.Entry;\nclass C {\n  #[Route(\"GET\", \"/x\")]\n  static function h(int r) -> int { return r; }\n}\n#[Entry] function main() -> void {}\n";
    assert!(check_src(src).is_ok(), "{:?}", check_src(src));
}

#[test]
fn route_on_instance_method_requires_static() {
    let src = "package Main;\nimport Core.Runtime.Entry;\nclass C {\n  #[Route(\"GET\", \"/x\")]\n  function h(int r) -> int { return r; }\n}\n#[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("instance #[Route] method must fail");
    assert!(has_code(&errs, "E-ROUTE-METHOD-STATIC"), "{errs:?}");
}

#[test]
fn attribute_on_a_class_parses_then_fails_at_check_as_a_target_error() {
    // DEC-194 slice 2a: attributes now PARSE on a class (the plumbing the user-attribute system builds
    // on), but no attribute *targets* a class yet, so the rejection moved from a parse-stage error to a
    // CHECK-stage `E-ATTR-TARGET` — the class attribute is validated, never silently accepted.
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Route(\"GET\", \"/\")]\nclass Foo {}\n#[Entry] function main() -> void {}\n";
    let tokens = lex(src).expect("lex ok");
    Parser::new(tokens)
        .parse_program()
        .expect("a `#[…]` on a class now PARSES (2a)");
    let errs =
        check_src(src).expect_err("no class-target attribute exists yet → must fail at check");
    assert!(has_code(&errs, "E-ATTR-TARGET"), "{errs:?}");
}

// ── DEC-194 slice 2b-1: the `#[Attribute]` marker declares a class as a user attribute ──────────

#[test]
fn attribute_marker_declares_a_class_attribute_and_checks_clean() {
    // A class carrying the bare `#[Attribute]` marker IS a user-defined attribute — accepted on a class
    // (the one class-target attribute so far), not `E-ATTR-TARGET`. (The raw `check` path does not
    // enforce the import; import-gating is asserted separately below.)
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Attribute]\nclass Tag { constructor(public string label) {} }\n#[Entry] function main() -> void {}\n";
    assert!(check_src(src).is_ok(), "{:?}", check_src(src));
}

#[test]
fn attribute_marker_with_arguments_is_not_yet_supported() {
    // 2b-1 accepts only the BARE marker; `targets`/`repeatable` arguments arrive in 2b-2, so a marker
    // with arguments is a clean, explicit `E-ATTRIBUTE-ARGS` rather than silent tolerance.
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Attribute(repeatable)]\nclass Tag {}\n#[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("marker with args must fail (2b-1)");
    assert!(has_code(&errs, "E-ATTRIBUTE-ARGS"), "{errs:?}");
}

#[test]
fn attribute_marker_bare_without_import_is_rejected() {
    // The marker obeys "nothing in the wind": bare `#[Attribute]` needs `import Core.Runtime.Attribute;`.
    // Import-gating lives in `enforce_injected_discipline` (the `check_and_expand` path), so assert it
    // directly here.
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Attribute]\nclass Tag {}\n#[Entry] function main() -> void {}\n";
    let prog = Parser::new(lex(src).expect("lex ok"))
        .parse_program()
        .expect("parse ok");
    let errs = enforce_injected_discipline(&prog);
    assert!(
        errs.iter().any(|e| e.code == Some("E-INJECTED-TYPE-BARE")),
        "{errs:?}"
    );
}

#[test]
fn user_attribute_declared_and_applied_to_class_and_function_checks_clean() {
    // DEC-194 2b-3: a class marked `#[Attribute]` is usable as `#[Tag(...)]` on any target (all-targets
    // default this slice), validated against its constructor. (Raw `check` skips import-gating.)
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        #[Attribute]\n\
        class Tag { constructor(public string label) {} }\n\
        #[Tag(\"widget\")]\n\
        class Widget {}\n\
        #[Tag(\"handler\")]\n\
        function process() -> void {}\n\
        #[Entry] function main() -> void { process(); }\n";
    assert!(check_src(src).is_ok(), "{:?}", check_src(src));
}

#[test]
fn user_attribute_wrong_argument_count_is_rejected() {
    // The attribute use is validated against the attribute class's constructor arity (compile-time —
    // the better-than-PHP guarantee).
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        #[Attribute]\n\
        class Tag { constructor(public string label) {} }\n\
        #[Tag()]\n\
        class Widget {}\n\
        #[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("wrong attribute arity must fail");
    assert!(has_code(&errs, "E-ATTRIBUTE-ARITY"), "{errs:?}");
}

#[test]
fn user_attribute_wrong_argument_type_is_rejected() {
    // 2b-3b: each attribute argument is type-checked against the attribute class's constructor parameter
    // at COMPILE time (the better-than-PHP guarantee). `#[Tag(123)]` where `Tag(string label)` is rejected.
    let src = "package Main;\nimport Core.Runtime.Entry;\n\
        #[Attribute]\n\
        class Tag { constructor(public string label) {} }\n\
        #[Tag(123)]\n\
        class Widget {}\n\
        #[Entry] function main() -> void {}\n";
    let errs = check_src(src).expect_err("wrong attribute arg type must fail");
    assert!(has_code(&errs, "E-ATTRIBUTE-ARG-TYPE"), "{errs:?}");
}

#[test]
fn attribute_on_a_non_function_non_class_item_is_still_a_parse_error() {
    // enum/interface/trait/etc. keep the parse-stage rejection until their target slices land.
    let src = "package Main;\nimport Core.Runtime.Entry;\n#[Route(\"GET\", \"/\")]\nenum E { A }\n#[Entry] function main() -> void {}\n";
    let tokens = lex(src).expect("lex ok");
    let err = Parser::new(tokens)
        .parse_program()
        .expect_err("attribute on an enum must fail to parse");
    assert_eq!(err.code, Some("E-ATTR-TARGET"), "{err:?}");
}
