use super::natives::*;
use crate::value::Value;

/// Build a compiled `Regex` value for a pattern (the helper the natives consume).
fn re(pattern: &str) -> Value {
    let mut o = String::new();
    regex_compile(&[Value::Str(pattern.into())], &mut o).expect("valid pattern")
}

/// `Value::Str` → its text (`Value` has no `PartialEq` — it holds `f64`).
fn text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.as_str().to_string(),
        other => panic!("expected a string value, got {other:?}"),
    }
}

/// A `Value::List` of strings → `Vec<String>`.
fn texts(v: &Value) -> Vec<String> {
    match v {
        Value::List(xs) => xs.iter().map(text).collect(),
        other => panic!("expected a list value, got {other:?}"),
    }
}

#[test]
fn compile_validates_and_faults_on_unsupported() {
    let mut o = String::new();
    // A valid regular pattern compiles to a `Regex` instance carrying the bare pattern.
    match re(r"\d{4}") {
        Value::Instance(inst) => {
            assert_eq!(inst.class.as_ref(), "Regex");
            assert_eq!(text(&inst.get_field("pattern").unwrap()), r"\d{4}");
        }
        other => panic!("compile returned {other:?}"),
    }
    // Unbalanced — a clean fault, never a panic.
    assert!(regex_compile(&[Value::Str("(".into())], &mut o).is_err());
    // Backreferences are unsupported by the linear-time engine → rejected at compile (ReDoS-safe).
    assert!(regex_compile(&[Value::Str(r"(\w)\1".into())], &mut o).is_err());
}

#[test]
fn matches_find_find_all_split() {
    let mut o = String::new();
    let word = re(r"\w+");
    assert!(matches!(
        regex_matches(&[word.clone(), Value::Str("ab cd".into())], &mut o),
        Ok(Value::Bool(true))
    ));
    assert!(matches!(
        regex_matches(&[re(r"\d+"), Value::Str("no digits".into())], &mut o),
        Ok(Value::Bool(false))
    ));
    // find: first whole match, else null.
    assert!(matches!(
        regex_find(&[re(r"\d+"), Value::Str("x 42 y".into())], &mut o),
        Ok(Value::Str(s)) if s == "42"
    ));
    assert!(matches!(
        regex_find(&[re(r"\d+"), Value::Str("none".into())], &mut o),
        Ok(Value::Null)
    ));
    // findAll: every whole match.
    let all = regex_find_all(&[word.clone(), Value::Str("a bb ccc".into())], &mut o).unwrap();
    assert_eq!(texts(&all), vec!["a", "bb", "ccc"]);
    // split.
    let parts = regex_split(&[re(r",\s*"), Value::Str("p, q,r".into())], &mut o).unwrap();
    assert_eq!(texts(&parts), vec!["p", "q", "r"]);
}

#[test]
fn find_groups_named_only_and_null() {
    let mut o = String::new();
    let date = re(r"(?<y>\d{4})-(?<m>\d{2})");
    match regex_find_groups(&[date.clone(), Value::Str("2026-06".into())], &mut o).unwrap() {
        Value::Map(pairs) => {
            // Named captures in group-index order; numbered captures omitted.
            assert_eq!(pairs.len(), 2);
            assert_eq!(pairs[0].0, crate::value::HKey::Str("y".into()));
            assert_eq!(text(&pairs[0].1), "2026");
            assert_eq!(pairs[1].0, crate::value::HKey::Str("m".into()));
            assert_eq!(text(&pairs[1].1), "06");
        }
        other => panic!("findGroups returned {other:?}"),
    }
    // No match → null.
    assert!(matches!(
        regex_find_groups(&[date, Value::Str("nope".into())], &mut o),
        Ok(Value::Null)
    ));
}

#[test]
fn find_all_groups_one_map_per_match() {
    let mut o = String::new();
    let pair = re(r"(?<k>\w+)=(?<v>\d+)");
    match regex_find_all_groups(&[pair.clone(), Value::Str("a=1 b=22 c=333".into())], &mut o)
        .unwrap()
    {
        Value::List(matches) => {
            assert_eq!(matches.len(), 3);
            // Each element is a named-group map in group-index order.
            let map_at = |i: usize| match &matches[i] {
                Value::Map(pairs) => pairs.clone(),
                other => panic!("expected a map, got {other:?}"),
            };
            let m0 = map_at(0);
            assert_eq!(m0[0].0, crate::value::HKey::Str("k".into()));
            assert_eq!(text(&m0[0].1), "a");
            assert_eq!(m0[1].0, crate::value::HKey::Str("v".into()));
            assert_eq!(text(&m0[1].1), "1");
            assert_eq!(text(&map_at(2)[1].1), "333");
        }
        other => panic!("findAllGroups returned {other:?}"),
    }
    // No match → empty list (not null — the all-matches contract).
    match regex_find_all_groups(&[pair, Value::Str("nope".into())], &mut o).unwrap() {
        Value::List(xs) => assert!(xs.is_empty()),
        other => panic!("expected an empty list, got {other:?}"),
    }
}

#[test]
fn replace_all() {
    let mut o = String::new();
    assert!(matches!(
        regex_replace(
            &[re(r"\d+"), Value::Str("a1b22c".into()), Value::Str("#".into())],
            &mut o
        ),
        Ok(Value::Str(s)) if s == "a#b#c"
    ));
}

#[test]
fn quote_meta_escapes_the_crate_meta_set_and_round_trips() {
    let mut o = String::new();
    // regex::escape's exact meta-set is escaped; non-meta bytes (letters, spaces) pass through.
    assert!(matches!(
        regex_quote_meta(&[Value::Str(r"a.b (c) [d]".into())], &mut o),
        Ok(Value::Str(s)) if s == r"a\.b \(c\) \[d\]"
    ));
    // Round-trip: a quoted string compiles and matches itself literally, not as a pattern.
    let quoted = match regex_quote_meta(&[Value::Str("a.b+c".into())], &mut o).unwrap() {
        Value::Str(s) => s.as_str().to_string(),
        other => panic!("quoteMeta returned {other:?}"),
    };
    let lit = re(&quoted);
    assert!(matches!(
        regex_matches(&[lit.clone(), Value::Str("a.b+c".into())], &mut o),
        Ok(Value::Bool(true))
    ));
    assert!(matches!(
        regex_matches(&[lit, Value::Str("aXbbbc".into())], &mut o),
        Ok(Value::Bool(false))
    ));
}

#[test]
fn non_regex_first_arg_is_a_clean_fault() {
    let mut o = String::new();
    // A value that isn't a Regex instance → fault, never a panic (EV-7).
    assert!(regex_matches(&[Value::Int(1), Value::Str("x".into())], &mut o).is_err());
}

#[test]
fn php_emission_shapes() {
    let nats = regex_natives();
    let emit = |name: &str, a: &[&str]| {
        let args: Vec<String> = a.iter().map(|s| (*s).to_string()).collect();
        (nats.iter().find(|n| n.name == name).unwrap().php)(&args)
    };
    assert_eq!(emit("compile", &["$p"]), "new Regex($p)");
    assert_eq!(
        emit("matches", &["$re", "$s"]),
        "__phorj_regex_matches($re, $s)"
    );
    assert_eq!(emit("find", &["$re", "$s"]), "__phorj_regex_find($re, $s)");
    assert_eq!(
        emit("findGroups", &["$re", "$s"]),
        "__phorj_regex_find_groups($re, $s)"
    );
    assert_eq!(
        emit("findAllGroups", &["$re", "$s"]),
        "__phorj_regex_find_all_groups($re, $s)"
    );
    assert_eq!(emit("quoteMeta", &["$s"]), "__phorj_regex_quote_meta($s)");
    assert_eq!(
        emit("replaceCallback", &["$re", "$s", "$cb"]),
        "__phorj_regex_replace_callback($re, $s, $cb)"
    );
}
