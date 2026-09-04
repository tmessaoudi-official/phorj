//! Unit tests for the DEC-318 / DEC-331-S3.2 `#[Config]` entry-injection desugar.
//!
//! Split out of `mod.rs` per Invariant 13 (soft cap 300 / hard cap 500): the Part B widening
//! pushed the single file to 485 lines, 15 short of the hard cap, and split-as-you-go is the
//! DEFAULT rather than a cleanup deferred until the cap is hit.

use super::desugar_config;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn run(src: &str) -> Result<crate::ast::Program, Vec<String>> {
    let prog = Parser::new(lex(src).expect("lex"))
        .parse_program()
        .expect("parse");
    desugar_config(prog).map_err(|ds| {
        ds.into_iter()
            .map(|d| d.code.unwrap_or_default().to_string())
            .collect()
    })
}

/// Like [`run`] but keeps each diagnostic's `(code, line, col, message)` — `run` maps to the CODE
/// only, which is enough for "did it reject?" but throws away exactly the information a
/// per-parameter-reporting test needs to prove. Added after the DEC-268 completeness lens showed
/// `every_missing_provider_is_reported` could be satisfied by a mutant that reported `params[0]` twice
/// and never named the second parameter.
fn run_diags(src: &str) -> Vec<(String, u32, u32, String)> {
    let prog = Parser::new(lex(src).expect("lex"))
        .parse_program()
        .expect("parse");
    match desugar_config(prog) {
        Ok(_) => Vec::new(),
        Err(ds) => ds
            .into_iter()
            .map(|d| {
                (
                    d.code.unwrap_or_default().to_string(),
                    d.line,
                    d.col,
                    d.message,
                )
            })
            .collect(),
    }
}

const BASE: &str = "package Main;\nimport Core.Runtime.Entry; import Core.Runtime.EntryKind;\nimport Core.Runtime.Config;\n\
                    class AppConfig { }\n";

#[test]
fn injects_provider_call_and_drops_the_param() {
    let src = format!(
        "{BASE}#[Config] function appConfig(): AppConfig {{ return new AppConfig(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(AppConfig config): void {{ }}\n"
    );
    let prog = run(&src).expect("desugar ok");
    let main = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main present");
    assert!(main.params.is_empty(), "param must be dropped");
    match main.body.first() {
        Some(crate::ast::Stmt::VarDecl { name, ty, init, .. }) => {
            assert_eq!(name, "config");
            assert!(matches!(ty, crate::ast::Type::Named { name, .. } if name == "AppConfig"));
            assert!(matches!(init, crate::ast::Expr::Call { callee, .. }
                if matches!(&**callee, crate::ast::Expr::Ident(n, _) if n == "appConfig")));
        }
        other => panic!("expected injected VarDecl, got {other:?}"),
    }
    // Post-rewrite, the entry classifies as an ordinary CLI role.
    assert!(matches!(
        crate::ast::entry_role(main),
        Some(crate::ast::EntryRole::Cli)
    ));
}

/// S3.2 Part B — DEC-331 D4's §1 surface injects TWO typed parameters
/// (`function web(Http.ServeConfig cfg, AppSettings app)`), which the one-parameter limit rejected.
/// Both must be injected, IN DECLARATION ORDER, and the entry must still classify as a CLI role.
#[test]
fn injects_every_param_in_declaration_order() {
    let src = format!(
        // ANTI-ALPHABETICAL on purpose: `Zeta` sorts AFTER `Alpha`, but is declared FIRST. The
        // previous fixture used `AppConfig`/`AppSettings`, where declaration order happened to equal
        // the `providers` BTreeMap key order — so an implementation zipping params against
        // `providers.values()` would have passed. This fixture discriminates (DEC-268 completeness).
        "{BASE}class Zeta {{ }}\nclass Alpha {{ }}\n\
         #[Config] function zetaCfg(): Zeta {{ return new Zeta(); }}\n\
         #[Config] function alphaCfg(): Alpha {{ return new Alpha(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(Zeta z, Alpha a): void {{ }}\n"
    );
    let prog = run(&src).expect("desugar ok");
    let main = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main present");
    assert!(main.params.is_empty(), "both params must be dropped");
    // Order matters: `cfg` is declared first, so its decl must come first — a reversed splice
    // would still type-check yet change evaluation order, which the PHP leg would then disagree
    // with (Invariant 1). Pin the pairing of name → provider, not just the count.
    let got: Vec<(String, String)> = main
        .body
        .iter()
        .take(2)
        .map(|s| match s {
            crate::ast::Stmt::VarDecl { name, init, .. } => {
                let callee = match init {
                    crate::ast::Expr::Call { callee, .. } => match &**callee {
                        crate::ast::Expr::Ident(n, _) => n.clone(),
                        other => panic!("expected Ident callee, got {other:?}"),
                    },
                    other => panic!("expected Call init, got {other:?}"),
                };
                (name.clone(), callee)
            }
            other => panic!("expected injected VarDecl, got {other:?}"),
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("z".to_string(), "zetaCfg".to_string()),
            ("a".to_string(), "alphaCfg".to_string()),
        ]
    );
    assert!(matches!(
        crate::ast::entry_role(main),
        Some(crate::ast::EntryRole::Cli)
    ));
}

/// EVERY missing provider is reported, not just the first — a two-param entry with neither
/// provider declared must not make the developer recompile twice to see the second name.
#[test]
fn every_missing_provider_is_reported() {
    // A provider for an UNRELATED type, so the program has opted into config injection (DEC-474 as
    // narrowed) and the per-parameter diagnostics are the accurate report rather than a signature
    // complaint. Without it the entry is declined and this test would assert on an empty list.
    let src = format!(
        "{BASE}class AppSettings {{ }}\nclass Other {{ }}\n\
         #[Config] function other(): Other {{ return new Other(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(AppConfig cfg, AppSettings app): void {{ }}\n"
    );
    // Assert the MESSAGES and SPANS, not just the codes: a mutant that reported `params[0]` twice
    // and never named `AppSettings` satisfies a codes-only assertion, which is precisely the defect
    // this test exists to prevent (DEC-268 completeness lens).
    let ds = run_diags(&src);
    assert_eq!(
        ds.len(),
        2,
        "one diagnostic per unresolved parameter: {ds:?}"
    );
    assert!(ds.iter().all(|(c, ..)| c == "E-CONFIG-MISSING"), "{ds:?}");
    assert!(
        ds[0].3.contains("`AppConfig`") && ds[1].3.contains("`AppSettings`"),
        "each parameter must be named, in order: {ds:?}"
    );
    assert_ne!(
        (ds[0].1, ds[0].2),
        (ds[1].1, ds[1].2),
        "the two diagnostics must sit on DIFFERENT spans, not one span twice: {ds:?}"
    );
}

/// REGRESSION GUARD for a surface I briefly deleted. A GENERIC config type resolves, and always has:
/// provider keys and parameter keys are built the SAME lossy way (both drop `args`), so
/// `Map<string, string>` keys as `Map` on both sides and the lookup matches. I added an
/// `args.is_empty()` filter believing generics had only ever produced a nonsense error; the DEC-268
/// parity lens refuted that with an executed HEAD control showing this exact shape running
/// byte-identically on all three legs. Rejecting it would have deleted a working language surface —
/// Invariant 15 territory, not a session's call.
#[test]
fn a_generic_config_type_still_resolves() {
    let src = format!(
        "{BASE}#[Config] function settings(): Map<string, string> {{ return [\"env\" => \"prod\"]; }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(Map<string, string> cfg): void {{ }}\n"
    );
    let prog = run(&src).expect("a generic config type must still resolve");
    let main = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main present");
    assert!(main.params.is_empty(), "the generic param must be injected");
    assert!(matches!(
        main.body.first(),
        Some(crate::ast::Stmt::VarDecl { name, init, .. })
            if name == "cfg"
                && matches!(init, crate::ast::Expr::Call { callee, .. }
                    if matches!(&**callee, crate::ast::Expr::Ident(n, _) if n == "settings"))
    ));
}

/// Pins the CURRENT (and disliked) behaviour recorded as DEC-455.6, PENDING a developer ruling: a
/// multi-parameter entry whose parameter types have no providers is still a config candidate, so it
/// reports `E-CONFIG-MISSING` per parameter rather than the `E-ENTRY-SIG` that names the valid `Cli`
/// shapes. Before Part B this program said `E-ENTRY-SIG` + `E-MAIN-SIGNATURE`.
///
/// This test exists so the trade-off is VISIBLE and any change to it is deliberate — it is not an
/// endorsement. If DEC-455.6 rules for option (a) or (b), this test changes with it.
#[test]
fn a_multi_param_entry_of_non_provider_types_reports_config_missing() {
    // With a provider present the program has opted into config injection, so primitive parameters
    // that resolve to nothing are reported per parameter (DEC-474 as narrowed). The no-provider
    // case is the opposite and is covered by `an_entry_with_no_resolvable_provider_is_left_for_e_entry_sig`.
    let src = format!(
        "{BASE}class Other {{ }}\n#[Config] function other(): Other {{ return new Other(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(int argc, string argv): void {{ }}\n"
    );
    let ds = run_diags(&src);
    assert_eq!(
        ds.iter().map(|(c, ..)| c.as_str()).collect::<Vec<_>>(),
        vec!["E-CONFIG-MISSING", "E-CONFIG-MISSING"],
        "a program WITH providers reports per parameter: {ds:?}"
    );
    assert!(
        ds[0].3.contains("`int`") && ds[1].3.contains("`string`"),
        "each primitive parameter type is named: {ds:?}"
    );
}

#[test]
fn missing_provider_is_e_config_missing() {
    // The case DEC-474's narrowing exists to preserve: a provider IS declared, but for another
    // type — the classic typo'd return type. This must still name `AppConfig`, not complain about
    // the entry signature.
    let src = format!(
        "{BASE}class Other {{ }}\n#[Config] function other(): Other {{ return new Other(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(AppConfig config): void {{ }}\n"
    );
    assert_eq!(run(&src).unwrap_err(), vec!["E-CONFIG-MISSING"]);
}

#[test]
fn duplicate_providers_are_e_config_dup() {
    let src = format!(
        "{BASE}#[Config] function a(): AppConfig {{ return new AppConfig(); }}\n\
         #[Config] function b(): AppConfig {{ return new AppConfig(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(AppConfig config): void {{ }}\n"
    );
    assert_eq!(run(&src).unwrap_err(), vec!["E-CONFIG-DUP"]);
}

#[test]
fn provider_with_params_or_void_is_e_config_sig() {
    let with_params =
        format!("{BASE}#[Config] function a(int x): AppConfig {{ return new AppConfig(); }}\n");
    assert_eq!(run(&with_params).unwrap_err(), vec!["E-CONFIG-SIG"]);
    let void_ret = format!("{BASE}#[Config] function a(): void {{ }}\n");
    assert_eq!(run(&void_ret).unwrap_err(), vec!["E-CONFIG-SIG"]);
}

#[test]
fn valid_entry_shapes_pass_through_untouched() {
    // argv + zero-arg entries have entry_role != None and must not be rewritten,
    // even with a provider present.
    let src = format!(
        "{BASE}#[Config] function appConfig(): AppConfig {{ return new AppConfig(); }}\n\
         #[Entry(kind: EntryKind.Cli)] function main(List<string> args): void {{ }}\n"
    );
    let prog = run(&src).expect("ok");
    let main = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main");
    assert_eq!(main.params.len(), 1, "argv param must survive");
}

#[test]
fn config_marker_with_args_is_e_attribute_args() {
    let src =
        format!("{BASE}#[Config(\"x\")] function a(): AppConfig {{ return new AppConfig(); }}\n");
    assert_eq!(run(&src).unwrap_err(), vec!["E-ATTRIBUTE-ARGS"]);
}

#[test]
fn no_config_no_candidate_is_identity() {
    let src = "package Main;\nimport Core.Runtime.Entry; import Core.Runtime.EntryKind;\n#[Entry(kind: EntryKind.Cli)] function main(): void { }\n";
    let prog = run(src).expect("ok");
    let main = prog
        .items
        .iter()
        .find_map(|it| match it {
            crate::ast::Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        })
        .expect("main");
    assert!(
        main.params.is_empty() && main.body.is_empty(),
        "must be untouched"
    );
}

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
