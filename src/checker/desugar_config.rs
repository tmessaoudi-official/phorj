//! DEC-318 — typed-config entry injection: `#[Config]` provider + `#[Entry] main(config: T)`.
//!
//! A `#[Config]`-attributed ZERO-ARG top-level function returning a concrete type is the program's
//! typed-config provider. An `#[Entry]` function may then declare ONE parameter of that type, and this
//! pass injects the wiring:
//!
//! ```text
//!   #[Config] function appConfig() -> AppConfig { return new AppConfig(...); }
//!   #[Entry]  function main(config: AppConfig) -> void { ... }
//! ```
//! desugars the entry to
//! ```text
//!   #[Entry]  function main() -> void { AppConfig config = appConfig(); ... }
//! ```
//!
//! A PRE-CHECK desugar (mirrors [`crate::checker::desugar_di`] / `desugar_db`): the rewrite happens
//! BEFORE the type-checker, so the injected declaration type-checks like hand-written code, the
//! `#[Entry]` role rules (`entry_role`) see an ordinary zero-arg CLI entry, and every backend — and
//! the transpiled PHP — sees the same explicit call (Inv-5; the injection is PURE, so it stays inside
//! the byte-identity spine). No runtime container, no reflection: config is a plain function call.
//!
//! PRECEDENCE — never touch a signature that is already a valid entry: `()`, `(List<string>)` (argv)
//! and `(Request) -> Response` (web) all have `entry_role(f) != None` and pass through unchanged. Only
//! an entry with `entry_role == None`, EXACTLY ONE plain named-type parameter, and a CLI return
//! (`void`/`int`/none) is a config-entry candidate; anything else keeps its ordinary `E-ENTRY-SIG`.
//!
//! Provider rules (each `E-CONFIG-SIG` unless noted): zero parameters; a concrete named return type
//! (not `void`); top-level function only (`E-CONFIG-TARGET` on a method); at most one provider per
//! returned type (`E-CONFIG-DUP`); the bare marker takes no arguments (`E-ATTRIBUTE-ARGS`, matching
//! `#[Entry]`). A config-entry whose parameter type has no provider is `E-CONFIG-MISSING`. A provider
//! nobody injects is fine — it is an ordinary callable function.
//!
//! Import discipline (wind rule): the `Config` marker is gated by `import Core.Runtime.Config;`
//! (`preludes.rs` `bare_types`, the `Entry` precedent) — enforced by `enforce_injected_discipline`
//! upstream of this pass.

use crate::ast::{Expr, FunctionDecl, Item, Program, Stmt, Type};
use crate::diagnostic::{Diagnostic, Stage};
use std::collections::BTreeMap;

/// Run the DEC-318 config-entry injection over `program`. A no-op (identity) when no `#[Config]`
/// attribute and no config-entry candidate appears.
pub fn desugar_config(program: Program) -> Result<Program, Vec<Diagnostic>> {
    let mut errs: Vec<Diagnostic> = Vec::new();

    // ── Collect providers: `#[Config]` zero-arg top-level fns, keyed by returned type name. ──
    let mut providers: BTreeMap<String, String> = BTreeMap::new(); // type name → provider fn name
    for it in &program.items {
        match it {
            Item::Function(f) => {
                let Some(attr) = f.attrs.iter().find(|a| a.is_config()) else {
                    continue;
                };
                if !attr.args.is_empty() {
                    errs.push(err(
                        attr.span,
                        "`#[Config]` takes no arguments — it is a bare marker".into(),
                        "E-ATTRIBUTE-ARGS",
                        Some("write it as `#[Config]`".into()),
                    ));
                    continue;
                }
                let ret_name = match &f.ret {
                    Some(Type::Named { name, .. }) if name != "void" => Some(name.clone()),
                    _ => None,
                };
                if !f.params.is_empty() || ret_name.is_none() {
                    errs.push(err(
                        f.span,
                        format!(
                            "`#[Config]` provider `{}` must take no parameters and return a concrete type",
                            f.name
                        ),
                        "E-CONFIG-SIG",
                        Some("shape: `#[Config] function appConfig() -> AppConfig { ... }`".into()),
                    ));
                    continue;
                }
                let ty = ret_name.expect("checked above");
                if let Some(first) = providers.get(&ty) {
                    errs.push(err(
                        f.span,
                        format!(
                            "duplicate `#[Config]` provider for `{ty}` — `{first}` already provides it"
                        ),
                        "E-CONFIG-DUP",
                        Some("a program declares at most ONE provider per config type".into()),
                    ));
                    continue;
                }
                providers.insert(ty, f.name.clone());
            }
            Item::Class(c) => {
                // `#[Config]` is top-level-only: a method provider has no injection story (whose
                // instance?), so reject it loudly rather than ignore it.
                for m in &c.members {
                    if let crate::ast::ClassMember::Method(mf) = m {
                        if let Some(attr) = mf.attrs.iter().find(|a| a.is_config()) {
                            errs.push(err(
                                attr.span,
                                "`#[Config]` on a method — providers are top-level functions only"
                                    .into(),
                                "E-CONFIG-TARGET",
                                Some("move the provider to a top-level function".into()),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Fast path: nothing to inject and nothing wrong.
    if providers.is_empty() && errs.is_empty() {
        let any_candidate = program
            .items
            .iter()
            .any(|it| matches!(it, Item::Function(f) if config_entry_param(f).is_some()));
        if !any_candidate {
            return Ok(program);
        }
    }

    // ── Rewrite config-entry candidates. ──
    let mut prog = program;
    for it in &mut prog.items {
        let Item::Function(f) = it else { continue };
        let Some((param_ty_name, param_span)) = config_entry_param(f) else {
            continue;
        };
        match providers.get(&param_ty_name) {
            Some(provider) => {
                let p = f.params.remove(0);
                let init = Expr::Call {
                    callee: Box::new(Expr::Ident(provider.clone(), param_span)),
                    args: Vec::new(),
                    type_args: Vec::new(),
                    span: param_span,
                };
                f.body.insert(
                    0,
                    Stmt::VarDecl {
                        ty: p.ty,
                        name: p.name,
                        init,
                        mutable: false,
                        span: param_span,
                    },
                );
            }
            None => errs.push(err(
                param_span,
                format!(
                    "entry takes `{param_ty_name}` but no `#[Config]` provider returns `{param_ty_name}`"
                ),
                "E-CONFIG-MISSING",
                Some(format!(
                    "declare one: `#[Config] function appConfig() -> {param_ty_name} {{ ... }}` (import Core.Runtime.Config;)"
                )),
            )),
        }
    }

    if errs.is_empty() {
        Ok(prog)
    } else {
        Err(errs)
    }
}

/// The config-entry candidate test: an `#[Entry]` function that is NOT already a valid entry role,
/// with exactly one plain named-type parameter (not `List<string>`/`Request` — those belong to the
/// argv/web roles) and a CLI-shaped return. Returns the parameter's type name + span.
fn config_entry_param(f: &FunctionDecl) -> Option<(String, crate::token::Span)> {
    if !f.attrs.iter().any(|a| a.is_entry()) || crate::ast::entry_role(f).is_some() {
        return None;
    }
    if f.params.len() != 1 {
        return None;
    }
    let ret_cli = match &f.ret {
        None => true,
        Some(Type::Named { name, args, .. }) => {
            args.is_empty() && (name == "void" || name == "int")
        }
        Some(_) => false,
    };
    if !ret_cli {
        return None;
    }
    match &f.params[0].ty {
        Type::Named { name, span, .. } => Some((name.clone(), *span)),
        _ => None,
    }
}

fn err(
    span: crate::token::Span,
    msg: String,
    code: &'static str,
    hint: Option<String>,
) -> Diagnostic {
    let d = Diagnostic::new(Stage::Type, msg, span.line, span.col).with_code(code);
    match hint {
        Some(h) => d.with_hint(h),
        None => d,
    }
}

#[cfg(test)]
mod tests {
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

    const BASE: &str = "package Main;\nimport Core.Runtime.Entry;\nimport Core.Runtime.Config;\n\
                        class AppConfig { }\n";

    #[test]
    fn injects_provider_call_and_drops_the_param() {
        let src = format!(
            "{BASE}#[Config] function appConfig(): AppConfig {{ return new AppConfig(); }}\n\
             #[Entry] function main(AppConfig config): void {{ }}\n"
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

    #[test]
    fn missing_provider_is_e_config_missing() {
        let src = format!("{BASE}#[Entry] function main(AppConfig config): void {{ }}\n");
        assert_eq!(run(&src).unwrap_err(), vec!["E-CONFIG-MISSING"]);
    }

    #[test]
    fn duplicate_providers_are_e_config_dup() {
        let src = format!(
            "{BASE}#[Config] function a(): AppConfig {{ return new AppConfig(); }}\n\
             #[Config] function b(): AppConfig {{ return new AppConfig(); }}\n\
             #[Entry] function main(AppConfig config): void {{ }}\n"
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
             #[Entry] function main(List<string> args): void {{ }}\n"
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
        let src = format!(
            "{BASE}#[Config(\"x\")] function a(): AppConfig {{ return new AppConfig(); }}\n"
        );
        assert_eq!(run(&src).unwrap_err(), vec!["E-ATTRIBUTE-ARGS"]);
    }

    #[test]
    fn no_config_no_candidate_is_identity() {
        let src = "package Main;\nimport Core.Runtime.Entry;\n#[Entry] function main(): void { }\n";
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
}
