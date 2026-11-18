//! B1b — `parent.constructor(…)` forwarding, lowered by *front-end inlining* before any backend runs.
//!
//! A subclass with its own constructor does not auto-run the parent's (matching PHP, which never
//! auto-chains `parent::__construct`). `parent.constructor(args)` forwards explicitly, running the
//! parent constructor's effect on the **existing** instance. The VM's synthetic `<Class>::new`
//! allocates a *new* instance, so the parent ctor cannot simply be *called*; instead this pass splices
//! the parent constructor's effect — parameter bindings, promotions, field initializers, and body — as
//! a fresh-scoped `Stmt::Block` in place of the forwarding statement. Because the *same* lowered AST
//! feeds every backend (interpreter / VM / transpiler), `interp ≡ VM ≡ PHP` byte-identity holds by
//! construction; there is no new `Op` or `Value`. Same "front-end sugar, expanded out before backends"
//! discipline as `type` aliases / `html"…"` / erased generics. Runs LAST in `cli::check_and_expand`, so
//! the cloned parent body is already fully de-sugared.
//!
//! Scope: single inheritance (the checker rejects the multiple-inheritance forms with
//! `E-PARENT-CTOR-MI`); MI constructor forwarding lands with B2. `parent.constructor(…)` is
//! statement-only inside a constructor body (checker `E-PARENT-CTOR-STMT`/`-OUTSIDE`), so every
//! occurrence is reachable here and the backends never see a `ParentCall{method:"constructor"}`.

mod snap;

use crate::ast::{body_forwards, CatchClause, ClassMember, CtorParam, Expr, Item, Program, Stmt};
use crate::token::Span;
use snap::{between, find_owner, resolve_target, snapshot, Snap};

/// Inline every `parent.constructor(…)` forwarding statement (B1b). Returns the rewritten program.
pub fn inline_parent_ctors(mut program: Program) -> Program {
    // DEC-585: record which constructors forward BEFORE the calls are spliced away, so the backends'
    // prelude (`ast::field_initializers`) still knows which ancestors the call initializes.
    for item in &mut program.items {
        if let Item::Class(c) = item {
            c.ctor_forwards = c
                .members
                .iter()
                .any(|m| matches!(m, ClassMember::Constructor { body, .. } if body_forwards(body)));
        }
    }
    let snap = snapshot(&program);
    for item in &mut program.items {
        if let Item::Class(c) = item {
            let lexical = c.name.clone();
            for m in &mut c.members {
                if let ClassMember::Constructor { body, .. } = m {
                    inline_stmts(body, &lexical, &snap);
                }
            }
        }
    }
    program
}

/// Rewrite every statement-position `parent.constructor(…)` within `stmts` (and nested bodies). The
/// `lexical` class is the one whose constructor body these statements belong to — it determines which
/// parent `parent.constructor` resolves to.
fn inline_stmts(stmts: &mut [Stmt], lexical: &str, snap: &Snap) {
    for s in stmts.iter_mut() {
        inline_stmt(s, lexical, snap);
    }
}

fn inline_stmt(s: &mut Stmt, lexical: &str, snap: &Snap) {
    match s {
        Stmt::Expr(e, span) | Stmt::Discard(e, span) => {
            if let Expr::ParentCall {
                method,
                ancestor,
                args,
                ..
            } = e
            {
                if method == "constructor" {
                    // Clone the parts before dropping the borrow of `e`, then replace the statement.
                    let anc = ancestor.clone();
                    let args = args.clone();
                    let sp = *span;
                    *s = build_inline(lexical, anc.as_deref(), &args, snap, sp);
                }
            }
        }
        Stmt::Block(b, _) => inline_stmts(b, lexical, snap),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            inline_stmts(then_block, lexical, snap);
            if let Some(eb) = else_block {
                inline_stmts(eb, lexical, snap);
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Using { body, .. } => {
            inline_stmts(body, lexical, snap)
        }
        Stmt::CFor {
            init, step, body, ..
        } => {
            if let Some(i) = init {
                inline_stmt(i, lexical, snap);
            }
            if let Some(st) = step {
                inline_stmt(st, lexical, snap);
            }
            inline_stmts(body, lexical, snap);
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            inline_stmts(body, lexical, snap);
            for CatchClause { body, .. } in catches.iter_mut() {
                inline_stmts(body, lexical, snap);
            }
            if let Some(fb) = finally_block {
                inline_stmts(fb, lexical, snap);
            }
        }
        Stmt::Destructure {
            else_block: Some(eb),
            ..
        } => inline_stmts(eb, lexical, snap),
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Return { .. }
        | Stmt::Throw { .. }
        | Stmt::Destructure {
            else_block: None, ..
        }
        | crate::stmt_leaves!() => {}
    }
}

/// Build the inlined `Stmt::Block` for `parent.constructor(args)` written in `lexical`. Resolves the
/// target parent (direct parent, or the named `ancestor`), finds the nearest class up the chain that
/// declares a constructor (PHP's inherited `__construct`), and splices its effect onto `this`. Returns
/// an empty block when the chain has no constructor (a no-op forward, checker-validated to take no
/// arguments).
fn build_inline(
    lexical: &str,
    ancestor: Option<&str>,
    args: &[Expr],
    snap: &Snap,
    span: Span,
) -> Stmt {
    let empty = Stmt::Block(Vec::new(), span);
    let Some(target) = resolve_target(lexical, ancestor, snap) else {
        return empty;
    };
    let Some(owner) = find_owner(&target, snap) else {
        return empty;
    };
    let owner_snap = &snap[&owner];
    let (params, body) = owner_snap
        .own_ctor
        .clone()
        .expect("find_owner picked an own ctor");

    let mut out: Vec<Stmt> = Vec::new();
    // ① bind each parent parameter to its argument (a let-init reads the *outer* scope, so same-name
    //    forwarding `parent.constructor(x)` with parent param `x` is correct).
    for (p, arg) in params.iter().zip(args) {
        out.push(Stmt::VarDecl {
            ty: p.ty.clone(),
            name: p.name.clone(),
            init: arg.clone(),
            mutable: false,
            span,
        });
    }
    // ② promote each visibility-modified parameter into its field.
    for p in &params {
        if is_promoted(p) {
            out.push(assign_this(
                &p.name,
                Expr::Ident(p.name.clone(), span),
                span,
            ));
        }
    }
    // ③ initialize the forwarded ancestors (DEC-585): the target's chain, base-first, stopping at a
    //    class whose own constructor forwards (its inlined body initializes what lies above it); then
    //    any class between the target and `lexical` (a named-ancestor forward skips their constructors,
    //    never their initializers: DEC-575).
    for (fname, init) in snap[&target].chain_inits.iter().chain(
        between(lexical, &target, snap)
            .iter()
            .flat_map(|c| &snap[c].field_inits),
    ) {
        out.push(assign_this(fname, init.clone(), span));
    }
    // ④ the owner's constructor body, with its own `parent.constructor(…)` (the grandparent) inlined
    //    against the owner as the new lexical class.
    let mut owner_body = body;
    inline_stmts(&mut owner_body, &owner, snap);
    out.extend(owner_body);

    Stmt::Block(out, span)
}

/// A ctor param is promoted to a field iff it carries a visibility modifier (mirrors `construct.rs`).
fn is_promoted(p: &CtorParam) -> bool {
    p.modifiers.iter().any(|m| m.is_member_visibility())
}

/// `this.<name> = <value>;`
fn assign_this(name: &str, value: Expr, span: Span) -> Stmt {
    Stmt::Assign {
        target: Expr::Member {
            object: Box::new(Expr::This(span)),
            name: name.to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span,
        },
        value,
        span,
    }
}
