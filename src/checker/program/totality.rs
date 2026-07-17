//! Program pass — control-flow totality: termination, divergence, definite field assignment,
//! return-path coverage, narrowing guards.

use super::*;

impl Checker {
    /// A block diverges iff *any* of its statements diverges (everything after a diverging statement
    /// is dead, so the block as a whole never falls through).
    pub(in crate::checker) fn block_terminates(&self, stmts: &[crate::ast::Stmt]) -> bool {
        stmts.iter().any(|s| self.stmt_terminates(s))
    }

    pub(in crate::checker) fn stmt_terminates(&self, s: &crate::ast::Stmt) -> bool {
        use crate::ast::Stmt;
        match s {
            Stmt::Return { .. } => true,
            Stmt::Block(b, _) => self.block_terminates(b),
            Stmt::If {
                then_block,
                else_block: Some(eb),
                ..
            } => self.block_terminates(then_block) && self.block_terminates(eb),
            // An `if` with no `else` always has a path (the false branch) that falls through.
            Stmt::If {
                else_block: None, ..
            } => false,
            // A condition loop diverges only when it cannot exit: an always-true condition with no
            // `break` bound to it. A `do { … } while (…)` additionally diverges when its body diverges
            // on the guaranteed first iteration. A plain `while`/`for` body may run zero times, so a
            // diverging body alone does NOT make the loop diverge.
            Stmt::While {
                cond,
                body,
                post_cond,
                ..
            } => {
                (*post_cond && self.block_terminates(body))
                    || (is_true_lit(cond) && !breaks_this_loop(body))
            }
            Stmt::CFor { cond, body, .. } => {
                let cond_always = match cond {
                    None => true,
                    Some(c) => is_true_lit(c),
                };
                cond_always && !breaks_this_loop(body)
            }
            // `for (T x in iter)` always terminates over a finite list — never a divergence source.
            Stmt::Expr(e, _) => self.expr_is_never(e),
            // A `throw` always diverges (it unwinds out of the current frame; M-faults 2b).
            Stmt::Throw { .. } => true,
            // A `try` diverges iff control can never leave it normally: a `finally` that itself
            // diverges forces divergence; otherwise every exit edge must diverge — the body AND
            // every catch body. (An uncaught throw out of the body also diverges, so requiring the
            // body to terminate is sound — if it falls through normally, so does the `try`.)
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                if finally_block
                    .as_ref()
                    .is_some_and(|fb| self.block_terminates(fb))
                {
                    return true;
                }
                self.block_terminates(body)
                    && catches.iter().all(|c| self.block_terminates(&c.body))
            }
            _ => false,
        }
    }

    /// Definite assignment (Soundness Batch D): does **every completing path** through `stmts` assign
    /// `this.field` before the path ends? A path that *diverges* (throw/panic/infinite loop — not a
    /// plain `return`, which completes construction) is vacuously fine; an early `return` before the
    /// assignment is NOT (the object is constructed with the field unset). Conservative and sound: any
    /// `return` reached before the assignment fails the check.
    pub(in crate::checker) fn block_assigns_field(
        &self,
        stmts: &[crate::ast::Stmt],
        field: &str,
    ) -> bool {
        for s in stmts {
            if self.stmt_assigns_field(s, field) {
                return true;
            }
            if self.stmt_diverges_no_return(s) {
                return true; // no completing path continues past here
            }
            if stmt_has_return(s) {
                return false; // an early return completes construction without the assignment
            }
        }
        false
    }

    /// Whether *every completing path* through a single statement assigns `this.field`.
    pub(in crate::checker) fn stmt_assigns_field(&self, s: &crate::ast::Stmt, field: &str) -> bool {
        use crate::ast::Stmt;
        match s {
            Stmt::Assign { target, .. } => is_this_field(target, field),
            Stmt::Block(b, _) => self.block_assigns_field(b, field),
            Stmt::If {
                then_block,
                else_block: Some(eb),
                ..
            } => self.block_assigns_field(then_block, field) && self.block_assigns_field(eb, field),
            // An `if` with no else, or a loop body (may run zero times), does not assign on all paths.
            _ => false,
        }
    }

    /// Whether a statement *always* diverges without returning (throw / `never` expr / infinite loop /
    /// a block or both-branch `if` that does). The `return`-excluding dual of [`Self::stmt_terminates`]
    /// — for definite assignment a `return` is a *completing* path, not a saving divergence.
    pub(in crate::checker) fn stmt_diverges_no_return(&self, s: &crate::ast::Stmt) -> bool {
        use crate::ast::Stmt;
        match s {
            Stmt::Throw { .. } => true,
            Stmt::Expr(e, _) => self.expr_is_never(e),
            Stmt::Block(b, _) => b.iter().any(|x| self.stmt_diverges_no_return(x)),
            Stmt::If {
                then_block,
                else_block: Some(eb),
                ..
            } => {
                then_block.iter().any(|x| self.stmt_diverges_no_return(x))
                    && eb.iter().any(|x| self.stmt_diverges_no_return(x))
            }
            Stmt::While { cond, body, .. } => is_true_lit(cond) && !breaks_this_loop(body),
            Stmt::CFor { cond, body, .. } => {
                let cond_always = match cond {
                    None => true,
                    Some(c) => is_true_lit(c),
                };
                cond_always && !breaks_this_loop(body)
            }
            _ => false,
        }
    }

    /// Whether an expression has the bottom type `never` — recognised *without* re-checking (emits no
    /// diagnostics): a call to a free function whose signature returns `never`, or an `if`/`match`
    /// expression every arm of which is itself `never`. Method/closure `never`-returns are deferred
    /// (need receiver typing) — see the design's KNOWN_ISSUES.
    pub(in crate::checker) fn expr_is_never(&self, e: &crate::ast::Expr) -> bool {
        use crate::ast::Expr;
        match e {
            Expr::Call { callee, .. } => {
                match &**callee {
                    Expr::Ident(name, _) => {
                        // A `never`-typed fault intrinsic (`panic`/`todo`/`unreachable` — not
                        // `assert`, which is `unit`), or a user function declared `-> never`
                        // (M-faults 2a).
                        matches!(name.as_str(), "panic" | "todo" | "unreachable")
                            || self
                                .funcs
                                .get(name)
                                .and_then(|v| v.first())
                                .is_some_and(|s| s.ret == Ty::Never)
                    }
                    // DEC-238: a QUALIFIED `never` call diverges too — a never-returning NATIVE
                    // (`Runtime.exit(1)`) or a `never` STATIC method (`DatabaseError.fail(e)`). Same
                    // conservative direction: only a provable `Ty::Never` return counts.
                    Expr::Member {
                        object,
                        name,
                        safe: false,
                        ..
                    } => {
                        if let Expr::Ident(q, _) = &**object {
                            let native_never = self
                                .imports
                                .get(q)
                                .and_then(|m| crate::native::index_of(m, name))
                                .is_some_and(|idx| crate::native::registry()[idx].ret == Ty::Never);
                            let static_never = self.classes.get(q).is_some_and(|info| {
                                info.methods
                                    .get(name)
                                    .and_then(|v| v.first())
                                    .is_some_and(|s| s.ret == Ty::Never)
                            });
                            native_never || static_never
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            Expr::If {
                then_expr,
                else_expr,
                ..
            } => self.expr_is_never(then_expr) && self.expr_is_never(else_expr),
            Expr::Match { arms, .. } => {
                !arms.is_empty() && arms.iter().all(|a| self.expr_is_never(&a.body))
            }
            _ => false,
        }
    }

    /// Return-on-all-paths gate (M-RT totality cluster). A function whose declared return type carries
    /// a value must return (or diverge) on every path; `never` is the inverse — it must provably
    /// diverge. `void` (the no-annotation default), `empty`, and `<error>` (poison) are exempt.
    pub(in crate::checker) fn check_return_totality(
        &mut self,
        ret: &Ty,
        body: &[crate::ast::Stmt],
        span: Span,
    ) {
        match ret {
            // `void` (the common nothing, incl. the no-annotation default) and `empty` (the holdable
            // nothing) are both value-less: a function may fall off the end (it implicitly produces
            // the empty value), so neither requires a `return` on all paths. `<error>` is poison.
            Ty::Void | Ty::Empty | Ty::Error => {}
            Ty::Never => {
                if !self.block_terminates(body) {
                    self.err_coded(
                        span,
                        "a `never` function must never return, but this body can fall through",
                        "E-NEVER-RETURN",
                        Some("a `-> never` function must diverge on every path (e.g. an infinite loop); drop the `never` return type if it can return normally".into()),
                    );
                }
            }
            _ => {
                if !self.block_terminates(body) {
                    self.err_coded(
                        span,
                        format!("function does not return `{ret}` on all paths"),
                        "E-MISSING-RETURN",
                        Some("add a `return` (or diverge) on every path — e.g. an `if` without an `else` leaves the false branch falling through".into()),
                    );
                }
            }
        }
    }

    /// Check a statement sequence in the *current* scope (no scope push), flagging the first
    /// unreachable statement after a diverging one (`W-UNREACHABLE`, once per dead region). Used for
    /// function, constructor, and `set`-hook bodies; `check_block` wraps it in a fresh scope for
    /// nested `{ … }` blocks.
    pub(in crate::checker) fn check_body(&mut self, stmts: &[crate::ast::Stmt]) {
        let mut dead = false;
        let mut warned = false;
        for s in stmts {
            if dead && !warned {
                self.warn_coded(
                    Self::stmt_span(s),
                    "unreachable code: control never reaches this statement",
                    "W-UNREACHABLE",
                    Some(
                        "a preceding statement always returns or diverges; remove the dead code"
                            .into(),
                    ),
                );
                warned = true;
            }
            self.check_stmt(s);
            // Early-return narrowing (S5.3-T3): a guard `if (cond) { <diverges> }` means `cond` is
            // FALSE for every statement after it in this block — so install the false-polarity
            // narrowings into the current scope (they persist to the block's end, then pop with it).
            // Sound regardless of an `else`: reaching past a diverging then-block implies `cond` false.
            for (name, ty) in self.guard_if_narrowings(s) {
                let m = self.lookup_binding(&name).map(|(_, m)| m).unwrap_or(false);
                self.declare_binding(&name, ty, m, Self::stmt_span(s));
            }
            if self.stmt_terminates(s) {
                dead = true;
            }
        }
    }

    /// The narrowings a *guard* statement imposes on the rest of its block: an `if (cond) { … }` (no
    /// if-let binding) whose then-block diverges (`return`/`throw`/…) leaves `cond` false on the
    /// fall-through path, so the rest of the block sees the `polarity = false` narrowing (S5.3-T3).
    /// empty for any other statement.
    pub(in crate::checker) fn guard_if_narrowings(
        &self,
        s: &crate::ast::Stmt,
    ) -> Vec<(String, Ty)> {
        use crate::ast::Stmt;
        if let Stmt::If {
            cond,
            bind: None,
            then_block,
            ..
        } = s
        {
            if self.block_terminates(then_block) {
                let mut n = self.narrow_from_condition(cond, false);
                // Lockstep bound (DEC-184): the VM compiler replicates only the DIRECT then-block
                // primitive narrowing, not an early-return TAIL. A UNION variable narrowed to a
                // discriminable primitive here (`if (!(x is int)) { return; }` ⇒ `x: int` for the
                // tail) would therefore be checker-accepts/VM-rejects. Drop those — both backends then
                // see the un-narrowed union in the tail and reject arithmetic on it identically. An
                // OPTIONAL narrowed to its inner is kept (the optional local already carries the inner
                // `CTy` on the VM), and a class narrowing is kept (member access resolves via the field
                // table, no local-`CTy` narrowing needed). General fix tracked as W2-12.
                n.retain(|(name, ty)| {
                    let prim = matches!(ty, Ty::Int | Ty::Float | Ty::String | Ty::Bool | Ty::Null);
                    let from_optional =
                        matches!(self.lookup_binding(name), Some((Ty::Optional(_), _)));
                    !prim || from_optional
                });
                return n;
            }
        }
        Vec::new()
    }

    pub(in crate::checker) fn check_block(&mut self, stmts: &[crate::ast::Stmt]) {
        self.push_scope();
        self.check_body(stmts);
        self.pop_scope();
    }
}
