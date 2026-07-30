//! Tests for `desugar_db` — split out of `desugar_db.rs` (Invariant 13, M-Decomp).
//!
//! `desugar_db.rs` is the largest file in the tree at ~3150 lines. DEC-356 added three `Expr` recursion
//! arms and a `Stmt` leaf-macro arm there, so the inline test module moved here rather than pushing the
//! parent further over its ceiling.
use super::snake_case;

#[test]
fn snake_case_camel_boundaries() {
    // The `SnakeToCamel` core case (DEC-208 slice B2): a camelCase field → its snake_case column.
    assert_eq!(snake_case("userName"), "user_name");
    assert_eq!(snake_case("firstName"), "first_name");
    assert_eq!(snake_case("streetName"), "street_name");
    assert_eq!(snake_case("postalCode"), "postal_code");
    assert_eq!(snake_case("homeAddress"), "home_address");
}

#[test]
fn snake_case_acronyms() {
    // An interior/trailing ACRONYM run stays together; the run ends where a lowercase word begins.
    assert_eq!(snake_case("userId"), "user_id");
    assert_eq!(snake_case("userID"), "user_id");
    assert_eq!(snake_case("httpServer"), "http_server");
    assert_eq!(snake_case("parseHTTPResponse"), "parse_http_response");
}

#[test]
fn snake_case_digits_and_noop() {
    // A digit is a word boundary before an uppercase; an all-lowercase name is unchanged.
    assert_eq!(snake_case("field2Name"), "field2_name");
    assert_eq!(snake_case("id"), "id");
    assert_eq!(snake_case("plain"), "plain");
}
