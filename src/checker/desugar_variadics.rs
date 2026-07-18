//! DEC-298: rewrite a variadic parameter `T ...name` into a plain `List<T> name` in the AST, so every
//! backend sees an ordinary `List<T>` parameter (Invariant #5 — compile-time sugar expanded OUT before
//! any backend). The matching CALL-site rewrite (trailing args collected into one `[..]` list literal)
//! rides the `default_fills` side-table; this pass handles the DECLARATION side. Free functions only in
//! v1 — a variadic method/lambda param is rejected at check (`E-VARIADIC-UNSUPPORTED`), so a program
//! reaching this post-check pass only ever has variadic FREE-function params.

use crate::ast::{Item, Program, Type};

/// Rewrite every variadic free-function parameter `T ...name` to a non-variadic `List<T> name`, so the
/// interpreter, VM, and transpiler all see a plain `List<T>` param (the transpiler emits `array $name`,
/// byte-identical to the collected `f([a, b, c])` call the `default_fills` rewrite produces).
pub fn desugar_variadic_params(mut program: Program) -> Program {
    for item in &mut program.items {
        if let Item::Function(f) = item {
            for p in &mut f.params {
                if p.variadic {
                    let elem = p.ty.clone();
                    p.ty = Type::Named {
                        name: "List".into(),
                        args: vec![elem],
                        span: p.span,
                    };
                    p.variadic = false;
                }
            }
        }
    }
    program
}
