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
    // counts the whole chain as one). A recursive walk therefore overflowed the native stack on a
    // pathological `kind: a.a.a.…` chain (reproduced: `phg check` aborted at ~200k segments) —
    // exactly the left-associative-chain hazard `src/limits.rs` documents. Iterative accumulation
    // is O(depth) heap, never stack.
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

/// Resolve a program **entry point** (`main` / `handle`) — the single source of truth all backends
/// share so they invoke the same function (Batch-1 D, `docs/specs/2026-06-27-class-entry-points-design.md`).
///
/// An entry is **either** a top-level free function named `name` (returns `Some((None, decl))`) **or**
/// a `static` method named `name` on some class (`Some((Some(class), decl))`). An *instance* method
/// named `name` is **not** an entry (an ordinary method). Top-level wins the scan order, but a valid
/// program has at most one entry — [`entry_point_count`] backs the checker's `E-MULTIPLE-MAIN`, so by
/// the time any backend calls this the entry is unambiguous.
pub fn entry_point<'a>(
    program: &'a Program,
    name: &str,
) -> Option<(Option<&'a str>, &'a FunctionDecl)> {
    for item in &program.items {
        if let Item::Function(f) = item {
            if f.name == name {
                return Some((None, f));
            }
        }
    }
    for item in &program.items {
        if let Item::Class(c) = item {
            for m in &c.members {
                if let ClassMember::Method(f) = m {
                    if f.name == name && f.modifiers.contains(&Modifier::Static) {
                        return Some((Some(c.name.as_str()), f));
                    }
                }
            }
        }
    }
    None
}

/// How many distinct entry points named `name` a program declares (a top-level function plus every
/// class-static method of that name). `> 1` is the checker's `E-MULTIPLE-MAIN` — an ambiguous entry is
/// an error, never a silent pick.
pub fn entry_point_count(program: &Program, name: &str) -> usize {
    let mut n = 0;
    for item in &program.items {
        match item {
            Item::Function(f) if f.name == name => n += 1,
            Item::Class(c) => {
                for m in &c.members {
                    if let ClassMember::Method(f) = m {
                        if f.name == name && f.modifiers.contains(&Modifier::Static) {
                            n += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    n
}
