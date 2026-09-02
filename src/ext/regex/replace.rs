//! The replacement grammar of `Regex.replace` — phorj's OWN, expanded here on the Rust backends and by
//! the PHP twin `__phorj_regex_expand` on the transpile leg, so the three legs agree by construction
//! (DEC-461, panel C1: the `regex` crate's default expansion and PCRE's `preg_replace` disagree on
//! `\1`, `$$`, `$1a` and `${name}`, and every leg exited 0).
//!
//! Grammar (everything else is literal, including `\1` — a back-reference is never a replacement):
//!
//! | form        | meaning                                                   |
//! |-------------|-----------------------------------------------------------|
//! | `$$`        | one literal `$`                                           |
//! | `$N`        | group N (maximal run of digits — `$1a` is group 1 then `a`) |
//! | `${N}`      | group N                                                   |
//! | `$name`     | named group (maximal `[A-Za-z0-9_]` run after a letter/`_`) |
//! | `${name}`   | named group                                               |
//!
//! A group that did not participate (or does not exist) expands to the empty string.

/// A group reference inside a replacement template.
pub enum GroupRef<'a> {
    Index(usize),
    Name(&'a str),
}

/// Expand `template` against one match; `group` resolves a reference to its text (`None` = absent).
pub fn expand_replacement(
    template: &str,
    group: &dyn Fn(GroupRef<'_>) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(pos) = rest.find('$') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        let mut chars = after.chars();
        match chars.next() {
            Some('$') => {
                out.push('$');
                rest = &after[1..];
            }
            Some('{') => match after.find('}') {
                Some(close) => {
                    let inner = &after[1..close];
                    let r = if !inner.is_empty() && inner.bytes().all(|b| b.is_ascii_digit()) {
                        inner.parse().ok().map(GroupRef::Index)
                    } else if !inner.is_empty() {
                        Some(GroupRef::Name(inner))
                    } else {
                        None
                    };
                    match r {
                        Some(r) => out.push_str(&group(r).unwrap_or_default()),
                        None => out.push_str("${}"), // empty braces are literal
                    }
                    rest = &after[close + 1..];
                }
                None => {
                    out.push('$');
                    rest = after;
                }
            },
            Some(c) if c.is_ascii_digit() => {
                let end = after
                    .find(|ch: char| !ch.is_ascii_digit())
                    .unwrap_or(after.len());
                if let Ok(n) = after[..end].parse::<usize>() {
                    out.push_str(&group(GroupRef::Index(n)).unwrap_or_default());
                }
                rest = &after[end..];
            }
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let end = after
                    .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                    .unwrap_or(after.len());
                out.push_str(&group(GroupRef::Name(&after[..end])).unwrap_or_default());
                rest = &after[end..];
            }
            _ => {
                out.push('$');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups(r: GroupRef<'_>) -> Option<String> {
        match r {
            GroupRef::Index(1) | GroupRef::Name("x") => Some("a".into()),
            GroupRef::Index(2) => None, // present in the pattern, did not participate
            _ => None,
        }
    }

    #[test]
    fn grammar_rows() {
        for (template, expected) in [
            ("\\1-", "\\1-"),
            ("$$", "$"),
            ("$1a", "aa"),
            ("[${x}]", "[a]"),
            ("<$x>", "<a>"),
            ("${1}${1}", "aa"),
            ("$2|$9|$nope|${}", "|||${}"),
            ("$", "$"),
            ("${x", "${x"),
            ("$-", "$-"),
        ] {
            assert_eq!(
                expand_replacement(template, &groups),
                expected,
                "{template}"
            );
        }
    }
}
