//! Tests: `for`-`in` over iterators, maps, and error propagation.
use super::super::*;
use super::wp;

#[test]
fn foreach_over_iterator_implementor_runs_on_both_backends() {
    // DEC-257: any `Core.IteratorModule<T>` implementor is foreach-able — the checker lowers the loop
    // to a hasNext/next while-pull before either backend runs. Also exercises the
    // interface-typed receiver (`Iterator<int> it`) and a nested lowered loop.
    let src = wp(r#"import Core.Output;
import Core.IteratorModule;
import Core.IteratorModule.Iterator;
class Countdown implements Iterator<int> {
    constructor(private mutable int n) {}
    function hasNext(): bool { return this.n > 0; }
    function next(): int { this.n = this.n - 1; return this.n + 1; }
}
function total(Iterator<int> it): int {
    mutable int s = 0;
    for (int x in it) { s = s + x; }
    return s;
}
function main(): void {
    for (int a in new Countdown(2)) {
        for (int b in new Countdown(a)) { Output.printLine("{a}:{b}"); }
    }
    Output.printLine("sum={total(new Countdown(4))}");
}"#);
    let expected = "2:2\n2:1\n1:1\nsum=10\n";
    assert_eq!(cmd_run(&src).unwrap(), expected);
    assert_eq!(cmd_treewalk(&src).unwrap(), expected);
}

#[test]
fn foreach_over_throwing_iterator_requires_declare_or_catch() {
    // DEC-257 ruled auto-propagation: a throwing `next()` makes the loop legal only when the
    // fault is caught by an enclosing try OR declared by the enclosing function.
    let base = r#"import Core.Output;
import Core.IteratorModule;
class FeedError implements Error { constructor(public string message) {} }
class Feed implements Iterator<int> {
    constructor(private mutable int n) {}
    function hasNext(): bool { return this.n > 0; }
    function next(): int throws FeedError { this.n = this.n - 1; return this.n + 1; }
}
"#;
    // Undeclared and uncaught — the loop site errors.
    let bad = wp(&format!(
        "{base}function main(): void {{ for (int x in new Feed(2)) {{ Output.printLine(\"{{x}}\"); }} }}"
    ));
    let err = cmd_run(&bad).unwrap_err();
    assert!(err.contains("E-CALL-UNHANDLED"), "{err}");
    assert!(err.contains("iterating this value can throw"), "{err}");
    // Caught by an enclosing try — clean, and runs.
    let good = wp(&format!(
        "{base}function main(): void {{ try {{ for (int x in new Feed(2)) {{ Output.printLine(\"{{x}}\"); }} }} catch (FeedError e) {{ Output.printLine(\"caught\"); }} }}"
    ));
    let expected = "2\n1\n";
    assert_eq!(cmd_run(&good).unwrap(), expected);
    assert_eq!(cmd_treewalk(&good).unwrap(), expected);
}

#[test]
fn foreach_untyped_key_value_bindings_infer_from_the_map() {
    // DEC-280: `foreach (m as k => v)` — both bindings inferred, exactly like the single-binding
    // form; mixed typed/untyped is legal too. The values are usable at their inferred types
    // (`v + 0` proves int).
    let src = wp(r#"import Core.Output;
function main(): void {
    var m = ["a" => 1, "b" => 2];
    foreach (m as k => v) { Output.printLine("{k}={v + 0}"); }
    foreach (m as string k2 => v2) { Output.printLine("{k2}:{v2}"); }
}"#);
    let expected = "a=1\nb=2\na:1\nb:2\n";
    assert_eq!(cmd_run(&src).unwrap(), expected);
    assert_eq!(cmd_treewalk(&src).unwrap(), expected);
}

#[test]
fn foreach_over_non_iterator_class_still_errors() {
    let src = wp(r#"import Core.Output;
class Plain {}
function main(): void { for (int x in new Plain()) { Output.printLine("{x}"); } }"#);
    let err = cmd_run(&src).unwrap_err();
    assert!(err.contains("`for`-`in` requires"), "{err}");
    assert!(err.contains("Iterator<T>"), "{err}");
}

#[test]
fn no_import_means_no_method_position_sugar() {
    // DEC-274 rule (3): nothing in the wind — without the module OR function import, the method
    // form does not compile.
    let src = wp(r#"import Core.Output;
function main(): void { Output.printLine("abc".upperCase()); }"#);
    let err = cmd_run(&src).unwrap_err();
    assert!(err.contains("no method `upperCase`"), "{err}");
}
