//! Compiler unit tests — which receivers pay for run-time hook dispatch (plan row 4b). A read of a
//! name some class hooks compiles to `GetFieldOrHook` only when the receiver's class, or a class
//! that extends it, has that hook; a hook in an unrelated hierarchy leaves the fast `GetField`.

use super::*;
use crate::chunk::Op;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn ops_of(src: &str, func: &str) -> Vec<Op> {
    let src = format!(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; {src} \
         #[Entry(kind: EntryKind.Cli)] function main(): void {{}}"
    );
    let tokens = lex(&src).expect("lex ok");
    let prog = Parser::new(tokens).parse_program().expect("parse ok");
    let program = compile(&prog).expect("compile ok");
    let f = program
        .functions
        .iter()
        .find(|f| f.name == func)
        .expect("function");
    f.chunk.code.clone()
}

const SRC: &str = "open class Base { constructor(public int n) {} } class Sub extends Base {} \
    open class Other { constructor(public int n) {} } class Hooked extends Other { int n { get => 1; } } \
    function viaBase(Base b): int { return b.n; } function viaOther(Other o): int { return o.n; }";

#[test]
fn a_hook_in_an_unrelated_hierarchy_keeps_the_plain_field_read() {
    let base = ops_of(SRC, "viaBase");
    assert!(
        base.iter().any(|op| matches!(op, Op::GetField(_))),
        "{base:?}"
    );
    assert!(
        !base.iter().any(|op| matches!(op, Op::GetFieldOrHook(..))),
        "{base:?}"
    );
}

#[test]
fn a_hook_in_a_descendant_dispatches_at_run_time() {
    let other = ops_of(SRC, "viaOther");
    assert!(
        other.iter().any(|op| matches!(op, Op::GetFieldOrHook(..))),
        "{other:?}"
    );
}
