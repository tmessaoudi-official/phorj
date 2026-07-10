use phorj::interpreter::interpret;
use phorj::parser::Parser;
use phorj::tokenizer::lex;

/// The complete sample program from the language design spec (§6), verbatim.
const SAMPLE: &str = r#"
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
        return "Hello {name}";
    }
}

function main() -> void {
    Greeter g = Greeter("Tak");
    Output.printLine(g.greet());

    List<Shape> shapes = [Circle(2.0), Rect(3.0, 4.0)];
    for (Shape s in shapes) {
        Output.printLine("area = {area(s)}");
    }
}
"#;

fn run(src: &str) -> Result<String, phorj::diagnostic::Diagnostic> {
    let tokens = lex(src).expect("lex ok");
    let prog = Parser::new(tokens).parse_program().expect("parse ok");
    interpret(&prog)
}

#[test]
fn sample_program_runs_and_prints_expected_output() {
    let out = run(SAMPLE).expect("sample should run clean");
    assert_eq!(out, "Hello Tak\narea = 12.56636\narea = 12\n");
}

#[test]
fn di_inject_in_field_initializer_runs_not_panics() {
    // Regression (DI v1 6C): `inject<T>()` in a FIELD INITIALIZER (not a function body) must be
    // expanded by `desugar_di` — else it survives to the backend and panics `unreachable!`. `desugar_di`
    // walks all expression positions, so this runs and prints the injected value.
    let src = "package Main;\n\
        import Core.Output;\n\
        import Core.DI.Injectable;\n\
        import Core.DI.inject;\n\
        #[Injectable] class Db { constructor() {} function n(): int { return 7; } }\n\
        class Svc {\n\
            private Db db = inject<Db>();\n\
            constructor() {}\n\
            function n(): int { return this.db.n(); }\n\
        }\n\
        function main(): void {\n\
            Svc s = new Svc();\n\
            Output.printLine(\"{s.n()}\");\n\
        }\n";
    // Go through `check_and_expand` (where `desugar_di` lives) then interpret the expanded program —
    // the real `phg run` pipeline. The bare `interpret` helper skips expansion, so it is not the path
    // that exercises DI.
    let tokens = lex(src).expect("lex ok");
    let prog = Parser::new(tokens).parse_program().expect("parse ok");
    let expanded = phorj::cli::check_and_expand(&prog, src).expect("expand ok");
    let out = interpret(&expanded).expect("field-initializer inject should run, not panic");
    assert_eq!(out.trim(), "7");
}

#[test]
fn di_field_injection_synthesizes_constructor_when_absent() {
    // Slice 3: an injectable with an injected field and NO explicit constructor — `fold_injected_fields`
    // must SYNTHESIZE a constructor (the `None` arm) with the promoted param, so the field is wired and
    // set at construction. Exercises the synthesis branch end-to-end (field actually reads back).
    let src = "package Main;\n\
        import Core.Output;\n\
        import Core.DI.Injectable;\n\
        import Core.DI.inject;\n\
        #[Injectable] class Clock { constructor() {} function n(): int { return 3; } }\n\
        #[Injectable] class Logger { private Clock clock; function m(): int { return this.clock.n(); } }\n\
        function main(): void { Logger l = inject<Logger>(); Output.printLine(\"{l.m()}\"); }\n";
    let tokens = lex(src).expect("lex ok");
    let prog = Parser::new(tokens).parse_program().expect("parse ok");
    let expanded = phorj::cli::check_and_expand(&prog, src).expect("expand ok");
    let out = interpret(&expanded).expect("synthesized-ctor field injection should run");
    assert_eq!(out.trim(), "3");
}

#[test]
fn program_without_main_errors() {
    let e = run(r#"function helper() -> int { return 1; }"#).unwrap_err();
    assert!(e.message.contains("main"), "{}", e.message);
}

#[test]
fn division_by_zero_does_not_panic() {
    let e = run(r#"import Core.Output;
function main() -> void { Output.printLine("{1 / 0}"); }"#)
    .unwrap_err();
    assert!(e.message.contains("division by zero"), "{}", e.message);
}
