//! PHP transpiler — the block-closure fallback for a lambda whose body emits a STATEMENT.
//!
//! Split out of `expr.rs`, which is grandfathered under Invariant 13 and must not grow.
//!
//! PHP's `echo` is a statement, not an expression, so the arrow form `fn() => echo "x"` is a parse
//! error. That is exactly what a void-bodied lambda over `Output.printLine` produced — and
//! `function() => Output.printLine(msg)` is the natural shape for a `Runtime.onShutdown` handler, so
//! the first `examples/guide/on-shutdown.phg` ran correctly on BOTH native backends while the
//! transpiled file died with `syntax error, unexpected token "echo"`. Only the PHP leg saw it.
//!
//! The fallback emits the block-closure form instead, which takes statements by construction.
//! Captures must then be listed explicitly: `function () use (…)` does not capture implicitly the
//! way `fn` does, so dropping the `use` clause would turn a captured local into an undefined
//! variable at run time rather than a compile error.

use super::*;

impl Transpiler {
    /// Emit a lambda literal. Moved here whole from `expr.rs` (Invariant 13 — that file is
    /// grandfathered and must not grow), which also puts the arrow form and its statement-shaped
    /// fallback in one place instead of one calling into the other across modules.
    pub(super) fn emit_lambda(
        &mut self,
        params: &[crate::ast::Param],
        body: &crate::ast::LambdaBody,
    ) -> Result<String, String> {
        let ps = params
            .iter()
            .map(|p| format!("${}", p.name))
            .collect::<Vec<_>>()
            .join(", ");
        match body {
            LambdaBody::Expr(e) => {
                // T6: type the params in a fresh scope so arithmetic in the arrow body
                // specializes (`function(int a, int b) => a + b` → `$a + $b`).
                self.push_scope();
                for p in params {
                    self.declare(&p.name);
                    self.declare_kind(&p.name, kind_of_type(&p.ty));
                }
                let body_php = self.emit_expr(e)?;
                self.pop_scope();
                // PHP's `echo` is a STATEMENT, so `fn() => echo "x"` does not parse — see
                // `lambda_stmt` for why that shape reaches here at all.
                if let Some(php) = self.stmt_bodied_closure(&ps, params, body, &body_php) {
                    return Ok(php);
                }
                Ok(format!("fn({ps}) => {body_php}"))
            }
            LambdaBody::Block(stmts) => {
                // Compute captures: free variables that are enclosing locals, not
                // top-level function names, variants, or classes.
                let caps: Vec<String> = crate::ast::free_vars(params, body)
                    .into_iter()
                    .filter(|n| {
                        self.is_local(n)
                            && !self.funcs.contains(n)
                            && !self.variants.contains(n)
                            && !self.classes.contains(n)
                    })
                    .map(|n| format!("${n}"))
                    .collect();
                let use_clause = if caps.is_empty() {
                    String::new()
                } else {
                    format!(" use ({})", caps.join(", "))
                };
                // Emit the block body into a temporary buffer (swapping `self.out`)
                // so `emit_stmt` can write indented lines, then collect them as the
                // inline closure body. Params and captures are declared in a fresh
                // scope so inner expressions resolve them correctly.
                let saved_out = std::mem::take(&mut self.out);
                let saved_indent = self.indent;
                self.indent = 0;
                self.push_scope();
                // Declare captures first (so params can shadow same-named captures).
                for cap in &caps {
                    // Strip the leading `$` to get the bare name.
                    self.declare(&cap[1..]);
                }
                for p in params {
                    self.declare(&p.name);
                    self.declare_kind(&p.name, kind_of_type(&p.ty));
                }
                for s in stmts {
                    self.emit_stmt(s)?;
                }
                self.pop_scope();
                self.indent = saved_indent;
                let body_php = std::mem::replace(&mut self.out, saved_out);
                // The body_php has one "line" per statement (each ends with '\n' from
                // `self.line()`). Trim trailing whitespace and join with spaces for a
                // compact inline representation.
                let body_php = body_php
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                Ok(format!("function({ps}){use_clause} {{ {body_php} }}"))
            }
        }
    }

    /// `Some(php)` when `body_php` is statement-shaped and needs the block-closure form; `None` to
    /// keep the ordinary arrow form, which stays the path for every value-returning body.
    pub(super) fn stmt_bodied_closure(
        &mut self,
        ps: &str,
        params: &[crate::ast::Param],
        body: &crate::ast::LambdaBody,
        body_php: &str,
    ) -> Option<String> {
        if !body_php.starts_with("echo ") {
            return None;
        }
        let caps: Vec<String> = crate::ast::free_vars(params, body)
            .into_iter()
            .filter(|n| {
                self.is_local(n)
                    && !self.funcs.contains(n)
                    && !self.variants.contains(n)
                    && !self.classes.contains(n)
            })
            .map(|n| format!("${n}"))
            .collect();
        let use_clause = if caps.is_empty() {
            String::new()
        } else {
            format!(" use ({})", caps.join(", "))
        };
        Some(format!("function({ps}){use_clause} {{ {body_php}; }}"))
    }
}
