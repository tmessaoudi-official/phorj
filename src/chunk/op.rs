//! The `Op` set — every variant extends THREE exhaustive matches in the same commit:
//! `vm::exec_op`, `BytecodeProgram::validate`, `compiler::stack_effect` (Invariant 3).

use super::*;

/// One VM instruction. Typed operands — no raw-byte decode (decision M2-7).
/// Jump targets are absolute instruction indices (decision P2-2).
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Push `consts[idx]`.
    Const(usize),
    // Type-specialized arithmetic (the checker guarantees operand types).
    AddI,
    SubI,
    MulI,
    DivI,
    RemI,
    AddF,
    SubF,
    MulF,
    DivF,
    RemF,
    // Exact `decimal` arithmetic (M-NUM S1) — `+ - *` only (`/`/`%` deferred to S2). Each pops two
    // values (a `Decimal`, or a mixed `Decimal`/`Int` — the int widens to scale 0 in the kernel) and
    // pushes the exact `Decimal` result, or a clean `"decimal overflow"` fault on any i128 overflow.
    // Dispatch into the single-sourced `value::decimal_add/sub/mul` kernels (interpreter parity).
    AddD,
    SubD,
    MulD,
    // Exact `decimal` remainder (bare `%`, 2026-06-27): pops two values, pushes the exact `Decimal`
    // remainder (`value::decimal_rem`) or a `"decimal modulo by zero"` / overflow fault.
    RemD,
    // Exact-or-fault `decimal` division (bare `/`, 2026-06-27): pops two values, pushes the exact
    // quotient (`value::decimal_div_exact`, minimal scale) or faults — non-terminating
    // (`"decimal division is not exact"`), zero divisor, or i128/scale overflow. `Decimal.div`
    // (rounded) stays a native call.
    DivD,
    // Bitwise ops on `int` operands (primitives P2; the checker guarantees int operands). Shifts
    // fault on a negative count, yield 0 / sign-fill for a count ≥ 64.
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    /// Negate the top of stack (int or float).
    Neg,
    /// Logical not (bool).
    Not,
    /// Bitwise NOT of the top of stack (int).
    BitNot,
    // Comparison / equality — runtime-generic (decision P2-8).
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    /// Discard the top of stack.
    Pop,
    /// Push a copy of the local at stack slot `n`.
    GetLocal(usize),
    /// Pop and store into the local at stack slot `n` (set-and-pop, decision P2-4).
    SetLocal(usize),
    /// Unconditional jump to absolute instruction index.
    Jump(usize),
    /// Pop a bool; if false, jump to absolute instruction index (decision P2-5).
    JumpIfFalse(usize),
    /// Pop `n` values, concatenate their `as_display` (interpolation), push the `Str`.
    Concat(usize),
    /// Pop `n` values into a `List` (top-of-stack is the last element).
    MakeList(usize),
    /// Pop `2n` values (key/value pairs, in source order: …, k_n, v_n on top) into an
    /// insertion-ordered `Map` via the shared `value::build_map` kernel (M-RT S3). Carries a count,
    /// not a pool index — like `MakeList`, no `validate` index-bounds arm is needed.
    MakeMap(usize),
    /// Pop an index and a container; push the element clone. **Polymorphic** (M-RT S3): a `List`
    /// uses a bounds-checked int index ("list index out of range" on OOB); a `Map` does a key lookup
    /// via `value::map_index` ("map key not found" when absent).
    Index,
    /// Pop a value, an index, and a container; copy-on-write set `container[index] = value` and push
    /// the (possibly new) container (M-mut.5). **Polymorphic**: a `List` does a bounds-checked int
    /// set (`value::list_set`); a `Map` updates-or-appends by key (`value::map_set`). Value types are
    /// acyclic, so `Rc::make_mut` reclaims fully — no GC. The caller writes the pushed container back
    /// to its place (e.g. `SetLocal`).
    SetIndex,
    /// Pop a value and an index; copy-on-write set `local[index] = value` **in place in the local
    /// slot** (M-DOGFOOD W8). Unlike `SetIndex` (which pops the container off the stack — so the local
    /// slot still holds a second `Rc`, forcing `Rc::make_mut` to deep-copy on every write, making
    /// imperative array algorithms O(n²)), this mutates the container directly in its slot, so
    /// `make_mut` sees a refcount of 1 in the common case and mutates in place — O(1) per write. COW is
    /// preserved: a genuinely shared container still copies. The checker restricts index-assign to a
    /// local container, so the target is always a slot. Pushes nothing.
    SetIndexLocal(usize),
    /// Pop a value and `depth` index values; nested copy-on-write set `local[i0][i1]…=value` **in
    /// place in the local slot** (Spec nested-value-index-assign). `depth ≥ 2` (the `depth==1` case is
    /// `SetIndexLocal`). The indices were pushed in source order `i0..i_{depth-1}` (so `i_{depth-1}` is
    /// on top); this pops them, reverses to source order, and calls the shared `value::set_nested`
    /// kernel on the root slot — `make_mut` root-to-leaf, so COW is preserved (a shared level copies).
    /// The checker restricts the base to a local. Pushes nothing.
    SetPathLocal(usize, usize),
    /// Pop a list; push its length as an `Int`.
    Len,
    /// Pop an iterable (`List`/`Set`); push a `List` of its elements in iteration order (B1 iteration
    /// protocol). Emitted once at the top of a `for`-`in` loop so the loop body indexes a plain list
    /// regardless of the source collection. Single-sourced with the interpreter via
    /// [`crate::value::iter_elements`], so iteration order is byte-identical (`run≡runvm`).
    IterElems,
    /// Pop two ints (`end` on top, then `start`) and push a `List<int>` materialization of the
    /// range: `[start, …, end-1]` exclusive, or `[start, …, end]` inclusive (the `bool` flag). Empty
    /// when `start >= end` (exclusive) / `start > end` (inclusive). Built via Rust's native
    /// `start..end` / `start..=end`, which stop at `i64::MAX` without a counter overflow (EV-7).
    /// Carries no static index, so — like `GetEnumField` — it needs no `validate` arm (decision
    /// S1-R, M3 S1.2).
    MakeRange(bool),
    /// **Green-thread concurrency (M6 W4 / S4.3).** The five ops below carry no pool index, so — like
    /// `MakeRange` — none needs a `validate` arm. In the **step-2 synchronous-degenerate** model their
    /// bodies are immediate (no scheduler): `spawn` runs the task eagerly, `recv`/`join` on an
    /// unavailable value fault cleanly. Build step 4 keeps the op set fixed and only rewires the bodies
    /// to drive the shared `green::sched` scheduler (suspending the coroutine at `recv`/`join`/`yield`).
    /// Quarantined from the PHP oracle — the transpiler never emits these (`E-CONCURRENCY-NO-PHP`).
    ///
    /// `Spawn`: pop the spawned call's result and push a `Task` wrapping it (step-2 eager: the call has
    /// already run, leaving its value on top). The general form — used when the spawned operand is a
    /// method/overloaded/closure call whose function index isn't known at compile time. The cooperative
    /// driver (S4.3) rejects this form (a non-free-function `spawn` is a documented follow-up), matching
    /// the interpreter, so `run≡runvm`.
    Spawn,
    /// `SpawnCall(func_idx, argc)`: spawn the **free function** at `func_idx`, taking the top `argc`
    /// values as its arguments (the call is *not* run before this op — the compiler lowers a
    /// single-overload `spawn f(args)` to args-push + this, so the function body is the task's root). On
    /// the synchronous path this runs the function inline and registers the finished task (byte-identical
    /// to `<call>; Spawn`); the cooperative driver (S4.3) instead defers it as a scheduler task whose
    /// coroutine root is `func_idx`'s own frame — traced identically to the interpreter (no lambda frame).
    SpawnCall(usize, usize),
    /// `ChannelNew`: push a fresh empty `Channel`.
    ChannelNew,
    /// `ChannelSend`: pop the value (top) then the channel; enqueue the value; push `Unit` (the void
    /// result a statement-expression discards).
    ChannelSend,
    /// `ChannelRecv`: pop the channel; push the front value, or fault ("recv from empty channel") when
    /// empty (step 2 has no scheduler to yield to — step 4 suspends instead).
    ChannelRecv,
    /// `Join`: pop the task; push its result, or fault ("join on incomplete task") if not yet complete
    /// (never happens in the step-2 eager model — the task always finished at `spawn`).
    Join,
    /// Call the native (built-in) function at `native::registry()[idx]` with the top `argc` values
    /// as its arguments (source order): pop them, run the native's shared `eval` (which may append
    /// to the program output), and push its return value. The migrated former `Op::Print`
    /// (`console.println` is `CallNative(native::CONSOLE_PRINTLN, 1)`); the namespaced stdlib's one
    /// runtime entry point (M3 Wave 1). Pushes a result, so it carries a `validate` arm (the index
    /// is bounded by the registry length).
    CallNative(usize, usize),
    /// Call `functions[idx]`: its args are already on top of the stack; the new frame's
    /// local window opens at `stack.len() - functions[idx].arity` (decision P3-1, P3-3).
    Call(usize),
    /// Call an *overloaded* free function (M-RT dynamic dispatch). The args are on top of the stack
    /// as for `Op::Call`; the first operand is an index into [`BytecodeProgram::overloads`], the
    /// second is the argument count. At runtime the top `argc` values' types select the
    /// most-specific matching overload (`dispatch::select_overload`), and its function index is
    /// called exactly like `Op::Call`. The selection is byte-identical to the interpreter's (same
    /// `ParamKind`s, same selector). A no-match/ambiguous selection is a clean runtime fault. The
    /// set index is bounds-checked in `validate` (its target indices are checked there too).
    CallOverload(usize, usize),
    /// Call an *overloaded static* method `ClassName.m(args)` (Statics-B). Identical at runtime to
    /// [`Op::CallOverload`] — same operands (overload-set index, argument count), same selector
    /// (`dispatch::select_overload` over the top `argc` argument values) — but the compiler pushes a
    /// dummy receiver into slot 0 *below* the args first (a static method's compiled frame reserves
    /// slot 0 for `this`, which it never reads), so the selected body's `arity` (`1 + nparams`) pops
    /// the dummy together with the args. The only reason this is a distinct op from `CallOverload` is
    /// the compiler's `stack_effect`: this form pops one extra value (the dummy), so its net effect is
    /// `-argc`, not `1 - argc`. `validate` bounds-checks it exactly like `CallOverload`.
    CallStaticOverload(usize, usize),
    /// Pop the return value, unwind the current frame (truncate its slot window), pop the
    /// frame, push the return value onto the caller's stack. End execution when the last
    /// (`main`) frame returns (decision P3-2).
    Return,
    /// Construct an enum value from `enum_descs[idx]`: pop `desc.arity` payload values (in
    /// source order — top of stack is the last field) and push `Value::Enum` (decision P4-3).
    MakeEnum(usize),
    /// Pop the scrutinee and push a `Bool`: whether it is a `Value::Enum` whose variant equals
    /// `enum_descs[idx].variant`. Variant names are globally unique (the checker keys them by
    /// name), so the variant string alone disambiguates. Used by `match` arm dispatch (P4-7).
    MatchTag(usize),
    /// Pop an enum value and push a clone of its payload element `i`. The compiler only emits
    /// this for an index a preceding `MatchTag` already proved in range (P4-7); a defensive
    /// runtime fault covers misuse (EV-7).
    GetEnumField(usize),
    /// Abort with a fixed runtime-fault message selected by [`FaultMsg`]. Generalizes the former
    /// `MatchFail` (M3 S2.5): both the `match` exhaustiveness backstop and `opt!`-on-null lower to
    /// this one op, so S2 adds **no new `Op` variant**. The message text lives in the handler (not
    /// the const pool), keeping both backends byte-identical (the `agree_err` oracle classifies
    /// faults by body substring). Carries no static index, so — like `MakeRange` — it needs no
    /// `validate` arm.
    Fault(FaultMsg),
    /// Construct a class instance from `class_descs[idx]`: pop `desc.fields.len()` promoted-field
    /// values (in declaration order — top of stack is the last field), zip them with
    /// `desc.fields`, and push a `Value::Instance`. Emitted only inside a synthetic constructor
    /// function, after its promoted params have been loaded (decision P4-4).
    MakeInstance(usize),
    /// Pop an instance and push a clone of its field named `names[idx]`. A read of an absent field
    /// (a checker-valid but uninitialized explicit `Field` member) faults
    /// `no field \`{name}\` on \`{class}\`` — byte-identical to the interpreter (decision P4-5).
    GetField(usize),
    /// Pop a value then an instance and set the instance's field `names[idx]` to that value
    /// (M-mut.6): `o.f = e`. Mutates the shared `Rc<Instance>` cell in place (handle semantics), so
    /// the write is visible through every binding to the same instance. Pushes nothing (statement
    /// form). The field is checker-guaranteed to exist and be `mutable`.
    SetField(usize),
    /// Push a clone of the program-level static field `static_inits[idx]`'s current value (M-mut.7):
    /// `ClassName.field` static read. The slot lives in the VM's runtime `statics` vector.
    GetStatic(usize),
    /// Pop the top value and store it into the static slot `idx` (M-mut.7): `ClassName.field = e`.
    /// Pushes nothing (statement form); the field is checker-guaranteed a `static mutable`.
    SetStatic(usize),
    /// Call an instance method `(name_idx, argc)`: the receiver and its `argc` args sit on the
    /// stack as `[.., receiver, arg0 … arg_{argc-1}]`. At runtime, resolve
    /// `(receiver.class, names[name_idx])` through `BytecodeProgram.methods` to a function index and
    /// open a frame whose slot 0 is the receiver (`this`) and slots `1..=argc` are the args
    /// (decision P4-6). Method existence is checker-enforced; the resolution-miss fault is a
    /// defensive backstop (byte-identical to the interpreter).
    CallMethod(usize, usize),
    /// `parent`/super dispatch `(func_idx, argc)` (M-RT super/parent): like [`Self::CallMethod`] but
    /// the target function index is **baked at compile time** (a non-virtual call), not resolved from
    /// the receiver's runtime class — so an override's `parent.m()` reaches the version it shadows.
    /// The stack is `[.., this, arg0 … arg_{argc-1}]` (the receiver is the current `this`, pushed by the
    /// compiler); slot 0 of the new frame is the receiver, slots `1..=argc` the args.
    CallParent(usize, usize),
    /// Build a `Value::Closure` from `functions[idx]`: pop the top `functions[idx].n_captures`
    /// values (captures, in sorted order — invariant #8), then push the closure. For a named
    /// function reference (0 captures), nothing is popped. (M3 S3, Task 4.)
    MakeClosure(usize),
    /// Call a first-class function value: pop `argc` args (source order, top is the last),
    /// pop the `Value::Closure` beneath them, push captures then args into the new frame's slot
    /// window (layout `[caps.., args..]` mirrors the sub-compiler's `[caps, params]` ordering),
    /// and open a frame. Carries no static table index, so — like `GetEnumField` — it needs no
    /// `validate` arm. (M3 S3, Task 4.)
    CallValue(usize),
    /// Pop the top value and push a `Bool`: whether it is a `Value::Instance` whose class equals the
    /// carried class name (`value instanceof TypeName`, M-RT S1). A non-instance value is `false`,
    /// never a fault — matching the interpreter and PHP's `instanceof`. The class name is carried
    /// inline (like `Fault(FaultMsg)`), not via a pool index, so — like `MakeRange`/`Fault` — it
    /// needs no `validate` arm.
    IsInstance(String),
    /// Pop the top value and unwind it as a thrown exception (M-faults 2b): the value is stashed in
    /// `Vm::pending_throw` and a [`THROW_SENTINEL`] fault is raised, which the run loop turns into a
    /// search of the handler stack (`Op::PushHandler`). Carries no static index — like
    /// `Fault`/`MakeRange` it needs no `validate` arm.
    Throw,
    /// Install an exception handler whose catch landing pad is the carried code index, capturing the
    /// current frame depth and stack height (M-faults 2b). On a `Throw`, the run loop unwinds to the
    /// topmost handler, truncates frames/stack to the captured marks, pushes the thrown value, and
    /// jumps to the landing pad. The only index-carrying op of the three, so — like `Jump` — its
    /// target is bounds-checked in `validate`.
    PushHandler(usize),
    /// Remove the most-recently-installed handler (M-mut: the try body completed without throwing, or
    /// control is transferring out of the try). Carries no index — no `validate` arm.
    PopHandler,
}
