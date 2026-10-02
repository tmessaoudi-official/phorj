//! Parity review C1/C2 — a lifted class `implements` a PHP builtin interface phorj has no counterpart
//! for, and `phg check` rejects the draft with `E-IFACE-IMPL` ("not an interface"). The lifter's
//! contract is that what it cannot do is said LOUDLY (`// CANNOT LIFT:`), and that a draft it can
//! finish type-checks.

use crate::lift::lifter::lift_source;

fn check(phg: &str) -> Result<(), String> {
    let prog = crate::cli::parse_program(phg).map_err(|e| format!("parse: {e}"))?;
    crate::cli::check_and_expand(&prog, phg).map(|_| ())
}

fn lifted(src: &str) -> String {
    lift_source(src).unwrap_or_else(|e| panic!("expected a lift, refused: {e}\n{src}"))
}

const BAG: &str = "<?php\nclass Bag implements Countable {\n\
    /** @param list<int> $items */\n\
    public function __construct(private array $items) {}\n\
    public function count(): int { return 3; }\n}\n";

#[test]
fn a_php_builtin_interface_is_dropped_and_named_in_a_cannot_lift_note() {
    let out = lifted(BAG);
    assert!(!out.contains("implements Countable"), "{out}");
    assert!(
        out.contains("// CANNOT LIFT: `Bag` implements the PHP builtin interface `Countable`"),
        "{out}"
    );
    check(&out).unwrap_or_else(|e| panic!("the draft must type-check: {e}\n{out}"));
}

#[test]
fn stringable_is_dropped_without_a_note_because_to_string_is_lifted() {
    let src = "<?php\nclass Tag implements Stringable {\n\
        public function __construct(private string $n) {}\n\
        public function __toString(): string { return $this->n; }\n}\n";
    let out = lifted(src);
    assert!(!out.contains("implements"), "{out}");
    assert!(out.contains("#[ToString]"), "{out}");
    assert!(!out.contains("CANNOT LIFT"), "nothing is lost here: {out}");
    check(&out).unwrap_or_else(|e| panic!("the draft must type-check: {e}\n{out}"));
}

#[test]
fn a_leading_backslash_and_mixed_case_still_match() {
    let out = lifted(
        "<?php\nclass Z implements \\JsonSerializable, \\iteratoraggregate {\n\
         public function jsonSerialize(): int { return 1; }\n}\n",
    );
    assert!(!out.contains("class Z implements"), "{out}");
    assert!(
        out.contains("`JsonSerializable`") && out.contains("`IteratorAggregate`"),
        "{out}"
    );
}

#[test]
fn a_user_declared_interface_is_kept() {
    let out = lifted(
        "<?php\ninterface Shape { public function area(): int; }\n\
         class Sq implements Shape { public function area(): int { return 1; } }\n",
    );
    assert!(out.contains("implements Shape"), "{out}");
    assert!(!out.contains("CANNOT LIFT"), "{out}");
}

#[test]
fn notes_are_deduped_and_in_first_seen_order() {
    let src =
        "<?php\nclass A implements Countable { public function count(): int { return 1; } }\n\
        class B implements Countable { public function count(): int { return 2; } }\n";
    let out = lifted(src);
    assert_eq!(out.matches("// CANNOT LIFT: `A`").count(), 1, "{out}");
    assert_eq!(out.matches("// CANNOT LIFT: `B`").count(), 1, "{out}");
    assert!(out.find("`A`").unwrap() < out.find("`B`").unwrap(), "{out}");
}
