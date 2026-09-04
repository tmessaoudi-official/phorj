//! `Core.Database` typed-hydration diagnostics (`queryInto` / `queryScalar` / `queryMap`). These
//! live in the `desugar_db` pass, which runs in the CLI front end rather than `checker::check`, so
//! they go through `front_end_diagnostics` — the same pipeline `phg check` and the LSP run.
use super::support::prog;

fn db_errors(classes: &str, body: &str) -> Vec<crate::diagnostic::Diagnostic> {
    let src = format!(
        "import Core.Database; import Core.Database.Connection; {classes} \
         function main() -> void {{ Connection db = new Connection(\"sqlite::memory:\"); {body} }}"
    );
    crate::cli::front_end_diagnostics(&prog(&src))
}

fn has(classes: &str, body: &str, code: &str) {
    let e = db_errors(classes, body);
    assert!(
        e.iter().any(|d| d.code == Some(code)),
        "expected {code}, got {e:?}"
    );
}

const USER: &str = "class User { constructor(public string name, public int age) {} }";

#[test]
fn query_into_needs_a_row_type_to_infer_from() {
    has(
        USER,
        "var rows = db.prepare(\"SELECT 1\").queryInto();",
        "E-DB-INTO-NO-TYPE",
    );
}

#[test]
fn a_row_class_needs_a_constructor() {
    has(
        "class Bare { }",
        "List<Bare> rows = db.prepare(\"SELECT 1\").queryInto();",
        "E-DB-HYDRATE-NO-CTOR",
    );
}

#[test]
fn every_constructor_parameter_must_be_a_promoted_field() {
    has(
        "class U { constructor(string name) {} }",
        "List<U> rows = db.prepare(\"SELECT 1\").queryInto();",
        "E-DB-HYDRATE-UNPROMOTED",
    );
}

#[test]
fn a_field_needs_a_column_accessor() {
    // NOT `List<int>`: a scalar list is a legitimate ARRAY column (DEC-208 slice K). A `Map` has no
    // column shape at all, which is what this code is for.
    has(
        "class U { constructor(public Map<string, int> m) {} }",
        "List<U> rows = db.prepare(\"SELECT 1\").queryInto();",
        "E-DB-HYDRATE-FIELD-TYPE",
    );
}

#[test]
fn query_scalar_reads_into_a_scalar() {
    has(
        USER,
        "User n = db.prepare(\"SELECT 1\").queryScalar();",
        "E-DB-SCALAR-BAD-TYPE",
    );
}

#[test]
fn query_map_needs_a_map_sink() {
    has(
        USER,
        "var m = db.prepare(\"SELECT 1\").queryMap();",
        "E-DB-MAP-BAD-SINK",
    );
    has(
        USER,
        "List<int> m = db.prepare(\"SELECT 1\").queryMap();",
        "E-DB-MAP-BAD-SINK",
    );
}

#[test]
fn query_map_key_and_value_types_are_checked() {
    has(
        USER,
        "Map<float, string> m = db.prepare(\"SELECT 1\").queryMap();",
        "E-DB-MAP-KEY-TYPE",
    );
    has(
        USER,
        "Map<string, Map<string, int>> m = db.prepare(\"SELECT 1\").queryMap();",
        "E-DB-MAP-VALUE-TYPE",
    );
}
