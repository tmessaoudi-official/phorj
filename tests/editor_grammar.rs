//! The TextMate grammar both editors load (`editors/vscode/syntaxes/phorj.tmLanguage.json` — VS Code
//! directly, JetBrains through its TextMate bundle support) colours a reserved word as a NAME in member
//! position and as a KEYWORD everywhere else (DEC-531, scout row 5k).
//!
//! There is no TextMate engine in this crate, so `scopes` is a minimal one over the grammar's
//! single-line `match` rules: at each position the earliest match wins, ties go to the rule listed
//! first (TextMate's own rule), and a capture's scope beats the match's. Multi-line `begin`/`end`
//! rules (strings, block comments) are skipped, so the probe lines avoid them. `fancy-regex` stands
//! in for Oniguruma; the constructs used here (look-around, `\b`, classes) mean the same in both.
#![cfg(feature = "regex")]

use phorj::json::Json;

struct Rule {
    re: fancy_regex::Regex,
    name: Option<String>,
    captures: Vec<(usize, String)>,
}

fn grammar() -> Json {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/editors/vscode/syntaxes/phorj.tmLanguage.json"
    ))
    .expect("grammar readable");
    Json::parse(&text).expect("grammar is valid JSON")
}

fn str_of(j: &Json) -> Option<&str> {
    match j {
        Json::Str(s) => Some(s),
        _ => None,
    }
}

fn arr_of(j: &Json) -> &[Json] {
    match j {
        Json::Arr(a) => a,
        _ => &[],
    }
}

/// The top-level rules in order, `include`s expanded (one level — that is all the grammar uses).
fn rules(g: &Json) -> Vec<Rule> {
    let repo = g.get("repository").expect("repository");
    let mut out = Vec::new();
    let mut push = |p: &Json| {
        let Some(src) = p.get("match").and_then(str_of) else {
            return;
        };
        let mut captures = Vec::new();
        if let Some(Json::Obj(caps)) = p.get("captures") {
            for (k, v) in caps {
                if let (Ok(n), Some(name)) = (k.parse(), v.get("name").and_then(str_of)) {
                    captures.push((n, name.to_string()));
                }
            }
        }
        out.push(Rule {
            re: fancy_regex::Regex::new(src).unwrap_or_else(|e| panic!("{src}: {e}")),
            name: p.get("name").and_then(str_of).map(str::to_string),
            captures,
        });
    };
    for p in arr_of(g.get("patterns").expect("patterns")) {
        match p.get("include").and_then(str_of) {
            Some(inc) => {
                let r = repo.get(&inc[1..]).expect("included rule exists");
                for q in arr_of(r.get("patterns").unwrap_or(&Json::Arr(Vec::new()))) {
                    push(q);
                }
                push(r);
            }
            None => push(p),
        }
    }
    out
}

/// The scope the grammar gives the `nth` occurrence (0-based) of `word` in `line`.
fn scope_of(line: &str, word: &str, nth: usize) -> String {
    let at = line.match_indices(word).nth(nth).expect("word present").0;
    let rules = rules(&grammar());
    let mut pos = 0;
    while pos < line.len() {
        let mut best: Option<(usize, &Rule, fancy_regex::Captures<'_, str>)> = None;
        for r in &rules {
            if let Ok(Some(c)) = r.re.captures_from_pos(line, pos) {
                let start = c.get(0).unwrap().start();
                if best.as_ref().is_none_or(|(b, _, _)| start < *b) {
                    best = Some((start, r, c));
                }
            }
        }
        let Some((_, rule, caps)) = best else { break };
        let whole = caps.get(0).unwrap();
        for (n, name) in &rule.captures {
            if let Some(m) = caps.get(*n) {
                if m.start() <= at && at < m.end() {
                    return name.clone();
                }
            }
        }
        if whole.start() <= at && at < whole.end() {
            return rule.name.clone().unwrap_or_else(|| "<unscoped>".into());
        }
        pos = if whole.end() > pos {
            whole.end()
        } else {
            pos + 1
        };
    }
    "<none>".into()
}

#[test]
fn a_keyword_member_is_coloured_as_a_name() {
    let line =
        "  public function match(int x): int { return match (x) { default => r.match(1) }; }";
    assert_eq!(scope_of(line, "match", 0), "entity.name.function.phorj");
    assert_eq!(scope_of(line, "match", 1), "keyword.control.phorj");
    assert_eq!(scope_of(line, "match", 2), "entity.name.function.phorj");
    assert_eq!(
        scope_of("  public int type = 1;", "type", 0),
        "variable.other.property.phorj"
    );
    assert_eq!(
        scope_of("  constructor(public string type) {}", "type", 0),
        "variable.other.property.phorj"
    );
    assert_eq!(
        scope_of("var s = r with { type = 2 };", "type", 0),
        "variable.other.property.phorj"
    );
    assert_eq!(
        scope_of("return this.type ?? C::open;", "type", 0),
        "variable.other.property.phorj"
    );
    assert_eq!(
        scope_of("return this.type ?? C::open;", "open", 0),
        "variable.other.property.phorj"
    );
}

#[test]
fn a_keyword_in_keyword_position_keeps_its_colour() {
    assert_eq!(
        scope_of("type Id = int;", "type", 0),
        "keyword.declaration.phorj"
    );
    assert_eq!(
        scope_of("  if (c) { return; } else { break; }", "return", 0),
        "keyword.control.phorj"
    );
    assert_eq!(
        scope_of("  if (c) { return; } else { break; }", "else", 0),
        "keyword.control.phorj"
    );
    assert_eq!(
        scope_of("var y = x with { a = 1 };", "with", 0),
        "storage.modifier.phorj"
    );
    assert_eq!(
        scope_of("var y = c ? true : null;", "true", 0),
        "constant.language.phorj"
    );
    assert_eq!(
        scope_of("var y = new Rule();", "new", 0),
        "keyword.operator.word.phorj"
    );
    assert_eq!(
        scope_of("function main(): void {}", "function", 0),
        "keyword.declaration.phorj"
    );
    // DEC-532: a throw-expression keeps `throw`'s keyword colour in each position.
    for line in [
        "  return s ?? throw new Bad(\"x\");",
        "  return match (a) { \"x\" => 1, default => throw e };",
        "  return if (c) { throw e } else { 1 };",
        "  var f = function(int x): int throws Bad => throw e;",
    ] {
        assert_eq!(
            scope_of(line, "throw ", 0),
            "keyword.control.phorj",
            "{line}"
        );
    }
}

#[test]
fn the_member_word_list_is_the_reserved_list() {
    // Drift guard: the grammar's word list must be the lexer's, minus `constructor`.
    let g = grammar();
    let km = g
        .get("repository")
        .and_then(|r| r.get("keyword-members"))
        .expect("rule set");
    let first = str_of(arr_of(km.get("patterns").unwrap())[0].get("match").unwrap()).unwrap();
    let alt = first
        .split("\\b(")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap();
    let mut words: Vec<&str> = alt.split('|').collect();
    let mut want: Vec<&str> = phorj::tokenizer::RESERVED_WORDS
        .iter()
        .copied()
        .filter(|w| *w != "constructor")
        .collect();
    words.sort_unstable();
    want.sort_unstable();
    assert_eq!(words, want);
}
