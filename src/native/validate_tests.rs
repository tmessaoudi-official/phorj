use super::*;
use crate::value::Value;

fn b(f: fn(&[Value], &mut String) -> Result<Value, String>, input: &str) -> bool {
    match f(&[Value::Str(input.into())], &mut String::new()).unwrap() {
        Value::Bool(t) => t,
        other => panic!("expected bool, got {other:?}"),
    }
}

// Expected sets pinned to real `php -n` preg_match over the identical patterns.

#[test]
fn is_int_matches_php() {
    for ok in ["42", "-7", "+9", "0", "007"] {
        assert!(b(is_int_native, ok), "{ok}");
    }
    for no in ["", "3.14", "abc", "1e3", "ab c", "+", "-"] {
        assert!(!b(is_int_native, no), "{no}");
    }
}

#[test]
fn is_number_matches_php() {
    for ok in ["42", "-7", "+9", "3.14", "-0.5"] {
        assert!(b(is_number_native, ok), "{ok}");
    }
    for no in ["", "12.", ".5", "1e3", "abc", "1.2.3"] {
        assert!(!b(is_number_native, no), "{no}");
    }
}

#[test]
fn is_alpha_matches_php() {
    for ok in ["abc", "DEADbeef", "Hello"] {
        assert!(b(is_alpha_native, ok), "{ok}");
    }
    for no in ["", "abc1", "ab c", "café"] {
        assert!(!b(is_alpha_native, no), "{no}");
    }
}

#[test]
fn is_alnum_matches_php() {
    for ok in ["42", "abc", "abc1", "DEADbeef", "1e3"] {
        assert!(b(is_alnum_native, ok), "{ok}");
    }
    for no in ["", "ab c", "a-b", "3.14"] {
        assert!(!b(is_alnum_native, no), "{no}");
    }
}

#[test]
fn is_hex_matches_php() {
    for ok in ["42", "abc", "abc1", "DEADbeef", "1e3", "FF00"] {
        assert!(b(is_hex_native, ok), "{ok}");
    }
    for no in ["", "xyz", "g1", "0x1f"] {
        assert!(!b(is_hex_native, no), "{no}");
    }
}

// ctype-class predicates — pinned to PHP `ctype_*` semantics (== Rust `is_ascii_*`). Every one is
// false on "". The `\x0b`/`\x0c`/`\x7f` cases pin the byte-boundaries a regex/std method could miss.
#[test]
fn is_lower_matches_php() {
    for ok in ["abc", "phorj", "z"] {
        assert!(b(is_lower_native, ok), "{ok}");
    }
    for no in ["", "Abc", "abc1", "a b", "café"] {
        assert!(!b(is_lower_native, no), "{no}");
    }
}

#[test]
fn is_upper_matches_php() {
    for ok in ["ABC", "PHORJ", "Z"] {
        assert!(b(is_upper_native, ok), "{ok}");
    }
    for no in ["", "Abc", "ABC1", "A B"] {
        assert!(!b(is_upper_native, no), "{no}");
    }
}

#[test]
fn is_whitespace_matches_php() {
    // ctype_space = { space \t \n 0x0B(vtab) 0x0C(ff) \r }. 0x0B is the byte std
    // `is_ascii_whitespace` omits — so it MUST be counted here to stay byte-identical.
    for ok in [" ", "\t", "\n", "\x0b", "\x0c", "\r", " \t\n\x0b\x0c\r"] {
        assert!(b(is_whitespace_native, ok), "{ok:?}");
    }
    for no in ["", "a", " a ", "\u{00a0}"] {
        assert!(!b(is_whitespace_native, no), "{no:?}");
    }
}

#[test]
fn is_punct_matches_php() {
    for ok in ["!", "!?.", "@#$%", "{}[]"] {
        assert!(b(is_punct_native, ok), "{ok}");
    }
    for no in ["", "a!", "1", " ", "!a"] {
        assert!(!b(is_punct_native, no), "{no}");
    }
}

#[test]
fn is_control_matches_php() {
    for ok in ["\t", "\n", "\r", "\x00", "\x1f", "\x7f"] {
        assert!(b(is_control_native, ok), "{ok:?}");
    }
    for no in ["", "a", "\t a", " "] {
        assert!(!b(is_control_native, no), "{no:?}");
    }
}

#[test]
fn is_visible_matches_php() {
    // ctype_graph = printable EXCLUDING space (0x21–0x7E).
    for ok in ["Phorj!", "a", "~"] {
        assert!(b(is_visible_native, ok), "{ok}");
    }
    for no in ["", "a b", " ", "a\t", "café", "\x7f"] {
        assert!(!b(is_visible_native, no), "{no:?}");
    }
}

#[test]
fn is_printable_matches_php() {
    // ctype_print = printable INCLUDING space (0x20–0x7E).
    for ok in ["Phorj 9!", " ", "a b c"] {
        assert!(b(is_printable_native, ok), "{ok}");
    }
    for no in ["", "a\tb", "a\nb", "café", "\x7f"] {
        assert!(!b(is_printable_native, no), "{no:?}");
    }
}
