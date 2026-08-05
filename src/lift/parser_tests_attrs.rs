//! LIFT-ATTR parser tests — the `#[…]` grammar, at the level the lifter cannot observe.
//!
//! The end-to-end cases live in `lifter_tests_attrs.rs`. What is pinned HERE is the parsed SHAPE: the
//! name is kept in its source spelling, backslashes and any leading root marker included, because
//! resolution happens one layer up and `\Attribute` vs `Attribute` resolve to different classes. A
//! parser that normalized either away would make the lifter's rule unimplementable.
//!
//! Split into its own file: `parser_tests.rs` sits at its size-gate ceiling (Invariant 13).

use super::ast::{PhpExpr, PhpItem};
use super::lexer::lex_php;
use super::parser::parse_php;

fn attrs_of(php: &str) -> Vec<(String, usize)> {
    let toks = lex_php(php).expect("lex");
    let prog = parse_php(toks).expect("parse");
    prog.items
        .iter()
        .flat_map(|it| match it {
            PhpItem::Function(f) => f.attrs.clone(),
            PhpItem::Class(c) => c.attrs.clone(),
            _ => Vec::new(),
        })
        .map(|a| (a.name, a.args.len()))
        .collect()
}

#[test]
fn a_leading_root_marker_is_kept_in_the_name() {
    // `\Attribute` is PHP's built-in; a bare `Attribute` inside `namespace App;` is `App\Attribute`.
    // Dropping the `\` here would erase the distinction before anything could act on it.
    assert_eq!(
        attrs_of("<?php\n#[\\Attribute]\nclass C {}\n"),
        [("\\Attribute".to_string(), 0)]
    );
    assert_eq!(
        attrs_of("<?php\n#[Attribute]\nclass C {}\n"),
        [("Attribute".to_string(), 0)]
    );
}

#[test]
fn a_qualified_name_keeps_its_backslashes() {
    assert_eq!(
        attrs_of("<?php\n#[ORM\\Column]\nclass C {}\n"),
        [("ORM\\Column".to_string(), 0)]
    );
    assert_eq!(
        attrs_of("<?php\n#[\\Doctrine\\ORM\\Mapping\\Column]\nclass C {}\n"),
        [("\\Doctrine\\ORM\\Mapping\\Column".to_string(), 0)]
    );
}

#[test]
fn one_group_can_hold_several_attributes_and_is_flattened() {
    // PHP's `#[A, B(1)]` is two attributes. The grouping carries no meaning phorj can express (it
    // writes one `#[…]` per attribute), so it is flattened rather than represented.
    assert_eq!(
        attrs_of("<?php\n#[A, B(1)]\nclass C {}\n"),
        [("A".to_string(), 0), ("B".to_string(), 1)]
    );
}

#[test]
fn separate_groups_accumulate_in_source_order() {
    assert_eq!(
        attrs_of("<?php\n#[A]\n#[B]\nclass C {}\n"),
        [("A".to_string(), 0), ("B".to_string(), 0)]
    );
}

#[test]
fn a_named_argument_parses_as_a_named_argument() {
    let toks = lex_php("<?php\n#[Route(path: \"/x\", name: \"home\")]\nclass C {}\n").expect("lex");
    let prog = parse_php(toks).expect("parse");
    let PhpItem::Class(c) = &prog.items[0] else {
        panic!("expected a class");
    };
    let names: Vec<&str> = c.attrs[0]
        .args
        .iter()
        .map(|a| match a {
            PhpExpr::NamedArg { name, .. } => name.as_str(),
            other => panic!("expected a named argument, got {other:?}"),
        })
        .collect();
    assert_eq!(names, ["path", "name"]);
}

#[test]
fn a_static_access_argument_is_not_mistaken_for_a_named_argument() {
    // `::` lexes as its own token, so the `name :` lookahead cannot fire on `Attribute::TARGET_CLASS`.
    let toks = lex_php("<?php\n#[Attribute(Attribute::TARGET_CLASS)]\nclass C {}\n").expect("lex");
    let prog = parse_php(toks).expect("parse");
    let PhpItem::Class(c) = &prog.items[0] else {
        panic!("expected a class");
    };
    assert!(
        !matches!(c.attrs[0].args[0], PhpExpr::NamedArg { .. }),
        "a `::` access was read as a named argument: {:?}",
        c.attrs[0].args[0]
    );
}

#[test]
fn the_line_number_points_at_the_attribute() {
    let toks = lex_php("<?php\n\n\n#[A]\nclass C {}\n").expect("lex");
    let prog = parse_php(toks).expect("parse");
    let PhpItem::Class(c) = &prog.items[0] else {
        panic!("expected a class");
    };
    assert_eq!(c.attrs[0].line, 4);
}

#[test]
fn an_unclosed_attribute_is_a_parse_error_not_a_silent_skip() {
    let toks = lex_php("<?php\n#[A\nclass C {}\n").expect("lex");
    let err = parse_php(toks).expect_err("an unclosed `#[` must not parse");
    assert!(err.contains("close the attribute"), "{err}");
}

/// The argument loop runs `while !at(RParen)`, and `advance()` CLAMPS at the last token rather than
/// running off the end — so an argument list that never closes must be terminated by an ERROR from the
/// expression parser, not by the loop noticing. This pins that it is (a regression here would hang, not
/// fail, which is why it gets its own test rather than being assumed).
#[test]
fn an_unterminated_argument_list_errors_rather_than_spinning() {
    let toks = lex_php("<?php\n#[A(").expect("lex");
    let err = parse_php(toks).expect_err("an unterminated `(` must not parse");
    assert!(!err.is_empty(), "{err}");
}
