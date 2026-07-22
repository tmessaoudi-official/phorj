//! Bytecode chunk + instruction set for the M2 VM.
//! See docs/specs/2026-06-15-m2-bytecode-vm-design.md (§4, §5).
//! P2 scope: full M1 expression/statement surface for `main` (see
//! the M2 compiler/VM design (docs/specs archive)). P4a adds single-payload enums + `match`;
//! P4b adds classes (construction + constructor promotion + field reads); P4c adds methods + `this`.
//! Reuses `value::Value` directly for scalars, lists, *and* enums/instances — the VM mirrors the
//! interpreter's value-semantics object model (clone-on-use, no heap; decision P4-1). An arena is
//! a deferred, bench-gated perf milestone, not a correctness requirement.

use crate::value::Value;
use std::collections::{BTreeMap, HashMap};

/// Hashable identity of an internable constant. `Value` can't derive `Hash`/`Eq` (it holds `f64`
/// and composite types), so the constant pool dedups via this projection: floats by their bit
/// pattern (`to_bits`), strings by content, the rest by value. Composite constants (`List`,
/// instances, enums) are never interned — they have no key and always get a fresh slot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConstKey {
    Int(i64),
    /// `f64::to_bits` — so `+0.0`/`-0.0` and distinct `NaN`s key apart, and equal floats dedup.
    Float(u64),
    Bool(bool),
    Str(crate::phstr::PhStr),
    Unit,
}

impl ConstKey {
    /// The dedup key for a scalar constant, or `None` for a composite (never interned).
    fn of(v: &Value) -> Option<ConstKey> {
        Some(match v {
            Value::Int(n) => ConstKey::Int(*n),
            Value::Float(x) => ConstKey::Float(x.to_bits()),
            Value::Bool(b) => ConstKey::Bool(*b),
            Value::Str(s) => ConstKey::Str(s.clone()),
            Value::Unit => ConstKey::Unit,
            _ => return None,
        })
    }
}

/// Which fixed runtime fault an [`Op::Fault`] raises. The message lives here (single-sourced) so the
/// VM and the tree-walking interpreter stay byte-identical — the `agree_err` oracle classifies
/// faults by body substring (M3 S2.5).
// `Panic`/`Assert` carry a compile-time-literal message, so `FaultMsg` is no longer `Copy` (M-faults
// 2a). `Op` is already `Clone` (not `Copy`), so this costs nothing beyond a rare String clone when a
// fault op is dispatched (a fault is terminal — it happens at most once per run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultMsg {
    /// Non-exhaustive `match` fall-through (checker-unreachable backstop).
    NonExhaustiveMatch,
    /// `opt!` force-unwrap of a `null` value.
    ForceUnwrapNull,
    /// `panic("msg")` — an explicit programmer abort. The message is a compile-time string literal.
    Panic(String),
    /// `todo()` — an unimplemented path.
    Todo,
    /// `unreachable()` — a path the programmer asserts can't happen.
    Unreachable,
    /// A failed `assert(cond[, "msg"])`. The (literal) message is empty when none was given.
    Assert(String),
}

impl FaultMsg {
    /// The fault body. Must match the interpreter's `rt(..)` text for each fault so both backends
    /// classify to the same `FaultKind` (and, here, stay byte-identical — single-sourced).
    pub fn message(&self) -> String {
        match self {
            FaultMsg::NonExhaustiveMatch => "non-exhaustive match at runtime".to_string(),
            FaultMsg::ForceUnwrapNull => "force-unwrap of null".to_string(),
            FaultMsg::Panic(m) => format!("panic: {m}"),
            FaultMsg::Todo => "todo: not yet implemented".to_string(),
            FaultMsg::Unreachable => "entered unreachable code".to_string(),
            FaultMsg::Assert(m) if m.is_empty() => "assertion failed".to_string(),
            FaultMsg::Assert(m) => format!("assertion failed: {m}"),
        }
    }
}

mod op;
mod validate;
pub use self::op::Op;

#[cfg(test)]
mod tests;

/// Fault body used to carry a [`Op::Throw`]'s value across the `Result<_, String>` fault channel
/// (and the higher-order-native `ClosureInvoker` boundary) without a dedicated error enum: the
/// thrown `Value` is stashed in `Vm::pending_throw` / `Interp::pending_throw` and this token is
/// returned; the run loop / `CallNative` site recognises it and rebuilds the throw (M-faults 2b).
/// Not a valid source identifier, so it can never collide with a real fault message.
pub const THROW_SENTINEL: &str = "__phorj_throw__";

/// `Runtime.exit(code)` (DEC-238 slice 2) — the CLEAN-termination sentinel. The native returns
/// `Err("__phorj_exit__:<code>")`; each backend's TOP-LEVEL run loop intercepts it and converts to
/// a normal `(stdout-so-far, code)` completion riding the existing Batch-1-B exit-code channel —
/// no trace, no error framing, output flushed. Mid-frame propagation reuses the ordinary error
/// path, so `finally` blocks between the exit call and `main` do NOT run (exit is immediate, the
/// PHP `exit()` semantic — documented).
pub const EXIT_SENTINEL_PREFIX: &str = "__phorj_exit__:";

/// Parse an error message as the exit sentinel: `Some(code)` iff it IS a clean exit.
pub fn exit_sentinel_code(msg: &str) -> Option<i64> {
    msg.strip_prefix(EXIT_SENTINEL_PREFIX)?.parse().ok()
}

/// A unit of compiled bytecode: instructions, a constant pool, and a per-instruction
/// source-line table (for runtime-error reporting).
#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub code: Vec<Op>,
    pub consts: Vec<Value>,
    pub lines: Vec<u32>,
    /// Build-time interning table: scalar constant → its pool index, so `add_const` dedups
    /// repeated literals instead of growing the pool per occurrence. Not part of the emitted
    /// bytecode — it only steers `add_const` while a `Chunk` is under construction.
    const_index: HashMap<ConstKey, usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a constant, returning its pool index. A repeated scalar (same int / bit-equal float /
    /// equal string / bool / unit) reuses its existing slot, so the pool grows with *distinct*
    /// values, not occurrences — keeping the constant pool (and the future P4/P5 GC root set that
    /// scans it) lean. Composite constants have no key and always get a fresh slot.
    pub fn add_const(&mut self, v: Value) -> usize {
        if let Some(key) = ConstKey::of(&v) {
            if let Some(&idx) = self.const_index.get(&key) {
                return idx;
            }
            let idx = self.consts.len();
            self.const_index.insert(key, idx);
            self.consts.push(v);
            idx
        } else {
            self.consts.push(v);
            self.consts.len() - 1
        }
    }

    /// Append an instruction tagged with its source line.
    pub fn emit(&mut self, op: Op, line: u32) {
        self.code.push(op);
        self.lines.push(line);
    }
}

/// A compiled function: name, parameter count, and its own bytecode chunk. Each function
/// owns its chunk so its jump targets and constant pool are self-contained (decision P3-1).
/// `n_captures` is the number of captured values that `Op::MakeClosure` pops from the enclosing
/// frame before constructing the closure; it is 0 for named free functions, constructors, and
/// methods (they are never constructed via `MakeClosure` with captures).
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    /// Number of captured values this closure pops off the enclosing stack at construction time
    /// (`Op::MakeClosure`). Always 0 for named functions, constructors, and methods.
    pub n_captures: usize,
    /// `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow): int `+`/`-`/`*`/unary-`-` in this function WRAP on overflow
    /// (call the `value::int_wrapping_*` kernels) instead of faulting. Single source of the wrap fact —
    /// interp, VM (`exec_op`), and JIT all read it; div/rem stay checked. A `true` function is
    /// `E-TRANSPILE-UNCHECKED` (no PHP analog, §14 LADDER) and quarantined from the differential's PHP leg.
    /// `false` for every function without the attribute (the common case).
    pub unchecked: bool,
    /// W7 (JIT union params): `true` per frame slot whose DECLARED param type is a scalar-only
    /// union (members ⊆ {`int`, `float`, `bool`, `string`} — the checker already validated the
    /// union itself). Slot-aligned: a method's slot 0 (`this`) is `false`; lambdas record none
    /// (all-`false`). Compiler-stamped checker fact, read ONLY by the unboxed JIT to seed such
    /// params as tagged `Dyn` cells — without the seed, a mid-call-chain method that both takes
    /// and consumes the union param deadlocks the JIT's call-sig fixpoint (its return kind can
    /// never land, so the later chain sites that would prove the union are never reached).
    /// Interp/VM/transpiler ignore it. Empty ⇒ no union params (the common case).
    pub dyn_params: Vec<bool>,
    pub chunk: Chunk,
}

/// A static descriptor for one enum variant: its enum type, variant name, and payload arity.
/// Built once in the compiler pre-pass (every variant of every declared enum) and indexed by
/// `Op::MakeEnum`/`Op::MatchTag` — the enum analogue of the constant pool (decision P4-2).
#[derive(Debug, Clone)]
pub struct EnumDesc {
    pub ty: std::rc::Rc<str>,
    pub variant: std::rc::Rc<str>,
    pub arity: usize,
    /// DEC-302 backed enum: the variant's scalar backing value (`int`/`string`), `None` for a plain
    /// enum. Read by `Op::EnumValue` (`s.value`) and `Op::EnumFrom` (`from`/`tryFrom`); single-sourced
    /// from the AST literal via `const_literal`, byte-identical to the interpreter's `enum_backing`.
    pub backing: Option<crate::value::Value>,
}

/// A static descriptor for one class: its name and the ordered list of promoted-field names a
/// constructor populates. Built once in the compiler pre-pass and indexed by `Op::MakeInstance`
/// (decision P4-2/P4-4). Explicit (non-promoted) `Field` members are *not* listed here — like the
/// interpreter, construction populates only promoted ctor params; reading an explicit field is a
/// runtime `no field` fault.
#[derive(Debug, Clone)]
pub struct ClassDesc {
    pub class: std::rc::Rc<str>,
    pub fields: Vec<String>,
    /// The class's full slot layout (M-perf S1b), shared (`Rc`) onto every instance `MakeInstance`
    /// builds. Built from [`crate::ast::class_field_layout`] so it is identical to the layout the
    /// interpreter uses for the same class — the two backends agree on `name → slot`.
    pub layout: std::rc::Rc<crate::value::ClassLayout>,
}

/// A whole compiled program: every top-level function (free functions, then synthetic
/// constructors, then methods), the index of `main`, and the program-level descriptor tables
/// shared across all functions — enum-variant descriptors, class descriptors, an interned
/// member/field-name pool indexed by `Op::GetField`, and the `(class, method) → function index`
/// dispatch table read by `Op::CallMethod` (decision P4-2/P4-6).
#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    pub functions: Vec<Function>,
    pub main: usize,
    /// Batch-1 D: `true` when the entry (`main`) is a class-`static` method rather than a top-level
    /// function. A static method's compiled `Function` reserves slot 0 for `$this` (arity = `1 +
    /// params`), so the VM pushes a dummy receiver before invoking the entry.
    pub main_is_static: bool,
    /// The entry's **user-declared** parameter count (0 or 1), independent of the `$this` slot a static
    /// method carries — the VM pushes argv as the next slot iff this is 1 (`E-MAIN-SIGNATURE` bounds it).
    pub main_params: usize,
    pub enum_descs: Vec<EnumDesc>,
    pub class_descs: Vec<ClassDesc>,
    pub names: Vec<String>,
    pub methods: HashMap<(String, String), usize>,
    /// The transitively-flattened interface set each class implements, keyed by class name — the
    /// runtime `instanceof`-against-an-interface table (M-RT S2). Built once by
    /// [`crate::ast::class_implements`] (the same call the interpreter + checker make), so the
    /// `Op::IsInstance` test is byte-identical across backends. A `BTreeMap`/sorted values keep it
    /// deterministic for `disasm`.
    pub class_implements: BTreeMap<String, Vec<String>>,
    /// Static class hierarchy for the reflection enumeration natives (`Core.Reflection.interfaces`/…),
    /// built once via [`crate::native::ClassTables::from_program`] and shared verbatim with the
    /// interpreter + transpiler so reflection is byte-identical (M-Reflect Tier-2).
    pub class_tables: crate::native::ClassTables,
    /// Initial values of the program's `static` class fields (M-mut.7), in compiler-assigned index
    /// order. The VM seeds its runtime `statics` vector from a clone of this at startup, so
    /// program-lifetime static state begins at the once-at-load literal values. Indexed by
    /// `Op::GetStatic`/`Op::SetStatic`.
    pub static_inits: Vec<Value>,
    /// Overload dispatch tables (M-RT method/function overloading), indexed by the set id an
    /// `Op::CallOverload` carries (free functions) or a `method_overloads` value (methods). Each entry
    /// pairs an overload's parameter kinds with the function index to call; `dispatch::select_overload`
    /// picks the most-specific match at runtime. Empty in the common no-overloads program.
    pub overloads: Vec<crate::dispatch::OverloadSet>,
    /// `(class, method)` → an index into [`Self::overloads`], for the overloaded methods only (M-RT).
    /// `Op::CallMethod` consults this after resolving the receiver's class: present ⇒ dynamic-dispatch
    /// over the set by the argument values; absent ⇒ the single-method `methods` entry. Empty in the
    /// common program.
    pub method_overloads: HashMap<(String, String), usize>,
}
