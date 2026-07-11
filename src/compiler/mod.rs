//! AST → bytecode compiler (M2 P1–P3). A dedicated pass over the type-checked AST,
//! emitting a `Chunk` the VM executes. Mirrors the tree-walker's semantics so
//! `runvm` output is byte-identical to `run` (the differential oracle).
//!
//! P2 scope: `main`-only programs — literals, arithmetic, comparison, logical
//! short-circuit, unary, interpolation, `println`, list literals, locals, `if`/`else`,
//! `for…in`, blocks. P3 added user function calls + call frames + recursion (multi-function
//! compile → `BytecodeProgram`). P4a adds single-payload enums (`Variant(args)` construction)
//! and exhaustive `match` (lowered to scrutinee-spill + per-arm tag/literal tests + payload
//! re-extraction; decision P4-7). P4b adds classes: each constructor compiles to a synthetic
//! function (promoted-field `MakeInstance` + body with the instance in scope), `ClassName(args)`
//! resolves to a `Call` into it, and `obj.field` lowers to `GetField` (decisions P4-4/P4-5). P4c
//! adds methods + `this`: each method compiles to a function with the receiver at slot 0,
//! `obj.m(args)` lowers to `CallMethod` (runtime dispatch on the receiver's class), and `this` /
//! bare field reads resolve against the receiver (decision P4-6). The compiler now covers the full
//! M1 surface. Enums, instances, and lists are value-native `Value` (no heap; P4-1).

use crate::ast::{
    free_vars, BinaryOp, ClassDecl, ClassMember, CtorParam, Expr, FunctionDecl, Item, LambdaBody,
    MatchArm, Modifier, Param, Pattern, Program, Stmt, StrPart, Type, UnaryOp, Visibility,
};
use crate::chunk::{BytecodeProgram, Chunk, ClassDesc, EnumDesc, FaultMsg, Function, Op};
use crate::diagnostic::Diagnostic;
use crate::value::Value;
use std::collections::HashMap;

/// Numeric operand kind, inferred just enough to pick int- vs float-specialized
/// arithmetic ops (decision P2-6).
#[derive(Clone, Copy, PartialEq)]
enum NumTy {
    Int,
    Float,
    /// `decimal` (M-NUM S1) — picks the `AddD/SubD/MulD` ops. The compiler emits a decimal op when
    /// *either* operand is decimal (the `decimal ⊕ int` widen happens in the value kernel).
    Decimal,
}

/// The compiler's class-aware view of a declared type (M2 Wave 4). Derived *structurally* from the
/// AST's declared `Type` annotations — the checker has already verified full types, so the compiler
/// only re-derives the little it needs: the numeric head (to pick int- vs float-specialized
/// arithmetic) and, for an instance, *which class* it is. Knowing the class lets `ctype` walk
/// `obj.field` / `c.method()` / a class-typed enum payload to the underlying numeric type — closing
/// the pre-Wave-4 gap where a field read on an arbitrary instance or a method result was
/// unclassifiable. `Other` stays the catch-all for everything non-numeric/non-class (bool, string,
/// unit, list, map, set, optional) — the compiler only needs to *reject* those as arithmetic
/// operands, not tell them apart — except a **list**, whose element type *is* reachable as an
/// operand via indexing (`xs[i] + 1`, since M3 S1.1), so `List(elem)` carries it; everything else
/// non-numeric/non-class (bool, string, unit, map, set, optional) stays `Other`.
#[derive(Clone, PartialEq)]
enum CTy {
    Int,
    Float,
    /// `decimal` (M-NUM S1). Tracked distinctly so a decimal-valued operand — including a
    /// decimal-typed field read, map-index, or method result — picks the `AddD/SubD/MulD` ops on the
    /// VM. Omitting it would make the VM reject what the interpreter accepts (the CTy-operand trap).
    Decimal,
    /// A `string` — not a numeric operand, but tracked distinctly so the compiler can lower
    /// `string + string` to `Op::Concat(2)` (Phase 1 string slice) instead of an `AddI`/`AddF`. A
    /// string operand that collapsed to `Other` would make the VM reject what the interpreter
    /// accepts (the CTy-operand trap), so every string-producing operand must resolve here.
    Str,
    /// A class instance, carrying its class name so `ctype` can resolve `obj.field` / `obj.m()`.
    Class(String),
    /// A `List<elem>`, carrying its element type so `ctype(Index)` (`xs[i]`) resolves to the element
    /// — which can be an arithmetic operand since M3 S1.1 (e.g. `xs[0] + 1` → `AddI`).
    List(Box<CTy>),
    /// A `Map<key, val>`, carrying both so `ctype(Index)` (`m[k]`) resolves to the **value** type —
    /// which can be an arithmetic operand (e.g. `m["a"] + 1` → `AddI`). Without this, a map-index
    /// operand collapses to `Other` and `num_ty` errors on the VM only — a `run`↔`runvm` break
    /// (M-RT S3, the same reason `List` carries its element type).
    Map(Box<CTy>, Box<CTy>),
    /// A function type `(params) -> ret` — not a numeric operand; carried for future lambda support.
    Fn {
        params: Vec<CTy>,
        ret: Box<CTy>,
    },
    Other,
}

/// A declared local: its name, its class-aware type (for `num_ty`/`ctype`), and the lexical depth
/// it lives at (for scope cleanup). Its stack slot is its index in `locals`.
struct Local {
    name: String,
    ty: CTy,
    depth: u32,
}

/// Per-function metadata gathered in the pre-pass: its index in `BytecodeProgram.functions`
/// and its declared return type (for `ctype` of a call result — decision P3-6). A class return
/// type lets `f().field` resolve.
struct FnMeta {
    index: usize,
    ret: CTy,
    /// Class-aware param types, so a bare named-function reference in value position resolves to a
    /// `CTy::Fn` (lets `var f = namedfn; f(x)` dispatch through `CallValue` like a lambda local).
    params: Vec<CTy>,
    /// `Some(set_id)` when this name is overloaded (M-RT): a call emits `Op::CallOverload(set_id,
    /// argc)` (runtime dynamic dispatch) instead of a direct `Op::Call(index)`. `None` — the common
    /// single-overload case — keeps using `index`. Filled in a post-pass once all overloads are seen.
    overload: Option<usize>,
    /// `Some(i)` when this (erased generic) function's result echoes its `i`-th argument's type
    /// (`id<T>(T x) -> T`); copied from `FunctionDecl::generic_ret_from_param`. Lets `ctype` recover
    /// the erased result's operand type from the argument so `id(7) + 1` specializes on the VM (S2.1).
    generic_ret_from_param: Option<usize>,
}

/// Per-variant metadata gathered in the pre-pass: its index into the `enum_descs` table (for
/// `MakeEnum`/`MatchTag`) and the class-aware type of each payload field (so a payload binding —
/// including a class-typed one — resolves through `ctype`). Decision P4-2.
struct VariantMeta {
    index: usize,
    field_tags: Vec<CTy>,
}

/// One step from the `$match` scrutinee to a sub-value: an enum-payload index (`Op::GetEnumField`)
/// or a named instance field (`Op::GetField` into the names pool). The mixed sequence lets a binding
/// reach, e.g., `Wrapper(Point { x })` — an enum field then a struct field — with no new `Op` (S5.2).
#[derive(Clone)]
enum PathSeg {
    Enum(usize),
    Field(usize),
}

/// A `match`-arm payload binding: the name, the slot of the hidden `$match` scrutinee local, and
/// the `path` from the scrutinee to the bound value. Bindings are *re-extracted* at each use
/// (`GetLocal $match` + a `GetEnumField`/`GetField` per path step) rather than stored as stack
/// locals, which keeps arm bodies stack-neutral and sidesteps mid-expression slot bookkeeping (P4-7).
struct MatchBinding {
    name: String,
    match_slot: usize,
    path: Vec<PathSeg>,
    ty: CTy,
}

struct Compiler<'a> {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: u32,
    fns: &'a HashMap<String, FnMeta>,
    /// Function arities, indexed parallel to `BytecodeProgram.functions` — lets `stack_effect`
    /// account for `Op::Call` (which pops `arity` args and pushes one result).
    arities: &'a [usize],
    /// Lambda sub-functions accumulated while compiling this function's body. Drained into the
    /// program's top-level `functions` array after the enclosing function is compiled.
    extra_functions: Vec<Function>,
    /// The table index of the *first* function in `extra_functions` (= `program.functions.len()`
    /// at the time this compiler was created). Lambda indices are `>= base_fn_idx`.
    base_fn_idx: usize,
    /// `n_captures` for each lambda in `extra_functions` (parallel array, same index).
    /// `stack_effect(MakeClosure(idx))` reads `lambda_n_captures[idx - base_fn_idx]`.
    lambda_n_captures: Vec<usize>,
    /// Variant name → its descriptor metadata (construction + pattern dispatch).
    variants: &'a HashMap<String, VariantMeta>,
    /// The shared enum-descriptor table — `stack_effect` reads `MakeEnum`'s payload arity from it.
    enum_descs: &'a [EnumDesc],
    /// Class name → the index of its synthetic constructor function (for `ClassName(args)`).
    classes: &'a HashMap<String, usize>,
    /// `(class, field)` → `(static slot index, field CTy)` (M-mut.7). `ClassName.field` lowers to
    /// `Op::GetStatic(idx)` / `Op::SetStatic(idx)` via the index; the `CTy` lets `ctype` resolve a
    /// static used as an arithmetic operand (`C.total + 1` specializes — without it the VM would
    /// reject what the interpreter accepts, the documented CTy-operand trap).
    statics_index: &'a HashMap<(String, String), (usize, CTy)>,
    /// Class constants (Feature A): `(class, NAME)` → (inlined literal `Value`, operand `CTy`).
    /// Inheritance + traits already flattened (the shared [`crate::ast::class_consts`] table). A
    /// `ClassName.NAME` access emits the literal via `Op::Const` (no runtime store) and `ctype` reads
    /// the `CTy` so a const is a first-class arithmetic operand — same CTy-operand discipline as a
    /// static.
    consts_index: &'a HashMap<(String, String), (Value, CTy)>,
    /// The shared class-descriptor table — `stack_effect` reads `MakeInstance`'s field count from it.
    class_descs: &'a [ClassDesc],
    /// Field/member name → its index in `BytecodeProgram.names` (for `GetField`/`CallMethod`).
    /// Pre-built from every declared field + method name so member lowering is a lookup, not a mutation.
    names_index: &'a HashMap<String, usize>,
    /// In a method or constructor body, the local slot holding the receiver (`this`): `0` for a
    /// method, the post-promotion instance slot for a constructor. `None` in a free function.
    /// `Expr::This` and a bare field read both load from this slot (decision P4-5/P4-6).
    this_slot: Option<usize>,
    /// Field name → class-aware type of the *current* class (empty outside a method/ctor). Lets a
    /// bare field name (`total`, resolved as `this.total`) work as an arithmetic operand and lets
    /// `expr` lower it to `GetLocal(this) + GetField` when it isn't a local/param/binding. This is
    /// exactly `class_field_ctys[cur_class]`, kept as a direct ref for the bare-field path.
    field_tags: &'a HashMap<String, CTy>,
    /// Program-wide class name → (field name → type) table (M2 Wave 4). `ctype` walks it to resolve
    /// a field read on an *arbitrary* instance (`p.x`, `a.inner.x`), not just `this`.
    class_field_ctys: &'a HashMap<String, HashMap<String, CTy>>,
    /// Program-wide `(class, method) → return type` table (M2 Wave 4). `ctype` reads it to resolve
    /// a method-call result (`c.get() + 1`).
    method_rets: &'a HashMap<(String, String), CTy>,
    /// Program-wide `(class, method) → echoed-param index` for a generic method whose result is exactly
    /// one of its own params (`pick<T>(T a, T b) -> T` ⇒ 0). `ctype` recovers the operand type of
    /// `u.pick(7, 8) + 1` from that argument — the method analog of `FnMeta.generic_ret_from_param`
    /// (S2.1). Empty unless a generic method echoes a param, so non-generic programs are untouched.
    method_generic_ret_from_param: &'a HashMap<(String, String), usize>,
    /// Checker reified-operand side-table (S2.1-broad): `expr span.start → operand CTy` for
    /// `Call`/`Member`/`Index` results. `ctype` consults this FIRST, so a generic method result / field
    /// read / `List<T>` return specializes as the operand the checker proved (closing the run↔runvm
    /// CTy-operand trap for erased-to-`mixed` results). Empty on the run-family `compile` path.
    reified_operands: &'a HashMap<usize, CTy>,
    /// Program-wide `(class, method) → function index` table (slice B0). A non-overloaded static call
    /// `ClassName.method(args)` lowers to a dummy-receiver push + `Op::Call(idx)` via this index —
    /// the class is known at compile time, so no runtime dispatch is needed.
    methods: &'a HashMap<(String, String), usize>,
    /// Program-wide `(class, method) → overload-set id` table (Statics-B), the static-call twin of the
    /// instance `method_overloads` table the VM's `Op::CallMethod` consults. An *overloaded* static
    /// call lowers to a dummy-receiver push + `Op::CallOverload(set_id, argc)`, which selects the
    /// matching body at runtime by the argument types — the same selector the interpreter runs.
    method_overloads: &'a HashMap<(String, String), usize>,
    /// The class whose body is being compiled (a method or constructor), or `None` in a free
    /// function. `ctype(This)` resolves to `Class(cur_class)`.
    cur_class: Option<String>,
    /// Direct parents (`extends`) of every class, for `parent`/super resolution (M-RT super/parent).
    /// `Some` only when compiling a method/constructor body (set after construction, like `cur_class`);
    /// `None` for a free function. A `parent.m()` resolves its target via this + the `methods` table
    /// (which already encodes dispatch origins), matching the interpreter's `resolve_parent_method`.
    parent_parents: Option<&'a std::collections::BTreeMap<String, Vec<String>>>,
    /// Active `match`-arm bindings (a stack; innermost shadows). Populated while compiling an arm
    /// body, truncated after.
    match_bindings: Vec<MatchBinding>,
    /// When compiling a synthetic constructor body, holds the code indices of the body's `return`
    /// statements (redirected to the ctor epilogue instead of an `Op::Return`). `None` outside a
    /// ctor body. The interpreter discards a ctor body's return and always yields the promoted
    /// instance (`construct`); the epilogue mirrors that exactly (decision P4-4).
    ctor_return_jumps: Option<Vec<usize>>,
    /// Base-relative operand-stack height, tracked so `match` can spill its scrutinee to the
    /// correct slot even mid-expression. Reset to `locals.len()` at each statement boundary and
    /// fixed at `&&`/`||`/`match` control-flow merges; otherwise maintained by `emit`.
    height: usize,
    /// Stack of enclosing loops (M-mut.3). `break`/`continue` emit a placeholder `Jump` recorded in
    /// the innermost frame and patched to the loop's exit / continue target at loop end (the
    /// `ctor_return_jumps` backpatch model). `body_base` is the `locals.len()` both forms pop down
    /// to (dropping body-scope locals while keeping the loop's own locals live). No new `Op` (F5).
    loop_frames: Vec<LoopFrame>,
    /// Stack of enclosing `try` contexts whose `finally` (and any pending `PopHandler`) must run
    /// when control transfers out of them via `return`/`break`/`continue` (M-faults 2b). Innermost
    /// last; pushed while compiling a try *body* or a `catch` body, popped after.
    finally_stack: Vec<TryCtx>,
}

/// One enclosing `try`/`catch` context for finally-on-transfer codegen (M-faults 2b).
struct TryCtx {
    /// The `finally` block to re-emit before a transfer (cloned from the AST), or `None`.
    finally: Option<Vec<Stmt>>,
    /// Whether a `PopHandler` must precede the finally on a transfer — true inside the try *body*
    /// (the handler is still installed), false inside a `catch` body (the unwind already consumed
    /// the handler).
    has_handler: bool,
    /// `loop_frames.len()` when this context was entered — lets `break`/`continue` run only the
    /// finally blocks nested inside the target (innermost) loop.
    loop_depth: usize,
}

/// One enclosing loop's break/continue backpatch state (M-mut.3).
struct LoopFrame {
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
    body_base: usize,
}

/// Compile a whole program: a pre-pass indexes every top-level function (so calls — including
/// forward references and recursion — resolve to a static index), then each function body is
/// compiled into its own `Chunk`. Parameters occupy slots `0..arity` at the base of the frame
/// window; every function ends with an implicit `Unit` return (P3-7).
pub fn compile(program: &Program) -> Result<BytecodeProgram, Diagnostic> {
    // The compiler tracks no source position yet, so every fault becomes a position-less
    // compile-stage `Diagnostic` (renders `compile error: …`, unchanged from before).
    compile_program(program).map_err(Diagnostic::compile)
}

/// Like [`compile`], but seeded with the checker's **reified-operand side-table** (S2.1-broad):
/// `expr span.start -> Ty`. Each entry is converted to its operand [`CTy`] (dropping `Other`, so a
/// non-operand result never overrides `ctype`'s normal resolution) and consulted FIRST in `ctype`, so a
/// generic method result / field read specializes as the arithmetic operand the checker proved. Empty
/// map ⇒ byte-identical to [`compile`] (the run-family `compile` path stays unchanged).
pub fn compile_with(
    program: &Program,
    reified: &std::collections::HashMap<usize, crate::types::Ty>,
) -> Result<BytecodeProgram, Diagnostic> {
    let reified_ctys: std::collections::HashMap<usize, CTy> = reified
        .iter()
        .filter_map(|(&span, ty)| match ty_to_cty(ty) {
            CTy::Other => None,
            cty => Some((span, cty)),
        })
        .collect();
    compile_program_with(program, &reified_ctys).map_err(Diagnostic::compile)
}

/// A ctor param is *promoted* to a field iff it carries a visibility modifier (matching
/// `interpreter::construct` / the checker's promotion rule).
/// Extract the literal text of a string-literal expression (one `StrPart::Literal`). The checker
/// guarantees a fault intrinsic's message is such a literal (M-faults 2a); defaults to empty.
fn str_literal(e: Option<&Expr>) -> String {
    if let Some(Expr::Str(parts, _)) = e {
        if let [crate::ast::StrPart::Literal(s)] = &parts[..] {
            return s.clone();
        }
    }
    String::new()
}

fn is_promoted(p: &CtorParam) -> bool {
    p.modifiers.iter().any(|m| {
        matches!(
            m,
            Modifier::Public | Modifier::Private | Modifier::Protected
        )
    })
}

/// Resolve a declared type annotation into the compiler's class-aware `CTy` (M2 Wave 4), derived
/// purely structurally from the AST. The numeric heads map to `Int`/`Float`; the known
/// primitive/container head names collapse to `Other` (their element types are never operands in
/// the M1 surface); any *other* named type is a user-defined class, kept as `Class(name)` so a
/// field/method read through it resolves. An `Optional` is `Other` (no `null` in M1).
/// The catchable type name(s) of a `catch` clause type (M-faults 2b): one name for a class /
/// interface, or one per member for a union `catch (A | B e)`. The checker has rejected any
/// non-`Error` member (`E-CATCH-TYPE`), so other `Type` shapes never reach here.
fn catch_clause_names(ty: &Type) -> Vec<String> {
    match ty {
        Type::Named { name, .. } => vec![name.clone()],
        Type::Union(members, _) => members.iter().flat_map(catch_clause_names).collect(),
        _ => Vec::new(),
    }
}

/// The compile-time operand type of a `catch` binding (M-faults 2b): the class for a single-type
/// clause (so `e.field` specializes), else `Other` for a union (no single class).
fn catch_binding_cty(ty: &Type) -> CTy {
    match ty {
        Type::Named { name, .. } => CTy::Class(name.clone()),
        _ => CTy::Other,
    }
}

fn resolve_cty(ty: &Type) -> CTy {
    match ty {
        Type::Named { name, args, .. } => match name.as_str() {
            "int" => CTy::Int,
            "float" => CTy::Float,
            "decimal" => CTy::Decimal,
            // Track the element type so `xs[i]` can be an arithmetic operand (M3 S1.1); a bare
            // `List` (no arg) defaults its element to `Other`.
            "List" => CTy::List(Box::new(args.first().map_or(CTy::Other, resolve_cty))),
            // Track key+value types so `m[k]` can be an arithmetic operand (M-RT S3); a bare `Map`
            // (no args) defaults both to `Other`.
            "Map" => CTy::Map(
                Box::new(args.first().map_or(CTy::Other, resolve_cty)),
                Box::new(args.get(1).map_or(CTy::Other, resolve_cty)),
            ),
            // `string` is tracked distinctly so `string + string` lowers to `Op::Concat` (the
            // string slice).
            "string" => CTy::Str,
            "bool" | "void" | "Set" => CTy::Other,
            other => CTy::Class(other.to_string()),
        },
        // An optional carries its inner's `CTy` (not `Other`): once narrowed (if-let, `??`, `?.`,
        // `match`) the value is the inner `T`, so `if (var x = opt) { x + 1 }` specializes. The
        // checker forbids using a bare `T?` as an operand, so tagging the optional local with the
        // inner `CTy` never mis-specializes an un-narrowed access (M3 S2.4).
        Type::Optional { inner, .. } => resolve_cty(inner),
        // `var` carries no annotation; operand inference reads the initializer expression instead.
        Type::Infer(_) => CTy::Other,
        // An erased generic type parameter (M-RT S7): the value is of an unknown concrete type, so
        // it is never a specialized numeric operand — exactly the `Other` case.
        Type::Erased(_) => CTy::Other,
        // A function type — carry its structure for future lambda support; not a numeric operand.
        Type::Function { params, ret, .. } => CTy::Fn {
            params: params.iter().map(resolve_cty).collect(),
            ret: Box::new(resolve_cty(ret)),
        },
        // `[T; N]` is a list at runtime (the length is compile-time only), so its operand type is the
        // list's element type — `pair[i]` specializes exactly like `xs[i]` (Phase 1 types slice).
        Type::FixedList { elem, .. } => CTy::List(Box::new(resolve_cty(elem))),
        // A union value is not a specialized arithmetic operand (M-RT S4); after `instanceof`/type-
        // pattern narrowing the *narrowed local* carries the concrete `CTy`, not the union local.
        Type::Union(..) => CTy::Other,
        // An intersection value is likewise not a specialized arithmetic operand (M-RT S5); member
        // access dispatches through the concrete instance with no specialization.
        Type::Intersection(..) => CTy::Other,
    }
}

/// A bare type NAME (a `match` type-pattern head or an `is`/`instanceof` right operand) → its operand
/// [`CTy`], mirroring [`resolve_cty`]'s `Type::Named` arm. Threads a discriminable primitive through
/// as a first-class arithmetic operand so a *narrowed* primitive specializes on the VM
/// (`match x { int i => i * 2 }`, `if (x is int) { x + 1 }` — the CTy-operand trap, Invariant 7).
/// A non-primitive name is a class/interface. `bool`/`null` are never arithmetic operands (no
/// dedicated `CTy` variant → `Other`).
fn cty_of_type_name(name: &str) -> CTy {
    match name {
        "int" => CTy::Int,
        "float" => CTy::Float,
        "string" => CTy::Str,
        "decimal" => CTy::Decimal,
        "bool" | "null" => CTy::Other,
        other => CTy::Class(other.to_string()),
    }
}

/// A checker [`crate::types::Ty`] → operand [`CTy`], mirroring [`resolve_cty`] (which maps the AST
/// `Type`). Used to give a native module-qualified call (`List.length(xs)`, `Text.parseInt(s)`) its
/// return operand type, so its result is a valid arithmetic operand (`List.length(xs) - 1`) on the VM —
/// without it `ctype` would recurse into the bare module qualifier `List` and error, a `run`↔`runvm`
/// break (the documented CTy-operand trap). Only the operand-relevant shapes are tracked; the rest
/// collapse to `Other`.
fn ty_to_cty(ty: &crate::types::Ty) -> CTy {
    use crate::types::Ty;
    match ty {
        Ty::Int => CTy::Int,
        Ty::Float => CTy::Float,
        Ty::Decimal => CTy::Decimal,
        Ty::String => CTy::Str,
        Ty::List(e) => CTy::List(Box::new(ty_to_cty(e))),
        Ty::Map(k, v) => CTy::Map(Box::new(ty_to_cty(k)), Box::new(ty_to_cty(v))),
        // A native that returns `T?` (e.g. `Text.parseInt -> int?`) yields the inner `T` once narrowed
        // — same discipline as `resolve_cty(Optional)`; the checker forbids a bare `T?` as an operand.
        Ty::Optional(inner) => ty_to_cty(inner),
        Ty::Named(n, _) => CTy::Class(n.clone()),
        _ => CTy::Other,
    }
}

/// The operand [`CTy`] of native registry entry `idx`'s return type.
fn native_ret_cty(idx: usize) -> CTy {
    crate::native::registry()
        .get(idx)
        .map_or(CTy::Other, |n| ty_to_cty(&n.ret))
}

// impl/free cohesion split (M-Decomp W4.1): program driver + stmt/expr/match clusters.
mod expr;
mod matches;
mod program;
mod stmt;
use ctors::*;
use program::*;

mod ctors;
mod cty;
mod emit;

#[cfg(test)]
mod tests;
