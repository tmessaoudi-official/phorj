//! DEC-239 pipe `|>` checker tests: contextual pipe-lambda typing, the stranded-lambda and
//! void-piping rejections, and the loud compile-time arity/void divergences recorded as
//! phorj-better vs PHP (which defers both to runtime).

use super::support::*;

/// Type-check `src` with pipes lowered first — the same `lower_pipes` → `check` order the CLI
/// pipeline runs (`errors_of` checks the RAW tree, where pipes only hit the graceful LSP arm).
fn pipe_errors_of(src: &str) -> Vec<Diagnostic> {
    match check(&lower_pipes(prog(src))) {
        Ok(_warnings) => Vec::new(),
        Err(e) => e,
    }
}

#[test]
fn contextual_pipe_lambda_types_from_the_piped_value() {
    // `v` takes `int` from the piped `5`; the body's arithmetic and the `int` binding both check.
    let errs =
        pipe_errors_of("function main(): void { int r = 5 |> (v => v * 2 + 1); discard r; }");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn contextual_pipe_lambda_body_errors_use_the_inferred_type() {
    // `v` is `string` (from the piped literal) — `v * 2` must be a loud cross-type error.
    let errs =
        pipe_errors_of("function main(): void { int r = \"s\" |> (v => v * 2); discard r; }");
    assert!(!errs.is_empty(), "string * int must not check");
}

#[test]
fn stranded_pipe_lambda_is_rejected_with_the_pipe_specific_code() {
    // `|>` binds looser than `+`, so the `+ 1` applies to the LAMBDA (uniform RHS grammar) —
    // stranding it without a pipe context. E-PIPE-LAMBDA-CONTEXT, never the generic `var` error.
    let errs =
        pipe_errors_of("function main(): void { int r = 5 |> (v => v * 3) + 1; discard r; }");
    assert!(
        errs.iter().any(|e| e.code == Some("E-PIPE-LAMBDA-CONTEXT")),
        "{errs:?}"
    );
}

#[test]
fn piping_void_into_a_pipe_lambda_is_a_capture_error() {
    let errs = pipe_errors_of(
        "function sink(int x): void {} \
         function main(): void { int r = 5 |> sink |> (v => v); discard r; }",
    );
    assert!(
        errs.iter().any(|e| e.code == Some("E-VOID-CAPTURE")),
        "{errs:?}"
    );
}

#[test]
fn piping_void_mid_chain_into_a_function_is_a_loud_error() {
    // Probe (DEC-239 recorded divergence): PHP coerces void→null and pipes garbage; phorj rejects.
    let errs = pipe_errors_of(
        "function sink(int x): void {} \
         function inc(int x): int { return x + 1; } \
         function main(): void { int r = 5 |> sink |> inc; discard r; }",
    );
    assert!(!errs.is_empty(), "void must not flow through a pipe");
}

#[test]
fn piping_into_a_two_param_function_is_a_loud_arity_error() {
    // Probe A (DEC-239): single-arg application enforced at COMPILE time (PHP: runtime TypeError).
    let errs = pipe_errors_of(
        "function add(int a, int b): int { return a + b; } \
         function main(): void { int r = 5 |> add; discard r; }",
    );
    assert!(
        !errs.is_empty(),
        "2-param callee must not accept one piped arg"
    );
}
