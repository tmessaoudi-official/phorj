//! Expression compilation — binary operators (checked kernels, short-circuit shapes).

use super::*;

impl Compiler<'_> {
    pub(in crate::compiler) fn compile_binary(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        line: u32,
    ) -> Result<(), String> {
        use BinaryOp::*;
        // Short-circuit logical ops desugar to jumps (decision P2-5).
        match op {
            And => {
                self.expr(lhs)?;
                let l_false = self.emit_jump(Op::JumpIfFalse(0), line); // pops lhs
                let h_merge = self.height; // both branches converge to one bool above this
                self.expr(rhs)?;
                let l_end = self.emit_jump(Op::Jump(0), line);
                self.patch_jump(l_false);
                self.height = h_merge; // false-path: reset before pushing the literal `false`
                self.emit_const(Value::Bool(false), line);
                self.patch_jump(l_end);
                return Ok(());
            }
            Or => {
                self.expr(lhs)?;
                let l_rhs = self.emit_jump(Op::JumpIfFalse(0), line); // pops lhs
                let h_merge = self.height;
                self.emit_const(Value::Bool(true), line);
                let l_end = self.emit_jump(Op::Jump(0), line);
                self.patch_jump(l_rhs);
                self.height = h_merge; // rhs-path: reset before evaluating rhs
                self.expr(rhs)?;
                self.patch_jump(l_end);
                return Ok(());
            }
            Coalesce => {
                // `a ?? b`: keep `a` unless it is null, without re-evaluating it. Stash `a` in a
                // scratch local (the `$match`-scrutinee trick), test it against `null`; if null,
                // evaluate `b` and overwrite the slot with it. No new `Op` (decision S2-OPS).
                self.expr(lhs)?; // [a] — a lands in the scratch slot
                                 // The scratch slot is `a`'s frame-relative position (top of stack), NOT
                                 // `locals.len()`: live transients (e.g. earlier interpolation segments) may sit
                                 // below it, so `add_local`'s index would be wrong. Mirrors `compile_match`'s
                                 // `m_slot = self.height - 1`. Addressed numerically by Get/SetLocal — no `Local` entry.
                let slot = self.height - 1;
                self.emit(Op::GetLocal(slot), line); // [a, a]
                self.emit_const(Value::Null, line); // [a, a, null]
                self.emit(Op::Eq, line); // [a, bool]
                let keep = self.emit_jump(Op::JumpIfFalse(0), line); // [a]; jump if a != null
                let h_merge = self.height;
                self.expr(rhs)?; // [a, b]
                self.emit(Op::SetLocal(slot), line); // [b] — overwrite the slot with b
                self.patch_jump(keep); // keep-path arrives with [a]; both leave one value at `slot`
                self.height = h_merge;
                return Ok(());
            }
            _ => {}
        }
        // Strict ops: evaluate both, then emit.
        match op {
            // `string + string` → concatenation: reuse `Op::Concat(2)` (no new Op). The checker
            // guarantees both operands are `string`, and `ctype` resolves every string-producing
            // operand to `CTy::Str`, so this lowers byte-identically to the interpreter's `Str + Str`
            // (Phase 1 string slice).
            Add if matches!(self.ctype(lhs), Ok(CTy::Str)) => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.emit(Op::Concat(2), line);
            }
            // `**` power has no dedicated `Op`: it lowers (type-directed) to a `Core.Math` native
            // call — `ipow` for `int**int`, `pow` for `float**float` — keeping the no-new-Op
            // invariant. The native dispatches into `value::int_pow`/`float_pow`, the *same* kernels
            // the interpreter's `**` arm uses, so interp/VM compute and fault identically. The
            // registry index is resolved at compile time, so no `import Core.Math` is required.
            Pow => {
                let leaf = match self.num_ty(lhs)? {
                    NumTy::Int => "integerPower",
                    NumTy::Float => "pow",
                    // `decimal ** _` is rejected by the checker (decimal supports only `+ - *`).
                    NumTy::Decimal => unreachable!("decimal `**` rejected by checker"),
                };
                let idx = crate::native::index_of("Core.Math", leaf)
                    .expect("Core.Math.integerPower/pow are registered natives");
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.emit(Op::CallNative(idx, 2), line);
            }
            Add | Sub | Mul | Div | Rem => {
                // `decimal` arithmetic (M-NUM S1): emit `AddD/SubD/MulD` when EITHER operand is
                // decimal (`decimal ⊕ int` widens the int in the value kernel) — `num_ty(lhs)` alone
                // would mis-classify `int * decimal`. The checker allows all of decimal `+ - * % /`
                // (`/` is exact-or-fault), so any of them reaches the decimal path. Probe both operands; a probe that errs
                // (a genuinely unresolvable operand) falls through to the int/float path's error.
                let lhs_dec = matches!(self.ctype(lhs), Ok(CTy::Decimal));
                let rhs_dec = matches!(self.ctype(rhs), Ok(CTy::Decimal));
                let nt = if lhs_dec || rhs_dec {
                    NumTy::Decimal
                } else {
                    self.num_ty(lhs)?
                };
                self.expr(lhs)?;
                self.expr(rhs)?;
                let emit = match (op, nt) {
                    (Add, NumTy::Int) => Op::AddI,
                    (Add, NumTy::Float) => Op::AddF,
                    (Add, NumTy::Decimal) => Op::AddD,
                    (Sub, NumTy::Int) => Op::SubI,
                    (Sub, NumTy::Float) => Op::SubF,
                    (Sub, NumTy::Decimal) => Op::SubD,
                    (Mul, NumTy::Int) => Op::MulI,
                    (Mul, NumTy::Float) => Op::MulF,
                    (Mul, NumTy::Decimal) => Op::MulD,
                    (Div, NumTy::Int) => Op::DivI,
                    (Div, NumTy::Float) => Op::DivF,
                    (Rem, NumTy::Int) => Op::RemI,
                    (Rem, NumTy::Float) => Op::RemF,
                    // Exact decimal `%` and exact-or-fault `/` (2026-06-27).
                    (Rem, NumTy::Decimal) => Op::RemD,
                    (Div, NumTy::Decimal) => Op::DivD,
                    _ => unreachable!("arithmetic op set"),
                };
                self.emit(emit, line);
            }
            Eq => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.emit(Op::Eq, line);
            }
            NotEq => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.emit(Op::Ne, line);
            }
            Lt | Gt | Le | Ge => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.emit(
                    match op {
                        Lt => Op::Lt,
                        Gt => Op::Gt,
                        Le => Op::Le,
                        Ge => Op::Ge,
                        _ => unreachable!("outer arm restricts `op` to Lt/Gt/Le/Ge"),
                    },
                    line,
                );
            }
            // Bitwise binaries (primitives P2): int-only (checker-guaranteed), so the int Op is
            // emitted directly — no `NumTy` dispatch.
            BitAnd | BitOr | BitXor | Shl | Shr => {
                self.expr(lhs)?;
                self.expr(rhs)?;
                self.emit(
                    match op {
                        BitAnd => Op::BitAnd,
                        BitOr => Op::BitOr,
                        BitXor => Op::BitXor,
                        Shl => Op::Shl,
                        Shr => Op::Shr,
                        _ => {
                            unreachable!("outer arm restricts `op` to BitAnd/BitOr/BitXor/Shl/Shr")
                        }
                    },
                    line,
                );
            }
            Pipe => unreachable!("`|>` is lowered to a call in the parser"),
            And | Or | Coalesce => unreachable!("handled above"),
        }
        Ok(())
    }
}
