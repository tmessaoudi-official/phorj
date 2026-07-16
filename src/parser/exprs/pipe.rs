//! Pipe-specific parsing (DEC-239): the contextually-typed pipe lambda `x |> (v => …)` and the
//! `%` placeholder shape validation `x |> f(%, 2)`. The pipe's precedence-climbing itself lives in
//! `climb.rs`; this file holds what only a pipe RHS can produce.

use super::*;

impl Parser {
    /// Parse one pipe RHS operand at `right_bp`, with the `%`-placeholder flag set and the shape
    /// validated before returning. Called only from the pipe arm of `parse_binary_from`.
    pub(in crate::parser) fn parse_pipe_rhs(&mut self, right_bp: u8) -> Result<Expr, Diagnostic> {
        let saved = self.pipe_rhs;
        self.pipe_rhs = true;
        // A pipe RHS starting `( ident =>` is the contextually-typed pipe lambda
        // `x |> (v => v * 2 + 1)` — an expression-body lambda whose single param omits its type
        // (it flows from the piped value; the checker resolves the `Type::Infer` marker). Legal
        // ONLY here: everywhere else `( ident =>` stays a parse error, so no valid program changes
        // meaning. After the lambda the climb continues at `right_bp` — the RHS grammar stays
        // uniform, so `x |> (v => v) + 1` binds the `+` to the LAMBDA (a loud
        // E-PIPE-LAMBDA-CONTEXT), never silently to the pipe result. (PENDING fork: an ergonomic
        // carve-out binding trailing tight-ops to the pipe result instead — a developer
        // adjudication, deliberately not self-ruled; erroring now is the additive-relaxable
        // choice.)
        let rhs = if matches!(self.peek(), TokenKind::LParen)
            && matches!(self.peek2(), TokenKind::Ident(_))
            && matches!(self.peek3(), TokenKind::FatArrow)
        {
            let lam = self.parse_contextual_pipe_lambda();
            lam.and_then(|lam| self.parse_binary_from(lam, right_bp))
        } else {
            self.parse_binary(right_bp)
        };
        self.pipe_rhs = saved;
        let rhs = rhs?;
        Self::validate_pipe_placeholders(&rhs)?;
        Ok(rhs)
    }

    /// `( ident => expr )` — the contextually-typed pipe lambda (DEC-239): an expression-body
    /// lambda in pipe-RHS position whose single parameter omits its type. The param carries
    /// [`Type::Infer`] (the `var` marker), which the checker resolves from the piped value's type —
    /// the DEC-201 contextual-typing precedent. The caller has already confirmed the
    /// `( ident =>` head, so this cannot backtrack.
    fn parse_contextual_pipe_lambda(&mut self) -> Result<Expr, Diagnostic> {
        let sp = self.peek_span();
        self.expect(&TokenKind::LParen, "'(' to open a pipe lambda `(v => …)`")?;
        let psp = self.peek_span();
        let name = self.expect_ident("a parameter name in a pipe lambda `(v => …)`")?;
        self.expect(&TokenKind::FatArrow, "'=>' in a pipe lambda `(v => …)`")?;
        let body = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')' to close a pipe lambda `(v => …)`")?;
        Ok(Expr::Lambda {
            params: vec![crate::ast::Param {
                ty: Type::Infer(psp),
                name,
                default: None,
                span: psp,
            }],
            ret: None,
            throws: Vec::new(),
            body: crate::ast::LambdaBody::Expr(Box::new(body)),
            span: sp,
        })
    }

    /// DEC-239 placeholder shape rule: a `%` may stand ONLY as a whole direct argument of the
    /// pipe's TOP-LEVEL RHS call (`x |> f(%, 2)`); `f(% + 1)` and nested `g(%)` are rejected —
    /// nesting is the lambda's job. A nested pipe inside the RHS owns (and has already validated)
    /// its own placeholders, so the walk stops at [`Expr::Pipe`] nodes.
    fn validate_pipe_placeholders(rhs: &Expr) -> Result<(), Diagnostic> {
        if let Expr::Call { callee, args, .. } = rhs {
            Self::reject_placeholders(callee)?;
            for a in args {
                if !matches!(a, Expr::PipePlaceholder(_)) {
                    Self::reject_placeholders(a)?;
                }
            }
            return Ok(());
        }
        Self::reject_placeholders(rhs)
    }

    /// Error on any [`Expr::PipePlaceholder`] in this subtree (stopping at nested pipes, which own
    /// their placeholders). Uses a manual worklist over borrowed children — no clone, no recursion
    /// depth to guard.
    fn reject_placeholders(e: &Expr) -> Result<(), Diagnostic> {
        let mut work: Vec<&Expr> = vec![e];
        while let Some(e) = work.pop() {
            match e {
                Expr::PipePlaceholder(sp) => {
                    return Err(Diagnostic::new(
                        Stage::Parse,
                        "a pipe placeholder `%` must be a whole argument of the pipe's top-level \
                         call — `x |> f(%, 2)`",
                        sp.line,
                        sp.col,
                    )
                    .with_code("E-PIPE-PLACEHOLDER")
                    .with_hint(
                        "for anything nested, use a pipe lambda instead: \
                         `x |> (v => f(g(v), v + 1))`",
                    ));
                }
                // A nested pipe validated its own RHS when it was parsed; its placeholders are its
                // own. Its LHS may still carry strays of THIS pipe — keep walking that side only.
                Expr::Pipe { lhs, .. } => work.push(lhs),
                _ => crate::ast::push_subexprs(e, &mut work),
            }
        }
        Ok(())
    }
}
