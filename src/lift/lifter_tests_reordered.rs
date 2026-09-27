//! Row 4i — PHP builtins whose phorj native exists but takes its arguments in another order or shape
//! (`lifter/reordered.rs`, `lifter/sprintf.rs`), so the registry's same-order `lift_from` cannot
//! invert them. Each lift is to the native whose PHP template IS the builtin — never an approximation.

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

fn refused(src: &str) -> String {
    match lift_source(src) {
        Err(e) => e,
        Ok(out) => panic!("expected a refusal, lifted:\n{out}"),
    }
}

#[test]
fn strict_in_array_implode_and_explode_take_the_receiver_form() {
    let out = lifted(
        "<?php\n/** @param list<string> $xs */\n\
         function f(array $xs, string $s): string {\n\
             if (in_array($s, $xs, true)) { return implode(', ', explode(' ', $s)); }\n\
             return implode('-', $xs);\n\
         }",
    );
    assert!(out.contains("List.contains(xs, s)"), "{out}");
    assert!(out.contains("s.split(\" \").join(\", \")"), "{out}");
    assert!(out.contains("xs.join(\"-\")"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_loose_in_array_and_a_limited_explode_are_refused_by_name() {
    // Loose `in_array` compares with `==`: `in_array("1", ["01"])` is TRUE in PHP.
    let e = refused("<?php\n/** @param list<string> $xs */\nfunction f(array $xs, string $s): bool { return in_array($s, $xs); }");
    assert!(e.contains("LOOSE") && e.contains("`true`"), "{e}");
    let e = refused("<?php\nfunction f(string $s): int { return count(explode(',', $s, 2)); }");
    assert!(e.contains("limit"), "{e}");
}

#[test]
fn str_replace_chains_over_a_literal_search_array() {
    let out = lifted(
        "<?php\nfunction f(string $s): string {\n\
             return str_replace(['a', 'b'], '', str_replace(['x', 'y'], ['1', '2'], str_replace('-', '_', $s)));\n\
         }",
    );
    assert!(
        out.contains("s.replace(\"-\", \"_\").replace(\"x\", \"1\").replace(\"y\", \"2\").replace(\"a\", \"\").replace(\"b\", \"\")"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_mismatched_replace_pair_or_a_count_argument_is_refused() {
    // PHP pads a shorter replacement array with "" — spelled out, not guessed.
    let e = refused(
        "<?php\nfunction f(string $s): string { return str_replace(['a', 'b'], ['1'], $s); }",
    );
    assert!(e.contains("same length"), "{e}");
    let e =
        refused("<?php\nfunction f(string $s): string { return str_replace('a', 'b', $s, $n); }");
    assert!(e.contains("count"), "{e}");
}

#[test]
fn a_string_only_sprintf_is_an_interpolation() {
    let out = lifted(
        "<?php\nfunction f(string $a, string $b): string {\n\
             return sprintf('%s de « %s » ' . '— 100%%', $a, strtoupper($b));\n\
         }",
    );
    assert!(
        out.contains("\"{a} de « {b.upperCase()} » — 100%\""),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn an_int_or_padded_directive_or_a_computed_format_is_refused() {
    let e = refused("<?php\nfunction f(int $n): string { return sprintf('%d items', $n); }");
    assert!(e.contains("%d") && e.contains("%s"), "{e}");
    let e = refused("<?php\nfunction f(string $a): string { return sprintf('%5s', $a); }");
    assert!(e.contains("%5s"), "{e}");
    let e =
        refused("<?php\nfunction f(string $fmt, string $a): string { return sprintf($fmt, $a); }");
    assert!(e.contains("literal"), "{e}");
    let e = refused("<?php\nfunction f(string $a): string { return sprintf('%s %s', $a); }");
    assert!(e.contains("2") && e.contains("1"), "{e}");
}

#[test]
fn php_int_max_is_the_int_bound() {
    let out = lifted("<?php\nfunction f(): int { return PHP_INT_MAX; }");
    assert!(out.contains("return 9223372036854775807;"), "{out}");
    checks_clean(&out);
}

#[test]
fn two_effectful_arguments_are_never_reordered() {
    // `xs.join(sep)` evaluates `xs` first; PHP evaluates `$sep` first. With a call on both sides the
    // swap would reorder their effects, so the builtin is left as the plain (loud) call.
    let out = lifted(
        "<?php\nfunction sep(): string { echo 's'; return ','; }\n\
         /** @return list<string> */\nfunction parts(): array { echo 'p'; return ['a']; }\n\
         function f(): string { return implode(sep(), parts()); }\n\
         /** @return list<int> */\nfunction g(): array { return array_map(pick(), ints()); }",
    );
    assert!(out.contains("implode(sep(), parts())"), "{out}");
    assert!(!out.contains(".join("), "{out}");
    assert!(out.contains("array_map(pick(), ints())"), "{out}");
}

#[test]
fn only_a_literal_true_strict_flag_and_an_inert_replacement_qualify() {
    for flag in ["false", "$strict"] {
        let out = lifted(&format!(
            "<?php\n/** @param list<string> $xs */\n\
             function f(array $xs, string $s, bool $strict): bool {{ return in_array($s, $xs, {flag}); }}"
        ));
        assert!(!out.contains("contains"), "{flag}: {out}");
    }
    // A chain re-evaluates the replacement once per needle, so it must not do anything.
    let e = refused(
        "<?php\nfunction r(): string { return '-'; }\n\
         function f(string $s): string { return str_replace(['a', 'b'], r(), $s); }",
    );
    assert!(e.contains("inert"), "{e}");
}

#[test]
fn a_property_read_next_to_an_effect_is_not_reordered_but_two_reads_are() {
    // `build()` may set `$this->sep`: PHP reads the separator BEFORE the call, the receiver form after.
    let out = lifted(
        "<?php\nclass B {\n  public string $sep = ',';\n  public string $a = 'a';\n  public string $b = 'b';\n\
           /** @return list<string> */\n  public function build(): array { $this->sep = ';'; return ['x']; }\n\
           public function f(): string { return implode($this->sep, $this->build()); }\n\
           public function g(string $s): string { return str_replace($this->a, $this->b, $s); }\n}",
    );
    assert!(out.contains("implode(this.sep, this.build())"), "{out}");
    assert!(out.contains("s.replace(this.a, this.b)"), "{out}");
}
