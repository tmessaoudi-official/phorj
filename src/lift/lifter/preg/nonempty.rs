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

/// What the whitelist parse learned about a pattern it fully understands.
pub(super) struct Facts {
    /// Every match consumes at least one character.
    pub(super) consumes: bool,
    /// A group that can match empty is repeated more than once (`(a?)+`, `(x|)*`): PCRE reports the
    /// group as the empty string its last iteration matched, the native engines as its last
    /// NON-empty iteration, so a captured group's text differs.
    pub(super) repeated_nullable_group: bool,
}

/// Parse `pattern`, or say which construct is not on the whitelist.
pub(super) fn analyse(pattern: &str) -> Result<Facts, String> {
    let body = strip_flag_prefix(pattern);
    let mut p = Parse {
        c: body.chars().collect(),
        i: 0,
        repeated_nullable_group: false,
    };
    let min = p.alt()?;
    if p.i < p.c.len() {
        return Err("an unbalanced `)`".into());
    }
    Ok(Facts {
        consumes: min > 0,
        repeated_nullable_group: p.repeated_nullable_group,
    })
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
    repeated_nullable_group: bool,
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
        let group = self.peek() == Some('(');
        let atom = self.atom()?;
        let (factor, repeats) = self.quantifier();
        if group && atom == 0 && repeats {
            self.repeated_nullable_group = true;
        }
        Ok(atom.saturating_mul(factor))
    }

    /// The minimum repetition count of a quantifier at the cursor (1 when there is none), and
    /// whether it can repeat its atom more than once.
    fn quantifier(&mut self) -> (usize, bool) {
        let (factor, repeats) = match self.peek() {
            Some('*') => {
                self.i += 1;
                (0, true)
            }
            Some('?') => {
                self.i += 1;
                (0, false)
            }
            Some('+') => {
                self.i += 1;
                (1, true)
            }
            Some('{') => match self.counted() {
                Some(counted) => counted,
                None => return (1, false),
            },
            _ => return (1, false),
        };
        if matches!(self.peek(), Some('?' | '+')) {
            self.i += 1; // lazy / possessive
        }
        (factor, repeats)
    }

    /// `{n}`, `{n,}` or `{n,m}` at the cursor: advance past it and return `n` and whether it can
    /// repeat more than once. Anything else is a literal `{` (as PCRE reads it) and is left for
    /// [`Parse::atom`].
    fn counted(&mut self) -> Option<(usize, bool)> {
        let rest: String = self.c[self.i + 1..].iter().collect();
        let close = rest.find('}')?;
        let inner = &rest[..close];
        let (lo, hi) = inner.split_once(',').unwrap_or((inner, inner));
        let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        if !digits(lo) || !(hi.is_empty() || digits(hi)) {
            return None;
        }
        let n: usize = lo.parse().ok()?;
        let more = match inner.split_once(',') {
            None => n > 1,
            Some((_, "")) => true,
            Some((_, hi)) => hi.parse::<usize>().ok()? > 1,
        };
        self.i += 1 + inner.chars().count() + 1;
        Some((n, more))
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
    use super::analyse;

    /// `Ok(())` when every match consumes at least one character; otherwise why it is not proven.
    fn consumes(pattern: &str) -> Result<(), String> {
        if analyse(pattern)?.consumes {
            Ok(())
        } else {
            Err("may match the empty string".into())
        }
    }

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

    #[test]
    fn a_repeated_group_that_can_match_empty_is_flagged() {
        use super::analyse;
        for p in [
            r"(a?)+b",
            r"(x|)*b",
            r"(a*){2,}b",
            r"(a?){1,3}b",
            r"(?:(a?)b?)+c",
        ] {
            assert!(analyse(p).unwrap().repeated_nullable_group, "{p}");
        }
        for p in [
            r"(a)+b",
            r"(a?)b",
            r"(a?){0,1}b",
            r"(a|b)*c",
            r"(a?)?b",
            r"[ab]+(c)",
        ] {
            assert!(!analyse(p).unwrap().repeated_nullable_group, "{p}");
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
