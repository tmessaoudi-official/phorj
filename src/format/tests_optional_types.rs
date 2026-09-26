//! Row 5y — a postfix `?` on a function or intersection type must keep its parentheses.
//!
//! `?` binds to the nearest type, so `(string) => int?` is a function RETURNING `int?` and
//! `A & B?` is an intersection with an optional member. The printer used to parenthesize an
//! optional only around a union, so `((string) => int)? f` formatted to `(string) => int? f` and
//! `(A & B)? x` to `A & B? x` — idempotent, and a different program. Idempotence cannot catch it
//! (the wrong output is a fixed point), which is why these assert that meaning is preserved.

use super::tests::{assert_meaning_preserved, fmt};

#[test]
fn a_nullable_function_type_keeps_its_parentheses() {
    let src = "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n\
         function pick(((int) => int)? f, int k): int { if (f is null) { return -1; } return f(k); }\n\
         function none(): ((int) => int)? { return null; }\n\
         function half(int k): int? { return k / 2; }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
         (int) => int twice = function(int k): int { return k * 2; };\n\
         (int) => int? h = half;\n\
         Output.printLine(\"{pick(none(), 3)} {pick(twice, 3)} {h(8) ?? 0}\");\n}\n";
    let out = fmt(src);
    assert!(out.contains("function pick(((int) => int)? f"), "{out}");
    assert!(out.contains("function none(): ((int) => int)?"), "{out}");
    // Not over-parenthesized: a function RETURNING an optional has no outer `?` to protect.
    assert!(out.contains("(int) => int? h = half"), "{out}");
    assert_meaning_preserved(src);
}

#[test]
fn a_nullable_intersection_type_keeps_its_parentheses() {
    let src = "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n\
         interface A { function a(): int; }\ninterface B { function b(): int; }\n\
         class C implements A, B { function a(): int { return 1; } function b(): int { return 2; } }\n\
         function f((A & B)? x): int { if (x is null) { return 0; } return x.a() + x.b(); }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
         (A & B)? none = null;\n\
         Output.printLine(\"{f(none)} {f(new C())}\");\n}\n";
    let out = fmt(src);
    assert!(out.contains("function f((A & B)? x)"), "{out}");
    assert_meaning_preserved(src);
}
