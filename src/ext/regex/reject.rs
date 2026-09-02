//! The reject scans behind `Core.Regex` (DEC-461; round-4 panel R1–R7 / F1–F3), split out of
//! `engine.rs` (Invariant 13). Two scans, each ported VERBATIM to PHP in
//! `transpile/runtime_php_regex.rs` (`__phorj_regex_pcre_divergent`, `__phorj_regex_linear_unsupported`)
//! so a DYNAMIC pattern is refused identically on every leg; a LITERAL pattern is refused at check
//! time by `checker/calls/regex.rs` through the same functions.
//!
//! * [`pcre_divergent`] — applies to BOTH engines. Syntax the `regex` crate (and `fancy-regex`, which
//!   delegates the regular subset to it) reads DIFFERENTLY from PCRE, or that only one side accepts:
//!   class-set operators and nested classes (`[a-z&&[^aeiou]]`, `[[ab]]` — PCRE reads the brackets as
//!   characters), POSIX classes (`[[:alpha:]]` — ASCII to the crate, Unicode under PCRE's UCP), `\v`/`\V`
//!   (a vertical-tab LITERAL to the crate, a whitespace CLASS under PCRE), the crate-only `\<` `\>`
//!   `\b{…}` boundaries, the inline `u`/`R` flags, and the PCRE-only constructs neither crate implements
//!   (`\Q…\E`, `(?#…)`, `(?|…)`, `(?'n'…)`, `(?P=n)`, `(?P>n)`, `(?C…)`, `\X`, `\N`, `\0`, `\e`, `\c`).
//!   Every one of these was accepted by both legs with a DIFFERENT meaning, or faulted natively while
//!   PHP ran it, with every leg exiting 0 — the Invariant-14 case-3 shape.
//! * [`linear_unsupported`] — applies to the LINEAR engine only: PCRE's backtracking-only syntax that
//!   `Regex.compileBacktracking` exists for.
//!
//! What the scans are NOT: the `regex` crate's grammar. A dynamic pattern using some further construct
//! the crate refuses and PCRE accepts still faults natively and runs under PHP — disclosed in
//! KNOWN_ISSUES §Core.Regex. Literal patterns have no such gap (the crate itself validates them at
//! check time on every leg).

/// Why a pattern is refused, for the checker's code choice and hint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RejectKind {
    /// PCRE-class syntax the linear engine omits; `compileBacktracking` accepts it.
    LinearOnly,
    /// The native engines and PCRE disagree on it; no constructor makes it byte-identical.
    NotPortable,
    /// The pattern does not parse on the engine that would compile it.
    Invalid,
}

/// Why BOTH engines refuse `pattern`, if they do (see the module doc). Escape- and class-aware.
pub fn pcre_divergent(pattern: &str) -> Option<&'static str> {
    let b = pattern.as_bytes();
    let mut i = 0;
    let mut in_class = false;
    while i < b.len() {
        let c = b[i];
        if c == b'\\' {
            let Some(&n) = b.get(i + 1) else { break };
            match n {
                b'v' | b'V' => {
                    return Some(
                        "the `\\v`/`\\V` escape (a vertical-tab literal to the native engines, a \
                         whitespace class under PCRE)",
                    )
                }
                b'Q' | b'E' => return Some("`\\Q…\\E` quoting (PCRE-only)"),
                b'X' | b'N' | b'e' | b'c' | b'0' => {
                    return Some("a PCRE-only escape (`\\X`, `\\N`, `\\e`, `\\c`, `\\0`)")
                }
                b'<' | b'>' if !in_class => {
                    return Some("the `\\<`/`\\>` word boundaries (literal `<`/`>` under PCRE)")
                }
                b'b' if !in_class && b.get(i + 2) == Some(&b'{') => {
                    return Some("a `\\b{…}` boundary assertion (crate-only)")
                }
                _ => {}
            }
            i += 2;
            continue;
        }
        if in_class {
            match c {
                b']' => in_class = false,
                b'[' => {
                    return Some(if b.get(i + 1) == Some(&b':') {
                        "a POSIX class (`[[:alpha:]]` — ASCII to the native engines, Unicode under \
                         PCRE's UCP; write `\\p{…}` or an explicit range)"
                    } else {
                        "a nested character class (PCRE reads the inner brackets as characters)"
                    });
                }
                b'&' | b'-' | b'~' if b.get(i + 1) == Some(&c) => {
                    return Some(
                        "a class-set operator (`&&`, `--`, `~~` — PCRE reads them as characters)",
                    )
                }
                _ => {}
            }
            i += 1;
            continue;
        }
        match c {
            b'[' => {
                in_class = true;
                i += 1;
                if b.get(i) == Some(&b'^') {
                    i += 1;
                }
                if b.get(i) == Some(&b']') {
                    i += 1;
                }
                continue;
            }
            b'(' if b.get(i + 1) == Some(&b'?') => {
                match (b.get(i + 2), b.get(i + 3)) {
                    (Some(b'#'), _) => return Some("a `(?#…)` comment (PCRE-only)"),
                    (Some(b'|'), _) => return Some("a `(?|…)` branch-reset group (PCRE-only)"),
                    (Some(b'\''), _) => return Some("a `(?'name'…)` group (PCRE-only spelling)"),
                    (Some(b'P'), Some(b'=')) => {
                        return Some("a `(?P=name)` back-reference (PCRE-only spelling)")
                    }
                    (Some(b'P'), Some(b'>')) => return Some("a `(?P>name)` recursion (PCRE-only)"),
                    (Some(b'C'), _) => return Some("a `(?C…)` callout (PCRE-only)"),
                    _ => {}
                }
                // An inline flag group: letters (and `-`) up to `)` or `:`.
                let mut j = i + 2;
                while j < b.len() && (b[j].is_ascii_alphabetic() || b[j] == b'-') {
                    if b[j] == b'u' {
                        return Some(
                            "the inline `u` flag (the native engines are always Unicode; PCRE \
                             refuses `(?u)`/`(?-u)`)",
                        );
                    }
                    if b[j] == b'R' {
                        return Some(
                            "the inline `R` flag (CRLF mode to the native engines, recursion \
                             under PCRE)",
                        );
                    }
                    j += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Why the LINEAR engine refuses `pattern`, if it does. Escape- and character-class-aware, so `\+`
/// and `[+]+` are ordinary. Names the construct so the diagnostic can point at `compileBacktracking`.
pub fn linear_unsupported(pattern: &str) -> Option<&'static str> {
    let b = pattern.as_bytes();
    let mut i = 0;
    let mut in_class = false;
    // Whether the previous token was a quantifier (`*`, `+`, `?`, `{n,m}`), so a following `+` is
    // possessive rather than a second quantifier the crate would nest.
    let mut after_quantifier = false;
    while i < b.len() {
        let c = b[i];
        if c == b'\\' {
            let Some(&n) = b.get(i + 1) else { break };
            if !in_class {
                match n {
                    b'1'..=b'9' | b'g' | b'k' => return Some("a back-reference"),
                    b'h' | b'H' | b'R' | b'Z' | b'G' | b'K' => {
                        return Some("a PCRE-only escape (`\\h`, `\\R`, `\\Z`, `\\G`, `\\K`)")
                    }
                    _ => {}
                }
            }
            i += 2;
            after_quantifier = false;
            continue;
        }
        if in_class {
            if c == b']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'[' => {
                in_class = true;
                // A leading `]` (or `^]`) is literal inside a class.
                i += 1;
                if b.get(i) == Some(&b'^') {
                    i += 1;
                }
                if b.get(i) == Some(&b']') {
                    i += 1;
                }
                after_quantifier = false;
                continue;
            }
            b'(' => {
                if b.get(i + 1) == Some(&b'*') {
                    return Some("a PCRE verb `(*…)`");
                }
                if b.get(i + 1) == Some(&b'?') {
                    match (b.get(i + 2), b.get(i + 3)) {
                        (Some(b'='), _) | (Some(b'!'), _) => return Some("look-ahead"),
                        (Some(b'<'), Some(b'=')) | (Some(b'<'), Some(b'!')) => {
                            return Some("look-behind")
                        }
                        (Some(b'>'), _) => return Some("an atomic group"),
                        (Some(b'('), _) => return Some("a conditional group"),
                        (Some(b'R'), _) | (Some(b'0'..=b'9'), _) | (Some(b'&'), _) => {
                            return Some("a recursive group")
                        }
                        _ => {}
                    }
                }
                after_quantifier = false;
            }
            b'{' => {
                if b.get(i + 1) == Some(&b',') {
                    return Some("a `{,n}` quantifier");
                }
                // A `{n}`/`{n,m}` bound counts as a quantifier for the possessive check below.
                if let Some(close) = b[i..].iter().position(|&x| x == b'}') {
                    let inner = &b[i + 1..i + close];
                    if !inner.is_empty() && inner.iter().all(|x| x.is_ascii_digit() || *x == b',') {
                        i += close + 1;
                        after_quantifier = true;
                        continue;
                    }
                }
                after_quantifier = false;
            }
            b'*' | b'+' | b'?' => {
                if after_quantifier && c == b'+' {
                    return Some("a possessive quantifier");
                }
                // `??`, `*?`, `+?` are lazy (supported); `?` after a quantifier keeps the flag off.
                after_quantifier = c != b'?' || !after_quantifier;
                i += 1;
                continue;
            }
            _ => after_quantifier = false,
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_syntax_passes_both_scans() {
        for p in [
            r"[a-z]+",
            r"[^\]]",
            r"\p{L}+",
            r"[\[\]]",
            r"x{2,3}",
            r"(?i)a",
            r"(?P<n>a)b",
            r"\bword\b",
            r"[a-]",
            r"a\{b",
            r"(?<y>\d{4})-(?<m>\d{2})",
            r"[+]+",
            r"\+",
        ] {
            assert_eq!(pcre_divergent(p), None, "{p}");
            assert_eq!(linear_unsupported(p), None, "{p}");
        }
    }

    #[test]
    fn divergent_syntax_is_named() {
        for p in [
            r"[[ab]]",
            r"[a-z&&[^aeiou]]",
            r"[a-z--b]",
            r"[a~~b]",
            r"[[:alpha:]]",
            r"a\v",
            r"\V",
            r"(?-u)\w",
            r"(?u)a",
            r"(?R)a",
            r"\<a",
            r"a\>",
            r"\b{start}",
            r"\Qa\E",
            r"(?#c)a",
            r"(?|a)",
            r"(?'n'a)",
            r"(?P=n)",
            r"(?P>n)",
            r"(?C)a",
            r"\X",
            r"\N",
            r"\0",
            r"\e",
            r"\cA",
        ] {
            assert!(
                pcre_divergent(p).is_some(),
                "{p} must be rejected on both engines"
            );
        }
    }

    #[test]
    fn linear_only_syntax_is_named() {
        for p in [
            r"a++",
            r"a(?=b)",
            r"(a)\1",
            r"\hx",
            r"a\R",
            r"a\Z",
            r"a{,3}b",
            r"(?>a)b",
            r"a*+",
            r"(?<=a)b",
            r"\k<n>",
            r"(*ACCEPT)a",
        ] {
            assert!(
                linear_unsupported(p).is_some(),
                "{p} must be linear-rejected"
            );
        }
    }
}
