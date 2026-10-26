//! Row 4b step 5 — the unboxed JIT and the runtime-class ops. `GetFieldOrHook` replaced `GetField`
//! wherever a subclass may hook the name, and the JIT declined every function containing it, so a
//! hot read through an open class fell from native code to the VM (~57x, KNOWN_ISSUES
//! VM-WITH-STATIC-CLASS). The unboxed tier knows each receiver's EXACT class (`Kind::Inst` joins only
//! equal classes), so it resolves the site at compile time: the class's own `<name>$get` when it has
//! one, else the field — the oracle's lookup (own members and `use`d traits, never a parent).

use super::boxed::ub_int;
use super::*;

const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    open class Base { constructor(public mutable int n) {} }\n\
    class Sub extends Base { int n { get => 7; set(int v) {} } }\n\
    function viaBase(int k): int { Base b = new Base(3); mutable int s = 0; \
      for (mutable int i = 0; i < k; i++) { s = s + b.n; } return s; }\n\
    function viaSub(int k): int { Base b = new Sub(3); mutable int s = 0; \
      for (mutable int i = 0; i < k; i++) { s = s + b.n; } return s; }\n\
    function writes(int k): int { Base b = new Base(3); b.n = 5; return b.n + k; }\n\
    function writesSub(int k): int { Base b = new Sub(3); b.n = 5; return b.n + k; }\n\
    #[Entry(kind: EntryKind.Cli)]\n\
    function main(): void { Output.printLine(\"{viaBase(10)} {viaSub(10)} {writes(4)} {writesSub(4)}\"); }\n";

fn emits_get_field_or_hook(program: &BytecodeProgram, f: usize) -> bool {
    program.functions[f]
        .chunk
        .code
        .iter()
        .any(|op| matches!(op, crate::chunk::Op::GetFieldOrHook(..)))
}

#[test]
fn a_supertype_read_of_an_unhooked_instance_compiles_and_reads_the_field() {
    let program = compile_source(SRC);
    let f = func_index(&program, "viaBase");
    assert!(
        emits_get_field_or_hook(&program, f),
        "the site must be the runtime-class op"
    );
    let args = [Value::Int(1000)];
    assert_eq!(ub_int(&program, f, &args), 3000, "unboxed JIT result");
    assert_eq!(vm_int(&program, f, args.to_vec()), 3000, "VM oracle");
}

#[test]
fn a_supertype_read_of_a_hooked_instance_compiles_and_runs_the_hook() {
    let program = compile_source(SRC);
    let f = func_index(&program, "viaSub");
    assert!(
        emits_get_field_or_hook(&program, f),
        "the site must be the runtime-class op"
    );
    let args = [Value::Int(1000)];
    assert_eq!(ub_int(&program, f, &args), 7000, "unboxed JIT result");
    assert_eq!(vm_int(&program, f, args.to_vec()), 7000, "VM oracle");
}

fn emits_set_field_or_hook(program: &BytecodeProgram, f: usize) -> bool {
    program.functions[f]
        .chunk
        .code
        .iter()
        .any(|op| matches!(op, crate::chunk::Op::SetFieldOrHook(..)))
}

/// The write twin's FIELD case lowers to `SetField` and consumes the `Pop` after it — the shape the
/// pre-row-4b JIT compiled (it wrote the field even where a subclass setter should run).
#[test]
fn a_supertype_write_of_an_unhooked_instance_compiles_and_writes_the_field() {
    let program = compile_source(SRC);
    let f = func_index(&program, "writes");
    assert!(
        emits_set_field_or_hook(&program, f),
        "the write must be the runtime-class op"
    );
    let args = [Value::Int(4)];
    assert_eq!(ub_int(&program, f, &args), 9, "unboxed JIT result");
    assert_eq!(vm_int(&program, f, args.to_vec()), 9, "VM oracle");
}

/// The HOOK case resolves to `CallMethod(n$set, 1)` and then declines inside the setter: a void
/// method body (`Const(Unit)`) is outside the unboxed subset, exactly as the exact-class static path
/// declines the same call. Pinned by reason: if void methods are admitted, add a `ub_int` proof here.
#[test]
fn a_supertype_write_of_a_hooked_instance_declines_on_the_void_setter() {
    let program = compile_source(SRC);
    let f = func_index(&program, "writesSub");
    assert!(
        emits_set_field_or_hook(&program, f),
        "the write must be the runtime-class op"
    );
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect("a void setter is outside the unboxed subset — if it compiles, add a ub_int proof");
    assert!(format!("{err:?}").contains("Unit"), "{err:?}");
}

#[test]
fn the_runtime_class_ops_agree_with_the_oracle_end_to_end() {
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle);
    assert_eq!(oracle, "30 70 9 11\n");
}
