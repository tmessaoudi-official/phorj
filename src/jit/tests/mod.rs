//! Slice-1 JIT tests (run under `--features jit`). They prove the codegen substrate end-to-end: a
//! pure-int function (leaf or recursive) compiles to native code and produces the SAME value the VM
//! oracle does, a kernel fault surfaces with the SAME canonical string, calls compose across the
//! shared value stack, deep recursion faults with the VM's `"stack overflow"` at the same depth, and
//! anything outside the subset is default-denied. Byte-identity-under-`phg run` is the *next* (wiring)
//! slice — these establish the substrate the wiring rides on.

use super::*;
use super::{compile_and_run, Compiled, JitError, JitRun, REDO_ON_VM};
use crate::chunk::BytecodeProgram;
use crate::value::Value;

mod accumulator_elision;
mod boxed;
mod extreme_by;
mod hof_filter_map;
mod listcontains;
mod map_materialize;
mod math_verticals;
mod range_and_overflow;
mod string_scan;
mod sumby;
mod unboxed_flow;
mod unboxed_int;
mod verticals;

/// Compile loose source through the real front-end (loader → check → compile), same helper shape the
/// VM tests use.
fn compile_source(src: &str) -> BytecodeProgram {
    let unit = crate::loader::load_loose_src(src).unwrap();
    let checked = crate::cli::check_and_expand(&unit.program, &unit.diag_src).unwrap();
    crate::compiler::compile(&checked).unwrap()
}

fn func_index(program: &BytecodeProgram, name: &str) -> usize {
    program
        .functions
        .iter()
        .position(|f| f.name == name)
        .unwrap_or_else(|| panic!("no compiled function `{name}`"))
}

/// `Value` has no `PartialEq` (closures/`Rc`) — compare ints by matching the variant.
fn as_int(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        other => panic!("expected int, got {}", other.type_name()),
    }
}

/// Run a JIT-eligible function and unwrap its int value, panicking on fault/ineligibility — the
/// common shape for the control-flow tests below.
fn jit_int(program: &BytecodeProgram, f: usize, args: &[Value]) -> i64 {
    match compile_and_run(program, f, args).expect("function must be JIT-eligible") {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected fault: {m}"),
    }
}

/// The VM oracle's int result for the same entry + args (Invariant 2).
fn vm_int(program: &BytecodeProgram, f: usize, args: Vec<Value>) -> i64 {
    let (v, _stdout) = crate::vm::Vm::new(program)
        .run_entry(f, args)
        .expect("VM run_entry");
    as_int(&v)
}
