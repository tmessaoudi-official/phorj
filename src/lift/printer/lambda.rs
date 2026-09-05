//! Lift printer — lambdas. Lane R (2026-09-05): a PHP arrow closure lifts to phorj's
//! expression-bodied lambda, `function(int v): int => v * k`. A block-bodied lambda is not in the
//! lift subset yet (the parser refuses PHP's `function (…) { … }` first, so this arm is defensive).

use super::*;
use crate::ast::LambdaBody;

impl Printer {
    pub(super) fn lambda(
        &self,
        params: &[Param],
        ret: Option<&Type>,
        body: &LambdaBody,
    ) -> Result<String, String> {
        let LambdaBody::Expr(e) = body else {
            return Err("printer: a block-bodied lambda is outside the lift subset".into());
        };
        let ret = match ret {
            Some(t) => format!(": {}", ty(t)?),
            None => String::new(),
        };
        Ok(format!(
            "function({}){ret} => {}",
            self.params(params)?,
            self.expr(e)?
        ))
    }
}
