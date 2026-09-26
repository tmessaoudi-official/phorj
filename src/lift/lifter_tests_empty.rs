//! Row 4h — PHP's empty array `[]` where phorj needs to know the collection's type (DEC-214 part-2:
//! an empty literal is `E-EMPTY-LITERAL` unless it is constructed). Two shapes carry the type in the
//! program itself: a strict `=== []` on a variable DECLARED as a collection, and a `return [];` in a
//! function whose return type is declared. Anything else stays a bare `[]`, so the checker names it.

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

#[test]
fn a_strict_empty_test_on_a_declared_list_is_is_empty_in_both_orders() {
    let out = lifted(
        "<?php\n/** @param list<int> $xs */\n\
         function f(array $xs): bool { return $xs === [] || [] !== $xs; }",
    );
    assert!(
        out.contains("List.isEmpty(xs) || !List.isEmpty(xs)"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_declared_map_and_a_var_annotated_local_are_collections_too() {
    let out = lifted(
        "<?php\n/** @param array<string, int> $m */\n\
         function f(array $m): bool {\n\
             /** @var list<string> $seen */\n\
             $seen = [];\n\
             return $m === [] && $seen === [];\n\
         }",
    );
    assert!(
        out.contains("Map.isEmpty(m) && List.isEmpty(seen)"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn only_a_declared_non_null_collection_under_a_strict_test_is_rewritten() {
    // `"" === []` is FALSE in PHP and `"".isEmpty()` is true: a string must never take the rewrite.
    let out = lifted("<?php\nfunction f(string $s): bool { return $s === []; }");
    assert!(!out.contains("isEmpty"), "{out}");
    // A LOOSE `==` is also true for `null`, `false`, `0` and `\"\"` — not this test either.
    let out = lifted(
        "<?php\n/** @param list<int> $xs */\nfunction f(array $xs): bool { return $xs == []; }",
    );
    assert!(!out.contains("isEmpty"), "{out}");
    // `null === []` is false, and `xs.isEmpty()` on a `List<int>?` would not be the same question.
    let out = lifted(
        "<?php\n/** @param list<int>|null $xs */\nfunction f(?array $xs): bool { return $xs === []; }",
    );
    assert!(!out.contains("isEmpty"), "{out}");
}

#[test]
fn a_rebound_name_can_only_make_the_draft_loud() {
    // The registry is function-scoped, not flow-sensitive: the arrow function's `string $xs`
    // shadows the declared list. The qualified `List.isEmpty` then fails the checker — it never
    // becomes a silent `"".isEmpty()`, which would be TRUE where PHP's `"" === []` is false.
    let out = lifted(
        "<?php\n/** @param list<int> $xs */\n\
         function f(array $xs): bool { $g = fn (string $xs): bool => $xs === []; return $g(\"\"); }",
    );
    assert!(out.contains("List.isEmpty(xs)"), "{out}");
    assert!(!out.contains("xs.isEmpty()"), "{out}");
    let prog = crate::cli::parse_program(&out).expect("draft parses");
    let err = match crate::cli::check_and_expand(&prog, &out) {
        Err(e) => format!("{e:?}"),
        Ok(_) => panic!("a rebound name must fail the checker:\n{out}"),
    };
    assert!(
        err.contains("List.isEmpty") && err.contains("string"),
        "{err}"
    );
}

#[test]
fn a_nested_return_of_an_empty_array_takes_the_declared_return_type() {
    let out = lifted(
        "<?php\n/** @return list<int> */\n\
         function f(bool $a, bool $b): array {\n\
             if ($a) { if ($b) { return []; } }\n\
             return [1];\n\
         }\n\
         /** @return array<string, int> */\n\
         function g(): array { return []; }",
    );
    assert!(out.contains("return new List<int>();"), "{out}");
    assert!(out.contains("return new Map<string, int>();"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_method_return_inside_a_loop_is_typed() {
    let out = lifted(
        "<?php\nclass K {\n\
             /** @return list<string> */\n\
             public function none(): array { foreach ([1] as $i) { return []; } return []; }\n\
         }",
    );
    assert_eq!(
        out.matches("return new List<string>();").count(),
        2,
        "{out}"
    );
    checks_clean(&out);
}
