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

/// Step 5b: `with` on a class that has a subclass compiles to `CloneInstance` (the copy keeps the
/// RUNTIME class). The unboxed tier copies the exact class's slot word by word and clones each
/// string/list handle word, so two instances never own the same word. The string field is built at
/// run time (`t.tag + "x"`): a const-interned word is release-exempt, so only an owned one can show
/// a shared word being freed under the original — and each iteration allocates a fresh string
/// (`probe`), which would reuse a slot freed by a dying copy that had not cloned the word.
const CLONE_SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    open class Base { constructor(public int n) {} }\n\
    class Sub extends Base { int n { get => 7; } }\n\
    open class Tagged { constructor(public int x, public mutable string tag) {} }\n\
    class SubTagged extends Tagged {}\n\
    function cloneBase(int k): int { Base b = new Base(1); mutable int s = 0; \
      for (mutable int i = 0; i < k; i++) { Base c = b with { n = i }; s = s + c.n; } return s; }\n\
    function cloneSub(int k): int { Base b = new Sub(1); Base c = b with { n = k + 0 }; return c.n; }\n\
    function cloneTagged(int k): int { Tagged t = new Tagged(1, \"ab\"); t.tag = t.tag + \"x\"; mutable int s = 0; \
      for (mutable int i = 0; i < k; i++) { Tagged u = t with { x = i }; \
        string probe = u.tag + \"!\"; if (probe == \"abx!\") { s = s + u.x; } } return s + t.x; }\n\
    open class Wide { constructor(public int a, public int b, public int c, public int d, public int e, \
      public int f, public int g, public int h, public int i, public int j) {} }\n\
    class SubWide extends Wide {}\n\
    open class Listy { constructor(public List<int> xs, public int k) {} }\n\
    class SubListy extends Listy {}\n\
    function cloneTemp(int k): int { Base c = new Base(1) with { n = k + 0 }; return c.n; }\n\
    function cloneWide(int k): int { Wide w = new Wide(1, 2, 3, 4, 5, 6, 7, 8, 9, 10); mutable int s = 0; \
      for (mutable int i = 0; i < k; i++) { Wide v = w with { j = i }; s = s + v.j + v.a + v.i; } return s; }\n\
    function cloneList(int k): int { Listy l = new Listy([1, 2, 3], 0); mutable int s = 0; \
      for (mutable int i = 0; i < k; i++) { Listy m = l with { k = i }; List<int> probe = [i, i]; \
        s = s + m.k + m.xs[2] + probe[0]; } return s; }\n\
    #[Entry(kind: EntryKind.Cli)]\n\
    function main(): void { Output.printLine(\"{cloneBase(10)} {cloneSub(3)} {cloneTagged(10)} {cloneTemp(4)} \
      {cloneWide(3)} {cloneList(3)}\"); }\n";

fn emits_clone_instance(program: &BytecodeProgram, f: usize) -> bool {
    program.functions[f]
        .chunk
        .code
        .iter()
        .any(|op| matches!(op, crate::chunk::Op::CloneInstance))
}

#[test]
fn a_with_on_a_non_exact_class_compiles_and_copies_the_instance() {
    let program = compile_source(CLONE_SRC);
    for (name, arg, want) in [
        ("cloneBase", 1000, 499_500),
        ("cloneSub", 3, 7),
        ("cloneTagged", 1000, 499_501),
        // An OWNED temp receiver (released after the copy), a WIDE class (two-slot layout), and a
        // list field cloned per iteration with a fresh list allocated beside it.
        ("cloneTemp", 4, 4),
        ("cloneWide", 1000, 499_500 + 10 * 1000),
        ("cloneList", 1000, 2 * 499_500 + 3 * 1000),
    ] {
        let f = func_index(&program, name);
        assert!(
            emits_clone_instance(&program, f),
            "{name}: the `with` must be CloneInstance"
        );
        let args = [Value::Int(arg)];
        assert_eq!(
            ub_int(&program, f, &args),
            want,
            "{name}: unboxed JIT result"
        );
        assert_eq!(
            vm_int(&program, f, args.to_vec()),
            want,
            "{name}: VM oracle"
        );
    }
}

#[test]
fn the_clone_lowering_agrees_with_the_oracle_end_to_end() {
    let jit_out = crate::cli::cmd_run(CLONE_SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(CLONE_SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle);
    assert_eq!(oracle, "45 7 46 4 33 15\n");
}

/// The L2b `this`-field TAKE (`analyze/this_take.rs`) must stay off in a method holding a runtime-class
/// op: a hook (`D.b`) may read any field of `this`, so moving a field's word out at its "last" read
/// would leave the hook reading a poisoned slot. Owned temp receivers (`new D(..).m()`) are exactly
/// the owned-`this` shape the take applies to; the per-iteration `probe` allocation would reuse a
/// wrongly freed word.
const TAKE_SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    open class C { constructor(public string a, public string b) {} \
      function m(): string { return this.a + this.b; } function h(): string { return this.b; } \
      function n(): string { return this.a + this.h(); } \
      function w(): C { return this with { a = this.a + \"w\" }; } }\n\
    class D extends C { string b { get => this.a + \"!\"; } }\n\
    function f(int k): int { mutable int s = 0; for (mutable int i = 0; i < k; i++) { \
      string x = new D(\"p\" + \"q\", \"r\" + \"s\").m() + \"|\"; \
      string y = new D(\"p\" + \"q\", \"r\" + \"s\").n() + \"|\"; \
      string z = new D(\"p\" + \"q\", \"r\" + \"s\").w().b + \"|\"; string probe = \"zz\" + \"zz\"; \
      if (x == \"pqpq!|\") { s = s + 1; } if (y == \"pqpq!|\") { s = s + 10; } \
      if (z == \"pqw!|\") { s = s + 100; } } return s; }\n\
    #[Entry(kind: EntryKind.Cli)]\n\
    function main(): void { Output.printLine(\"{f(10)}\"); }\n";

#[test]
fn owned_this_methods_holding_runtime_class_ops_take_no_field() {
    let program = compile_source(TAKE_SRC);
    let f = func_index(&program, "f");
    let args = [Value::Int(1000)];
    assert_eq!(ub_int(&program, f, &args), 111_000, "unboxed JIT result");
    assert_eq!(vm_int(&program, f, args.to_vec()), 111_000, "VM oracle");
    assert_eq!(
        crate::cli::cmd_treewalk(TAKE_SRC).expect("oracle"),
        "1110\n"
    );
}

/// Pinned declines (each fails closed to the VM, never miscompiles): a class with a map field has no
/// clone repr, and an instance arriving as a parameter has no exact kind at the copy.
const DECLINE_SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    open class Mapped { constructor(public Map<string, int> m, public int k) {} }\n\
    class SubMapped extends Mapped {}\n\
    open class Base { constructor(public int n) {} }\n\
    class Sub extends Base {}\n\
    function cloneMapped(int k): int { Mapped a = new Mapped([\"x\" => 1], 0); Mapped b = a with { k = k + 0 }; \
      return b.k; }\n\
    function cloneParam(Base b, int k): int { Base c = b with { n = k + 0 }; return c.n; }\n\
    #[Entry(kind: EntryKind.Cli)]\n\
    function main(): void { Output.printLine(\"{cloneMapped(3)} {cloneParam(new Sub(1), 4)}\"); }\n";

#[test]
fn a_map_field_or_a_parameter_receiver_declines_the_clone_cleanly() {
    let program = compile_source(DECLINE_SRC);
    for (name, reason) in [
        ("cloneMapped", "StrIntMap"),
        ("cloneParam", "CloneInstance on"),
    ] {
        let f = func_index(&program, name);
        assert!(
            emits_clone_instance(&program, f),
            "{name}: the `with` must be CloneInstance"
        );
        let err = crate::jit::Compiled::compile_unboxed(&program, f)
            .err()
            .unwrap_or_else(|| panic!("{name} must decline — if it compiles, add a ub_int proof"));
        assert!(format!("{err:?}").contains(reason), "{name}: {err:?}");
    }
    assert_eq!(
        crate::cli::cmd_run(DECLINE_SRC).expect("jit-wired run ok"),
        crate::cli::cmd_treewalk(DECLINE_SRC).expect("oracle")
    );
}
