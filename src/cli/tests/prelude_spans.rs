//! KNOWN_ISSUES §interpolation-spans: an injected prelude's spans must ALL sit at or above
//! `INJECTED_SPAN_BASE`, including the sub-expressions of an interpolation. The rebaser used to shift
//! every token's `Span.start` but not the offset a string token's interpolation segment carries, so
//! an interpolated call in a prelude landed in the USER span range, where a span-keyed rewrite could
//! collide with a user call site.

use super::super::prelude_spans::lex_parse_injected;
use super::super::INJECTED_SPAN_BASE;

/// Every `start: N` in the parsed program's debug rendering — each `Span` the parser produced.
fn span_starts(debug: &str) -> Vec<usize> {
    debug
        .split("start: ")
        .skip(1)
        .map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().expect("a span start is a number")
        })
        .collect()
}

#[test]
fn an_injected_prelude_interpolation_keeps_its_spans_in_the_injected_range() {
    let src = "package Core.Probe;\n\
               function show(string x): string { return \"<{x.lowerCase()}|{ \"{x.upperCase()}\" }>\"; }\n";
    let program = lex_parse_injected(src, 0).expect("the probe prelude parses");
    let starts = span_starts(&format!("{program:?}"));
    // Non-vacuity: the probe has a package, a function, two interpolations and a nested string, so
    // a rendering that stopped exposing spans cannot pass on an empty list.
    assert!(
        starts.len() >= 10,
        "expected the probe to expose many spans, found {starts:?}"
    );
    let low: Vec<usize> = starts
        .into_iter()
        .filter(|s| *s < INJECTED_SPAN_BASE)
        .collect();
    assert!(
        low.is_empty(),
        "prelude spans below INJECTED_SPAN_BASE collide with user offsets: {low:?}"
    );
}
