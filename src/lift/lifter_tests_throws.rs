//! Row 4j2(b), DEC-547 — PHP exceptions are unchecked, phorj `throws` is enforced (DEC-068). The lifter
//! gives a function or method the `throws` its own uncaught `throw` statements need, and propagates it
//! along the STATIC call graph: each resolvable call to a throwing callee is spelled `f(…)?` (phorj's
//! propagation marker) and its caller declares the type too. What cannot be declared stays loud.

use super::lifter::lift_source;

fn lifted(src: &str) -> String {
    lift_source(src).unwrap_or_else(|e| panic!("expected a lift, refused: {e}\n{src}"))
}

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

const ERR: &str = "final class BadInputException extends \\RuntimeException {\n\
    public static function at(string $w): self { return new self('bad ' . $w); }\n}\n";

#[test]
fn a_direct_throw_and_its_two_hop_callers_declare_it_and_propagate() {
    let out = lifted(&format!(
        "<?php\n{ERR}\
         function fold(string $s): string {{ if ($s === '') {{ throw BadInputException::at('fold'); }} return $s; }}\n\
         function classify(string $s): string {{ return fold($s) . '!'; }}\n\
         function outer(string $s): string {{ return classify($s); }}\n\
         try {{ echo outer('x'); }} catch (BadInputException $e) {{ echo 'caught'; }}"
    ));
    assert!(
        out.contains("function fold(string s): string throws BadInputException"),
        "{out}"
    );
    assert!(
        out.contains("function classify(string s): string throws BadInputException"),
        "{out}"
    );
    assert!(
        out.contains("function outer(string s): string throws BadInputException"),
        "{out}"
    );
    assert!(
        out.contains("fold(s)?") && out.contains("classify(s)?"),
        "{out}"
    );
    // The top-level call sits in a `try` that catches the type: no `?` (and `main` cannot declare).
    assert!(out.contains(r#"Output.print("{outer(\"x\")}");"#), "{out}");
    checks_clean(&out);
}

#[test]
fn a_try_that_catches_the_type_or_its_mapped_base_stops_propagation() {
    let out = lifted(&format!(
        "<?php\n{ERR}\
         function fold(string $s): string {{ throw new BadInputException($s); }}\n\
         function own(string $s): string {{ try {{ return fold($s); }} catch (BadInputException $e) {{ return ''; }} }}\n\
         function base(string $s): string {{ try {{ return fold($s); }} catch (\\RuntimeException $e) {{ return ''; }} }}"
    ));
    assert!(out.contains("function own(string s): string {"), "{out}");
    assert!(out.contains("function base(string s): string {"), "{out}");
    assert!(!out.contains("fold(s)?"), "{out}");
    checks_clean(&out);
}

#[test]
fn methods_propagate_through_this_self_and_a_static_factory() {
    let out = lifted(&format!(
        "<?php\n{ERR}\
         final class Guard {{\n\
             private function check(string $s): void {{ if ($s === '') {{ throw BadInputException::at('check'); }} }}\n\
             public static function make(): self {{ return new self(); }}\n\
             public function run(string $s): string {{ $this->check($s); return self::label($s); }}\n\
             private static function label(string $s): string {{ if ($s === '?') {{ throw new BadInputException('label'); }} return $s; }}\n\
         }}"
    ));
    assert!(
        out.contains("check(string s): void throws BadInputException"),
        "{out}"
    );
    assert!(
        out.contains("run(string s): string throws BadInputException"),
        "{out}"
    );
    assert!(
        out.contains("label(string s): string throws BadInputException"),
        "{out}"
    );
    assert!(out.contains("this.check(s)?"), "{out}");
    assert!(
        out.contains("Guard::label(s)?") || out.contains("Guard.label(s)?"),
        "{out}"
    );
    assert!(out.contains("make(): Guard {"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_closure_body_and_a_constructor_cannot_declare_so_they_stay_loud() {
    let out = lifted(&format!(
        "<?php\n{ERR}\
         function fold(string $s): string {{ throw new BadInputException($s); }}\n\
         function mapped(string $s): string {{ $f = fn (string $x): string => fold($x); return $f($s); }}\n\
         final class Box {{ public function __construct(string $s) {{ fold($s); }} }}"
    ));
    // Neither the lambda nor the constructor can carry `throws`: no `?` inside either, and the
    // function holding the lambda does not inherit what the lambda's body throws.
    assert!(
        out.contains("function fold(string s): string throws BadInputException"),
        "{out}"
    );
    assert!(
        !out.contains("fold(x)?") && !out.contains("fold(s)?"),
        "{out}"
    );
    assert!(out.contains("function mapped(string s): string {"), "{out}");
}

#[test]
fn a_rethrow_of_a_caught_exception_declares_its_type() {
    let out = lifted(&format!(
        "<?php\n{ERR}\
         function fold(string $s): string {{ throw new BadInputException($s); }}\n\
         function logged(string $s): string {{ try {{ return fold($s); }} catch (BadInputException $e) {{ echo 'log'; throw $e; }} }}"
    ));
    assert!(
        out.contains("function logged(string s): string throws BadInputException"),
        "{out}"
    );
    checks_clean(&out);
}

/// Row 4j1 — a propagated call lifted as a RECEIVER (`strlen(fold($s))` → the receiver form) prints
/// parenthesized: `fold(s)?.m()` would lex `?.` as safe navigation and drop the propagation.
#[test]
fn a_propagated_receiver_is_parenthesized() {
    let out = lifted(&format!(
        "<?php\n{ERR}\
         function fold(string $s): string {{ throw new BadInputException($s); }}\n\
         function size(string $s): int {{ return strlen(fold($s)); }}"
    ));
    assert!(out.contains("(fold(s)?)."), "{out}");
    checks_clean(&out);
}

/// A method inherited from the parent — reached through `$this->` or `parent::` — is resolved up the
/// parent chain, and a thrown PHP base maps (`\RuntimeException` → `RuntimeError`).
#[test]
fn an_inherited_method_is_resolved_through_this_and_parent() {
    let out = lifted(
        "<?php\nclass Base { protected function boom(): void { throw new \\RuntimeException('x'); } }\n\
         final class Child extends Base {\n\
             public function go(): void { $this->boom(); }\n\
             public function up(): void { parent::boom(); }\n\
         }",
    );
    assert!(out.contains("boom(): void throws RuntimeError"), "{out}");
    assert!(out.contains("go(): void throws RuntimeError"), "{out}");
    assert!(out.contains("up(): void throws RuntimeError"), "{out}");
    assert!(out.contains("this.boom()?"), "{out}");
}
