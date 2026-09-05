//! DEC-397 — the function-scope hoist: PHP has FUNCTION scope, phorj has BLOCK scope.
//!
//! `src/lift/lifter/decls/statements.rs` emits a `VarDecl` at the site of a variable's first
//! assignment, so a variable first assigned inside a block is *declared* inside that block. The
//! reproducer, and the two errors it produced:
//!
//! ```php
//! function f(): int { if (true) { $b = 5; } $b = 7; return $b; }
//! ```
//! → `mutable var b = 5;` INSIDE the `if`, then `b = 7;` outside → `E-ASSIGN-UNKNOWN` +
//! `E-UNKNOWN-IDENT`.
//!
//! # Why this is much narrower than the ruled shape
//!
//! The ruling was "hoist the first assignment when its value is a literal". Measured against real PHP,
//! that is **unsound whenever the enclosing block is CONDITIONAL** — which is the common case:
//!
//! ```php
//! function f(bool $c): int { if ($c) { $b = 5; } return $b + 0; }
//! ```
//! `f(false)` prints **0** in PHP (reading an unassigned `$b` yields null + a warning, and `null + 0`
//! is `0`). A hoisted `mutable var b = 5;` makes it print **5**. [Verified against php-8.5.8:
//! `0|5`.] Worse, the hoist would make the file *compile* and be *wrong* — trading a loud
//! `E-UNKNOWN-IDENT` for a silent divergence that `tests/lift_roundtrip.rs` is built to catch.
//!
//! Reproducing PHP faithfully would need `T? b = null` plus an unwrap at every read, and the lifter
//! cannot infer `T` from untyped PHP locals (`mutable var b = null` is `E-INFER-NULL`). So the hoist is
//! restricted to blocks that **always execute**, and every other case is refused with a reason rather
//! than guessed (DEC-166).
//!
//! Always-executing, for this purpose: the function body itself, a bare `{ … }` block, and
//! `if (true) { … }` with no `elseif`/`else` — a literal-true condition, which is exactly the
//! reproducer's shape. `while`/`for`/`foreach`/`try`/`catch`/`finally` and any non-literal `if` are
//! CONDITIONAL: `while` may run zero times, a `try` body may throw part-way through.

use super::super::super::ast as php;
use std::collections::HashMap;

/// What the pre-scan decided for one function body.
pub(in crate::lift) struct HoistPlan {
    /// `(name, literal_initializer)` to declare at the top of the lifted body, in source order.
    pub(in crate::lift) hoists: Vec<(String, php::PhpExpr)>,
    /// Names that NEED hoisting for the output to compile but cannot be hoisted soundly, in source
    /// order. Each gets a `// CANNOT LIFT:` note — the draft still will not compile, which is
    /// in-contract for `phg lift`, but the reason is stated instead of left to `phg check`.
    pub(in crate::lift) blocked: Vec<String>,
}

/// One recorded sighting of a variable, in source order.
struct Sighting {
    /// A write whose right-hand side is a literal (the only kind that can be hoisted).
    literal_write: Option<php::PhpExpr>,
    /// True when this sighting is a write of any shape.
    is_write: bool,
    /// The chain of enclosing block ids. Empty = the function body's own top level.
    path: Vec<usize>,
    /// True when every block on `path` always executes.
    unconditional: bool,
}

/// Plan the hoists for `body`, given the names already bound as parameters.
///
/// A parameter is EXCLUDED: `declared` is already seeded with param names, so
/// `function f(int $b) { if (true) { $b = 5; } return $b; }` lifts correctly today, and hoisting it
/// would emit a second declaration — `E-SHADOW-LOCAL`, the very error DEC-397 says the lifter must not
/// produce.
pub(in crate::lift) fn plan(body: &[php::PhpStmt], params: &[String]) -> HoistPlan {
    let mut sightings: HashMap<String, Vec<Sighting>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut ctx = Ctx {
        path: Vec::new(),
        unconditional: true,
        next_block: 0,
        bound: params.to_vec(),
    };
    walk_block(body, &mut ctx, &mut sightings, &mut order);

    let mut hoists = Vec::new();
    let mut blocked = Vec::new();
    for name in order {
        if params.contains(&name) {
            continue;
        }
        let Some(seen) = sightings.get(&name) else {
            continue;
        };
        let Some(first) = seen.first() else { continue };
        // Only a variable whose FIRST sighting is a write needs anything: if a READ comes first, PHP is
        // reading an unassigned variable and no phorj shape reproduces that.
        if !first.is_write || first.path.is_empty() {
            continue;
        }
        // Nothing to fix unless it is also used OUTSIDE its first-assignment block — a block-local
        // variable already lifts correctly, and hoisting it would add a spurious declaration to output
        // that works today.
        let used_outside = seen
            .iter()
            .any(|s| !s.path.starts_with(first.path.as_slice()));
        if !used_outside {
            continue;
        }
        match (&first.literal_write, first.unconditional) {
            (Some(lit), true) => hoists.push((name, lit.clone())),
            _ => blocked.push(name),
        }
    }
    HoistPlan { hoists, blocked }
}

struct Ctx {
    path: Vec<usize>,
    unconditional: bool,
    next_block: usize,
    /// Names bound by a construct rather than an assignment (params, `foreach` bindings, `catch`
    /// vars). Never hoisted: the binding already declares them.
    bound: Vec<String>,
}

impl Ctx {
    /// Record a sighting at the current position.
    fn see(
        &self,
        name: &str,
        literal_write: Option<php::PhpExpr>,
        is_write: bool,
        sightings: &mut HashMap<String, Vec<Sighting>>,
        order: &mut Vec<String>,
    ) {
        if self.bound.iter().any(|b| b == name) {
            return;
        }
        let entry = sightings.entry(name.to_string()).or_default();
        if entry.is_empty() {
            order.push(name.to_string());
        }
        entry.push(Sighting {
            literal_write,
            is_write,
            path: self.path.clone(),
            unconditional: self.unconditional,
        });
    }
}

/// Descend into a nested block, marking whether it always executes.
fn nested(ctx: &mut Ctx, always: bool, f: &mut impl FnMut(&mut Ctx)) {
    let id = ctx.next_block;
    ctx.next_block += 1;
    let outer_uncond = ctx.unconditional;
    ctx.path.push(id);
    ctx.unconditional = outer_uncond && always;
    f(ctx);
    ctx.path.pop();
    ctx.unconditional = outer_uncond;
}

/// True for a condition phorj can prove always holds — a literal `true`. Anything else is treated as
/// conditional, deliberately: guessing wrong here is the unsound direction.
fn is_literal_true(cond: &php::PhpExpr) -> bool {
    matches!(cond, php::PhpExpr::Bool(true))
}

/// A right-hand side that can be re-evaluated at the top of the function with no observable
/// difference: a plain literal. Anything else (a call, `new`, a concat, another variable) either has
/// side effects or depends on state that does not exist yet at the hoist point.
fn literal_rhs(e: &php::PhpExpr) -> Option<php::PhpExpr> {
    matches!(
        e,
        php::PhpExpr::Int(_)
            | php::PhpExpr::Float(_)
            | php::PhpExpr::Str(_)
            | php::PhpExpr::Bool(_)
    )
    .then(|| e.clone())
}

fn walk_block(
    stmts: &[php::PhpStmt],
    ctx: &mut Ctx,
    sightings: &mut HashMap<String, Vec<Sighting>>,
    order: &mut Vec<String>,
) {
    for s in stmts {
        walk_stmt(s, ctx, sightings, order);
    }
}

fn walk_stmt(
    s: &php::PhpStmt,
    ctx: &mut Ctx,
    sightings: &mut HashMap<String, Vec<Sighting>>,
    order: &mut Vec<String>,
) {
    use php::PhpStmt as S;
    match s {
        S::Return(Some(e)) | S::Throw(e) | S::Expr(e) => walk_expr(e, ctx, sightings, order),
        S::Return(None) | S::Break | S::Continue => {}
        S::Echo(es) => {
            for e in es {
                walk_expr(e, ctx, sightings, order);
            }
        }
        S::Block(b) => nested(ctx, true, &mut |c| walk_block(b, c, sightings, order)),
        S::If {
            cond,
            then,
            elifs,
            els,
        } => {
            walk_expr(cond, ctx, sightings, order);
            // A literal-true `if` with no other arm is the one conditional shape that always runs.
            let always = is_literal_true(cond) && elifs.is_empty() && els.is_none();
            nested(ctx, always, &mut |c| walk_block(then, c, sightings, order));
            for (c_expr, body) in elifs {
                walk_expr(c_expr, ctx, sightings, order);
                nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
            }
            if let Some(body) = els {
                nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
            }
        }
        S::While { cond, body } => {
            walk_expr(cond, ctx, sightings, order);
            nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
        }
        S::For {
            init,
            cond,
            step,
            body,
        } => {
            for e in [init, cond, step].into_iter().flatten() {
                walk_expr(e, ctx, sightings, order);
            }
            nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
        }
        S::Foreach {
            array,
            key,
            value,
            body,
        } => {
            walk_expr(array, ctx, sightings, order);
            // The loop bindings are declared BY the `foreach`, so they must never be hoisted.
            let restore = ctx.bound.clone();
            ctx.bound.push(value.clone());
            if let Some(k) = key {
                ctx.bound.push(k.clone());
            }
            nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
            ctx.bound = restore;
        }
        S::Try {
            body,
            catches,
            finally_block,
        } => {
            // A `try` body can throw part-way through, so nothing inside it is unconditional.
            nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
            for cat in catches {
                let restore = ctx.bound.clone();
                if let Some(v) = &cat.var {
                    ctx.bound.push(v.clone());
                }
                nested(ctx, false, &mut |c| {
                    walk_block(&cat.body, c, sightings, order)
                });
                ctx.bound = restore;
            }
            if let Some(body) = finally_block {
                nested(ctx, false, &mut |c| walk_block(body, c, sightings, order));
            }
        }
    }
}

fn walk_expr(
    e: &php::PhpExpr,
    ctx: &mut Ctx,
    sightings: &mut HashMap<String, Vec<Sighting>>,
    order: &mut Vec<String>,
) {
    use php::PhpExpr as E;
    match e {
        // A plain `$x = <rhs>` is the only WRITE shape that can seed a declaration. The rhs is walked
        // FIRST so `$b = $b + 1` records the read before the write (source order within the statement).
        E::Assign { target, value } => {
            walk_expr(value, ctx, sightings, order);
            if let E::Var(name) = target.as_ref() {
                ctx.see(name, literal_rhs(value), true, sightings, order);
            } else {
                walk_expr(target, ctx, sightings, order);
            }
        }
        // `$x += 1` / `$x++` READ before they write, so they can never be a variable's first sighting
        // in a way that makes hoisting safe — recorded as a write with no literal.
        E::CompoundAssign { target, value, .. } => {
            walk_expr(value, ctx, sightings, order);
            if let E::Var(name) = target.as_ref() {
                ctx.see(name, None, true, sightings, order);
            } else {
                walk_expr(target, ctx, sightings, order);
            }
        }
        E::IncDec { target, .. } => {
            if let E::Var(name) = target.as_ref() {
                ctx.see(name, None, true, sightings, order);
            } else {
                walk_expr(target, ctx, sightings, order);
            }
        }
        E::Var(name) => ctx.see(name, None, false, sightings, order),
        // A named argument is a wrapper: its VALUE is an ordinary expression and can read a variable
        // (`f(limit: $n)`), so the walk must descend. Named args only reach a function BODY once the
        // expression parser admits them (attribute arg lists are outside every body) — recursing now
        // means that slice cannot silently skip a sighting and mis-hoist.
        E::NamedArg { value, .. } => walk_expr(value, ctx, sightings, order),
        // An arrow closure captures by value: its body READS enclosing locals (walk them) and its
        // parameters are its own (never a hoist candidate for the enclosing function).
        E::Closure { body, .. } => walk_expr(body, ctx, sightings, order),
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null | E::Name(_) => {}
        E::Interp(parts) => {
            for p in parts {
                if let php::PhpStrPart::Expr(inner) = p {
                    walk_expr(inner, ctx, sightings, order);
                }
            }
        }
        E::Array(elems) => {
            for el in elems {
                if let Some(k) = &el.key {
                    walk_expr(k, ctx, sightings, order);
                }
                walk_expr(&el.value, ctx, sightings, order);
            }
        }
        E::Unary { expr, .. } => walk_expr(expr, ctx, sightings, order),
        E::Binary { left, right, .. } => {
            walk_expr(left, ctx, sightings, order);
            walk_expr(right, ctx, sightings, order);
        }
        E::InstanceOf { value, .. } => walk_expr(value, ctx, sightings, order),
        E::Ternary { cond, then, els } => {
            walk_expr(cond, ctx, sightings, order);
            if let Some(t) = then {
                walk_expr(t, ctx, sightings, order);
            }
            walk_expr(els, ctx, sightings, order);
        }
        E::Call { args, .. } => {
            for a in args {
                walk_expr(a, ctx, sightings, order);
            }
        }
        E::New { args, .. } => {
            for a in args {
                walk_expr(a, ctx, sightings, order);
            }
        }
        E::MethodCall { recv, args, .. } => {
            walk_expr(recv, ctx, sightings, order);
            for a in args {
                walk_expr(a, ctx, sightings, order);
            }
        }
        E::Member { recv, .. } => walk_expr(recv, ctx, sightings, order),
        E::StaticCall { args, .. } => {
            for a in args {
                walk_expr(a, ctx, sightings, order);
            }
        }
        E::AppendSlot(inner) | E::Cast { value: inner, .. } => {
            walk_expr(inner, ctx, sightings, order);
        }
        E::Index { base, index } => {
            walk_expr(base, ctx, sightings, order);
            walk_expr(index, ctx, sightings, order);
        }
        E::Match { subject, arms } => {
            walk_expr(subject, ctx, sightings, order);
            for arm in arms {
                for c in arm.conds.iter().flatten() {
                    walk_expr(c, ctx, sightings, order);
                }
                walk_expr(&arm.body, ctx, sightings, order);
            }
        }
        E::ClassConst { .. } | E::StaticProp { .. } => {}
    }
}
