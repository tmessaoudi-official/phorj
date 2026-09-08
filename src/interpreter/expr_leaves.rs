//! Tree-walking interpreter — the LEAF evaluators the `eval` dispatch delegates to: identifiers,
//! interpolated strings, unary/binary operators and `match`.
//!
//! Split out of `expr.rs` under Invariant 13 (M-Decomp) when the `<=>` build (`eba09d4e`) pushed that
//! file past its size-baseline. `expr.rs` keeps the one big `eval` match — the DISPATCH over `Expr`,
//! which Invariant 3 requires to stay wildcard-free and total — and these are the arms substantial
//! enough to have their own function. They are called only from that dispatch, so `pub(super)` here
//! is the same reach it was there.

use super::*;

impl<'c> Interp<'c> {
    pub(super) fn eval_ident(&mut self, name: &str) -> R<Value> {
        if let Some(v) = self.frame.lookup(name) {
            return Ok(v.clone());
        }
        // bare field reference inside a method body (mirrors checker scope seeding)
        if let Some(Value::Instance(inst)) = &self.this {
            if let Some(v) = inst.get_field(name) {
                return Ok(v);
            }
        }
        // A4: bare named-function reference in value position (e.g. passing `f` to a higher-order
        // function that takes `(int)->int`).  The checker already verified the type; the interpreter
        // wraps it in a `Named` closure so `eval_call` can dispatch it uniformly.
        if self.funcs.contains_key(name) {
            return Ok(Value::Closure(Rc::new(ClosureData::Named(
                name.to_string(),
            ))));
        }
        rt(format!("undefined variable `{name}`"))
    }

    pub(super) fn eval_str(&mut self, parts: &[StrPart]) -> R<Value> {
        let mut s = String::new();
        for part in parts {
            match part {
                StrPart::Literal(lit) => s.push_str(lit),
                StrPart::Expr(e) => {
                    let v = self.eval(e)?;
                    match v.as_display() {
                        Some(text) => s.push_str(&text),
                        None => {
                            return rt(format!(
                                "cannot interpolate {} into a string",
                                v.type_name()
                            ))
                        }
                    }
                }
            }
        }
        Ok(Value::Str(s.into()))
    }

    pub(super) fn eval_unary(&mut self, op: UnaryOp, expr: &Expr) -> R<Value> {
        let v = self.eval(expr)?;
        match (op, v) {
            // `#[UncheckedOverflow]`: wrap (`-i64::MIN` → `i64::MIN`) instead of faulting; else checked.
            (UnaryOp::Neg, Value::Int(n)) if self.cur_unchecked => {
                Ok(Value::Int(crate::value::int_wrapping_neg(n)))
            }
            (UnaryOp::Neg, Value::Int(n)) => match crate::value::int_neg(n) {
                Ok(v) => Ok(Value::Int(v)),
                Err(msg) => rt(msg),
            },
            (UnaryOp::Neg, Value::Float(x)) => Ok(Value::Float(-x)),
            (UnaryOp::Neg, Value::Decimal { unscaled, scale }) => {
                // Negate via the shared kernel (checked — `i128::MIN` faults, never `-0` in render).
                match crate::value::decimal_neg(unscaled, scale) {
                    Ok(v) => Ok(v),
                    Err(msg) => rt(msg),
                }
            }
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOp::BitNot, Value::Int(n)) => Ok(Value::Int(crate::value::int_bitnot(n))),
            (op, v) => rt(format!("cannot apply {op:?} to {}", v.type_name())),
        }
    }

    pub(super) fn eval_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr) -> R<Value> {
        use BinaryOp::*;
        if matches!(op, And | Or) {
            let l = as_bool(&self.eval(lhs)?)?;
            return match op {
                And if !l => Ok(Value::Bool(false)),
                Or if l => Ok(Value::Bool(true)),
                _ => Ok(Value::Bool(as_bool(&self.eval(rhs)?)?)),
            };
        }
        if matches!(op, Coalesce) {
            // `a ?? b`: evaluate `b` only when `a` is null (short-circuit).
            let l = self.eval(lhs)?;
            return if matches!(l, Value::Null) {
                self.eval(rhs)
            } else {
                Ok(l)
            };
        }
        let l = self.eval(lhs)?;
        let r = self.eval(rhs)?;
        match op {
            Add | Sub | Mul | Pow | Div | Rem => arith(op, l, r, self.cur_unchecked),
            BitAnd | BitOr | BitXor | Shl | Shr => bitwise(op, l, r),
            Eq => Ok(Value::Bool(l.eq_val(&r))),
            NotEq => Ok(Value::Bool(!l.eq_val(&r))),
            Lt | Gt | Le | Ge => compare(op, l, r),
            // `<=>` yields an `int`, so it takes the shared `value::three_way` projection rather
            // than `compare`'s op->bool one (DEC-505/DEC-512).
            Spaceship => three_way(l, r),
            Pipe => unreachable!("`|>` is lowered to a call in the parser"),
            And | Or | Coalesce => unreachable!("handled above"),
        }
    }

    pub(super) fn eval_match(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> R<Value> {
        let value = self.eval(scrutinee)?;
        for arm in arms {
            let mut bindings = Vec::new();
            if match_pattern(&arm.pattern, &value, &self.class_implements, &mut bindings) {
                self.frame.push_scope();
                for (n, v) in bindings {
                    self.frame.declare(&n, v);
                }
                // An arm guard runs with the pattern's bindings in scope; a false guard falls
                // through to the next arm (discarding this arm's bindings first).
                if let Some(g) = &arm.guard {
                    match self.eval(g).and_then(|v| as_bool(&v)) {
                        Ok(true) => {}
                        Ok(false) => {
                            self.frame.pop_scope();
                            continue;
                        }
                        Err(e) => {
                            self.frame.pop_scope();
                            return Err(e);
                        }
                    }
                }
                let r = self.eval(&arm.body);
                self.frame.pop_scope();
                return r;
            }
        }
        rt(faults::FAULT_NON_EXHAUSTIVE_MATCH)
    }
}
