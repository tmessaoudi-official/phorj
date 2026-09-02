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
use std::rc::Rc;

use super::engine::{compiled, Caps, Compiled, Engine};
use super::replace::{expand_replacement, GroupRef};

/// Build the opaque `Regex` value: the bare pattern + the engine that compiled it (DEC-461). S1b: a
/// native carrier builds its own [`crate::value::ClassLayout`] — sorted names, matching what
/// `class_field_layout` derives for the injected `class Regex { constructor(public string pattern,
/// public string engine) {} }`, so eq/reflect parity holds (every `Regex` shares one layout).
fn regex_value(pattern: &str, engine: Engine) -> Value {
    let inst = Instance::new(
        "Regex".into(),
        crate::value::ClassLayout::from_sorted_names(&["engine", "pattern"]),
    );
    inst.set_field("pattern", Value::Str(pattern.into()));
    inst.set_field("engine", Value::Str(engine.name().into()));
    Value::Instance(Rc::new(inst))
}

/// The compiled engine behind a `Regex` instance argument (pattern + engine read off the value).
fn engine_of(v: &Value) -> Result<Rc<Compiled>, String> {
    match v {
        Value::Instance(inst) if inst.class.as_ref() == "Regex" => {
            let field = |name: &str| match inst.get_field(name) {
                Some(Value::Str(s)) => Ok(s.as_str().to_string()),
                _ => Err(format!("Regex value is missing its {name}")),
            };
            let pattern = field("pattern")?;
            let engine = Engine::from_name(&field("engine")?)
                .ok_or_else(|| "Regex value carries an unknown engine".to_string())?;
            compiled(&pattern, engine)
        }
        _ => Err(format!("Regex value expected, got {}", v.type_name())),
    }
}

/// The participating NAMED captures of one match as `(name, text)` pairs, in group-index order.
fn named_pairs(s: &str, names: &[String], caps: &Caps) -> Vec<(Value, Value)> {
    names
        .iter()
        .zip(&caps.groups)
        .filter(|(name, _)| !name.is_empty())
        .filter_map(|(name, g)| {
            g.map(|(a, b)| (Value::Str(name.as_str().into()), Value::Str(s[a..b].into())))
        })
        .collect()
}

// ---- natives ------------------------------------------------------------------------------------

/// `Regex.compile(string) -> Regex` — the LINEAR engine: validate + memoize; faults on an invalid or
/// PCRE-only pattern (`compileBacktracking` is the opt-in for those).
pub(super) fn regex_compile(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(p)] => {
            compiled(p, Engine::Linear)?;
            Ok(regex_value(p, Engine::Linear))
        }
        _ => Err("Regex.compile expects (string)".into()),
    }
}

/// `Regex.compileBacktracking(string) -> Regex` — the BACKTRACKING engine (DEC-461): PCRE-class syntax
/// under a step budget; a catastrophic pattern raises a typed fault instead of hanging.
pub(super) fn regex_compile_backtracking(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(p)] => {
            compiled(p, Engine::Backtracking)?;
            Ok(regex_value(p, Engine::Backtracking))
        }
        _ => Err("Regex.compileBacktracking expects (string)".into()),
    }
}

/// `Regex.matches(Regex, string) -> bool` — is there a match anywhere in the subject?
pub(super) fn regex_matches(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => Ok(Value::Bool(engine_of(re)?.is_match(s)?)),
        _ => Err("Regex.matches expects (Regex, string)".into()),
    }
}

/// `Regex.find(Regex, string) -> string?` — the first whole match, else `null`.
pub(super) fn regex_find(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => Ok(engine_of(re)?
            .find(s)?
            .map_or(Value::Null, |(a, b)| Value::Str(s[a..b].into()))),
        _ => Err("Regex.find expects (Regex, string)".into()),
    }
}

/// `Regex.findAll(Regex, string) -> List<string>` — every whole match (empty list if none).
pub(super) fn regex_find_all(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let out: Vec<Value> = engine_of(re)?
                .find_all(s)?
                .into_iter()
                .map(|(a, b)| Value::Str(s[a..b].into()))
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
            let engine = engine_of(re)?;
            match engine.captures_first(s)? {
                None => Ok(Value::Null),
                Some(caps) => Ok(Value::Map(Rc::new(build_map(named_pairs(
                    s,
                    &engine.group_names(),
                    &caps,
                ))?))),
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
            let engine = engine_of(re)?;
            let names = engine.group_names();
            let mut out: Vec<Value> = Vec::new();
            for caps in engine.captures_all(s)? {
                out.push(Value::Map(Rc::new(build_map(named_pairs(
                    s, &names, &caps,
                ))?)));
            }
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err("Regex.findAllGroups expects (Regex, string)".into()),
    }
}

/// `Regex.replace(Regex, string, string) -> string` — replace every match, expanding phorj's OWN
/// replacement grammar (`super::replace`): `$N`/`${N}`, `$name`/`${name}`, `$$`; `\1` is literal.
pub(super) fn regex_replace(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s), Value::Str(repl)] => {
            let engine = engine_of(re)?;
            let names = engine.group_names();
            let mut out = String::new();
            let mut last_end = 0;
            for caps in engine.captures_all(s)? {
                out.push_str(&s[last_end..caps.whole.0]);
                let text = |r: GroupRef<'_>| -> Option<String> {
                    let idx = match r {
                        GroupRef::Index(0) => return Some(s[caps.whole.0..caps.whole.1].into()),
                        GroupRef::Index(n) => n.checked_sub(1)?,
                        GroupRef::Name(n) => names.iter().position(|g| g == n)?,
                    };
                    caps.groups
                        .get(idx)
                        .copied()
                        .flatten()
                        .map(|(a, b)| s[a..b].to_string())
                };
                out.push_str(&expand_replacement(repl, &text));
                last_end = caps.whole.1;
            }
            out.push_str(&s[last_end..]);
            Ok(Value::Str(out.into()))
        }
        _ => Err("Regex.replace expects (Regex, string, string)".into()),
    }
}

/// `Regex.split(Regex, string) -> List<string>` — split the subject on matches.
pub(super) fn regex_split(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let out: Vec<Value> = engine_of(re)?
                .split(s)?
                .into_iter()
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
            let engine = engine_of(re)?;
            let names = engine.group_names();
            let mut out = String::new();
            let mut last_end = 0;
            for caps in engine.captures_all(s)? {
                let (ws, we) = caps.whole;
                out.push_str(&s[last_end..ws]);
                let pairs = named_pairs(s, &names, &caps);
                let m_val = regex_match_value(&s[ws..we], pairs)?;
                match call(cb, &[m_val])? {
                    Value::Str(r) => out.push_str(&r),
                    other => {
                        return Err(format!(
                            "Regex.replaceCallback callback must return string, got {}",
                            other.type_name()
                        ))
                    }
                }
                last_end = we;
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
            php: |a| format!("__phorj_regex_compile({}, 'linear')", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Regex",
            name: "compileBacktracking",
            params: vec![Ty::String],
            ret: regex_ty(),
            pure: true,
            eval: NativeEval::Pure(regex_compile_backtracking),
            lift_from: &[],
            php: |a| format!("__phorj_regex_compile({}, 'backtracking')", parg(a, 0)),
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
