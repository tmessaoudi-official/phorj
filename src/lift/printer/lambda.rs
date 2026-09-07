//! Lift printer — lambdas. Lane R (2026-09-05): a PHP arrow closure lifts to phorj's
//! expression-bodied lambda, `function(int v): int => v * k`. Lane L1b (2026-09-07) adds the
//! statement-bodied form, `function(int v): int { … }`, which PHP writes as
//! `function (…) [use (…)] : T { … }` — 23 of scout's 120 files use one.

use super::*;
use crate::ast::LambdaBody;

impl Printer {
    pub(super) fn lambda(
        &self,
        params: &[Param],
        ret: Option<&Type>,
        body: &LambdaBody,
    ) -> Result<String, String> {
        let ret_s = match ret {
            Some(t) => format!(": {}", ty(t)?),
            None => String::new(),
        };
        match body {
            LambdaBody::Expr(e) => Ok(format!(
                "function({}){ret_s} => {}",
                self.params(params)?,
                self.expr(e)?
            )),
            // A block body is rendered by a SUB-PRINTER seeded with this one's indent, because
            // statement printing appends to a buffer with `&mut self` while expression printing is
            // `&self` and returns a string. The sub-printer's output is spliced in; the closing
            // brace is emitted at the enclosing indent so the block lines up with the statement it
            // sits inside. `docs` is deliberately NOT cloned: a lambda has no declaration name, so
            // nothing in the doc map can address its body.
            LambdaBody::Block(stmts) => {
                let mut sub = Printer {
                    out: String::new(),
                    indent: self.indent + 1,
                    docs: Default::default(),
                };
                for s in stmts {
                    sub.stmt(s)?;
                }
                let pad = "    ".repeat(self.indent);
                Ok(format!(
                    "function({}){ret_s} {{\n{}{pad}}}",
                    self.params(params)?,
                    sub.out
                ))
            }
        }
    }
}
