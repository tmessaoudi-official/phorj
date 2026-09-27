//! Row 4j2(b) — the EXPRESSION half of the facts walk (`facts.rs`): the call sites a body makes and
//! the `throw` expressions inside it. Exhaustive over `PhpExpr` (Invariant 3): no `_` arm, so a new
//! expression form must decide whether it can call or throw.

use super::{leaf, Key, Walk};
use crate::lift::ast as php;

impl Walk<'_> {
    pub(super) fn exprs(&mut self, es: &[php::PhpExpr]) {
        es.iter().for_each(|e| self.expr(e));
    }

    pub(super) fn expr(&mut self, e: &php::PhpExpr) {
        use php::PhpExpr as E;
        match e {
            E::Throw(inner) => self.throw(inner),
            // Not entered: a lambda's throws are its own, and it cannot declare them.
            E::Closure { .. } | E::BlockClosure { .. } => {}
            E::Call { callee, args } => {
                if let E::Name(n) = &**callee {
                    self.call(Key::Function(leaf(n).to_string()));
                } else {
                    self.expr(callee);
                }
                self.exprs(args);
            }
            E::MethodCall {
                recv,
                name,
                args,
                nullsafe,
            } => {
                if let (E::Var(v), false, Some(c)) = (&**recv, nullsafe, self.class) {
                    if v == "this" {
                        self.call(Key::Method(c.to_string(), name.clone()));
                    }
                }
                self.expr(recv);
                self.exprs(args);
            }
            E::StaticCall { class, name, args } => {
                if let Some(c) = self.class_of(class) {
                    self.call(Key::Method(c, name.clone()));
                }
                self.exprs(args);
            }
            E::New { args, .. } => self.exprs(args),
            E::Interp(parts) => {
                for p in parts {
                    if let php::PhpStrPart::Expr(x) = p {
                        self.expr(x);
                    }
                }
            }
            E::Array(elems) => {
                for el in elems {
                    if let Some(k) = &el.key {
                        self.expr(k);
                    }
                    self.expr(&el.value);
                }
            }
            E::Destructure { value, .. }
            | E::NamedArg { value, .. }
            | E::Cast { value, .. }
            | E::Declared { value, .. }
            | E::InstanceOf { value, .. } => self.expr(value),
            E::Unary { expr, .. } | E::Spread(expr) | E::AppendSlot(expr) => self.expr(expr),
            E::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            E::Assign { target, value } | E::CompoundAssign { target, value, .. } => {
                self.expr(target);
                self.expr(value);
            }
            E::IncDec { target, .. } => self.expr(target),
            E::Ternary { cond, then, els } => {
                self.expr(cond);
                if let Some(t) = then {
                    self.expr(t);
                }
                self.expr(els);
            }
            E::Member { recv, .. } => self.expr(recv),
            E::Index { base, index } => {
                self.expr(base);
                self.expr(index);
            }
            E::Match { subject, arms } => {
                self.expr(subject);
                for arm in arms {
                    if let Some(conds) = &arm.conds {
                        self.exprs(conds);
                    }
                    self.expr(&arm.body);
                }
            }
            E::Int(_)
            | E::Float(_)
            | E::Str(_)
            | E::Bool(_)
            | E::Null
            | E::Var(_)
            | E::Name(_)
            | E::CallableRef(_)
            | E::EmptyColl(_)
            | E::ClassConst { .. }
            | E::StaticProp { .. } => {}
        }
    }
}
