//! Interpreter — the EXTERNAL call entry points.
//!
//! Three functions that share one concern: building a FRESH [`Interp`] over a program and invoking
//! something in it from outside the interpreter. They are split out of `mod.rs` (Invariant 13,
//! M-Decomp) because they form a cohesive unit, not because of a line count: everything here is a
//! way in from the serve runtime and the test harness, and nothing here participates in evaluation.
//!
//! The freshness is load-bearing, not incidental — see [`call_closure_in`] for why a reused
//! interpreter would silently change per-request semantics on a leg the byte-identity differential
//! cannot see.

use super::{attrs_unchecked, throw_what, CallScopes, Diagnostic, Interp, Program, Signal, Value};
use std::collections::HashMap;

/// Build a fresh [`Interp`] over `program`, collected and ready to call into.
///
/// Extracted so [`call_named`] and [`call_closure_in`] cannot drift: both are per-request serve
/// entry points, and a field that one initializes differently from the other is a backend divergence
/// that only shows up under `phg serve` — where the byte-identity differential does not reach
/// (Invariant 14 quarantines serve).
fn fresh_interp(program: &Program) -> Interp<'_> {
    let mut interp = Interp {
        funcs: HashMap::new(),
        classes: HashMap::new(),
        class_implements: std::collections::BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        method_origins: std::collections::BTreeMap::new(),
        variants: HashMap::new(),
        enum_variants: HashMap::new(),
        enum_backing: HashMap::new(),
        statics: HashMap::new(),
        consts: HashMap::new(),
        field_inits: HashMap::new(),
        layouts: HashMap::new(),
        frame: CallScopes::new(),
        this: None,
        cur_class: None,
        cur_unchecked: false,
        parent_parents: std::collections::BTreeMap::new(),
        parent_mro: std::collections::BTreeMap::new(),
        imports: HashMap::new(),
        out: String::new(),
        trace_stack: Vec::new(),
        depth: 0,
        pending_throw: None,
        coop: std::rc::Rc::new(std::cell::RefCell::new(crate::green::exec::Coop::new())),
        coop_suspend: None,
        program: None,
        debug: None,
    };
    interp.collect(program);
    interp
}

/// Invoke a first-class closure VALUE over `program`, returning its result plus the captured stdout.
///
/// The serve runtime's web path (DEC-331 D5) uses this once per request: the handler is a closure
/// the web entry constructed, not a named function, so [`call_named`] cannot reach it.
///
/// **A FRESH interpreter per call, exactly like [`call_named`].** This is what makes the two halves
/// of the ruled per-request semantics true at once: the closure's CAPTURES persist (they live in the
/// `Rc<ClosureData>`, which outlives any one interpreter), while program STATICS re-seed per request
/// (they live in the interpreter, which does not). Reusing one interpreter across requests would
/// silently flip the second half — and only on this leg, which the differential cannot see because
/// serve is Invariant-14 quarantined.
pub fn call_closure_in(
    program: &Program,
    closure: &Value,
    args: Vec<Value>,
) -> Result<(Value, String), Diagnostic> {
    let cd = match closure {
        Value::Closure(cd) => cd.clone(),
        v => {
            return Err(Diagnostic::runtime(format!(
                "cannot call {} as a function",
                v.type_name()
            )))
        }
    };
    let mut interp = fresh_interp(program);
    match interp.call_closure(cd, args) {
        Ok(v) => Ok((v, interp.out)),
        Err(Signal::Return(v)) => Ok((v, interp.out)),
        Err(Signal::Runtime(e)) => Err(e.with_frames(interp.snapshot_frames())),
        Err(Signal::Throw(v)) => Err(Diagnostic::runtime(format!(
            "uncaught exception `{}`",
            throw_what(&v)
        ))
        .with_frames(interp.snapshot_frames())),
        Err(Signal::Break | Signal::Continue) => {
            Err(Diagnostic::runtime("internal error: loop control escaped"))
        }
    }
}

/// Call a single named top-level function with pre-built `args`, returning its value plus the
/// captured stdout. The serve runtime (M6 W3, `crate::serve`) uses this to invoke
/// the registered web handler once per request — the one call the socket bridge needs. The
/// interpreter is the reference backend; interp ≡ VM (the differential harness) guarantees the
/// VM would compute identical bytes, so the spike does not need a VM `call_named` (deferred — the
/// VM has no return-value capture today). Mirrors [`interpret`] exactly, but enters an arbitrary
/// named function with caller-supplied arguments instead of an argument-less `main`.
pub fn call_named(
    program: &Program,
    name: &str,
    args: Vec<Value>,
) -> Result<(Value, String), Diagnostic> {
    let mut interp = fresh_interp(program);
    let set = match interp.funcs.get(name) {
        Some(v) => v.clone(),
        None => return Err(Diagnostic::runtime(format!("no `{name}` function"))),
    };
    // M-RT overloading: select the overload by the supplied argument values (single-overload sets
    // return directly). A selection fault surfaces as a runtime diagnostic.
    let f = match interp.select_free_overload(name, &set, &args) {
        Ok(f) => f,
        Err(Signal::Runtime(d)) => return Err(d),
        Err(_) => return Err(Diagnostic::runtime(format!("cannot resolve `{name}`"))),
    };
    if f.params.len() != args.len() {
        return Err(Diagnostic::runtime(format!(
            "`{name}` expects {} argument(s), got {}",
            f.params.len(),
            args.len()
        )));
    }
    let names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
    match interp.run_call(
        &f.name,
        &names,
        &f.body,
        args,
        None,
        None,
        attrs_unchecked(&f.attrs),
    ) {
        Ok(v) => Ok((v, interp.out)),
        Err(Signal::Return(v)) => Ok((v, interp.out)),
        // NOTE: the clean-exit sentinel is NOT intercepted here — this is the per-call entry the
        // serve runtime uses, where an `exit` inside a handler surfaces as an error (a 500), never
        // a silent worker death. Whole-program exit is intercepted in `run_program_main`.
        Err(Signal::Runtime(e)) => Err(e.with_frames(interp.snapshot_frames())),
        Err(Signal::Throw(v)) => Err(Diagnostic::runtime(format!(
            "uncaught exception `{}`",
            throw_what(&v)
        ))
        .with_frames(interp.snapshot_frames())),
        Err(Signal::Break | Signal::Continue) => {
            Err(Diagnostic::runtime("internal error: loop control escaped"))
        }
    }
}
