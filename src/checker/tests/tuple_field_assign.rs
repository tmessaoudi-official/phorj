//! DEC-536 (scout row 5s) — a named tuple's field is assignable through a `mutable` place: a local,
//! then any chain of `[i]` and named-tuple `.f` steps. `kept[0].tags = v` rebuilds that tuple with `tags`
//! replaced (Swift struct semantics; the tuple is a value, nothing aliases). The checker records each
//! tuple step's position exactly as for a read, so every backend sees the positional index chain
//! `kept[0][1] = v` it already supports.

use super::support::*;

fn codes(src: &str) -> Vec<&'static str> {
    errors_of(src).iter().filter_map(|e| e.code).collect()
}

fn clean(src: &str) {
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

fn has(src: &str, code: &str) {
    assert!(
        codes(src).contains(&code),
        "want {code} in {:?}",
        errors_of(src)
    );
}

fn body(b: &str) -> String {
    format!(
        "import Core.List; type Row = (id: int, tags: List<string>); \
         function f(Row r) -> int {{ {b} return 0; }} function main() -> void {{}}"
    )
}

#[test]
fn a_local_tuple_field_is_assignable() {
    clean(&body("mutable Row t = r; t.id = 2;"));
}

#[test]
fn a_list_element_tuple_field_is_assignable() {
    clean(&body(
        "mutable var kept = new List<Row>(); kept = List.append(kept, r); \
         kept[0].tags = List.append(kept[0].tags, \"b\");",
    ));
}

#[test]
fn a_nested_tuple_field_is_assignable() {
    clean(
        "type Inner = (x: int, y: int); type Outer = (inner: Inner, n: int); \
         function f(Outer o) -> int { mutable Outer t = o; t.inner.x = 5; return t.inner.x; } \
         function main() -> void {}",
    );
}

#[test]
fn an_element_of_a_tuple_field_list_is_assignable() {
    clean(&body("mutable Row t = r; t.tags[0] = \"z\";"));
}

#[test]
fn an_immutable_root_is_refused() {
    has(&body("Row t = r; t.id = 2;"), "E-ASSIGN-IMMUTABLE");
}

#[test]
fn a_value_of_the_wrong_type_is_refused() {
    has(&body("mutable Row t = r; t.id = \"x\";"), "E-ASSIGN-TYPE");
}

#[test]
fn an_unknown_or_positional_field_is_refused_by_name() {
    has(
        &body("mutable Row t = r; t.nope = 1;"),
        "E-TUPLE-UNKNOWN-FIELD",
    );
    has(
        "function f((int, int) p) -> int { mutable (int, int) t = p; t.a = 1; return 0; } \
         function main() -> void {}",
        "E-TUPLE-POSITIONAL-FIELD",
    );
}

#[test]
fn a_field_or_call_rooted_place_is_still_refused() {
    // The place must start at a local — the same boundary as `this.f[i] = e` today.
    has(
        "type Row = (id: int, tags: List<string>); \
         class Box { mutable Row row; constructor(Row r) { this.row = r; } \
         function bump(): void { this.row.id = 9; } } function main() -> void {}",
        "E-ASSIGN-TARGET",
    );
}

#[test]
fn a_class_field_step_under_a_local_root_is_refused_not_passed_to_the_compiler() {
    // `b` IS a mutable local, and `b.row` IS a tuple — but `.row` is a CLASS field, which the compiler's
    // index-chain flatten cannot walk. Letting it through would panic there (`unreachable!`).
    has(
        "type Row = (id: int, tags: List<string>); \
         class Box { mutable Row row; constructor(Row r) { this.row = r; } } \
         function f(Row r) -> int { mutable Box b = new Box(r); b.row.id = 9; return 0; } \
         function main() -> void {}",
        "E-ASSIGN-TARGET",
    );
    has(
        "type Row = (id: int, tags: List<string>); \
         class Box { mutable List<int> xs; constructor() { this.xs = [1]; } } \
         function f() -> int { mutable Box b = new Box(); b.xs[0] = 9; return 0; } \
         function main() -> void {}",
        "E-ASSIGN-TARGET",
    );
}

#[test]
fn a_call_rooted_place_is_refused() {
    has(
        "type Row = (id: int, tags: List<string>); function mk(Row r) -> Row { return r; } \
         function f(Row r) -> int { mk(r).id = 1; return 0; } function main() -> void {}",
        "E-ASSIGN-TARGET",
    );
}
