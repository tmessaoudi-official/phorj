//! `Regex.first` / `Regex.all` (DEC-554) and the `RegexMatch` value every match-returning native builds
//! (split out of `natives.rs`, Invariant 13).
//!
//! A match answers by POSITION and by NAME. `texts` and `starts` hold every group by index — group 0 is
//! the whole match — with `null` where a group did not take part; `groups` holds the participating NAMED
//! captures, as DEC-295 defined it. Offsets are BYTE offsets, the unit `String.substring` and
//! `String.length` use, and exactly what the PHP twin reads from `PREG_OFFSET_CAPTURE`.

use super::engine::Caps;
use super::natives::{engine_of, named_pairs};
use crate::value::{build_map, Instance, Value};
use std::rc::Rc;

/// The hand-built `RegexMatch`: sorted names, matching what `class_field_layout` derives for the
/// prelude's promoted constructor, so eq/reflect parity holds.
pub(super) fn match_value(s: &str, names: &[String], caps: &Caps) -> Result<Value, String> {
    let (ws, we) = caps.whole;
    let inst = Instance::new(
        "RegexMatch".into(),
        crate::value::ClassLayout::from_sorted_names(&["groups", "matched", "starts", "texts"]),
    );
    inst.set_field("matched", Value::Str(s[ws..we].into()));
    inst.set_field(
        "groups",
        Value::Map(Rc::new(build_map(named_pairs(s, names, caps))?)),
    );
    let (mut texts, mut starts) = (Vec::new(), Vec::new());
    for g in std::iter::once(Some(caps.whole)).chain(caps.groups.iter().copied()) {
        match g {
            Some((a, b)) => {
                texts.push(Value::Str(s[a..b].into()));
                let at = i64::try_from(a).map_err(|_| "Regex: a match offset overflows int")?;
                starts.push(Value::Int(at));
            }
            None => {
                texts.push(Value::Null);
                starts.push(Value::Null);
            }
        }
    }
    inst.set_field("texts", Value::List(Rc::new(texts)));
    inst.set_field("starts", Value::List(Rc::new(starts)));
    Ok(Value::Instance(Rc::new(inst)))
}

/// `Regex.first(Regex, string) -> RegexMatch?` — the first match, else `null`.
pub(super) fn regex_first(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let engine = engine_of(re)?;
            match engine.captures_first(s)? {
                None => Ok(Value::Null),
                Some(caps) => match_value(s, &engine.group_names(), &caps),
            }
        }
        _ => Err("Regex.first expects (Regex, string)".into()),
    }
}

/// `Regex.all(Regex, string) -> List<RegexMatch>` — every match, left to right (empty if none).
pub(super) fn regex_all(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [re, Value::Str(s)] => {
            let engine = engine_of(re)?;
            let names = engine.group_names();
            let out = engine
                .captures_all(s)?
                .iter()
                .map(|caps| match_value(s, &names, caps))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err("Regex.all expects (Regex, string)".into()),
    }
}
