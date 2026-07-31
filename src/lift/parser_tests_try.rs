//! LIFT-TRY parser tests — `try`/`catch`/`finally` entering the lift subset (2026-07-31).
//!
//! A sibling of `parser_tests.rs` rather than an addition to it: that file is a grandfathered
//! Invariant-13 breach, so the size gate fails when it grows.

use super::ast::PhpItem;
use super::parser::parse_php;

/// Parse a PHP fixture, panicking with the lift error if it does not.
fn parse(src: &str) -> super::ast::PhpProgram {
    parse_php(super::lexer::lex_php(src).expect("lexes")).expect("parses")
}

/// The lift error for a fixture that must NOT parse.
fn perr(src: &str) -> String {
    match super::lexer::lex_php(src) {
        Err(e) => e,
        Ok(toks) => parse_php(toks).expect_err("must fail").to_string(),
    }
}

/// LIFT-TRY: the shapes real PHP actually writes — a root-qualified type, a UNION type, PHP 8's
/// variable-less `catch (T)`, and `try`/`finally` with no catch at all.
#[test]
fn parses_try_catch_finally() {
    use crate::lift::ast::PhpStmt;
    let p =
        parse("<?php try { foo(); } catch (\\RuntimeException $e) { bar(); } finally { baz(); }");
    let PhpItem::Stmt(PhpStmt::Try {
        body,
        catches,
        finally_block,
    }) = &p.items[0]
    else {
        panic!("expected a try, got {:?}", p.items[0]);
    };
    assert_eq!(body.len(), 1);
    assert_eq!(catches.len(), 1);
    assert_eq!(catches[0].types, vec!["\\RuntimeException".to_string()]);
    assert_eq!(catches[0].var.as_deref(), Some("e"));
    assert!(finally_block.is_some());

    // A UNION catch keeps every member — narrowing to the first would change what is caught.
    let p = parse("<?php try { foo(); } catch (A | B $e) { bar(); }");
    let PhpItem::Stmt(PhpStmt::Try { catches, .. }) = &p.items[0] else {
        panic!("expected a try");
    };
    assert_eq!(catches[0].types, vec!["A".to_string(), "B".to_string()]);

    // PHP 8's variable-less form: no `$e` at all.
    let p = parse("<?php try { foo(); } catch (A) { bar(); }");
    let PhpItem::Stmt(PhpStmt::Try { catches, .. }) = &p.items[0] else {
        panic!("expected a try");
    };
    assert_eq!(catches[0].var, None);

    // `try`/`finally` with NO catch is legal PHP and must parse.
    let p = parse("<?php try { foo(); } finally { baz(); }");
    let PhpItem::Stmt(PhpStmt::Try {
        catches,
        finally_block,
        ..
    }) = &p.items[0]
    else {
        panic!("expected a try");
    };
    assert!(catches.is_empty());
    assert!(finally_block.is_some());
}

/// A bare `try { … }` with neither arm is a PHP syntax error; the lifter must SAY so rather than
/// quietly treating it as a block.
#[test]
fn rejects_a_try_with_no_catch_and_no_finally() {
    let e = perr("<?php try { foo(); }");
    assert!(e.contains("at least one `catch` or a `finally`"), "got {e}");
}
