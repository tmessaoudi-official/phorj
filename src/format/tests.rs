//! `phg format` tests. Two invariants gate the formatter:
//!   * **meaning preservation** — for runnable programs, `cmd_treewalk(src) == cmd_treewalk(fmt(src))`;
//!   * **idempotence** — `fmt(fmt(src)) == fmt(src)` (across the full language surface).
//!
//! Plus: comments survive, and an unparseable file is refused (never reformatted).

use super::format;
use crate::cli::cmd_treewalk;

fn fmt(src: &str) -> String {
    format(src).unwrap_or_else(|e| panic!("fmt failed: {e:?}\n--- src ---\n{src}"))
}

/// `fmt` is idempotent: a second pass changes nothing.
fn assert_idempotent(src: &str) {
    let once = fmt(src);
    let twice = fmt(&once);
    assert_eq!(
        once, twice,
        "not idempotent.\n--- once ---\n{once}\n--- twice ---\n{twice}"
    );
}

/// Formatting preserves runtime behavior: the program runs identically before and after.
fn assert_meaning_preserved(src: &str) {
    let before = cmd_treewalk(src);
    let after = cmd_treewalk(&fmt(src));
    assert_eq!(
        before, after,
        "behavior changed by fmt.\n--- before ---\n{before:?}\n--- after ---\n{after:?}\n--- fmt ---\n{}",
        fmt(src)
    );
    assert!(before.is_ok(), "sample must run: {before:?}");
    assert_idempotent(src);
}

#[test]
fn runnable_programs_keep_their_behavior() {
    // A spread of real surface: classes+ctor promotion, enums+match+guards, generics, lambdas+pipe,
    // optionals, ranges, string interpolation.
    let samples = [
        "package Main; import Core.Output;\nfunction main(): void { Output.printLine(\"hi\"); }",
        "package Main; import Core.Output;\n\
         function add<T>(T a, T b): T { return a; }\n\
         function main(): void { Output.printLine(\"{add(2, 3)}\"); }",
        "package Main; import Core.Output;\n\
         enum Shape { Circle(int r), Square(int s) }\n\
         function area(Shape s): int { return match (s) { Circle(r) => r * r, Square(x) => x * x }; }\n\
         function main(): void { Output.printLine(\"{area(new Circle(3))}\"); }",
        "package Main; import Core.Output;\n\
         class Point { constructor(public int x, public int y) {} function sum(): int { return this.x + this.y; } }\n\
         function main(): void { Point p = new Point(2, 5); Output.printLine(\"{p.sum()}\"); }",
        "package Main; import Core.Output;\n\
         function main(): void { var dbl = function(int x): int => x * 2; Output.printLine(\"{3 |> dbl}\"); }",
        "package Main; import Core.Output;\n\
         function main(): void { int? m = null; Output.printLine(\"{m ?? -1}\"); for (int i in 0..3) { Output.printLine(\"{i}\"); } }",
    ];
    for s in samples {
        assert_meaning_preserved(s);
    }
}

#[test]
fn full_surface_is_idempotent() {
    // Non-runnable fragments (library shapes): proves the printer handles every construct and is
    // stable. Idempotence + clean parse is the gate where we can't run the program.
    let samples = [
        "package Main;\ninterface Speaker { function speak(): string; }\n\
         class Dog implements Speaker { function speak(): string { return \"woof\"; } }",
        "package Main;\ntrait Greet { function hi(): string { return \"hi\"; } }\n\
         class P { use Greet; }",
        "package Main;\nclass Box<T> { constructor(public T value) {} }\n\
         function pick(int | string x): int { return 0; }",
        "package Main;\ntype UserId = int;",
        "package Main;\nclass C { int n { get => 1; } }",
        "package Main;\n\
         function f(): void { try { throw 1; } catch (Error e) { return; } finally { return; } }",
        "package Main;\nclass Pt { constructor(public int x, public int y) {} }\n\
         function f(Pt p): void { var Pt { x, y } = p; }",
        "package Main;\nfunction f(): void { var b = b\"\\x00ab\"; }",
        "package Main; import Core.Test;\ntest \"x\" { Test.assertTrue(true); }",
        "package Main;\nenum Opt<T> { Some(T v), None }\n\
         function f(Opt<int> o): int { return match (o) { Some(n) when n > 0 => n, Some(n) => 0, None() => -1 }; }",
    ];
    for s in samples {
        assert_idempotent(s);
    }
}

#[test]
fn the_arrow_return_syntax_normalizes_to_colon() {
    // `-> T` is a transition alias for `: T`; fmt canonicalizes to `:` (both parse the same).
    let out = fmt("package Main;\nfunction f() -> int { return 1; }");
    assert!(out.contains("function f(): int"), "{out}");
    assert!(!out.contains("->"), "{out}");
}

#[test]
fn comments_are_preserved() {
    let src = "package Main;\n// a header comment\nfunction main(): void { /* body */ return; }\n";
    let out = fmt(src);
    assert!(
        out.contains("// a header comment"),
        "own-line comment lost:\n{out}"
    );
    assert!(out.contains("/* body */"), "block comment lost:\n{out}");
    assert_eq!(out, fmt(&out), "comment-bearing output must be idempotent");
}

// ── width-canonical wrapping (DEC-187) ─────────────────────────────────────────────────────────

/// A statement value that overflows the column budget wraps; a short one stays on one line.
#[test]
fn long_call_args_wrap_short_stay_flat() {
    let src = "package Main;\nfunction main(): void {\n\
        var s = f(1, 2);\n\
        var l = someHelperWithAVeryLongName(argumentOne, argumentTwo, argumentThree, argumentFour, argumentFive);\n\
        }";
    let out = fmt(src);
    assert!(
        out.contains("var s = f(1, 2);"),
        "short call must stay flat:\n{out}"
    );
    assert!(
        out.contains("var l = someHelperWithAVeryLongName(\n        argumentOne,\n"),
        "long call args must wrap one per line:\n{out}"
    );
    assert_idempotent(src);
}

/// A long method chain breaks before each `.`; a gratuitously-broken SHORT chain collapses (the
/// width-canonical behaviour DEC-187 chose over preserving author breaks).
#[test]
fn method_chains_wrap_by_width_not_author_breaks() {
    // Long chain → breaks before each dot.
    let long = "package Main;\nfunction main(): void {\n\
        var r = source.mapEachValueWithCare(transformer).keepEveryMatching(predicate).collapseInto(combiner).done();\n\
        }";
    let out = fmt(long);
    assert!(
        out.contains("var r = source\n        .mapEachValueWithCare(transformer)\n"),
        "long chain must break before each dot:\n{out}"
    );
    assert_idempotent(long);

    // Gratuitously hand-broken SHORT chain → collapses to one line.
    let broken = "package Main;\nfunction main(): void {\n\
        var x = obj\n.a()\n.b();\n}";
    let out = fmt(broken);
    assert!(
        out.contains("var x = obj.a().b();"),
        "short chain must collapse:\n{out}"
    );
}

/// CRITICAL: an interpolation hole never breaks, even when the whole line overflows — a newline
/// inside `"{…}"` would change the string's value. The hole here is a *single call whose argument
/// list exceeds 100 columns* — a construct that DOES carry a width break point (`args_doc`), so the
/// test has teeth: if the hole were ever rendered width-aware (the plausible regression), those args
/// would break across lines and this assertion would fail. Correct behaviour keeps it one physical
/// line (the hole is emitted as flat `Text`).
#[test]
fn interpolation_holes_never_break() {
    let src = "package Main;\n\
        function main(): void {\n\
        var wide = \"value is {computeThing(alphaValue, betaValue, gammaValue, deltaValue, epsilonValue, zetaValue)}\";\n\
        }";
    let out = fmt(src);
    // The interpolated string literal must remain on a single physical line (no `\n` between the
    // quotes): the one output line bearing the hole must open AND close the string on itself.
    let hole_lines: Vec<&str> = out.lines().filter(|l| l.contains("value is {")).collect();
    assert_eq!(
        hole_lines.len(),
        1,
        "interpolation was split across lines:\n{out}"
    );
    assert!(
        hole_lines[0].trim_end().ends_with("\";"),
        "interpolation hole did not close on its own line:\n{}",
        hole_lines[0]
    );
    assert!(
        hole_lines[0].contains("zetaValue)}"),
        "the last hole argument left its line (hole broke):\n{}",
        hole_lines[0]
    );
    assert_idempotent(src);
}

/// A `match` expression that overflows wraps one arm per line; a short one stays inline.
#[test]
fn match_arms_wrap_by_width() {
    let src = "package Main;\n\
        function classify(int x): string {\n\
        return match (x) { 0 => \"zero value here\", 1 => \"one value here\", 2 => \"two value here\", 3 => \"three\" };\n\
        }";
    let out = fmt(src);
    assert!(
        out.contains("return match (x) {\n        0 => \"zero value here\","),
        "long match must wrap one arm per line:\n{out}"
    );
    assert_idempotent(src);
}

#[test]
fn unparseable_source_is_refused_not_reformatted() {
    assert!(format("package Main;\nfunction (").is_err());
    assert!(format("@@@ not phorj").is_err());
}

#[test]
fn declaration_visibility_survives_formatting() {
    // Regression: the printer used to drop top-level `internal`/`private` visibility on free functions
    // and types (only `Public` is the default and elided). Splitting public types across files relies
    // on these surviving, so assert they round-trip and the form is idempotent.
    let src = "package Main;\n\
        internal function scale(int n): int { return n; }\n\
        private function clamp(int n): int { return n; }\n\
        internal class Helper { constructor() {} }\n\
        private enum Mode { On(), Off() }\n\
        function main(): void {}";
    let out = fmt(src);
    assert!(
        out.contains("internal function scale"),
        "lost internal fn:\n{out}"
    );
    assert!(
        out.contains("private function clamp"),
        "lost private fn:\n{out}"
    );
    assert!(
        out.contains("internal class Helper"),
        "lost internal class:\n{out}"
    );
    assert!(
        out.contains("private enum Mode"),
        "lost private enum:\n{out}"
    );
    assert_idempotent(src);
}

/// DEC-239 fidelity: the pipe operator survives formatting as `|>` — it must NOT reformat into the
/// lowered call form (`x |> f` → `f(x)` was the pre-DEC-239 defect: the parser lowered pipes before
/// the printer ever saw them). Chains and precedence-parenthesized operands round-trip too.
#[test]
fn pipe_operator_survives_formatting() {
    let src = "package Main; import Core.Output;\n\
         function inc(int x): int { return x + 1; }\n\
         function main(): void { Output.printLine(\"{5 |> inc |> inc}\"); }";
    let out = fmt(src);
    assert!(
        out.contains("5 |> inc |> inc"),
        "pipe rewritten out by fmt:\n{out}"
    );
    assert_meaning_preserved(src);
    assert_idempotent(src);
}

/// DEC-253: the `A | B | null` nullable-union spelling canonicalizes to `(A | B)?` on format
/// (both spellings are the same type); a lone non-null remainder is just `T?`. Idempotent.
#[test]
fn nullable_union_spelling_canonicalizes() {
    let src = "package Main;\n\
         class A { constructor(public int x) {} }\n\
         class B { constructor(public string s) {} }\n\
         function f(): A | B | null { return null; }\n\
         function g(): A | null { return null; }\n\
         function main(): void {}";
    let out = fmt(src);
    assert!(out.contains("function f(): (A | B)?"), "{out}");
    assert!(out.contains("function g(): A?"), "{out}");
    assert_idempotent(src);
}
