//! M1 tree-walking evaluator. Walks the untyped AST against runtime `Value`s and
//! executes `main`. The type-checker (`crate::checker`) is the gate; this stage
//! assumes type-correct input and never panics on the faults types can't catch —
//! those become a runtime `Diagnostic`. See design spec `2026-06-15-m1-plan5-evaluator-design.md`.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::ast::{
    BinaryOp, ClassDecl, ClassMember, CtorParam, Expr, FunctionDecl, Item, LambdaBody, MatchArm,
    Modifier, Param, Pattern, Program, Stmt, StrPart, UnaryOp,
};
use crate::diagnostic::Diagnostic;
use crate::value::{ClosureData, EnumVal, Instance, Value};
use std::rc::Rc;

/// Non-local control flow threaded through `Result::Err` (EV-3). The runtime fault carries a
/// unified [`Diagnostic`] (stage `Runtime`); the tree-walker tracks no source position, so the
/// diagnostic has none (line 0) — the VM, which knows `Chunk.lines`, is the backend that locates
/// runtime faults.
enum Signal {
    Return(Value),
    /// `break;` — unwinds to the innermost loop, which catches it and stops (M-mut.3).
    Break,
    /// `continue;` — unwinds to the innermost loop, which catches it and starts the next iteration.
    Continue,
    /// `throw e;` — a checked exception, unwinding to the innermost enclosing `try` whose `catch`
    /// matches it, or out of `main` (uncaught) if none does (M-faults 2b). Distinct from `Runtime`:
    /// only a `Throw` is catchable; a `Runtime` fault (panic/index-OOB/…) passes straight through
    /// every `catch` — panics are uncatchable by design.
    Throw(Value),
    Runtime(Diagnostic),
}

type R<T> = Result<T, Signal>;

/// Sentinel fault body used to carry a `Signal::Throw` *across a higher-order-native boundary*
/// without changing the backend-shared `ClosureInvoker` signature (`Result<_, String>`). The
/// invoker stashes the thrown value in `Interp::pending_throw` and returns this string; the
/// `CallNative` site recognises it and rebuilds the `Throw` (M-faults 2b). The same trick is used
/// by the VM. The token is not a valid source identifier, so it can never collide with a real fault.
/// Single-sourced with the VM via [`crate::chunk::THROW_SENTINEL`].
const THROW_SENTINEL: &str = crate::chunk::THROW_SENTINEL;

/// The source line of a statement, for runtime trace frames (error-handling slice 1).
fn stmt_line(s: &Stmt) -> u32 {
    match s {
        Stmt::VarDecl { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::If { span, .. }
        | Stmt::For { span, .. }
        | Stmt::While { span, .. }
        | Stmt::CFor { span, .. }
        | Stmt::Throw { span, .. }
        | Stmt::Try { span, .. }
        | Stmt::Destructure { span, .. } => span.line,
        Stmt::Break(s)
        | Stmt::Continue(s)
        | Stmt::Block(_, s)
        | Stmt::Expr(_, s)
        | Stmt::Discard(_, s) => s.line,
    }
}

fn rt<T>(msg: impl Into<String>) -> R<T> {
    Err(Signal::Runtime(Diagnostic::runtime(msg)))
}

/// Flatten a runtime `Signal` to its message body for the higher-order-native callback boundary (a
/// [`crate::native::ClosureInvoker`] returns `Result<_, String>`, the backend-shared fault contract).
/// A `Return` escaping `call_closure` would be an interpreter bug — a closure's return value is
/// consumed inside the call, never surfaced — so it maps to a defensive internal-error string.
fn signal_msg(sig: Signal) -> String {
    match sig {
        Signal::Runtime(d) => d.message,
        Signal::Return(_) => "internal error: closure return escaped".to_string(),
        Signal::Break | Signal::Continue => "internal error: loop control escaped".to_string(),
        // A `Throw` is intercepted before this point at the native boundary (it becomes the
        // sentinel + `pending_throw`); reaching here would be an interpreter bug.
        Signal::Throw(_) => "internal error: throw escaped to native boundary".to_string(),
    }
}

/// The literal text of a fault intrinsic's string-literal message argument (M-faults 2a). The checker
/// guarantees it is a single `StrPart::Literal`; defaults to empty (e.g. a bare `assert(cond)`).
fn lit_msg(e: Option<&Expr>) -> String {
    if let Some(Expr::Str(parts, _)) = e {
        if let [crate::ast::StrPart::Literal(s)] = &parts[..] {
            return s.clone();
        }
    }
    String::new()
}

fn as_bool(v: &Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => rt(format!("expected bool, got {}", other.type_name())),
    }
}

/// The lexical block-scope stack of the *currently executing* call — a `Vec` of scopes
/// (innermost last), pushed/popped as the tree-walker enters and leaves blocks. No closures in
/// M1, so it captures no enclosing environment. NB despite the holding field being named `frame`,
/// this is **not** a call frame: it is the opposite concept from `vm::Frame`
/// (`{func, ip, slot_base}`, a reified call record). The tree-walker keeps its call records on the
/// native Rust stack, so the only per-call state it reifies is this scope chain.
struct CallScopes {
    scopes: Vec<HashMap<String, Value>>,
}

impl CallScopes {
    fn new() -> Self {
        CallScopes {
            scopes: vec![HashMap::new()],
        }
    }
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    fn declare(&mut self, name: &str, v: Value) {
        self.scopes
            .last_mut()
            .expect("scope stack always has a base scope")
            .insert(name.to_string(), v);
    }
    fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    /// Mutable view of an existing binding's slot (innermost scope that declares it). Used by
    /// index-assign so it can `Rc::make_mut` the container *in the slot* — copy-on-write stays
    /// correct (a genuinely shared `Rc` still copies), but a uniquely-owned container mutates in
    /// place instead of being cloned first, which is the difference between O(1) and O(n) per write.
    fn lookup_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
    }
    /// Overwrite an existing binding in the innermost scope that declares it (M-mut.1
    /// reassignment). Returns `false` if no scope holds `name` (defensive — the checker guarantees
    /// the binding exists and is `mutable`). Does NOT create a new binding.
    fn assign(&mut self, name: &str, v: Value) -> bool {
        for s in self.scopes.iter_mut().rev() {
            if let Some(slot) = s.get_mut(name) {
                *slot = v;
                return true;
            }
        }
        false
    }

    /// Snapshot every in-scope local as `(name, value)` for a value-dump (M-DX S3), inner scope
    /// shadowing outer, **sorted by name** for a deterministic dump (`HashMap` iteration order is
    /// nondeterministic). Clones the values (the dump is a side-channel; it must not disturb state).
    fn snapshot_locals(&self) -> Vec<(String, Value)> {
        let mut map: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
        for scope in &self.scopes {
            // outer → inner, so an inner binding overwrites an outer one of the same name
            for (name, value) in scope {
                map.insert(name.clone(), value.clone());
            }
        }
        map.into_iter().collect()
    }
}

pub struct Interp<'c> {
    /// Free-function overload sets (M-RT overloading): a name maps to one *or more* declarations.
    /// Length 1 in the common case; dynamic dispatch selects among >1 by the argument values.
    funcs: HashMap<String, Vec<FunctionDecl>>,
    classes: HashMap<String, ClassDecl>,
    /// Transitively-flattened interface set each class implements — the `instanceof` table, built
    /// once via [`crate::ast::class_implements`] and shared verbatim with the checker + VM so the
    /// runtime test never diverges (M-RT S2). Interfaces themselves are erased: there are no
    /// interface values, only this lookup.
    class_implements: std::collections::BTreeMap<String, Vec<String>>,
    /// Static class hierarchy for the reflection enumeration natives (`Core.Reflection.interfaces`/…),
    /// built once from the program and shared verbatim with the VM + transpiler so reflection is
    /// byte-identical (M-Reflect Tier-2).
    class_tables: crate::native::ClassTables,
    /// The fully-resolved method-dispatch table — `(class, name) -> (declaring_class, method)` — built
    /// once via [`crate::ast::class_method_origins`] and shared with the compiler's pre-flatten so a
    /// multi-parent / resolution-clause / renamed call resolves to the *same* body the VM dispatches
    /// to (M-RT S6b). Subsumes the parent-chain walk: `call_method` is now a single table lookup.
    method_origins: std::collections::BTreeMap<(String, String), (String, String)>,
    /// variant name -> (enum name, arity)
    variants: HashMap<String, (String, usize)>,
    /// Program-lifetime `static` field storage (M-mut.7), keyed by `(class, field)`. Seeded once at
    /// load from each static's literal-const initializer; read/written via `ClassName.field`.
    statics: HashMap<(String, String), Value>,
    /// Class constants (Feature A), keyed by `(class, NAME)` → inlined literal `Value`. Seeded once at
    /// load from the shared [`crate::ast::class_consts`] table (inheritance + traits already
    /// flattened), and read — before `statics` — on a `ClassName.NAME` access.
    consts: HashMap<(String, String), Value>,
    /// Expression field initializers (Feature B), keyed by class → ordered `(field, init_expr)` list
    /// (base-first across ancestors, declaration order). Seeded once at load from the shared
    /// [`crate::ast::field_initializers`]; evaluated per-instance in `construct` after promotion.
    field_inits: HashMap<String, Vec<(String, Expr)>>,
    /// The shared `name → slot` field layout per class (M-perf S1b), built once at load from
    /// [`crate::ast::class_field_layout`] — the *same* source the compiler/VM build their layouts
    /// from, so an interpreter-built instance and a VM-built instance of one class agree on slots.
    /// Consulted by `construct` (allocate + populate) and every field read/write.
    layouts: HashMap<String, std::rc::Rc<crate::value::ClassLayout>>,
    frame: CallScopes,
    this: Option<Value>,
    /// The **lexical** class of the currently-executing method/constructor body (M-RT super/parent) —
    /// the class that *declares* the running body, used to resolve a `parent` call (which is lexical,
    /// not keyed on the receiver's runtime class). `None` in a free function. Saved/restored by
    /// `run_call` alongside `this`.
    cur_class: Option<String>,
    /// `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow): true while the currently-executing free-function body is
    /// marked unchecked → int `+`/`-`/`*`/unary-`-` WRAP instead of faulting (via the `value::int_wrapping_*`
    /// kernels the VM also calls, so byte-identical). The single source of the wrap fact for the interp,
    /// set from the callee's attributes and saved/restored by `run_call` exactly like `cur_class`. Always
    /// `false` in a method/constructor (attributes are free-function-only).
    cur_unchecked: bool,
    /// Inheritance tables for `parent`/super resolution, cached once at load (mirrors `method_origins`).
    parent_parents: std::collections::BTreeMap<String, Vec<String>>,
    parent_mro: std::collections::BTreeMap<String, Vec<String>>,
    /// The program's import map (leaf or `as`-alias → dotted module), built once in `collect` —
    /// qualified native calls resolve through it first (`native::index_of_qualified`), so the
    /// DEC-277 aliased raw-native imports (`import Core.Native.Database as NativeDatabase;`)
    /// resolve, and a class name never leaf-captures a same-leaf native module.
    imports: HashMap<String, String>,
    out: String,
    /// Logical call stack for runtime stack traces (error-handling slice 1). A frame is pushed at each
    /// `run_call` entry (function name + current line) and popped **only on success** — an error path
    /// skips the pop, so at the top-level catch the stack still holds every active frame to snapshot.
    /// Names mirror the VM's compiled `Function.name` (`main`, `Class::method`, `Class::new`,
    /// `Class::name$set`) so `run`-traces are byte-identical to `runvm`-traces.
    trace_stack: Vec<crate::diagnostic::Frame>,
    /// Live call-frame depth, checked against [`crate::limits::MAX_CALL_DEPTH`] in `run_call`.
    /// Converts unbounded recursion into a clean `"stack overflow"` fault instead of a native
    /// stack abort — and uses the *same* limit as the VM, keeping the backends parity-identical.
    depth: usize,
    /// Carries a thrown value across a higher-order-native call boundary (M-faults 2b). When a
    /// closure passed to `List.map`/etc. throws, the invoker stows the value here and returns
    /// [`THROW_SENTINEL`]; the `CallNative` site rebuilds the `Throw` from it. `None` otherwise.
    pending_throw: Option<Value>,
    /// Green-thread coordination (M6 W4): the scheduler/id-allocator + finished-task results, mirroring
    /// the VM's `coop`. In the synchronous-degenerate path `spawn` runs eagerly and stores its result
    /// here (read by `join`); the cooperative driver shares one `Coop` across task interpreters.
    coop: std::rc::Rc<std::cell::RefCell<crate::green::exec::Coop>>,
    /// Green-thread cooperative cutover (S4.3): the suspension handle of the coroutine *this* task runs
    /// on. `Some` only while a task interpreter executes inside the cooperative driver
    /// ([`run_cooperative_interp`]); `None` on the ordinary synchronous run path, where `spawn` is eager
    /// and `recv`/`join` fault instead of blocking. The borrow is a closure-local of the driver's
    /// coroutine body (never escapes it), so it keeps `Interp` `'static`-movable into that `'static`
    /// closure — the deep-suspend mechanism the spike (`green::spike`) proved works without `unsafe`.
    coop_suspend: Option<&'c dyn crate::green::exec::Suspend>,
    /// The owning program AST, held only by a **cooperative** task interpreter so it can build a fresh
    /// child task interpreter when it evaluates a `spawn` (each green task = its own `Interp` over the
    /// same program, sharing only `coop`). `None` on the synchronous path (where `spawn` never defers).
    /// Read only by the native cooperative driver (`coop` module); legitimately dead when that module is
    /// absent (wasm, or a `--no-default-features` build without `green`).
    #[cfg_attr(
        not(all(feature = "green", not(target_arch = "wasm32"))),
        allow(dead_code)
    )]
    program: Option<std::rc::Rc<Program>>,
    /// An attached interactive-debugger session (M-DX S5, `phg debug`). `None` on every normal run —
    /// the hot `exec_stmt` path only consults it when it is `Some`. Interpreter-only + Dev-only;
    /// a pure side-channel (never affects stdout / the correctness spine). Not shared into a spawned
    /// green task's child interpreter (debugging is single-task in v1).
    debug: Option<crate::debug::DebugSession>,
}

/// Run a whole program: collect declarations, locate `main`, call it, and return
/// the captured stdout buffer (the Plan 6 CLI prints it to real stdout).
///
/// The tree-walker recurses on the native Rust stack, so deep recursion needs a generous stack for
/// the `run_call` depth guard (not a native abort) to be what stops it. That stack is supplied by
/// the caller — `cli::cmd_treewalk` runs the whole pipeline on a 256 MB worker thread — keeping this
/// function a plain recursive walk.
pub fn interpret(program: &Program) -> Result<String, Diagnostic> {
    interpret_main(program).map(|(out, _exit)| out)
}

/// Like [`interpret`], but also returns `main`'s exit code (Batch-1 B): the `int` it returns, or `0`
/// for a `void` `main` / a `main` that falls off the end. The CLI (`phg run`) maps this to the
/// process exit status; the stdout-only [`interpret`] wrapper preserves every existing caller and the
/// differential harness (which gates stdout identity).
pub fn interpret_main(program: &Program) -> Result<(String, i64), Diagnostic> {
    run_program_main(program, None)
}

/// Like [`interpret_main`] but with an attached interactive-debugger session (M-DX S5, `phg debug`).
/// Interpreter-only + Dev-only; a side-channel that never affects stdout or the parity spine.
pub fn interpret_debug(
    program: &Program,
    session: crate::debug::DebugSession,
) -> Result<(String, i64), Diagnostic> {
    run_program_main(program, Some(session))
}

fn run_program_main(
    program: &Program,
    debug: Option<crate::debug::DebugSession>,
) -> Result<(String, i64), Diagnostic> {
    let mut interp = Interp {
        funcs: HashMap::new(),
        classes: HashMap::new(),
        class_implements: std::collections::BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        method_origins: std::collections::BTreeMap::new(),
        variants: HashMap::new(),
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
        debug,
    };
    interp.collect(program);
    // Feature B-static: runtime static initializers run once, before `main`. A fault here surfaces
    // like any runtime fault (with the frames captured so far).
    if let Err(sig) = interp.eval_static_inits(program) {
        return Err(match sig {
            Signal::Runtime(e) => e.with_frames(interp.snapshot_frames()),
            Signal::Throw(v) => {
                Diagnostic::runtime(format!("uncaught exception `{}`", throw_what(&v)))
                    .with_frames(interp.snapshot_frames())
            }
            _ => Diagnostic::runtime("internal error: control escaped a static initializer"),
        });
    }
    // Batch-1 D: the entry is a top-level `function main` OR a class-static `main` method — the shared
    // `ast::entry_point` resolver picks the one (the checker's `E-MULTIPLE-MAIN` guarantees ≤1).
    let (entry_class, main) = match crate::ast::entry_for(program, crate::ast::EntryRole::Cli) {
        Some(e) => e,
        None => return Err(Diagnostic::runtime(
            "no entry point: running needs an `#[Entry]` function with a CLI signature (DEC-191). A library or web file \
                 still type-checks and transpiles — use `phg check` / `phg transpile`",
        )),
    };
    let names: Vec<String> = main.params.iter().map(|p| p.name.clone()).collect();
    // Batch-1 B: a one-parameter `main` receives the program argv (the same `List<string>` value
    // `Core.Process.args()` exposes); a zero-parameter `main` gets none. The checker's
    // `E-MAIN-SIGNATURE` guarantees the arity is 0 or 1, so this never under/over-supplies.
    let args = if names.is_empty() {
        vec![]
    } else {
        vec![crate::native::process_args_value()]
    };
    // A class-static entry has no receiver (`this = None`); the trace name mirrors the VM's
    // `Class::main` for a static method, or bare `main` for a top-level one.
    let call_name = match entry_class {
        Some(c) => format!("{c}::{}", main.name),
        None => main.name.clone(),
    };
    match interp.run_call(
        &call_name,
        &names,
        &main.body,
        args,
        None,
        None,
        attrs_unchecked(&main.attrs),
    ) {
        // `run_call` converts `main`'s `return n` into `Ok(Value::Int(n))` (and a fall-off-the-end
        // `void` `main` into `Ok(Value::Unit)`); `exit_code_of` maps both to the exit status.
        Ok(v) => Ok((interp.out, exit_code_of(&v))),
        // Defensive: a `Return` that escapes `run_call` uncaught carries the same exit value.
        Err(Signal::Return(v)) => Ok((interp.out, exit_code_of(&v))),
        // `Runtime.exit(code)` (DEC-238): the clean-exit sentinel is a NORMAL completion carrying
        // the chosen code on the Batch-1-B channel — output flushed, no trace.
        Err(Signal::Runtime(e)) if crate::chunk::exit_sentinel_code(&e.message).is_some() => {
            let code = crate::chunk::exit_sentinel_code(&e.message).expect("guarded");
            Ok((interp.out, code))
        }
        Err(Signal::Runtime(e)) => Err(e.with_frames(interp.snapshot_frames())),
        // An exception that escapes `main` uncaught (defensive — the checker's `E-UNCAUGHT-THROW`
        // guarantees `main` handles every throw, so this is unreachable for a checked program).
        Err(Signal::Throw(v)) => Err(Diagnostic::runtime(format!(
            "uncaught exception `{}`",
            throw_what(&v)
        ))
        .with_frames(interp.snapshot_frames())),
        // Checker-unreachable: `break`/`continue` are rejected outside a loop and caught by their
        // enclosing loop, so they never escape `main`'s body. Defensive (EV-7 parity).
        Err(Signal::Break | Signal::Continue) => {
            Err(Diagnostic::runtime("internal error: loop control escaped"))
        }
    }
}

/// The process exit code carried by `main`'s return value (Batch-1 B): the `int` it returned, or `0`
/// for anything else (a `void` `main` returns `Value::Unit`). The checker's `E-MAIN-SIGNATURE`
/// constrains `main`'s return to `void`/`int`, so the non-`Int` case is only the `void` path.
fn exit_code_of(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        _ => 0,
    }
}

/// The display name of a thrown value for an uncaught-exception message — its class for an
/// instance, else its type name (M-faults 2b).
fn throw_what(v: &Value) -> String {
    match v {
        Value::Instance(inst) => inst.class.to_string(),
        other => other.type_name().to_string(),
    }
}

/// The catchable type name(s) of a `catch` clause type (M-faults 2b): a single name for a class /
/// interface, or one per member for a union `catch (A | B e)`. The checker has already rejected any
/// non-`Error` member (`E-CATCH-TYPE`), so other `Type` shapes never reach here.
fn catch_type_names(ty: &crate::ast::Type) -> Vec<String> {
    match ty {
        crate::ast::Type::Named { name, .. } => vec![name.clone()],
        crate::ast::Type::Union(members, _) => members.iter().flat_map(catch_type_names).collect(),
        _ => Vec::new(),
    }
}

/// Call a single named top-level function with pre-built `args`, returning its value plus the
/// captured stdout. The serve runtime (M6 W3, `crate::serve`) uses this to invoke
/// `respond(bytes) -> bytes` once per request — the one entry the socket bridge needs. The
/// interpreter is the reference backend; `run` ≡ `runvm` (the differential harness) guarantees the
/// VM would compute identical bytes, so the spike does not need a VM `call_named` (deferred — the
/// VM has no return-value capture today). Mirrors [`interpret`] exactly, but enters an arbitrary
/// named function with caller-supplied arguments instead of an argument-less `main`.
pub fn call_named(
    program: &Program,
    name: &str,
    args: Vec<Value>,
) -> Result<(Value, String), Diagnostic> {
    let mut interp = Interp {
        funcs: HashMap::new(),
        classes: HashMap::new(),
        class_implements: std::collections::BTreeMap::new(),
        class_tables: crate::native::ClassTables::default(),
        method_origins: std::collections::BTreeMap::new(),
        variants: HashMap::new(),
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

// cohesion split (M-Decomp W4): stmt/expr/call/construct clusters.
mod call;
mod construct;
mod expr;
mod stmt;

// Cooperative green-thread driver (M6 W4 / S4.3) — native + `green` only (uses stackful coroutines,
// which corosensei cannot provide on wasm32; the wasm interpreter keeps the eager model).
#[cfg(all(feature = "green", not(target_arch = "wasm32")))]
mod coop;
#[cfg(all(feature = "green", not(target_arch = "wasm32")))]
pub use coop::run_cooperative_interp;

mod engine;
mod kernels;

use self::kernels::*;

#[cfg(test)]
mod tests;
