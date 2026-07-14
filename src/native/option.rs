//! `Core.Option<T>` combinators + `T?`↔`Option` conversions (Wave B slice B-2a, DEC-182).
//!
//! The `Option<T>` enum itself is compiler-injected (`cli::inject_core_modules`, `Core.Option` row) when a
//! program imports `Core.Option`; these are the natives that operate on it. Enums have no methods, so
//! the combinators are ordinary module natives reached UFCS-style (`opt.map(fn)` resolves to
//! `Option.map(opt, fn)` via the checker's `try_ufcs` first-param-unification path, exactly like
//! `List.map`). The higher-order ones (`map`/`andThen`/`filter`) call the closure via the
//! backend-supplied re-entrant [`ClosureInvoker`], so one body drives both the interpreter and the VM
//! (structural parity). They erase to gated `__phorj_option_*` PHP helpers (no PHP builtin analog).
//!
//! A runtime `Option` value is `Value::Enum` with `ty == "Option"` and variant `Some` (one payload) /
//! `None` (empty). The `ty` guard is safe: a user who declares their own `Option` shadows the injected
//! one (its prelude is skipped), so their values never reach these `Core.Option` natives.
use super::*;
use crate::types::Ty;
use crate::value::{EnumVal, Value};
use std::rc::Rc;

fn some(v: Value) -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "Option".into(),
        variant: "Some".into(),
        payload: vec![v],
    }))
}
fn none() -> Value {
    Value::Enum(Rc::new(EnumVal {
        ty: "Option".into(),
        variant: "None".into(),
        payload: vec![],
    }))
}

/// `Option.map(Option<T>, (T) -> U) -> Option<U>` — apply `f` to a `Some` payload, pass `None` through.
fn option_map(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(o), f] if o.ty.as_ref() == "Option" => match o.variant.as_ref() {
            "Some" => Ok(some(call(f, vec![o.payload[0].clone()])?)),
            _ => Ok(none()),
        },
        _ => Err("Option.map expects (Option<T>, (T) -> U)".into()),
    }
}

/// `Option.andThen(Option<T>, (T) -> Option<U>) -> Option<U>` — monadic bind (flatMap): `f` itself
/// returns an `Option`, so `Some(x)` becomes `f(x)` (not wrapped again) and `None` passes through.
fn option_and_then(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(o), f] if o.ty.as_ref() == "Option" => match o.variant.as_ref() {
            "Some" => call(f, vec![o.payload[0].clone()]),
            _ => Ok(none()),
        },
        _ => Err("Option.andThen expects (Option<T>, (T) -> Option<U>)".into()),
    }
}

/// `Option.filter(Option<T>, (T) -> bool) -> Option<T>` — keep a `Some` only if the predicate holds.
fn option_filter(args: &[Value], call: &mut ClosureInvoker) -> Result<Value, String> {
    match args {
        [Value::Enum(o), f] if o.ty.as_ref() == "Option" => match o.variant.as_ref() {
            "Some" => match call(f, vec![o.payload[0].clone()])? {
                Value::Bool(true) => Ok(Value::Enum(o.clone())),
                Value::Bool(false) => Ok(none()),
                other => Err(format!(
                    "Option.filter predicate must return bool, got {}",
                    other.type_name()
                )),
            },
            _ => Ok(none()),
        },
        _ => Err("Option.filter expects (Option<T>, (T) -> bool)".into()),
    }
}

/// `Option.getOrElse(Option<T>, T) -> T` — the `Some` payload, else the (eagerly evaluated) default.
/// Deliberately eager: the default is a plain value argument, always evaluated (unlike `??`'s lazy
/// RHS). A lazy/thunk form can be added later if wanted.
fn option_get_or_else(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Enum(o), default] if o.ty.as_ref() == "Option" => match o.variant.as_ref() {
            "Some" => Ok(o.payload[0].clone()),
            _ => Ok(default.clone()),
        },
        _ => Err("Option.getOrElse expects (Option<T>, T)".into()),
    }
}

/// `Option.ofNullable(T?) -> Option<T>` — lift a built-in nullable into the rich `Option`: `null` ⇒
/// `None`, any value ⇒ `Some(value)`. The explicit `T?`→`Option` bridge (no implicit coercion).
fn option_of_nullable(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Null] => Ok(none()),
        [v] => Ok(some(v.clone())),
        _ => Err("Option.ofNullable expects (T?)".into()),
    }
}

/// `Option.toNullable(Option<T>) -> T?` — the reverse bridge: `Some(x)` ⇒ `x`, `None` ⇒ `null`.
fn option_to_nullable(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Enum(o)] if o.ty.as_ref() == "Option" => match o.variant.as_ref() {
            "Some" => Ok(o.payload[0].clone()),
            _ => Ok(Value::Null),
        },
        _ => Err("Option.toNullable expects (Option<T>)".into()),
    }
}

/// The `Core.Option` registry entries. `T`/`U` are inferred at the call site by the generic-native
/// path and erased before any backend (M-RT S7b) — identical discipline to `Core.List`'s
/// `map`/`filter`. The combinators erase to gated `__phorj_option_*` helpers (set in `transpile/call.rs`,
/// emitted in `transpile/program.rs`) since PHP has no builtin over the injected `Some`/`None` classes.
pub(crate) fn option_natives() -> Vec<NativeFn> {
    let t = || Ty::Param("T".into());
    let u = || Ty::Param("U".into());
    let opt = |e: Ty| Ty::Named("Option".into(), vec![e]);
    vec![
        NativeFn {
            module: "Core.Option",
            name: "map",
            params: vec![opt(t()), Ty::Function(vec![t()], Box::new(u()), Vec::new())],
            ret: opt(u()),
            pure: true,
            eval: NativeEval::HigherOrder(option_map),
            php: |a| format!("__phorj_option_map({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Option",
            name: "andThen",
            params: vec![
                opt(t()),
                Ty::Function(vec![t()], Box::new(opt(u())), Vec::new()),
            ],
            ret: opt(u()),
            pure: true,
            eval: NativeEval::HigherOrder(option_and_then),
            php: |a| format!("__phorj_option_and_then({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Option",
            name: "filter",
            params: vec![
                opt(t()),
                Ty::Function(vec![t()], Box::new(Ty::Bool), Vec::new()),
            ],
            ret: opt(t()),
            pure: true,
            eval: NativeEval::HigherOrder(option_filter),
            php: |a| format!("__phorj_option_filter({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Option",
            name: "getOrElse",
            params: vec![opt(t()), t()],
            ret: t(),
            pure: true,
            eval: NativeEval::Pure(option_get_or_else),
            php: |a| format!("__phorj_option_get_or_else({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Option",
            name: "ofNullable",
            params: vec![Ty::Optional(Box::new(t()))],
            ret: opt(t()),
            pure: true,
            eval: NativeEval::Pure(option_of_nullable),
            php: |a| format!("__phorj_option_of_nullable({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Option",
            name: "toNullable",
            params: vec![opt(t())],
            ret: Ty::Optional(Box::new(t())),
            pure: true,
            eval: NativeEval::Pure(option_to_nullable),
            php: |a| format!("__phorj_option_to_nullable({})", parg(a, 0)),
        },
    ]
}

#[cfg(test)]
#[path = "option_tests.rs"]
mod tests;
