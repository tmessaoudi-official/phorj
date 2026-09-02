//! The two engines behind `Core.Regex` (DEC-461, REGEX-B) and the ONE query API the natives use.
//!
//! * [`Engine::Linear`] — `Regex.compile`: the `regex` crate, RE2-style, guaranteed linear-time,
//!   ReDoS-immune by construction. Its accepted syntax is exactly the *regular* subset PHP `preg_*`
//!   matches identically, so the byte-identity spine holds on every leg.
//! * [`Engine::Backtracking`] — `Regex.compileBacktracking`: `fancy-regex` (the 15th vetted
//!   dependency), PCRE-class syntax (look-around, back-references, atomic groups, possessive
//!   quantifiers) on a backtracking VM with a STEP BUDGET that raises a typed fault instead of
//!   hanging. It delegates the regular subset to `regex`, so the two engines agree wherever both
//!   accept a pattern.
//!
//! The linear engine REJECTS every PCRE-only construct up front ([`linear_unsupported`]): the crate
//! refuses most of them itself, but it silently read a possessive `a++` as `(a+)+` — `true` natively,
//! `false` under PCRE, with every leg exiting 0 (panel C2). The reject list is applied twice: at
//! compile time by the checker on a LITERAL pattern (`E-REGEX-UNSUPPORTED`), and at runtime here for a
//! dynamic one; the PHP twin `__phorj_regex_compile` ports the same scan, so a dynamic pattern faults on
//! every leg (`tests/differential.rs`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Which engine compiled a `Regex` value — carried on the value's `engine` field.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Engine {
    Linear,
    Backtracking,
}

impl Engine {
    /// The value-field spelling (also the PHP twin's second constructor argument).
    pub fn name(self) -> &'static str {
        match self {
            Engine::Linear => "linear",
            Engine::Backtracking => "backtracking",
        }
    }
    pub fn from_name(s: &str) -> Option<Engine> {
        match s {
            "linear" => Some(Engine::Linear),
            "backtracking" => Some(Engine::Backtracking),
            _ => None,
        }
    }
}

/// The backtracking engine's step budget — `fancy-regex`'s `backtrack_limit`. The same order of
/// magnitude as PHP's default `pcre.backtrack_limit` (1 000 000), so a pattern that trips one tends to
/// trip the other; the PHP helper maps `PREG_BACKTRACK_LIMIT_ERROR` to the same fault text.
pub const STEP_BUDGET: usize = 1_000_000;

/// The typed fault a budget overrun raises (byte-identical on every leg — the PHP helper throws it).
pub const STEP_BUDGET_FAULT: &str =
    "regex step budget exceeded: the pattern backtracks catastrophically on this subject";

/// A compiled pattern on either engine.
pub enum Compiled {
    Linear(::regex::Regex),
    Backtracking(fancy_regex::Regex),
}

/// One match's captures: the whole match plus every group by index (`None` = did not participate).
pub struct Caps {
    pub whole: (usize, usize),
    pub groups: Vec<Option<(usize, usize)>>,
}

thread_local! {
    /// Memoized engines keyed by (engine, bare pattern) — one pattern may legitimately be compiled on
    /// BOTH engines in one program, hence the engine in the key. Pure optimization.
    static CACHE: RefCell<HashMap<(Engine, String), Rc<Compiled>>> = RefCell::new(HashMap::new());
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

/// Validate `pattern` for `engine` WITHOUT compiling into the cache — the checker's compile-time gate
/// for literal patterns. `Err` carries the fault text a runtime `compile` would raise.
pub fn validate(pattern: &str, engine: Engine) -> Result<(), String> {
    build(pattern, engine).map(|_| ())
}

fn build(pattern: &str, engine: Engine) -> Result<Compiled, String> {
    match engine {
        Engine::Linear => {
            if let Some(what) = linear_unsupported(pattern) {
                return Err(format!(
                    "invalid or unsupported regex: {what} is not supported by the linear engine \
                     (use `Regex.compileBacktracking` for PCRE-class syntax)"
                ));
            }
            ::regex::Regex::new(pattern)
                .map(Compiled::Linear)
                .map_err(|e| format!("invalid or unsupported regex: {e}"))
        }
        Engine::Backtracking => fancy_regex::RegexBuilder::new(pattern)
            .backtrack_limit(STEP_BUDGET)
            .build()
            .map(Compiled::Backtracking)
            .map_err(|e| format!("invalid regex: {e}")),
    }
}

/// Compile `pattern` on `engine`, memoized. Returns a clean fault on an invalid/unsupported pattern.
pub fn compiled(pattern: &str, engine: Engine) -> Result<Rc<Compiled>, String> {
    let key = (engine, pattern.to_string());
    if let Some(re) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Ok(re);
    }
    let re = Rc::new(build(pattern, engine)?);
    CACHE.with(|c| c.borrow_mut().insert(key, re.clone()));
    Ok(re)
}

fn fancy_err(e: fancy_regex::Error) -> String {
    match e {
        fancy_regex::Error::RuntimeError(fancy_regex::RuntimeError::BacktrackLimitExceeded) => {
            STEP_BUDGET_FAULT.to_string()
        }
        other => format!("regex error: {other}"),
    }
}

impl Compiled {
    /// Every capture group's name by INDEX (groups 1..), `""` for an unnamed group — aligned with
    /// [`Caps::groups`], so a named-only view is a zip + filter (the natives' `findGroups` API).
    pub fn group_names(&self) -> Vec<String> {
        let all: Vec<Option<&str>> = match self {
            Compiled::Linear(re) => re.capture_names().collect(),
            Compiled::Backtracking(re) => re.capture_names().collect(),
        };
        all.into_iter()
            .skip(1)
            .map(|n| n.unwrap_or("").to_string())
            .collect()
    }

    pub fn is_match(&self, s: &str) -> Result<bool, String> {
        match self {
            Compiled::Linear(re) => Ok(re.is_match(s)),
            Compiled::Backtracking(re) => re.is_match(s).map_err(fancy_err),
        }
    }

    /// Every non-overlapping match with its captures, left to right.
    pub fn captures_all(&self, s: &str) -> Result<Vec<Caps>, String> {
        match self {
            Compiled::Linear(re) => Ok(re
                .captures_iter(s)
                .map(|c| Caps {
                    whole: c.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0)),
                    groups: (1..c.len())
                        .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                        .collect(),
                })
                .collect()),
            Compiled::Backtracking(re) => {
                let mut out = Vec::new();
                for c in re.captures_iter(s) {
                    let c = c.map_err(fancy_err)?;
                    out.push(Caps {
                        whole: c.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0)),
                        groups: (1..c.len())
                            .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                            .collect(),
                    });
                }
                Ok(out)
            }
        }
    }

    /// The first match's captures, if any.
    pub fn captures_first(&self, s: &str) -> Result<Option<Caps>, String> {
        match self {
            Compiled::Linear(re) => Ok(re.captures(s).map(|c| Caps {
                whole: c.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0)),
                groups: (1..c.len())
                    .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                    .collect(),
            })),
            Compiled::Backtracking(re) => Ok(re.captures(s).map_err(fancy_err)?.map(|c| Caps {
                whole: c.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0)),
                groups: (1..c.len())
                    .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                    .collect(),
            })),
        }
    }

    /// Every whole-match span, left to right.
    pub fn find_all(&self, s: &str) -> Result<Vec<(usize, usize)>, String> {
        match self {
            Compiled::Linear(re) => Ok(re.find_iter(s).map(|m| (m.start(), m.end())).collect()),
            Compiled::Backtracking(re) => re
                .find_iter(s)
                .map(|m| m.map(|m| (m.start(), m.end())).map_err(fancy_err))
                .collect(),
        }
    }

    /// The first whole-match span, if any.
    pub fn find(&self, s: &str) -> Result<Option<(usize, usize)>, String> {
        match self {
            Compiled::Linear(re) => Ok(re.find(s).map(|m| (m.start(), m.end()))),
            Compiled::Backtracking(re) => {
                Ok(re.find(s).map_err(fancy_err)?.map(|m| (m.start(), m.end())))
            }
        }
    }

    /// Split `s` on every match — the pieces BETWEEN matches (empty pieces included), assembled here
    /// so both engines share one definition (the `regex` crate's `split` semantics).
    pub fn split(&self, s: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut last = 0;
        for (start, end) in self.find_all(s)? {
            out.push(s[last..start].to_string());
            last = end;
        }
        out.push(s[last..].to_string());
        Ok(out)
    }
}
