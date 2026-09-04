//! DEC-318 — typed-config entry injection: `#[Config]` provider + `#[Entry] main(T config)`.
//!
//! A `#[Config]`-attributed ZERO-ARG top-level function returning a concrete type is the program's
//! typed-config provider. An `#[Entry]` function may then declare ONE OR MORE parameters of provider
//! types — each resolved BY TYPE — and this pass injects the wiring:
//!
//! ```text
//!   #[Config] function appConfig() -> AppConfig { return new AppConfig(...); }
//!   #[Entry(kind: EntryKind.Cli)]  function main(AppConfig config): void { ... }
//! ```
//! desugars the entry to
//! ```text
//!   #[Entry(kind: EntryKind.Cli)]  function main(): void { AppConfig config = appConfig(); ... }
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
//! an entry with `entry_role == None`, ONE OR MORE named-type parameters (generic ones included —
//! they key on the bare head; see `config_entry_params`), and a CLI return
//! (`void`/`int`/none) is a config-entry candidate; a signature that is not that shape at all (e.g. a
//! non-CLI return) keeps its ordinary `E-ENTRY-SIG`.
//!
//! NOT a filter on the parameter TYPES: since Part B any multi-parameter CLI-return entry is a
//! candidate. That once meant `main(int argc, string argv)` — a plain signature mistake — reported
//! `E-CONFIG-MISSING` per parameter, advising the developer to "declare
//! `#[Config] function appConfig() -> int`", which cannot be right.
//!
//! **DEC-474 closed it by candidacy, not by type filtering** (the fix that deletes nothing that
//! works): an entry where NO parameter resolves to a provider is declined as a config entry, its
//! buffered diagnostics are dropped, and the ordinary `E-ENTRY-SIG` names the valid shapes. With at
//! least ONE resolvable provider the intent is clearly config injection, so the per-parameter
//! `E-CONFIG-MISSING` stays — that half is the accurate message and is unchanged. Scalar providers
//! remain legal: a `#[Config]` returning `int` still registers, so `main(int n)` resolves when one
//! exists.
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
                // Key by the type's LEAF segment (audit 2026-07-22, P1). Leaf collisions across
                // packages are refused loudly below (E-CONFIG-DUP), never guessed.
                //
                // ⚠ This comment used to claim the leafing exists so "the entry may spell the type
                // qualified (`Cfg.AppConfig`) while the provider's return is bare — one type, two
                // legal spellings". The DEC-268 parity lens disproved that with a project control:
                // in a real multi-package project a qualified parameter spelling FAILS
                // (`entry takes `Cfg.AppConfig` but no `#[Config]` provider returns
                // `Cfg.AppConfig`` — the parameter key is NOT leafed, only the provider key is),
                // and in-file the checker rejects `Main.AppConfig` outright with E-UNKNOWN-TYPE.
                // So the two-spellings story is undeliverable either way. Pre-existing (identical
                // text at 92aa1dc), left as-is behaviourally and recorded rather than reworded into
                // something equally unverified — it is the same `leaf()` lossiness as DEC-455.4.
                let ty = leaf(&ret_name.expect("checked above")).to_string();
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
            Item::Trait(t) => {
                for m in &t.members {
                    if let crate::ast::ClassMember::Method(mf) = m {
                        if let Some(attr) = mf.attrs.iter().find(|a| a.is_config()) {
                            errs.push(err(
                                attr.span,
                                "`#[Config]` on a trait method — providers are top-level functions only"
                                    .into(),
                                "E-CONFIG-TARGET",
                                Some("move the provider to a top-level function".into()),
                            ));
                        }
                    }
                }
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
            .any(|it| matches!(it, Item::Function(f) if config_entry_params(f).is_some()));
        if !any_candidate {
            return Ok(program);
        }
    }

    // ── Rewrite config-entry candidates. ──
    let mut prog = program;
    for it in &mut prog.items {
        let Item::Function(f) = it else { continue };
        let Some(params) = config_entry_params(f) else {
            continue;
        };
        // Resolve EVERY parameter before mutating anything: a partially-rewritten entry (some params
        // injected, one left declared) would be a shape no later stage expects. Every unresolved type
        // is reported, not just the first — a two-param entry with neither provider declared must not
        // cost the developer two compiles to learn the second name.
        let mut resolved: Vec<&String> = Vec::with_capacity(params.len());
        // DEC-474 — buffer the per-parameter misses instead of reporting them immediately, because
        // whether they are the RIGHT diagnostic depends on how many parameters resolved overall.
        let mut missing: Vec<Diagnostic> = Vec::new();
        for (ty_name, span) in &params {
            match providers.get(leaf(ty_name)) {
                Some(provider) => resolved.push(provider),
                None => missing.push(err(
                    *span,
                    format!(
                        "entry takes `{ty_name}` but no `#[Config]` provider returns `{ty_name}`"
                    ),
                    "E-CONFIG-MISSING",
                    Some(format!(
                        "declare one: `#[Config] function appConfig() -> {} {{ ... }}` (import Core.Runtime.Config;)",
                        leaf(ty_name)
                    )),
                )),
            }
        }
        // DEC-474, NARROWED by the developer 2026-09-04. Part B widened candidacy to any
        // multi-parameter CLI-return entry, so `main(int argc, string argv)` — a plain signature
        // mistake — was told per parameter to "declare `#[Config] function appConfig() -> int`",
        // advice that cannot be right. Candidacy is declined for it, the buffered diagnostics are
        // dropped, and the ordinary `E-ENTRY-SIG` names the shapes an entry may actually have.
        //
        // The gate is "the program declares NO providers", NOT DEC-474's literal "no parameter
        // resolves". The literal rule also swallowed the commonest config mistake: a single
        // `main(AppConfig config)` whose provider return type is typo'd would report a SIGNATURE
        // error and never mention `#[Config]`. Once a program declares any provider the developer has
        // demonstrably opted into config injection, so an unresolvable parameter keeps the accurate
        // `E-CONFIG-MISSING` and names the type.
        if providers.is_empty() {
            continue;
        }
        if !missing.is_empty() {
            errs.extend(missing);
            continue; // leave the signature intact for the diagnostic
        }
        // Inject in DECLARATION ORDER. The providers are ordinary calls, so their order is observable
        // (a provider may print, or construct in a sequence the PHP leg must match) — splicing the
        // whole block at the front preserves it, where a loop of `insert(0, …)` would reverse it.
        //
        // DEC-473 — MEMOIZE PER ENTRY: one type, one instance. An entry declaring the same config
        // type twice (`main(AppConfig a, AppConfig b)`) used to emit two provider calls, so a
        // provider that reads a file, prints, or allocates ran TWICE and the two parameters held
        // DIFFERENT instances — observable both in output and in identity. The second and later
        // parameters of a type now bind to the FIRST one's variable instead of re-calling.
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        let mut decls: Vec<Stmt> = Vec::with_capacity(f.params.len());
        for ((p, provider), (ty_name, span)) in
            f.params.drain(..).zip(resolved).zip(params.iter().cloned())
        {
            let key = leaf(&ty_name).to_string();
            let init = match seen.get(&key) {
                // Binding to the first parameter's NAME (not re-calling) is what makes the two
                // parameters the same instance rather than two equal ones.
                Some(first) => Expr::Ident(first.clone(), span),
                None => {
                    seen.insert(key, p.name.clone());
                    Expr::Call {
                        callee: Box::new(Expr::Ident(provider.clone(), span)),
                        args: Vec::new(),
                        type_args: Vec::new(),
                        span,
                    }
                }
            };
            decls.push(Stmt::VarDecl {
                ty: p.ty,
                name: p.name,
                init,
                mutable: false,
                span,
            });
        }
        f.body.splice(0..0, decls);
    }

    if errs.is_empty() {
        Ok(prog)
    } else {
        Err(errs)
    }
}

/// The config-entry candidate test: an `#[Entry]` function that is NOT already a valid entry role,
/// with ONE OR MORE named-type parameters and a CLI-shaped return. Returns each parameter's type name
/// + span, in declaration order.
///
/// **S3.2 Part B widened this from exactly-one to N** — a multi-parameter config entry was rejected
/// outright before. ALL-OR-NOTHING: every parameter must be a named type, or the function is not a
/// config-entry candidate at all and keeps its ordinary `E-ENTRY-SIG`. NOTE this is about the SHAPE
/// (arity ≥ 1, named types, CLI return), not about whether the types are plausible config types — see
/// DEC-455.6 for the diagnostic-quality cost of that, pending a ruling.
///
/// A GENERIC parameter type is ACCEPTED, deliberately — and two drafts of this comment got that wrong
/// in opposite directions, so the history is recorded rather than quietly settled.
///
/// I first added an `args.is_empty()` guard here, believing generic parameters had only ever produced a
/// nonsense `E-CONFIG-MISSING` naming the bare head. **The DEC-268 parity lens refuted that with an
/// executed HEAD control:** `#[Config] function settings(): Map<string, string>` +
/// `main(Map<string, string> cfg)` **worked, and was byte-identical on all three legs.** It works
/// because provider keys and parameter keys are built the SAME lossy way — both take `Type::Named`'s
/// `name` and DROP `args` (see `ret_name` above) — so `Map<string, string>` keys as `Map` on both
/// sides and the lookup matches. Rejecting generics would have deleted a working, three-leg-green
/// language surface, which Invariant 15 makes the developer's call, not a session's.
///
/// What IS a real (pre-existing) defect, left as a PENDING question rather than "fixed" by removing the
/// feature: because both sides drop `args`, providers returning `Map<string, int>` and
/// `Map<string, string>` collide under one key `Map` — so one spuriously reports `E-CONFIG-DUP`, and a
/// mismatched pairing would inject the wrong provider. Recorded in the register under DEC-455.
fn config_entry_params(f: &FunctionDecl) -> Option<Vec<(String, crate::token::Span)>> {
    if !f.attrs.iter().any(|a| a.is_entry()) || crate::ast::entry_role(f).is_some() {
        return None;
    }
    if f.params.is_empty() {
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
    f.params
        .iter()
        .map(|p| match &p.ty {
            // NOTE: no `args.is_empty()` filter — see the doc above. A generic parameter type keys the
            // same lossy way the provider's return type does, so `Map<string, string>` resolves and has
            // always resolved. Adding a filter here regressed that surface and was reverted.
            Type::Named { name, span, .. } => Some((name.clone(), *span)),
            _ => None,
        })
        .collect()
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

/// The last dot-segment of a type name.
fn leaf(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

#[cfg(test)]
mod tests;
