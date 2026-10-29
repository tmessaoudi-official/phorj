//! Plan row 4b step 5: the unboxed lowering of `Op::GetFieldOrHook` / `Op::SetFieldOrHook`. The VM
//! decides them at run time (`vm/runtime_class.rs`); the unboxed tier knows the receiver's EXACT class
//! (`Kind::Inst` joins only equal classes), so each site resolves at compile time to the op the VM
//! would take for that class — its own `<name>$get`/`$set` (`CallMethod`, the `methods` table holding
//! own and `use`d-trait hooks, never a parent's, as the oracle looks them up) or the stored field
//! (`GetField` / `SetField`). Analyze and emit call the same function, so both see the same op.

use super::*;

/// A runtime-class op resolved for the unboxed tier.
pub(in crate::jit) struct Resolved {
    pub(in crate::jit) op: Op,
    /// The write's FIELD case: `SetField` pushes nothing, so the `Pop` the compiler emits after
    /// `SetFieldOrHook` (to drop its `Unit`) is consumed with it — the next ip is skipped.
    pub(in crate::jit) consumes_next_pop: bool,
}

/// The op the unboxed tier compiles at `code[ip]` given the kind stack, or `None` when that op is
/// not a runtime-class op (the caller then matches it itself). A receiver whose class is not known
/// exactly declines, as `GetField` / `SetField` / `CallMethod` do; so does a write whose `Pop` is not
/// the next op in the same block.
pub(in crate::jit) fn resolve_runtime_class_op(
    program: &BytecodeProgram,
    kinds: &[Kind],
    code: &[Op],
    is_leader: &[bool],
    ip: usize,
) -> Result<Option<Resolved>, JitError> {
    let (field, hook, argc) = match code[ip] {
        Op::GetFieldOrHook(f, h) => (f, h, 0),
        Op::SetFieldOrHook(f, h) => (f, h, 1),
        _ => return Ok(None),
    };
    let receiver = kinds.len().checked_sub(argc + 1).and_then(|i| kinds.get(i));
    let Some(Kind::Inst(c, _)) = receiver else {
        return Err(JitError::Unsupported(format!(
            "unboxed: {:?} on {receiver:?} (deferred)",
            code[ip]
        )));
    };
    let key = (
        program.class_descs[*c].class.to_string(),
        program.names[hook].clone(),
    );
    if program.methods.contains_key(&key) {
        return Ok(Some(Resolved {
            op: Op::CallMethod(hook, argc),
            consumes_next_pop: false,
        }));
    }
    if argc == 0 {
        return Ok(Some(Resolved {
            op: Op::GetField(field),
            consumes_next_pop: false,
        }));
    }
    let pop_follows = matches!(code.get(ip + 1), Some(Op::Pop))
        && !is_leader.get(ip + 1).copied().unwrap_or(true);
    if !pop_follows {
        return Err(JitError::Unsupported(
            "unboxed: SetFieldOrHook without its Pop (deferred)".to_string(),
        ));
    }
    Ok(Some(Resolved {
        op: Op::SetField(field),
        consumes_next_pop: true,
    }))
}
