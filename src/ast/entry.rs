//! Entry-point model (DEC-331 D1): the `#[Entry(kind: …)]` attribute, role classification, and the
//! backend-selection resolvers. Split out of `class_hierarchy.rs` (Inv 13, M-Decomp) so the entry
//! concern lives in one cohesive place. Re-exported flat via `ast::*`, so call sites are unchanged.

use super::*;

/// The `#[Entry]` marker, in every "nothing in the wind" import form (bare after
/// `import Core.Runtime.Entry;`, or fully-qualified) — single source [`crate::ast::Attribute::is_entry`].
/// (`#[Config]`, DEC-318, has no free-fn twin — call [`crate::ast::Attribute::is_config`] directly.)
pub fn is_entry_attr(a: &crate::ast::Attribute) -> bool {
    a.is_entry()
}

/// Build a synthetic `#[Entry(kind: EntryKind.<kind>)]` attribute (DEC-331 D1) at `span`. Single
/// constructor for every place the compiler/lifter/test-runner synthesizes an entry, so the `kind:`
/// named-arg shape is written once. The kind is the QUALIFIED injected-enum variant
/// (`EntryKind.Cli` / `EntryKind.Web`) — never a bare identifier ("nothing in the wind", DEC-337).
#[must_use]
pub fn entry_attr(kind: &str, span: Span) -> Attribute {
    Attribute {
        name: "Entry".to_string(),
        args: vec![Expr::NamedArg {
            name: "kind".to_string(),
            value: Box::new(Expr::Member {
                object: Box::new(Expr::Ident("EntryKind".to_string(), span)),
                name: kind.to_string(),
                safe: false,
                sep: crate::ast::MemberSep::Dot,
                span,
            }),
            span,
        }],
        span,
    }
}

/// The injected enum whose variants name every `#[Entry]` kind (DEC-337). Import-gated under
/// `Core.Runtime` (like `Entry`/`Config`); reached QUALIFIED (`EntryKind.Cli`), never bare.
pub const ENTRY_KIND_ENUM: &str = "EntryKind";

/// DEC-337: the surface FORM the `kind:` argument was written in. Distinct from [`EntryKind`]
/// (which classifies the variant NAME) — this drives the checker's qualification + import
/// enforcement so `Cli`/`Web` are never "in the wind": a bare `kind: Cli` is `E-INJECTED-VARIANT-BARE`,
/// a wrong qualifier is `E-ENTRY-KIND-UNKNOWN`, and an unimported `EntryKind` is `E-UNIMPORTED`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryKindForm {
    /// No `kind:` argument (bare `#[Entry]`, or args without a `kind:`).
    Missing,
    /// `kind: Cli` — an injected variant written UNQUALIFIED.
    Bare(String),
    /// `kind: <qual>.<name>` — the qualified form; `qual` must be `EntryKind`.
    Qualified { qual: String, name: String },
    /// A `kind:` value that is neither a bare ident nor an `A.B` member (e.g. a literal).
    Malformed,
}

/// Read the surface form of an `#[Entry]`'s `kind:` argument (DEC-337). Structural read only. The
/// qualifier chain is flattened to a dotted string, so both the member-imported short form
/// (`EntryKind.Cli`, `qual = "EntryKind"`) and the self-gating fully-qualified form
/// (`Core.Runtime.EntryKind.Cli`, `qual = "Core.Runtime.EntryKind"`) are recognized — mirroring how
/// the `#[Entry]` attribute accepts both the bare-after-import and fully-qualified spellings.
pub fn entry_kind_form(attr: &Attribute) -> EntryKindForm {
    for a in &attr.args {
        if let Expr::NamedArg {
            name: key, value, ..
        } = a
        {
            if key == "kind" {
                return match value.as_ref() {
                    Expr::Ident(v, _) => EntryKindForm::Bare(v.clone()),
                    Expr::Member {
                        object,
                        name,
                        safe: false,
                        sep: crate::ast::MemberSep::Dot,
                        ..
                    } => match flatten_dotted_path(object) {
                        Some(qual) => EntryKindForm::Qualified {
                            qual,
                            name: name.clone(),
                        },
                        None => EntryKindForm::Malformed,
                    },
                    _ => EntryKindForm::Malformed,
                };
            }
        }
    }
    EntryKindForm::Missing
}

/// Flatten a pure `Ident`/`Ident.Ident.…` member chain to a dotted string (`Core.Runtime.EntryKind`),
/// or `None` if any node is not a plain dotted member access. Used to read the `kind:` qualifier.
fn flatten_dotted_path(e: &Expr) -> Option<String> {
    // Walk the member chain ITERATIVELY, never recursively. `#[Entry(kind:)]` args bypass BOTH
    // depth guards — attribute args are never routed through `check_expr` (so `MAX_EXPR_DEPTH`
    // never fires) and member access parses left-associatively (so the parser's `MAX_NEST_DEPTH`
    // counts the whole chain as one). Keeping this flattening iterative removes ONE unbounded
    // per-segment recursion on a pathological `kind: a.a.a.…` chain (iterative accumulation is
    // O(depth) heap, never stack). It is NOT the only one, and does not by itself close the full
    // `phg check`/run abort: `enforce_injected::walk_expr` runs first in that pipeline and recurses
    // the same chain guard-free, so a truly pathological input still overflows there — the
    // pre-existing, GENERAL deep-left-associative-chain hazard `src/limits.rs` documents and
    // `KNOWN_ISSUES.md` tracks (it hits ordinary deep member expressions too, not just entry kinds).
    let mut rev: Vec<&str> = Vec::new();
    let mut cur: &Expr = e;
    loop {
        match cur {
            Expr::Ident(n, _) => {
                rev.push(n.as_str());
                break;
            }
            Expr::Member {
                object,
                name,
                safe: false,
                sep: crate::ast::MemberSep::Dot,
                ..
            } => {
                rev.push(name.as_str());
                cur = object;
            }
            _ => return None,
        }
    }
    rev.reverse();
    Some(rev.join("."))
}

/// The fully-qualified spelling of the entry-kind enum (`Core.Runtime.EntryKind`) — the self-gating
/// qualifier that needs no import, parallel to the fully-qualified `#[Core.Runtime.Entry]` attribute.
pub const ENTRY_KIND_ENUM_FQ: &str = "Core.Runtime.EntryKind";

/// DEC-331 D1: the ROLE an `#[Entry]` function plays, DECLARED by its `kind:` (never inferred, never
/// name-magic). The signature must agree with the declared kind ([`entry_role`] validates the shape).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryRole {
    /// `(): void` / `(): int` / `(List<string>): void` / `(List<string>): int` — the `phg run`
    /// entry. An `int` return is the process exit status (0–255); `void` exits 0 on clean.
    Cli,
    /// `(Request): Response` — the `phg serve` per-request handler.
    Web,
}

/// Classify an `#[Entry]` function's signature into the role its SHAPE would carry, or `None` when
/// it matches neither. DEC-331 D1: this is no longer the role SOURCE — the declared `kind:` is
/// ([`entry_declared_role`]) — it survives only as the shape VALIDATOR the checker uses to confirm
/// the signature agrees with the declared kind (a `kind: Web` on a non-`(Request): Response`
/// function is `E-ENTRY-SIG`). AST-level shape matching, checker-independent.
pub fn entry_role(f: &FunctionDecl) -> Option<EntryRole> {
    fn named_is(t: &crate::ast::Type, want: &str) -> bool {
        matches!(t, crate::ast::Type::Named { name, args, .. } if name == want && args.is_empty())
    }
    let ret_cli = match &f.ret {
        None => true, // no annotation on an entry is not valid Phorj anyway; checker rejects
        Some(t) => named_is(t, "void") || named_is(t, "int"),
    };
    let params_cli = f.params.is_empty()
        || (f.params.len() == 1
            && matches!(&f.params[0].ty, crate::ast::Type::Named { name, args, .. }
                if name == "List" && args.len() == 1
                    && matches!(&args[0], crate::ast::Type::Named { name, args, .. }
                        if name == "string" && args.is_empty())));
    if params_cli && ret_cli {
        return Some(EntryRole::Cli);
    }
    let web = f.params.len() == 1
        && named_is(&f.params[0].ty, "Request")
        && f.ret.as_ref().is_some_and(|t| named_is(t, "Response"));
    if web {
        return Some(EntryRole::Web);
    }
    None
}

/// DEC-331 S3.3b: does `f`'s signature agree with an explicitly DECLARED `kind:`? This — not
/// [`entry_role`] — is the checker's `E-ENTRY-SIG` gate, because the two are no longer the same
/// question. `entry_role` asks *"what role would this shape imply?"*, which was the right question
/// only while DEC-191 inferred the role; S3.1 retired inference, so the question is now *"is this
/// shape legal FOR the role the author declared?"* — and one shape can be legal for two roles.
///
/// Concretely: under D5 a `Web` entry is `(): void`, whose body calls `Http.serve(cfg, handler)`
/// (the handler is a closure argument, not the entry itself). Config parameters never reach here —
/// the `desugar_config` PRE-check (`src/cli/pipeline.rs:130`) erases them before the checker runs —
/// so spec §1's `function web(Http.ServeConfig cfg, AppSettings app): void` arrives zero-arg.
/// `(): void` is therefore legal for BOTH roles, which is fine: the role came from `kind:`.
///
/// `Web` accepts ONLY `(): void` (DEC-455.12, S3.3d). The pre-DEC-331 shape — the entry ITSELF being
/// `(Request): Response` — was retired from the serve RUNTIME by S3.3c (`E-SERVE-NO-HANDLER` refuses
/// it with a migration message); S3.3d narrows the CHECKER so it never type-checks at all, which is
/// the diagnostic a migrating user should actually hit. The narrowing waited for S3.3d because it
/// and the example migration must land together: narrowing alone would fail `phg check` on the
/// shipped `examples/web/*` and redden the byte-identity glob for a reason unrelated to what it
/// gates.
///
/// **The narrowing is HERE and must never be pushed down into [`entry_role`].** `desugar_config`
/// (`src/checker/desugar_config/mod.rs:250`) skips config param-erasure whenever
/// `entry_role(f).is_some()`. If `entry_role` stopped calling `(Request): Response` a `Web` shape,
/// that erasure would fire on it and the user would get an opaque arity complaint instead of
/// `E-ENTRY-SIG`. `entry_role` answers "what shape is this?"; this function answers "is that shape
/// legal for the DECLARED kind?" — only the second question changed.
///
/// `Cli` is deliberately NOT widened — it must never accept `(Request): Response`.
#[must_use]
pub fn entry_shape_matches(f: &FunctionDecl, declared: EntryRole) -> bool {
    match declared {
        EntryRole::Cli => entry_role(f) == Some(EntryRole::Cli),
        // `(): void` — the D5 serve shape. Read off `entry_role` deliberately NARROWED: a `Cli`
        // shape also covers `(): int` and `(List<string>): …`, and neither is a web entry.
        EntryRole::Web => {
            f.params.is_empty()
                && f.ret.as_ref().is_some_and(|t| {
                    matches!(t, crate::ast::Type::Named { name, args, .. }
                        if name == "void" && args.is_empty())
                })
        }
    }
}

/// DEC-331 D1: the reserved-but-unbuilt `#[Entry(kind: …)]` names — recognized by the checker (so
/// `kind: Desktop` is a clear "reserved kind" error, never "unknown"), none built yet.
pub const RESERVED_ENTRY_KINDS: [&str; 4] = ["Desktop", "Mobile", "Worker", "Embedded"];

/// DEC-331 D1: the outcome of reading an `#[Entry]`'s declared `kind:`. Drives the checker's coded
/// errors (`E-ENTRY-KIND-REQUIRED` / `-UNKNOWN` / `-RESERVED`) and — for the `Active` arm — every
/// backend's entry selection via [`entry_declared_role`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntryKind {
    /// No `kind:` argument (bare `#[Entry]`, or args without a `kind:`) — `E-ENTRY-KIND-REQUIRED`.
    Missing,
    /// `kind: Foo` where `Foo` is not a recognized kind name — `E-ENTRY-KIND-UNKNOWN`.
    Unknown(String),
    /// A recognized-but-unbuilt kind (Desktop/Mobile/Worker/Embedded) — `E-ENTRY-KIND-RESERVED`.
    Reserved(String),
    /// An active kind — `Cli`/`Web`. The role every backend selects on.
    Active(EntryRole),
}

/// Classify an `#[Entry]` attribute's declared `kind:` by its variant NAME (DEC-331 D1 / DEC-337).
/// The kind is written `kind: EntryKind.<Variant>` (qualified injected enum); the variant name is
/// read from either the qualified or a bare form so backends resolve the role regardless of surface
/// spelling — the checker separately enforces the qualified+imported form ([`entry_kind_form`]).
/// A missing/misshapen `kind:` resolves to [`EntryKind::Missing`] (→ `E-ENTRY-KIND-REQUIRED`).
/// Structural read only — attribute args are never type-checked.
pub fn parse_entry_kind(attr: &Attribute) -> EntryKind {
    let name = match entry_kind_form(attr) {
        EntryKindForm::Missing | EntryKindForm::Malformed => return EntryKind::Missing,
        EntryKindForm::Bare(n) => n,
        EntryKindForm::Qualified { name, .. } => name,
    };
    match name.as_str() {
        "Cli" => EntryKind::Active(EntryRole::Cli),
        "Web" => EntryKind::Active(EntryRole::Web),
        n if RESERVED_ENTRY_KINDS.contains(&n) => EntryKind::Reserved(n.to_string()),
        n => EntryKind::Unknown(n.to_string()),
    }
}

/// DEC-331 D1: the active role declared by a function's `#[Entry(kind: …)]`, or `None` when it
/// carries no entry attribute or a non-active kind. The backend-selection SSOT — [`entry_for`]
/// resolves on it. By the time any backend runs, the checker has proven the kind is `Active`.
pub fn entry_declared_role(f: &FunctionDecl) -> Option<EntryRole> {
    let attr = f.attrs.iter().find(|a| is_entry_attr(a))?;
    match parse_entry_kind(attr) {
        EntryKind::Active(r) => Some(r),
        _ => None,
    }
}

/// Every `#[Entry]`-attributed function in the program — top-level functions and class STATIC
/// methods (an attributed instance method is invalid; the checker rejects it, and this resolver
/// simply does not surface it). Returns `(class, decl)` pairs in declaration order; role
/// classification and the one-per-kind rule (`E-DUPLICATE-ENTRY-KIND`) live in the checker.
pub fn entry_candidates(program: &Program) -> Vec<(Option<&str>, &FunctionDecl)> {
    let mut out = Vec::new();
    for item in &program.items {
        match item {
            Item::Function(f) if f.attrs.iter().any(is_entry_attr) => out.push((None, f)),
            Item::Class(c) => {
                for m in &c.members {
                    if let ClassMember::Method(f) = m {
                        if f.attrs.iter().any(is_entry_attr)
                            && f.modifiers.contains(&Modifier::Static)
                        {
                            out.push((Some(c.name.as_str()), f));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// DEC-331 D1: resolve the program's entry for one role — what the backends call. Selection is by
/// the DECLARED kind ([`entry_declared_role`]). `None` when the program declares no entry of that
/// role (a library file, or `phg run` on a web-only program).
pub fn entry_for(program: &Program, role: EntryRole) -> Option<(Option<&str>, &FunctionDecl)> {
    entry_candidates(program)
        .into_iter()
        .find(|(_, f)| entry_declared_role(f) == Some(role))
}

// NOTE — the NAME-based entry resolver (`entry_point(program, "main")` /
// `entry_point_count`) was DELETED 2026-07-29. It resolved an entry by the magic names
// `main`/`handle`, which the `#[Entry(kind:)]` migration retired (DEC-331/DEC-337): the developer's
// rule is that **the name means nothing — a free function or a static method needs `#[Entry(..)]`
// to be an entry at all**. Both functions had ZERO callers, and their doc-comments were the source
// of a false claim repeated in three backends (*"the checker's `E-MULTIPLE-MAIN` guarantees ≤1"*) —
// `E-MULTIPLE-MAIN` has no emit site and is not the rule. The live rule is at most one entry PER
// KIND (`E-DUPLICATE-ENTRY-KIND`, `src/checker/program/entry_points.rs`); one `Cli` + one `Web` may
// coexist, which five shipped `examples/web/*` rely on. Use [`entry_candidates`] / [`entry_for`].
