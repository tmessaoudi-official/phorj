//! `Core.Result<T, E>` combinators (Wave B slice B-2b, DEC-185).
//!
//! The `Result<T, E>` enum itself is compiler-injected (`cli::inject_core_modules`, `Core.Result` row) when a
//! program imports `Core.Result`; these are the natives that operate on it. Enums have no methods, so the
//! combinators are ordinary module natives reached UFCS-style (`res.map(fn)` resolves to
//! `Result.map(res, fn)` via the checker's `try_ufcs` first-param-unification path, exactly like
//! `List.map`/`Option.map`). The higher-order ones (`map`/`mapErr`/`andThen`/`orElse`) call the closure
//! via the backend-supplied re-entrant [`ClosureInvoker`], so one body drives both the interpreter and
//! the VM (structural parity). They erase to gated `__phorj_result_*` PHP helpers (no PHP builtin analog).
//!
//! A runtime `Result` value is `Value::Enum` with `ty == "Result"` and variant `Success` (one payload:
//! the value) / `Failure` (one payload: the error). The `ty` guard is safe: a user who declares their own
//! `Result` shadows the injected one (its prelude is skipped), so their values never reach these natives.
//!
//! Set (DEC-185, developer-ruled "all"): `map` · `mapErr` · `andThen` · `getOrElse` · `toOption` ·
//! `orElse` · `isSuccess` · `isFailure`. `filter` is deliberately EXCLUDED — there is no error value to
//! synthesize on a `false` predicate (Rust omits `Result::filter` for the same reason).
use super::*;
use crate::types::Ty;
use crate::value::{EnumVal, Payload, Value};
use std::rc::Rc;

fn success(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "Result".into(),
        variant: "Success".into(),
        payload: Payload::One(v),
    }))
}
fn failure(e: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "Result".into(),
        variant: "Failure".into(),
        payload: Payload::One(e),
    }))
}
/// A `Core.Option` value — used only by `toOption` (the Result→Option bridge). Requires the program to
/// also `import Core.Option;` (the return type `Option<T>` is otherwise unknown, and the PHP `Some`/`None`
/// classes the transpiled helper references are emitted only by the Option injection).
fn some(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "Option".into(),
        variant: "Some".into(),
        payload: Payload::One(v),
    }))
}
fn none() -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "Option".into(),
        variant: "None".into(),
        payload: Payload::Zero,
    }))
}

/// `Result.map(Result<T,E>, (T) -> U) -> Result<U,E>` — apply `f` to a `Success` payload, pass `Failure`
/// through unchanged (error type `E` is preserved).
fn result_map(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(r), f] if r.ty.as_ref() == "Result" => match r.variant.as_ref() {
            "Success" => Ok(success(call(f, vec![r.payload[0].clone()])?)),
            _ => Ok(Value::Enum(r.clone())),
        },
        _ => Err("Result.map expects (Result<T,E>, (T) -> U)".into()),
    }
}

/// `Result.mapErr(Result<T,E>, (E) -> F) -> Result<T,F>` — apply `f` to a `Failure` payload (remapping the
/// error type `E` to `F`), pass `Success` through unchanged.
fn result_map_err(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(r), f] if r.ty.as_ref() == "Result" => match r.variant.as_ref() {
            "Failure" => Ok(failure(call(f, vec![r.payload[0].clone()])?)),
            _ => Ok(Value::Enum(r.clone())),
        },
        _ => Err("Result.mapErr expects (Result<T,E>, (E) -> F)".into()),
    }
}

/// `Result.andThen(Result<T,E>, (T) -> Result<U,E>) -> Result<U,E>` — monadic bind (flatMap) on the
/// success arm: `f` itself returns a `Result`, so `Success(x)` becomes `f(x)` (not wrapped again) and a
/// `Failure` passes through. Threads the error type `E` through the callback's own `Result`.
fn result_and_then(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(r), f] if r.ty.as_ref() == "Result" => match r.variant.as_ref() {
            "Success" => call(f, vec![r.payload[0].clone()]),
            _ => Ok(Value::Enum(r.clone())),
        },
        _ => Err("Result.andThen expects (Result<T,E>, (T) -> Result<U,E>)".into()),
    }
}

/// `Result.getOrElse(Result<T,E>, T) -> T` — the `Success` payload, else the (eagerly evaluated) default.
/// Deliberately eager: the default is a plain value argument, always evaluated. Mirrors `Option.getOrElse`.
fn result_get_or_else(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Enum(r), default] if r.ty.as_ref() == "Result" => match r.variant.as_ref() {
            "Success" => Ok(r.payload[0].clone()),
            _ => Ok(default.clone()),
        },
        _ => Err("Result.getOrElse expects (Result<T,E>, T)".into()),
    }
}

/// `Result.orElse(Result<T,E>, (E) -> Result<T,F>) -> Result<T,F>` — monadic bind on the error arm
/// (recovery): a `Failure(e)` becomes `f(e)` (which itself returns a `Result`), a `Success` passes through.
/// The Rust `Result::or_else` analog.
fn result_or_else(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(r), f] if r.ty.as_ref() == "Result" => match r.variant.as_ref() {
            "Failure" => call(f, vec![r.payload[0].clone()]),
            _ => Ok(Value::Enum(r.clone())),
        },
        _ => Err("Result.orElse expects (Result<T,E>, (E) -> Result<T,F>)".into()),
    }
}

/// `Result.toOption(Result<T,E>) -> Option<T>` — the Result→Option bridge: `Success(x)` ⇒ `Some(x)`,
/// `Failure(_)` ⇒ `None` (the error is dropped). Symmetric with `Option.toNullable`. Requires the program
/// to `import Core.Option;` too (for the `Option` type + its `Some`/`None` PHP classes).
fn result_to_option(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Enum(r)] if r.ty.as_ref() == "Result" => match r.variant.as_ref() {
            "Success" => Ok(some(r.payload[0].clone())),
            _ => Ok(none()),
        },
        _ => Err("Result.toOption expects (Result<T,E>)".into()),
    }
}

/// `Result.isSuccess(Result<T,E>) -> bool` — `true` for a `Success`, `false` for a `Failure`.
fn result_is_success(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Enum(r)] if r.ty.as_ref() == "Result" => {
            Ok(Value::Bool(r.variant.as_ref() == "Success"))
        }
        _ => Err("Result.isSuccess expects (Result<T,E>)".into()),
    }
}

/// `Result.isFailure(Result<T,E>) -> bool` — `true` for a `Failure`, `false` for a `Success`.
fn result_is_failure(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Enum(r)] if r.ty.as_ref() == "Result" => {
            Ok(Value::Bool(r.variant.as_ref() == "Failure"))
        }
        _ => Err("Result.isFailure expects (Result<T,E>)".into()),
    }
}

/// The `Core.Result` registry entries. `T`/`U`/`E`/`F` are inferred at the call site by the generic-native
/// path and erased before any backend (M-RT S7b) — identical discipline to `Core.Option`. `E` is threaded
/// through as a passenger (preserved by `map`, bound into the callback's `Result` by `andThen`); `mapErr`/
/// `orElse` introduce a fresh error type `F`. The combinators erase to gated `__phorj_result_*` helpers
/// (set in `transpile/call.rs`, emitted in `transpile/program.rs`) over the injected `Success`/`Failure`
/// PHP classes (`toOption` also references the Option injection's `Some`/`None`).
pub(crate) fn result_natives() -> Vec<NativeFn> {
    let t = || Ty::Param("T".into());
    let u = || Ty::Param("U".into());
    let e = || Ty::Param("E".into());
    let f = || Ty::Param("F".into());
    let res = |a: Ty, b: Ty| Ty::Named("Result".into(), vec![a, b]);
    let opt = |a: Ty| Ty::Named("Option".into(), vec![a]);
    vec![
        NativeFn {
            module: "Core.Result",
            name: "map",
            params: vec![
                res(t(), e()),
                Ty::Function(vec![t()], Box::new(u()), Vec::new()),
            ],
            ret: res(u(), e()),
            pure: true,
            eval: NativeEval::HigherOrder(result_map),
            lift_from: &[],
            php: |a| format!("__phorj_result_map({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Result",
            name: "mapErr",
            params: vec![
                res(t(), e()),
                Ty::Function(vec![e()], Box::new(f()), Vec::new()),
            ],
            ret: res(t(), f()),
            pure: true,
            eval: NativeEval::HigherOrder(result_map_err),
            lift_from: &[],
            php: |a| format!("__phorj_result_map_err({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Result",
            name: "andThen",
            params: vec![
                res(t(), e()),
                Ty::Function(vec![t()], Box::new(res(u(), e())), Vec::new()),
            ],
            ret: res(u(), e()),
            pure: true,
            eval: NativeEval::HigherOrder(result_and_then),
            lift_from: &[],
            php: |a| format!("__phorj_result_and_then({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Result",
            name: "getOrElse",
            params: vec![res(t(), e()), t()],
            ret: t(),
            pure: true,
            eval: NativeEval::Pure(result_get_or_else),
            lift_from: &[],
            php: |a| format!("__phorj_result_get_or_else({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Result",
            name: "orElse",
            params: vec![
                res(t(), e()),
                Ty::Function(vec![e()], Box::new(res(t(), f())), Vec::new()),
            ],
            ret: res(t(), f()),
            pure: true,
            eval: NativeEval::HigherOrder(result_or_else),
            lift_from: &[],
            php: |a| format!("__phorj_result_or_else({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Result",
            name: "toOption",
            params: vec![res(t(), e())],
            ret: opt(t()),
            pure: true,
            eval: NativeEval::Pure(result_to_option),
            lift_from: &[],
            php: |a| format!("__phorj_result_to_option({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Result",
            name: "isSuccess",
            params: vec![res(t(), e())],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(result_is_success),
            lift_from: &[],
            php: |a| format!("({} instanceof Success)", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Result",
            name: "isFailure",
            params: vec![res(t(), e())],
            ret: Ty::Bool,
            pure: true,
            eval: NativeEval::Pure(result_is_failure),
            lift_from: &[],
            php: |a| format!("({} instanceof Failure)", parg(a, 0)),
        },
    ]
}

#[cfg(test)]
#[path = "result_tests.rs"]
mod tests;
