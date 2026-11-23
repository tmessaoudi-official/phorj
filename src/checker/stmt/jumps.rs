//! Jumps: `break`/`continue` (`E-BREAK-OUTSIDE-LOOP`, `E-CONTINUE-OUTSIDE-LOOP`, moved here from
//! `core.rs`) and the rule that no `return`, `break` or `continue` may leave a `finally` block
//! (DEC-595 / DEC-596, `E-FINALLY-EXIT`). A `return` there silently discards a pending exception (PHP 8.6 deprecates it, C#'s CS0157
//! forbids it); a `break`/`continue` leaving one is a PHP compile-time fatal on 8.5 and 8.6 while the
//! native legs ran it. One rule, on every leg, so the three legs cannot disagree.
//!
//! The state is [`Checker::finally_loop_base`]: `Some(d)` while a `finally` block is checked, `d` being
//! the loop depth at its entry. A `return` anywhere under it leaves the block; a `break`/`continue`
//! leaves it exactly when no loop was opened inside it (`loop_depth == d`). A lambda starts a new
//! callable, so it saves and clears this state (see `expr/lambda.rs`), as it does `loop_depth`.

use super::*;

impl Checker {
    /// A `break` or `continue` (`kw`): it needs an enclosing loop (M-mut.3), and it may not leave a
    /// `finally` block.
    pub(super) fn check_jump(&mut self, span: Span, kw: &'static str) {
        self.check_finally_exit(span, Some(kw));
        if self.loop_depth > 0 {
            return;
        }
        let (msg, code) = if kw == "break" {
            ("`break` outside a loop", "E-BREAK-OUTSIDE-LOOP")
        } else {
            ("`continue` outside a loop", "E-CONTINUE-OUTSIDE-LOOP")
        };
        self.err_coded(
            span,
            msg,
            code,
            Some(format!(
                "`{kw}` may only appear inside a `for`/`while`/`do-while` loop"
            )),
        );
    }

    /// Check a `finally` block with the exit rule armed, restoring the outer state afterwards (a
    /// `finally` nested in a `finally` re-arms it at its own loop depth).
    pub(super) fn check_finally_block(&mut self, fb: &[crate::ast::Stmt]) {
        let outer = self.finally_loop_base.replace(self.loop_depth);
        self.check_block(fb);
        self.finally_loop_base = outer;
    }

    /// `E-FINALLY-EXIT` for a `return` (`jump: None`) or for a `break`/`continue` (`jump: Some(kw)`)
    /// that would leave the enclosing `finally` block.
    pub(super) fn check_finally_exit(&mut self, span: Span, jump: Option<&str>) {
        let Some(base) = self.finally_loop_base else {
            return;
        };
        let leaves = match jump {
            None => true,
            Some(_) => self.loop_depth == base,
        };
        if !leaves {
            return;
        }
        let what = jump.unwrap_or("return");
        self.err_coded(
            span,
            format!("a `{what}` cannot leave a `finally` block"),
            "E-FINALLY-EXIT",
            Some(match jump {
                None => "a `return` here would silently discard an error the `try` is propagating; \
                         return after the `try` instead"
                    .into(),
                Some(kw) => format!(
                    "a `{kw}` may only target a loop written inside the `finally`; move it after the `try`"
                ),
            }),
        );
    }
}
