//! Wave B B-2c part 2 (DEC-186) — resolve **imported injected-enum variants** to their qualified form
//! BEFORE the checker runs, so a bare (or `as`-aliased) variant brought in by `import Core.Result.Success;`
//! / `import Core.Option.None as Nothing;` / a group `import Core.Result.{ Success, Failure as Xzs };`
//! becomes the ordinary qualified `Enum.Variant` the rest of the pipeline already handles byte-identically.
//!
//! **Why rewrite to qualified rather than teach the resolver a third form:** the qualified construction
//! (`new Result.Success(v)`) and qualified pattern (`Result.Success(v) =>`) paths are already proven and
//! byte-identical across interp/VM/PHP (variant-qualification A2/B). Rewriting every imported use into that
//! form means we reuse them wholesale — no new resolution site, no bespoke bare-Ident→variant backend
//! rename, and `unwrap_new` still collapses the `Enum.Variant` callee to the bare variant for the backend.
//!
//! Two positions carry a variant name:
//!   * construction — `new X(args)` is `Expr::New(Call { callee: Ident(x), .. })`; when `x` is imported,
//!     the callee `Ident` becomes `Member { Ident(enum), realvariant }` (the qualified form).
//!   * `match` pattern — `Pattern::Variant { name: x, enum_qualifier: None, .. }` (the PARENS form
//!     `X(..)` / `X()`); when `x` is imported it gains `enum_qualifier = Some(enum)` and `name` becomes the
//!     real variant. A BARE zero-payload identifier `X =>` (no parens) parses as `Pattern::Binding` (a
//!     catch-all) — the existing zero-payload-needs-parens rule, identical for user/qualified/imported
//!     variants — so it is deliberately left untouched.
//!
//! Runs in [`crate::cli::check_and_expand_reified`] after prelude injection + qualifier collapse and before
//! `check_resolutions` — the single chokepoint every vm-compile path shares (Invariant 6). A no-op unless
//! at least one variant import is present, so programs without them are byte-for-byte unchanged.

use crate::ast::{
    ClassMember, Expr, FieldPat, Item, LambdaBody, MatchArm, Pattern, Program, Stmt, StrPart,
};
use std::collections::HashMap;

/// bound name (the `as` alias, else the variant leaf) → (enum name, real variant name).
type VarMap = HashMap<String, (String, String)>;

/// The raw variant-import bindings a program declares: for each `import Core.<Enum>.<Variant> [as A];`
/// whose `<Enum>` is an injected enum in this program that owns `<Variant>`, one `(bound, enum, variant)`.
/// Shared by the rewrite here AND the checker's collision check (`collect`) so the two never diverge on
/// what counts as a variant import. Non-matching Core paths (`Core.Http.Router` — a member TYPE import,
/// `Core.Output.printLine` — a function) yield nothing here and are handled by the existing import maps.
pub(crate) fn variant_import_bindings(items: &[Item]) -> Vec<(String, String, String)> {
    // The program's enums (post-injection) and their variant sets, for validation.
    let mut variants: HashMap<&str, std::collections::HashSet<&str>> = HashMap::new();
    for it in items {
        if let Item::Enum(e) = it {
            variants
                .entry(e.name.as_str())
                .or_default()
                .extend(e.variants.iter().map(|v| v.name.as_str()));
        }
    }
    let mut out = Vec::new();
    for it in items {
        if let Item::Import { path, alias, .. } = it {
            if path.len() == 3 && path[0] == "Core" {
                let (enum_name, variant) = (&path[1], &path[2]);
                if variants
                    .get(enum_name.as_str())
                    .is_some_and(|vs| vs.contains(variant.as_str()))
                {
                    let bound = alias.clone().unwrap_or_else(|| variant.clone());
                    out.push((bound, enum_name.clone(), variant.clone()));
                }
            }
        }
    }
    out
}

/// Rewrite every imported-variant use to its qualified form. A no-op when no variant imports are present.
pub fn resolve_variant_imports(program: Program) -> Program {
    let bindings = variant_import_bindings(&program.items);
    if bindings.is_empty() {
        return program;
    }
    // A name that also denotes a top-level item (class/enum/interface/trait/function) OR a variant of a
    // USER (non-injected) enum is a collision — left UNresolved here so it is never silently mis-rewritten
    // (else `import Core.Result.Success;` would hijack a local `enum Local { Success(..) }`'s bare
    // `new Success(..)`); the checker's `check_variant_import_collisions` then reports `E-IMPORT-CONFLICT`
    // and compilation stops. Injected enums are exempt from the variant side — their variants are exactly
    // what a variant import binds. Two imports binding the same name likewise drop out (kept only if
    // unique), so an ambiguous bare name is never rewritten to one arbitrary target.
    let mut local: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for it in &program.items {
        match it {
            Item::Class(c) => {
                local.insert(c.name.as_str());
            }
            Item::Enum(e) => {
                local.insert(e.name.as_str());
                if !e.injected {
                    local.extend(e.variants.iter().map(|v| v.name.as_str()));
                }
            }
            Item::Interface(i) => {
                local.insert(i.name.as_str());
            }
            Item::Trait(t) => {
                local.insert(t.name.as_str());
            }
            Item::Function(f) => {
                local.insert(f.name.as_str());
            }
            _ => {}
        }
    }
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for (b, _, _) in &bindings {
        *seen.entry(b.as_str()).or_default() += 1;
    }
    let map: VarMap = bindings
        .iter()
        .filter(|(b, _, _)| seen[b.as_str()] == 1 && !local.contains(b.as_str()))
        .map(|(b, e, v)| (b.clone(), (e.clone(), v.clone())))
        .collect();
    if map.is_empty() {
        return program;
    }

    let items = program
        .items
        .into_iter()
        .map(|item| match item {
            Item::Function(mut f) => {
                f.body = rblock(f.body, &map);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                for m in &mut c.members {
                    match m {
                        ClassMember::Method(f) => {
                            let body = std::mem::take(&mut f.body);
                            f.body = rblock(body, &map);
                        }
                        ClassMember::Constructor { body, .. } => {
                            let b = std::mem::take(body);
                            *body = rblock(b, &map);
                        }
                        ClassMember::Hook { get, set, .. } => {
                            if let Some(e) = get.take() {
                                *get = Some(rexpr(e, &map));
                            }
                            if let Some((p, body)) = set.take() {
                                *set = Some((p, rblock(body, &map)));
                            }
                        }
                        ClassMember::Field { init, .. } => {
                            if let Some(e) = init.take() {
                                *init = Some(rexpr(e, &map));
                            }
                        }
                    }
                }
                Item::Class(c)
            }
            other => other,
        })
        .collect();

    Program {
        package: program.package,
        items,
        span: program.span,
    }
}

#[path = "resolve_variant_imports_walk.rs"]
mod walk;
use walk::*;
