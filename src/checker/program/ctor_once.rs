//! DEC-524 (scout row 5j) — the constructor-once rule's flow check, shape ruled 2026-09-25 "exactly
//! once + read check". `assign_field.rs` lets an immutable, uninitialized, unpromoted field of the
//! current class be assigned in its own constructor body; this pass walks that body in statement
//! order and refuses the two things the permission alone cannot see:
//!
//! - `E-ASSIGN-IMMUTABLE-TWICE` — an assignment to such a field that may be the SECOND on some path:
//!   the field was possibly assigned already, or the assignment sits in a loop body (which can run
//!   again). PHP `readonly` throws at that write and Java `final` rejects it.
//! - `E-FIELD-READ-BEFORE-INIT` — a read of `this.<field>` (immutable or `mutable`, non-optional, no
//!   initializer, not promoted) where the field is not yet assigned on every path. Natively that
//!   read faults `no field x`, under PHP "must not be accessed before initialization".
//!
//! Every path's state is `(assigned, maybe)`: definitely vs possibly assigned. A branch that returns,
//! throws, breaks or continues is dropped from the merge that follows it. Deliberately NOT tracked
//! (disclosed): a read through a method call (`this.helper()`), `this` escaping before the fields
//! are set, and reads inside a lambda body — a lambda reads when it is CALLED, not where it is written.

use super::super::common_flow::is_this_field;
use super::*;
use crate::ast::{Expr, Stmt};
use std::collections::BTreeSet;

type Fields = BTreeSet<String>;

/// One path's knowledge: definitely assigned, and possibly assigned.
#[derive(Clone, Default)]
struct Path {
    assigned: Fields,
    maybe: Fields,
}

/// The walk's inputs and its findings (span, code, message), reported by the caller.
struct Walk<'a> {
    once: &'a Fields,
    reads: &'a Fields,
    found: Vec<(Span, &'static str, String)>,
    /// Every field assigned anywhere in the walk so far, on ANY path — including paths that then
    /// diverge. A `catch` or the code after a loop can be reached from the middle of a body whose
    /// own completing state never leaves it (a throw, a `break`), so its entry uses this set.
    touched: Fields,
}

impl Walk<'_> {
    /// Walk `stmts` from `p`; `None` when no path completes normally out of the block.
    fn block(&mut self, stmts: &[Stmt], mut p: Path, in_loop: bool) -> Option<Path> {
        for s in stmts {
            p = self.stmt(s, p, in_loop)?;
        }
        Some(p)
    }

    fn stmt(&mut self, s: &Stmt, p: Path, in_loop: bool) -> Option<Path> {
        match s {
            Stmt::VarDecl { init: e, .. } | Stmt::Expr(e, _) | Stmt::Discard(e, _) => {
                self.reads(e, &p);
                Some(p)
            }
            Stmt::Assign { target, value, .. } => {
                self.reads(value, &p);
                self.assign(target, p, in_loop)
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.reads(v, &p);
                }
                None
            }
            Stmt::Throw { value, .. } => {
                self.reads(value, &p);
                None
            }
            Stmt::Break(_) | Stmt::Continue(_) => None,
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.reads(cond, &p);
                let t = self.block(then_block, p.clone(), in_loop);
                let e = match else_block {
                    Some(eb) => self.block(eb, p, in_loop),
                    None => Some(p),
                };
                merge(t, e)
            }
            // A loop body may run zero times (so it assigns nothing definitely) or many times (so any
            // assignment in it can repeat). A do-while (`post_cond`) runs at least once, but a
            // repeated write is the same TWICE either way, so it takes the same arm.
            Stmt::While { cond, body, .. } => {
                self.reads(cond, &p);
                Some(self.lp(body, p))
            }
            Stmt::For { iter, body, .. } => {
                self.reads(iter, &p);
                Some(self.lp(body, p))
            }
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                ..
            } => {
                let mut p = match init {
                    Some(i) => self.stmt(i, p, in_loop)?,
                    None => p,
                };
                if let Some(c) = cond {
                    self.reads(c, &p);
                }
                let mut body = body.clone();
                body.extend(step.iter().map(|s| (**s).clone()));
                p = self.lp(&body, p);
                Some(p)
            }
            Stmt::Block(b, _) => self.block(b, p, in_loop),
            // Runs exactly once, like a block (DEC-364).
            Stmt::Using { init, body, .. } => {
                self.reads(init, &p);
                self.block(body, p, in_loop)
            }
            // The `else` of a refutable destructure must diverge (checked elsewhere); its reads still
            // see the state before the binding.
            Stmt::Destructure {
                init, else_block, ..
            } => {
                self.reads(init, &p);
                if let Some(eb) = else_block {
                    self.block(eb, p.clone(), in_loop);
                }
                Some(p)
            }
            // A catch can start from any point of the body: its state is the pre-`try` one, with
            // whatever the body may have assigned. `finally` runs on every path, from the same
            // conservative state.
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                let before = p.clone();
                let (b, body_touched) = self.touching(|w| w.block(body, p, in_loop));
                let mut entry = before.clone();
                entry.maybe.extend(body_touched);
                let mut out = b;
                let mut fin = before;
                for c in catches {
                    let (cp, catch_touched) =
                        self.touching(|w| w.block(&c.body, entry.clone(), in_loop));
                    fin.maybe.extend(catch_touched);
                    out = merge(out, cp);
                }
                fin.maybe.extend(entry.maybe);
                match finally_block {
                    Some(f) => {
                        let fp = self.block(f, fin, in_loop)?;
                        out.map(|mut o| {
                            o.assigned.extend(fp.assigned);
                            o.maybe.extend(fp.maybe);
                            o
                        })
                    }
                    None => out,
                }
            }
        }
    }

    /// A loop body walked with repetition in mind; the state after the loop adds only possibilities.
    fn lp(&mut self, body: &[Stmt], p: Path) -> Path {
        let (_, body_touched) = self.touching(|w| w.block(body, p.clone(), true));
        let mut out = p;
        out.maybe.extend(body_touched);
        out
    }

    /// Run `f` and also return what it assigned on any path (diverging ones included), keeping the
    /// walk-wide [`Walk::touched`] set cumulative.
    fn touching<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> (T, Fields) {
        let outer = std::mem::take(&mut self.touched);
        let r = f(self);
        let inner = std::mem::replace(&mut self.touched, outer);
        self.touched.extend(inner.iter().cloned());
        (r, inner)
    }

    fn assign(&mut self, target: &Expr, mut p: Path, in_loop: bool) -> Option<Path> {
        let field = match target {
            Expr::Member { name, .. } if is_this_field(target, name) => name.clone(),
            other => {
                self.reads(other, &p);
                return Some(p);
            }
        };
        if self.once.contains(&field) && (in_loop || p.maybe.contains(&field)) {
            let why = if in_loop {
                "inside a loop, which can run it again"
            } else {
                "after it may already have been assigned"
            };
            self.found.push((
                Self::span_of(target),
                "E-ASSIGN-IMMUTABLE-TWICE",
                format!(
                    "immutable field `{field}` is assigned {why} — it may be assigned only once"
                ),
            ));
        }
        p.assigned.insert(field.clone());
        self.touched.insert(field.clone());
        p.maybe.insert(field);
        Some(p)
    }

    /// Every `this.<field>` read in `e` whose field is not definitely assigned yet. Lambda bodies are
    /// skipped (see the module header).
    fn reads(&mut self, e: &Expr, p: &Path) {
        let mut work = vec![e];
        while let Some(x) = work.pop() {
            if let Expr::Lambda { .. } = x {
                continue;
            }
            if let Expr::Member { name, .. } = x {
                if is_this_field(x, name) && self.reads.contains(name) && !p.assigned.contains(name)
                {
                    self.found.push((
                        Self::span_of(x),
                        "E-FIELD-READ-BEFORE-INIT",
                        format!("field `{name}` is read before it is assigned on every path of the constructor"),
                    ));
                }
            }
            crate::ast::push_subexprs(x, &mut work);
        }
    }

    fn span_of(e: &Expr) -> Span {
        Checker::expr_span(e)
    }
}

/// Join two paths: a path that completed nowhere contributes nothing.
fn merge(a: Option<Path>, b: Option<Path>) -> Option<Path> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(a), Some(b)) => Some(Path {
            assigned: a.assigned.intersection(&b.assigned).cloned().collect(),
            maybe: a.maybe.union(&b.maybe).cloned().collect(),
        }),
    }
}

impl Checker {
    /// The fields the constructor-once PERMISSION covers: the class's OWN immutable instance fields
    /// with no initializer. Built from the AST members, never from
    /// `ClassInfo.fields`, which also holds inherited fields — a subclass constructor gets no
    /// permission on a parent's field.
    pub(in crate::checker) fn ctor_once_candidates(members: &[crate::ast::ClassMember]) -> Fields {
        Self::deferred_fields(members, true)
    }

    /// Own instance fields with no initializer, not static/const; `immutable_only` narrows to those
    /// without `mutable`. A ctor-PROMOTED field needs no exclusion: it is a constructor parameter, never
    /// a `ClassMember::Field`, so it cannot appear here (sabotage S7b, row 5j: allowing it changed
    /// nothing).
    fn deferred_fields(members: &[crate::ast::ClassMember], immutable_only: bool) -> Fields {
        use crate::ast::{ClassMember, Modifier};
        members
            .iter()
            .filter_map(|m| match m {
                ClassMember::Field {
                    modifiers,
                    name,
                    init: None,
                    ..
                } if !modifiers.contains(&Modifier::Static)
                    && !modifiers.contains(&Modifier::Const)
                    && !(immutable_only && modifiers.contains(&Modifier::Mutable)) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// Run the flow check over the constructor body of `type_name` and report its findings.
    pub(in crate::checker) fn check_ctor_once(
        &mut self,
        type_name: &str,
        members: &[crate::ast::ClassMember],
    ) {
        let Some(body) = members.iter().find_map(|m| match m {
            crate::ast::ClassMember::Constructor { body, .. } => Some(body),
            _ => None,
        }) else {
            return;
        };
        let once = Self::ctor_once_candidates(members);
        // An optional field defaults to null before the body runs, so reading it early is fine.
        let reads: Fields = Self::deferred_fields(members, false)
            .into_iter()
            .filter(|f| {
                !matches!(
                    self.classes.get(type_name).and_then(|i| i.fields.get(f)),
                    Some(Ty::Optional(_))
                )
            })
            .collect();
        if once.is_empty() && reads.is_empty() {
            return;
        }
        let mut w = Walk {
            once: &once,
            reads: &reads,
            found: Vec::new(),
            touched: Fields::new(),
        };
        w.block(body, Path::default(), false);
        for (span, code, msg) in w.found {
            let hint = match code {
                "E-ASSIGN-IMMUTABLE-TWICE" => "assign it exactly once on each path (both arms of an `if` is fine), or declare it `mutable`",
                _ => "assign `this.<field> = …` before this read, or give the field an initializer",
            };
            self.err_coded(span, msg, code, Some(hint.to_string()));
        }
    }
}
