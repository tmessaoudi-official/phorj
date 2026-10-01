//! Scout row 4l-b3 (DEC-554) — can a translated pattern match the EMPTY string?
//!
//! `preg_match_all` reads every match, and PCRE and phorj's engines disagree on where an empty match
//! lands (`/\d*/` on `a1b22c`: 6 matches in PHP, 4 in phorj). The lift therefore lifts only a pattern
//! PROVEN to consume at least one character on every match.
//!
//! The proof is a WHITELIST, not a blacklist: a small recursive-descent parse that knows the minimum
//! length of exactly the constructs below, and refuses — by name — anything else. A blacklist of
//! escapes that "look empty" misses the next one (`\K`, `\k<x>`, `\g1`, `\h`, `\R` all slipped past
//! the first version of this check); a construct this parse has never heard of cannot be proven to
//! consume, so it is not lifted.
//!
//! Known: a literal, `.`, a class `[…]`, `\d \D \w \W \s \S \t \n \r \f`, an escaped punctuation
//! character, `\p{…}` / `\pL` / `\P…`, the zero-width `\b \B \A \z \Z ^`, groups `( )`, `(?: )`,
//! `(?<name> )`, `(?P<name> )`, `(?> )` and the look-arounds, alternation, and the quantifiers
//! `* + ? {n} {n,} {n,m}` (each optionally lazy or possessive).

/// `Ok(())` when every match consumes at least one character; otherwise why it is not proven.
pub(super) fn consumes(pattern: &str) -> Result<(), String> {
    let body = strip_flag_prefix(pattern);
    let mut p = Parse {
        c: body.chars().collect(),
        i: 0,
    };
    let min = p.alt()?;
    if p.i < p.c.len() {
        return Err("an unbalanced `)`".into());
    }
    if min == 0 {
        return Err("may match the empty string".into());
    }
    Ok(())
}

/// The `(?is)`-style prefix `translate` puts in front of the pattern for the `i` / `s` modifiers.
fn strip_flag_prefix(p: &str) -> &str {
    if let Some(rest) = p.strip_prefix("(?") {
        if let Some(end) = rest.find(')') {
            if rest[..end].chars().all(|c| matches!(c, 'i' | 's')) && end > 0 {
                return &rest[end + 1..];
            }
        }
    }
    p
}

struct Parse {
    c: Vec<char>,
    i: usize,
}

impl Parse {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    /// Alternatives: the shortest one decides.
    fn alt(&mut self) -> Result<usize, String> {
        let mut min = self.seq()?;
        while self.peek() == Some('|') {
            self.i += 1;
            min = min.min(self.seq()?);
        }
        Ok(min)
    }

    fn seq(&mut self) -> Result<usize, String> {
        let mut total = 0usize;
        while !matches!(self.peek(), None | Some('|' | ')')) {
            total = total.saturating_add(self.item()?);
        }
        Ok(total)
    }

    fn item(&mut self) -> Result<usize, String> {
        let atom = self.atom()?;
        let factor = self.quantifier();
        Ok(atom.saturating_mul(factor))
    }

    /// The minimum repetition count of a quantifier at the cursor (1 when there is none).
    fn quantifier(&mut self) -> usize {
        let factor = match self.peek() {
            Some('*' | '?') => {
                self.i += 1;
                0
            }
            Some('+') => {
                self.i += 1;
                1
            }
            Some('{') => match self.counted() {
                Some(n) => n,
                None => return 1,
            },
            _ => return 1,
        };
        if matches!(self.peek(), Some('?' | '+')) {
            self.i += 1; // lazy / possessive
        }
        factor
    }

    /// `{n}`, `{n,}` or `{n,m}` at the cursor: advance past it and return `n`. Anything else is a
    /// literal `{` (as PCRE reads it) and is left for [`Parse::atom`].
    fn counted(&mut self) -> Option<usize> {
        let rest: String = self.c[self.i + 1..].iter().collect();
        let close = rest.find('}')?;
        let inner = &rest[..close];
        let (lo, hi) = inner.split_once(',').unwrap_or((inner, inner));
        let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        if !digits(lo) || !(hi.is_empty() || digits(hi)) {
            return None;
        }
        let n = lo.parse().ok()?;
        self.i += 1 + inner.chars().count() + 1;
        Some(n)
    }

    fn atom(&mut self) -> Result<usize, String> {
        let Some(ch) = self.peek() else {
            return Err("a pattern that ends inside an element".into());
        };
        self.i += 1;
        match ch {
            '(' => self.group(),
            '[' => {
                self.class()?;
                Ok(1)
            }
            '\\' => self.escape(),
            '^' => Ok(0),
            '*' | '+' | '?' => Err("a quantifier with nothing to repeat".into()),
            // Only the `D`-modifier `$` reaches here (a rewritten one is refused first): the very end.
            '$' => Ok(0),
            _ => Ok(1),
        }
    }

    fn group(&mut self) -> Result<usize, String> {
        let mut zero_width = false;
        if self.peek() == Some('?') {
            self.i += 1;
            match self.peek() {
                Some(':' | '>') => self.i += 1,
                Some('=' | '!') => {
                    self.i += 1;
                    zero_width = true;
                }
                Some('<') if matches!(self.c.get(self.i + 1), Some('=' | '!')) => {
                    self.i += 2;
                    zero_width = true;
                }
                Some('<') => self.name('>')?,
                Some('P') if self.c.get(self.i + 1) == Some(&'<') => {
                    self.i += 1;
                    self.name('>')?;
                }
                other => {
                    return Err(format!(
                        "the group form `(?{}` is not analysed",
                        other.map(String::from).unwrap_or_default()
                    ))
                }
            }
        }
        let min = self.alt()?;
        if self.peek() != Some(')') {
            return Err("an unclosed `(`".into());
        }
        self.i += 1;
        Ok(if zero_width { 0 } else { min })
    }

    /// Skip `<name>` (the cursor is on the opening delimiter).
    fn name(&mut self, close: char) -> Result<(), String> {
        self.i += 1;
        while let Some(c) = self.peek() {
            self.i += 1;
            if c == close {
                return Ok(());
            }
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
        }
        Err("a malformed group name".into())
    }

    fn escape(&mut self) -> Result<usize, String> {
        let Some(n) = self.peek() else {
            return Err("a trailing `\\`".into());
        };
        self.i += 1;
        match n {
            'd' | 'D' | 'w' | 'W' | 's' | 'S' | 't' | 'n' | 'r' | 'f' => Ok(1),
            'b' | 'B' | 'A' | 'z' | 'Z' => Ok(0),
            'p' | 'P' => {
                if self.peek() == Some('{') {
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == '}' {
                            return Ok(1);
                        }
                    }
                    Err("an unclosed `\\p{`".into())
                } else {
                    self.i += 1;
                    Ok(1)
                }
            }
            c if !c.is_ascii_alphanumeric() => Ok(1),
            c => Err(format!("the escape `\\{c}` is not analysed")),
        }
    }

    /// Skip a `[…]` class (the cursor is past the `[`): escapes, a leading `]`, nested classes and
    /// `[:posix:]` names are all skipped without being interpreted.
    fn class(&mut self) -> Result<(), String> {
        if self.peek() == Some('^') {
            self.i += 1;
        }
        let mut first = true;
        let mut depth = 1usize;
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '\\' => self.i += 1,
                ']' if first => {}
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                '[' if self.peek() == Some(':') => {
                    while let Some(d) = self.peek() {
                        self.i += 1;
                        if d == ']' {
                            break;
                        }
                    }
                }
                '[' => depth += 1,
                _ => {}
            }
            first = false;
        }
        Err("an unclosed `[`".into())
    }
}

#[cfg(test)]
mod tests {
    use super::consumes;

    #[test]
    fn a_pattern_with_a_part_every_match_must_consume_is_proven() {
        for p in [
            r"a+",
            r"x(a)?",
            r"([a-z])",
            r"[a-z](-)?([0-9])?",
            r"\d{2,}",
            r"(?<![a-z])rdc(?![a-z])",
            r"\p{L}+",
            r"\pL",
            r"<loc>\s*([^<\s]+)\s*</loc>",
            r"a|b",
            r"(?:ab)+c?",
            r"(?i)chauffage",
            r"[]a]+",
            r"[[:alpha:]]+",
            r"a{2}",
            r"a$",
        ] {
            assert_eq!(consumes(p), Ok(()), "{p}");
        }
    }

    #[test]
    fn a_pattern_that_can_match_empty_is_refused() {
        for p in [
            r"\d*",
            r"a?",
            r"(?:)",
            r"a{0}",
            r"x*|y",
            r"(a)|b*",
            r"(?=a)|a",
            r"\p{L}*",
            r"\pL?",
            r"\P{L}*",
            r"\b",
            r"^",
            r"(a*)+",
            r"(?<n>b?)",
            r"a??",
        ] {
            let e = consumes(p).expect_err(p);
            assert!(e.contains("empty string"), "{p}: {e}");
        }
    }

    /// What the parse has never heard of is not proven — each of these slipped past the first,
    /// blacklist-shaped version of this check.
    #[test]
    fn a_construct_the_parse_does_not_know_is_refused_by_name() {
        for (p, want) in [
            (r"a\K", r"\K"),
            (r"(?<x>a)\k<x>", r"\k"),
            (r"(a)\g1", r"\g"),
            (r"(a)\1", r"\1"),
            (r"\h+", r"\h"),
            (r"\R+", r"\R"),
            (r"a(?R)?b", "(?R"),
            (r"(?(1)a|b)", "(?("),
            (r"(a", "unclosed"),
            (r"a)", "unbalanced"),
        ] {
            let e = consumes(p).expect_err(p);
            assert!(e.contains(want), "{p}: want `{want}` in: {e}");
        }
    }
}
