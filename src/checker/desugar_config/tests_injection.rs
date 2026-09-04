//! Injection-resolution tests for the `#[Config]` desugar — DEC-473 (memoize per entry), DEC-474
//! (decline candidacy when a program declares no providers) and DEC-457 (reified injection keys).
//!
//! Split from `tests.rs` under Invariant 13: adding these three groups took that file to 533 lines,
//! past the 500 hard cap. The cohesion line is real rather than arbitrary — everything here is about
//! WHICH provider a parameter resolves to and how many times it is called, where `tests.rs` covers
//! provider validity and the diagnostics themselves.

use super::tests::run;

// ── DEC-473 — memoize per entry: one type, one instance ────────────────────────────────────────

/// Render the injected declarations of `main` as `name = <init>` pairs, so a test can assert WHICH
/// initialiser each parameter got — a call, or a reference to an earlier binding.
fn injected_inits(src: &str) -> Vec<(String, String)> {
    let prog = run(src).expect("desugar ok");
    let mut out = Vec::new();
    for it in &prog.items {
        let crate::ast::Item::Function(f) = it else {
            continue;
        };
        if f.name != "main" {
            continue;
        }
        for st in &f.body {
            if let crate::ast::Stmt::VarDecl { name, init, .. } = st {
                let rendered = match init {
                    crate::ast::Expr::Call { callee, .. } => match callee.as_ref() {
                        crate::ast::Expr::Ident(n, _) => format!("call {n}"),
                        _ => "call <?>".to_string(),
                    },
                    crate::ast::Expr::Ident(n, _) => format!("ref {n}"),
                    _ => "<other>".to_string(),
                };
                out.push((name.clone(), rendered));
            }
        }
    }
    out
}

const TWO_SAME: &str = r#"
class AppConfig { constructor(public int port) {} }
#[Config] function appConfig() -> AppConfig { return new AppConfig(1); }
#[Entry] function main(AppConfig a, AppConfig b) -> void { }
"#;

/// The DEC-473 defect: two parameters of one type emitted TWO provider calls, so a provider that
/// reads a file or prints ran twice and the parameters held DIFFERENT instances. The second must
/// now bind to the first rather than re-calling.
#[test]
fn a_repeated_config_type_calls_its_provider_once() {
    let inits = injected_inits(TWO_SAME);
    assert_eq!(
        inits,
        vec![
            ("a".to_string(), "call appConfig".to_string()),
            ("b".to_string(), "ref a".to_string()),
        ],
        "the second parameter of a repeated type must alias the first, not re-call the provider"
    );
    let calls = inits.iter().filter(|(_, i)| i.starts_with("call ")).count();
    assert_eq!(calls, 1, "one type, one instance");
}

/// The other direction, which memoization must not break: DISTINCT types each keep their own call,
/// and declaration order is preserved (a provider may print, and the PHP leg must match).
#[test]
fn distinct_config_types_each_keep_their_own_call() {
    let src = r#"
class AppConfig { constructor(public int port) {} }
class DbConfig { constructor(public string dsn) {} }
#[Config] function appConfig() -> AppConfig { return new AppConfig(1); }
#[Config] function dbConfig() -> DbConfig { return new DbConfig("x"); }
#[Entry] function main(AppConfig a, DbConfig d, AppConfig b) -> void { }
"#;
    assert_eq!(
        injected_inits(src),
        vec![
            ("a".to_string(), "call appConfig".to_string()),
            ("d".to_string(), "call dbConfig".to_string()),
            ("b".to_string(), "ref a".to_string()),
        ],
        "distinct types each call; the repeat aliases the FIRST of its own type, in declaration order"
    );
}

/// Three or more of the same type collapse to one call, not to one-per-pair.
#[test]
fn three_of_a_kind_still_call_once() {
    let src = r#"
class AppConfig { constructor(public int port) {} }
#[Config] function appConfig() -> AppConfig { return new AppConfig(1); }
#[Entry] function main(AppConfig a, AppConfig b, AppConfig c) -> void { }
"#;
    let inits = injected_inits(src);
    assert_eq!(
        inits.iter().filter(|(_, i)| i.starts_with("call ")).count(),
        1
    );
    assert_eq!(inits[1].1, "ref a");
    assert_eq!(
        inits[2].1, "ref a",
        "the third aliases the FIRST, not the second"
    );
}

// ── DEC-474 — decline candidacy when nothing resolves ──────────────────────────────────────────

/// The regression DEC-474 closes: a plain signature mistake, in a program with no `#[Config]` at
/// all, used to be told per parameter to declare a provider returning `int`. This pass must now
/// leave it entirely alone so the downstream `E-ENTRY-SIG` can name the shapes an entry may have.
#[test]
fn an_entry_with_no_resolvable_provider_is_left_for_e_entry_sig() {
    let src = r#"
#[Entry] function main(int argc, string argv) -> void { }
"#;
    let prog =
        run(src).expect("desugar must report NOTHING here — E-ENTRY-SIG is downstream's job");
    // …and it must leave the signature intact, since the downstream diagnostic describes it.
    let f = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main survives");
    assert_eq!(f.params.len(), 2, "the parameters must not be consumed");
    assert!(
        f.body.is_empty(),
        "nothing may be injected into a declined candidate"
    );
}

/// The other half, deliberately unchanged: with at least ONE resolvable provider the intent IS
/// config injection, so an unresolvable parameter still gets the accurate `E-CONFIG-MISSING`.
#[test]
fn one_resolvable_provider_keeps_e_config_missing_for_the_rest() {
    let src = r#"
class AppConfig { constructor(public int port) {} }
#[Config] function appConfig() -> AppConfig { return new AppConfig(1); }
#[Entry] function main(AppConfig a, int b) -> void { }
"#;
    assert_eq!(run(src).unwrap_err(), vec!["E-CONFIG-MISSING"]);
}

/// Scalar providers stay legal — the fix is by CANDIDACY, not by filtering parameter types, so a
/// `#[Config]` returning `int` still resolves an `int` parameter.
#[test]
fn a_scalar_provider_still_resolves_a_scalar_parameter() {
    let src = r#"
#[Config] function port() -> int { return 8080; }
#[Entry] function main(int p) -> void { }
"#;
    let prog = run(src).expect("a scalar provider is legal");
    let f = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main survives");
    assert!(
        f.params.is_empty(),
        "the parameter is injected, not left declared"
    );
    assert_eq!(f.body.len(), 1, "one injected declaration");
}

// ── DEC-457 — reified injection keys ───────────────────────────────────────────────────────────

/// The DEC-457 defect: `Map<string, string>` and `Map<string, int>` both keyed as `Map`, so
/// declaring providers for both was rejected as a duplicate — and an entry taking one could have
/// received the other. They are now distinct keys and both resolve.
#[test]
fn two_generic_providers_of_one_head_are_distinct() {
    let src = r#"
#[Config] function names() -> List<string> { return ["a"]; }
#[Config] function counts() -> List<int> { return [1]; }
#[Entry] function main(List<string> n, List<int> c) -> void { }
"#;
    assert_eq!(
        injected_inits(src),
        vec![
            ("n".to_string(), "call names".to_string()),
            ("c".to_string(), "call counts".to_string()),
        ],
        "each parameter must resolve to the provider of its OWN reified type"
    );
}

/// The other direction, which reification must not break: a genuine duplicate — two providers of
/// the SAME reified type — is still `E-CONFIG-DUP`.
#[test]
fn a_genuine_duplicate_is_still_rejected() {
    let src = r#"
#[Config] function a() -> List<int> { return [1]; }
#[Config] function b() -> List<int> { return [2]; }
#[Entry] function main(List<int> m) -> void { }
"#;
    assert_eq!(run(src).unwrap_err(), vec!["E-CONFIG-DUP"]);
}

/// A generic parameter whose ARGUMENTS differ from every provider is unresolved — the bug being
/// that under the old bare-head key it would have silently resolved to the wrong provider.
#[test]
fn a_mismatched_type_argument_does_not_silently_resolve() {
    let src = r#"
#[Config] function names() -> List<string> { return ["a"]; }
#[Entry] function main(List<int> c) -> void { }
"#;
    assert_eq!(
        run(src).unwrap_err(),
        vec!["E-CONFIG-MISSING"],
        "`List<int>` must NOT be satisfied by a `List<string>` provider"
    );
}

/// Non-generic types are unaffected: the key is still the bare leaf when there are no arguments,
/// so every pre-DEC-457 program keeps resolving exactly as before.
#[test]
fn a_plain_type_still_keys_on_its_leaf() {
    let src = r#"
class AppConfig { constructor(public int port) {} }
#[Config] function appConfig() -> AppConfig { return new AppConfig(1); }
#[Entry] function main(AppConfig c) -> void { }
"#;
    assert_eq!(
        injected_inits(src),
        vec![("c".to_string(), "call appConfig".to_string())]
    );
}
