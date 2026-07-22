//! `Core.Regex` — a ReDoS-safe regular-expression engine over a compiler-injected `Regex` class
//! value (`docs/specs/2026-06-28-core-regex-design.md`). Backed by the `regex` crate (the project's
//! 2nd vetted dependency, `docs/specs/2026-06-27-dependency-policy.md`): a RE2-style finite automaton
//! with **guaranteed linear-time matching** (ReDoS-immune by construction). Its restricted feature
//! set (no backreferences / lookaround) is exactly the *regular* subset PHP `preg_*` matches
//! identically, so the byte-identity spine holds; an unsupported pattern is rejected at
//! [`Regex.compile`] (a clean fault), never reaching a backend.
//!
//! A compiled `Regex` value is a `Value::Instance { class: "Regex", fields: { pattern } }` carrying
//! the **bare** pattern (no delimiters), built directly by `compile` (the hand-built-value technique,
//! exactly like `Core.Json`'s `jnode`). The user constructs one only via `Regex.compile`. The
//! engines are memoized in a thread-local cache keyed by the bare pattern, recovering "compile once,
//! reuse" with no new `Value` variant. The PHP transpile is a peer emission target only — the
//! engine runs natively on both Rust backends (the dependency-policy native-runtime rule).

use crate::native::*;
use crate::types::Ty;
use crate::value::{build_map, Instance, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    /// Memoized compiled engines, keyed by the **bare** pattern. `compile` populates it (and is the
    /// only place a compile error can surface); the query natives look up the cached engine, with a
    /// recompile fallback that cannot fault (the pattern was already validated by `compile`). Pure
    /// optimization — semantics are identical without the cache.
    static CACHE: RefCell<HashMap<String, Rc<::regex::Regex>>> = RefCell::new(HashMap::new());
}

/// Compile `pattern` (bare, Unicode), memoizing the engine. Returns a clean fault on an invalid or
/// unsupported (backref/lookaround) pattern — the `regex` crate's own compile error, surfaced
/// uniformly so the failure is identical on both backends.
pub(super) fn compiled(pattern: &str) -> Result<Rc<::regex::Regex>, String> {
    if let Some(re) = CACHE.with(|c| c.borrow().get(pattern).cloned()) {
        return Ok(re);
    }
    match ::regex::Regex::new(pattern) {
        Ok(re) => {
            let re = Rc::new(re);
            CACHE.with(|c| c.borrow_mut().insert(pattern.to_string(), re.clone()));
            Ok(re)
        }
        Err(e) => Err(format!("invalid or unsupported regex: {e}")),
    }
}

/// Build the opaque `Regex` value holding the bare pattern. S1b: a native carrier builds its own
/// single-field [`crate::value::ClassLayout`] (`["pattern"]`) — independent of any injected prelude,
/// and self-consistent (every `Regex` shares the same one-slot layout, so eq/reflect parity holds).
fn regex_value(pattern: &str) -> Value {
    let inst = Instance::new(
        "Regex".into(),
        crate::value::ClassLayout::from_sorted_names(&["pattern"]),
    );
    inst.set_field("pattern", Value::Str(pattern.into()));
    Value::Instance(Rc::new(inst))
}

/// Extract the bare pattern from a `Regex` instance argument.
fn as_pattern(v: &Value) -> Result<String, String> {
    match v {
        Value::Instance(inst) if inst.class.as_ref() == "Regex" => inst
            .get_field("pattern")
            .and_then(|p| match p {
                Value::Str(s) => Some(s.as_str().to_string()),
                _ => None,
            })
            .ok_or_else(|| "Regex value is missing its pattern".to_string()),
        _ => Err(format!("Regex value expected, got {}", v.type_name())),
    }
}

// ---- natives ------------------------------------------------------------------------------------

/// `Regex.compile(string) -> Regex` — validate + memoize; faults on an invalid/unsupported pattern.
pub(super) fn regex_compile(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(p)] => {
            compiled(p)?; // validate now (and cache); the value carries only the bare pattern.
            Ok(regex_value(p))
        }
        _ => Err("Regex.compile expects (string)".into()),
    }
}

/// `Regex.matches(Regex, string) -> bool` — is there a match anywhere in the subject?
pub(super) fn regex_matches(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let pat = as_pattern(re)?;
            Ok(Value::Bool(compiled(&pat)?.is_match(s)))
        }
        _ => Err("Regex.matches expects (Regex, string)".into()),
    }
}

/// `Regex.find(Regex, string) -> string?` — the first whole match, else `null`.
pub(super) fn regex_find(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let pat = as_pattern(re)?;
            Ok(compiled(&pat)?
                .find(s)
                .map_or(Value::Null, |m| Value::Str(m.as_str().into())))
        }
        _ => Err("Regex.find expects (Regex, string)".into()),
    }
}

/// `Regex.findAll(Regex, string) -> List<string>` — every whole match (empty list if none).
pub(super) fn regex_find_all(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let pat = as_pattern(re)?;
            let out: Vec<Value> = compiled(&pat)?
                .find_iter(s)
                .map(|m| Value::Str(m.as_str().into()))
                .collect();
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err("Regex.findAll expects (Regex, string)".into()),
    }
}

/// `Regex.findGroups(Regex, string) -> Map<string, string>?` — the **named** captures of the first
/// match, keyed by group name, else `null`. Numbered-only captures are omitted (named is the API).
pub(super) fn regex_find_groups(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let pat = as_pattern(re)?;
            let engine = compiled(&pat)?;
            match engine.captures(s) {
                None => Ok(Value::Null),
                Some(caps) => {
                    let mut pairs: Vec<(Value, Value)> = Vec::new();
                    for name in engine.capture_names().flatten() {
                        if let Some(m) = caps.name(name) {
                            pairs.push((Value::Str(name.into()), Value::Str(m.as_str().into())));
                        }
                    }
                    Ok(Value::Map(Rc::new(build_map(pairs)?)))
                }
            }
        }
        _ => Err("Regex.findGroups expects (Regex, string)".into()),
    }
}

/// `Regex.findAllGroups(Regex, string) -> List<Map<string, string>>` — the **named** captures of
/// EVERY match, one map per match (empty list if none). The grouped counterpart of `findAll` (whole
/// matches) and the all-matches counterpart of `findGroups`; mirrors PHP `preg_match_all` with
/// `PREG_SET_ORDER`, named-only (numbered captures omitted — named is the API, as in `findGroups`).
pub(super) fn regex_find_all_groups(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let pat = as_pattern(re)?;
            let engine = compiled(&pat)?;
            let names: Vec<&str> = engine.capture_names().flatten().collect();
            let mut out: Vec<Value> = Vec::new();
            for caps in engine.captures_iter(s) {
                let mut pairs: Vec<(Value, Value)> = Vec::new();
                for name in &names {
                    if let Some(m) = caps.name(name) {
                        pairs.push((Value::Str((*name).into()), Value::Str(m.as_str().into())));
                    }
                }
                out.push(Value::Map(Rc::new(build_map(pairs)?)));
            }
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err("Regex.findAllGroups expects (Regex, string)".into()),
    }
}

/// `Regex.replace(Regex, string, string) -> string` — replace every match. The replacement uses the
/// `$1` / `${name}` capture syntax shared by the `regex` crate and PHP `preg_replace`.
pub(super) fn regex_replace(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s), Value::Str(repl)] => {
            let pat = as_pattern(re)?;
            Ok(Value::Str(
                compiled(&pat)?
                    .replace_all(s, repl.as_str())
                    .into_owned()
                    .into(),
            ))
        }
        _ => Err("Regex.replace expects (Regex, string, string)".into()),
    }
}

/// `Regex.split(Regex, string) -> List<string>` — split the subject on matches.
pub(super) fn regex_split(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let pat = as_pattern(re)?;
            let out: Vec<Value> = compiled(&pat)?
                .split(s)
                .map(|p| Value::Str(p.into()))
                .collect();
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err("Regex.split expects (Regex, string)".into()),
    }
}

/// `Regex.quoteMeta(string) -> string` — escape every regex metacharacter so the text matches
/// literally (PHP `preg_quote`, but see DEC-296). Uses the `regex` crate's own [`::regex::escape`]
/// as the oracle; the PHP twin reproduces its exact meta-set (`__phorj_regex_quote_meta`), NOT
/// `preg_quote` (whose set differs), so all three backends agree byte-for-byte. Takes a bare string,
/// not a `Regex` — you quote text *before* building a pattern from it.
pub(super) fn regex_quote_meta(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(::regex::escape(s).into())),
        _ => Err("Regex.quoteMeta expects (string)".into()),
    }
}

/// Build the injected `RegexMatch` value (DEC-295) — the typed carrier a `replaceCallback` closure
/// receives. Hand-built like [`regex_value`]: class `RegexMatch`, a two-slot layout (`groups`,
/// `matched`) matching the prelude's promoted constructor fields. `groups` holds ONLY participating
/// named captures (like `regex_find_groups`), so `group()` returns `null` for a non-participating one
/// — the same contract the PHP twin gets via `PREG_UNMATCHED_AS_NULL` + a null-filter.
fn regex_match_value(matched: &str, groups: Vec<(Value, Value)>) -> Result<Value, String> {
    let inst = Instance::new(
        "RegexMatch".into(),
        crate::value::ClassLayout::from_sorted_names(&["groups", "matched"]),
    );
    inst.set_field("matched", Value::Str(matched.into()));
    inst.set_field("groups", Value::Map(Rc::new(build_map(groups)?)));
    Ok(Value::Instance(Rc::new(inst)))
}

/// `Regex.replaceCallback(Regex, string, (RegexMatch) -> string) -> string` — replace every match with
/// the callback's result (PHP `preg_replace_callback`, DEC-295). Higher-order: the backend invoker
/// runs the closure per match. Matches are non-overlapping, left-to-right — the gap before each match
/// is copied verbatim, the match is replaced by the closure's returned string, and the tail after the
/// last match is appended. Mirrors `preg_replace_callback`'s assembly for the regular (non-zero-width)
/// subset the engine shares with PCRE.
pub(super) fn regex_replace_callback(
    args: &[Value],
    call: &mut ClosureInvoker,
) -> Result<Value, String> {
    match args {
        [re, Value::Str(s), cb] => {
            let pat = as_pattern(re)?;
            let engine = compiled(&pat)?;
            let names: Vec<&str> = engine.capture_names().flatten().collect();
            let mut out = String::new();
            let mut last_end = 0;
            for caps in engine.captures_iter(s) {
                let whole = caps.get(0).expect("group 0 always exists in a match");
                out.push_str(&s[last_end..whole.start()]);
                let mut pairs: Vec<(Value, Value)> = Vec::new();
                for name in &names {
                    if let Some(m) = caps.name(name) {
                        pairs.push((Value::Str((*name).into()), Value::Str(m.as_str().into())));
                    }
                }
                let m_val = regex_match_value(whole.as_str(), pairs)?;
                match call(cb, vec![m_val])? {
                    Value::Str(r) => out.push_str(&r),
                    other => {
                        return Err(format!(
                            "Regex.replaceCallback callback must return string, got {}",
                            other.type_name()
                        ))
                    }
                }
                last_end = whole.end();
            }
            out.push_str(&s[last_end..]);
            Ok(Value::Str(out.into()))
        }
        _ => Err("Regex.replaceCallback expects (Regex, string, (RegexMatch) -> string)".into()),
    }
}

// ---- registry -----------------------------------------------------------------------------------

/// The `Core.Regex` registry entries. `Regex` is the compiler-injected class
/// (`cli::inject_core_modules`, `Core.Regex` row) — referenced as a bare `Ty::Named`; the type resolves because a call
/// to any of these natives requires `import Core.Regex;`, which triggers the injection before the
/// checker runs. The `php` emitters reference the `__phorj_regex_*` runtime helpers
/// (`transpile/program.rs`); the injected `Regex` class transpiles to a PHP class with a public
/// `$pattern` (the bare pattern), so a global helper can build the `/u`-delimited form.
pub fn regex_natives() -> Vec<NativeFn> {
    let regex_ty = || Ty::Named("Regex".to_string(), vec![]);
    let list_str = || Ty::List(Box::new(Ty::String));
    let opt_str = || Ty::Optional(Box::new(Ty::String));
    let opt_map = || {
        Ty::Optional(Box::new(Ty::Map(
            Box::new(Ty::String),
            Box::new(Ty::String),
        )))
    };
    vec![
        NativeFn {
            module: "Core.Regex",
            name: "compile",
            params: vec![Ty::String],
            ret: regex_ty(),
            pure: true,
            eval: NativeEval::Pure(regex_compile),
            lift_from: &[],
            php: |a| format!("new Regex({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "matches",
            params: vec![regex_ty(), Ty::String],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(regex_matches),
            lift_from: &[],
            php: |a| format!("__phorj_regex_matches({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "find",
            params: vec![regex_ty(), Ty::String],
            ret: opt_str(),
            pure: true,
            eval: NativeEval::Pure(regex_find),
            lift_from: &[],
            php: |a| format!("__phorj_regex_find({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "findAll",
            params: vec![regex_ty(), Ty::String],
            ret: list_str(),
            pure: true,
            eval: NativeEval::Pure(regex_find_all),
            lift_from: &[],
            php: |a| format!("__phorj_regex_find_all({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "findGroups",
            params: vec![regex_ty(), Ty::String],
            ret: opt_map(),
            pure: true,
            eval: NativeEval::Pure(regex_find_groups),
            lift_from: &[],
            php: |a| format!("__phorj_regex_find_groups({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "findAllGroups",
            params: vec![regex_ty(), Ty::String],
            ret: Ty::List(Box::new(Ty::Map(
                Box::new(Ty::String),
                Box::new(Ty::String),
            ))),
            pure: true,
            eval: NativeEval::Pure(regex_find_all_groups),
            lift_from: &[],
            php: |a| {
                format!(
                    "__phorj_regex_find_all_groups({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        NativeFn {
            module: "Core.Regex",
            name: "replace",
            params: vec![regex_ty(), Ty::String, Ty::String],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(regex_replace),
            lift_from: &[],
            php: |a| {
                format!(
                    "__phorj_regex_replace({}, {}, {})",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2)
                )
            },
        },
        NativeFn {
            module: "Core.Regex",
            name: "split",
            params: vec![regex_ty(), Ty::String],
            ret: list_str(),
            pure: true,
            eval: NativeEval::Pure(regex_split),
            lift_from: &[],
            php: |a| format!("__phorj_regex_split({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "quoteMeta",
            params: vec![Ty::String],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(regex_quote_meta),
            lift_from: &[],
            php: |a| format!("__phorj_regex_quote_meta({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "replaceCallback",
            params: vec![
                regex_ty(),
                Ty::String,
                Ty::Function(
                    vec![Ty::Named("RegexMatch".to_string(), vec![])],
                    Box::new(Ty::String),
                    Vec::new(),
                ),
            ],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::HigherOrder(regex_replace_callback),
            lift_from: &[],
            php: |a| {
                format!(
                    "__phorj_regex_replace_callback({}, {}, {})",
                    parg(a, 0),
                    parg(a, 1),
                    parg(a, 2)
                )
            },
        },
    ]
}
