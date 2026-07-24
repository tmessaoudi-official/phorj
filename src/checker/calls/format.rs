//! Call checking — `String.format` directive analysis (W3-5/DEC-199), split from `calls/core.rs`
//! per Invariant 13. The compile-time gate that mirrors the runtime `text_format` renderer.

use super::*;

impl Checker {
    /// W3-5 / DEC-199 slice 1: type-check `String.format(spec, args)` — a real `%`-sprintf native
    /// (`text_format` / `__phorj_format`), NOT a desugar. Validates arg 0 is a `string` and arg 1 a
    /// list, then — for a LITERAL spec — gates the directive set at compile time (`%s`/`%d`/`%%` this
    /// slice; anything else, incl. width/precision/flags/`%f`/`%N$`, is `E-FORMAT-UNSUPPORTED`) and,
    /// when the values are a list literal, checks the directive count (`E-FORMAT-ARG-COUNT`). A runtime
    /// spec / non-literal list is left to the strict runtime renderer (which faults on a bad directive,
    /// a `%d` type mismatch, or a count mismatch). Returns `Ty::String`.
    pub(in crate::checker) fn check_string_format(
        &mut self,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        use crate::ast::{Expr, StrPart};
        if args.len() != 2 {
            return self.err_coded(
                span,
                format!(
                    "`String.format` expects 2 arguments (a format string and a list of values), found {}",
                    args.len()
                ),
                "E-FORMAT-ARGS",
                Some("call it as `String.format(\"%s = %d\", [name, count])`".into()),
            );
        }
        let spec_ty = self.check_expr(&args[0]);
        if !matches!(spec_ty, Ty::String | Ty::Error) {
            self.err_coded(
                Self::expr_span(&args[0]),
                format!("`String.format`'s format string must be a `string`, found `{spec_ty}`"),
                "E-FORMAT-SPEC-TYPE",
                None,
            );
        }
        // Values: a LIST LITERAL may be heterogeneous printable scalars (`["Ada", 3, 50]` — `%s`/`%d`
        // consume them by position), so check each element individually rather than as a homogeneous
        // `List<T>` (which would reject the mix / an empty `[]`). A non-literal list arg (a `List<T>`
        // variable) is accepted by its list type; the strict runtime renderer enforces per-directive
        // element types (`%d` needs an int) with clean faults.
        let scalar_ok = |t: &Ty| {
            matches!(
                t,
                Ty::Int | Ty::Float | Ty::Decimal | Ty::Bool | Ty::String | Ty::Error
            )
        };
        match &args[1] {
            Expr::List(items, _) => {
                for it in items {
                    let t = self.check_expr(it);
                    if !scalar_ok(&t) {
                        self.err_coded(
                            Self::expr_span(it),
                            format!(
                                "`String.format` values must be printable scalars, found `{t}`"
                            ),
                            "E-FORMAT-ARG-TYPE",
                            Some("`%s`/`%d` format `int`/`float`/`decimal`/`bool`/`string`".into()),
                        );
                    }
                }
            }
            other => {
                let t = self.check_expr(other);
                if !matches!(t, Ty::List(_) | Ty::FixedList(..) | Ty::Error) {
                    self.err_coded(
                        Self::expr_span(other),
                        format!("`String.format`'s values must be a list, found `{t}`"),
                        "E-FORMAT-ARGS-TYPE",
                        Some("pass the values as a list — `String.format(\"%s\", [x])`".into()),
                    );
                }
            }
        }
        // Compile-time gate for a LITERAL spec (the common case): only `%s`/`%d`/`%%` this slice, and
        // (against a list literal) an exact directive/value count. A runtime spec is validated at runtime.
        if let Expr::Str(parts, _) = &args[0] {
            if parts.iter().all(|p| matches!(p, StrPart::Literal(_))) {
                let spec: String = parts
                    .iter()
                    .map(|p| match p {
                        StrPart::Literal(s) => s.as_str(),
                        StrPart::Expr(_) => "",
                    })
                    .collect();
                match analyze_format_directives(&spec) {
                    Ok(info) => {
                        if let Expr::List(items, _) = &args[1] {
                            let len = items.len();
                            if info.positional && info.sequential {
                                self.err_coded(
                                    span,
                                    "`String.format` cannot mix positional (`%N$`) and sequential directives in one spec".to_string(),
                                    "E-FORMAT-MIXED-POSITIONAL",
                                    Some("use all-positional (`%1$s %2$s`) or all-sequential (`%s %s`), not both".into()),
                                );
                            } else if info.positional {
                                // Positional: reuse + reorder allowed, but every value must be referenced
                                // and no index may exceed the value count.
                                if info.max_arg > len {
                                    self.err_coded(
                                        span,
                                        format!("`String.format` references `%{}$` but was given only {len} value(s)", info.max_arg),
                                        "E-FORMAT-ARG-COUNT",
                                        Some("a positional index must be between 1 and the number of values".into()),
                                    );
                                } else if info.referenced.len() != len {
                                    let unused = (1..=len)
                                        .find(|k| !info.referenced.contains(k))
                                        .unwrap_or(len);
                                    self.err_coded(
                                        span,
                                        format!("`String.format` never references value {unused} of {len} (every value must be used)"),
                                        "E-FORMAT-ARG-COUNT",
                                        Some("reference every value with a `%N$` (reuse/reorder is allowed)".into()),
                                    );
                                }
                            } else if info.seq_count != len {
                                self.err_coded(
                                    span,
                                    format!(
                                        "`String.format` uses {} directive(s) but was given {len} value(s)",
                                        info.seq_count
                                    ),
                                    "E-FORMAT-ARG-COUNT",
                                    Some("give exactly one value per `%s`/`%d` (use `%%` for a literal `%`)".into()),
                                );
                            }
                        }
                    }
                    Err(bad) => {
                        self.err_coded(
                            span,
                            bad,
                            "E-FORMAT-UNSUPPORTED",
                            Some(
                                "this version supports `%s`/`%d`/`%f`/`%%`, scientific `%e`/`%E`, shortest-repr \
                                 `%g`/`%G`, integer-radix `%x`/`%X`/`%o`/`%b`, `%N$` positional, flags `-`/`0`/`+`, \
                                 width, precision on `%s` (truncate) and the float conversions. Precision on `%d` is \
                                 deliberately unsupported (PHP silently ignores it)"
                                    .into(),
                            ),
                        );
                    }
                }
            }
        }
        Ty::String
    }
}

/// Structured analysis of a LITERAL `String.format` spec — how many sequential directives, whether any
/// positional (`%N$`) directives appear, the highest explicit index, and the set of referenced indices.
/// Lets `check_string_format` validate the value count against BOTH the sequential and the positional
/// (reuse/reorder/no-unused) rules.
#[derive(Default)]
pub(in crate::checker) struct FormatSpecInfo {
    seq_count: usize,
    positional: bool,
    sequential: bool,
    max_arg: usize,
    referenced: std::collections::BTreeSet<usize>,
}

/// W3-5/DEC-199: scan a LITERAL `String.format` spec (`%%` is a literal `%`), returning its
/// [`FormatSpecInfo`] or the first unsupported-directive message for `E-FORMAT-UNSUPPORTED`. Uses the
/// SAME [`crate::native::parse_format_directive`] the runtime renderer uses, so the compile-time gate and
/// `text_format` accept exactly the same specs.
pub(in crate::checker) fn analyze_format_directives(spec: &str) -> Result<FormatSpecInfo, String> {
    let mut info = FormatSpecInfo::default();
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            continue;
        }
        let d = crate::native::parse_format_directive(&mut chars)?;
        match d.arg {
            Some(n) => {
                info.positional = true;
                info.max_arg = info.max_arg.max(n);
                info.referenced.insert(n);
            }
            None => {
                info.sequential = true;
                info.seq_count += 1;
            }
        }
    }
    Ok(info)
}
