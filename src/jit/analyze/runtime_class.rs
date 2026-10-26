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

/// The clone repr of a field kind for `rt_u_clone_value` (`None` = a scalar word, copied as-is), or
/// `Err` for a kind the copy cannot carry (a map has no clone repr; anything else is not an
/// instance field in the unboxed subset). Analyze and emit both gate on it.
pub(in crate::jit) fn clone_repr(fk: Kind) -> Result<Option<i64>, JitError> {
    match fk {
        Kind::Int => Ok(None),
        Kind::Str(_) => Ok(Some(2)),
        Kind::StrList(_) => Ok(Some(3)),
        Kind::IntList(_) => Ok(Some(4)),
        Kind::DynList(_) => Ok(Some(5)),
        other => Err(JitError::Unsupported(format!(
            "unboxed: CloneInstance field kind {other:?} (deferred)"
        ))),
    }
}

/// Analyze `Op::CloneInstance` (step 5b) — the stack effect and gates the emit arm
/// (`emit_unboxed/clone.rs`) relies on: an exact `Kind::Inst(c)` receiver, a class `MakeInstance`
/// could build (every field ctor-initialized, ≤ 15), and every field kind known and copyable. Pushes
/// the OWNED copy.
pub(in crate::jit) fn analyze_clone_instance(
    program: &BytecodeProgram,
    info: &UbGraphInfo,
    kinds: &mut Vec<Kind>,
) -> Result<(), JitError> {
    let k = kinds
        .pop()
        .ok_or_else(|| JitError::Codegen("unboxed analyze: CloneInstance underflow".to_string()))?;
    let Kind::Inst(c, _) = k else {
        return Err(JitError::Unsupported(format!(
            "unboxed: CloneInstance on {k:?} (deferred)"
        )));
    };
    let desc = &program.class_descs[c];
    let nf = desc.fields.len();
    if desc.layout.len() != nf || nf > 15 {
        return Err(JitError::Unsupported(
            "unboxed: CloneInstance of a non-ctor-initialized or >15-field class (deferred)"
                .to_string(),
        ));
    }
    for j in 0..nf {
        let fk = info.field_kind(c, j).ok_or_else(|| {
            JitError::Unsupported(
                "unboxed: CloneInstance before class signature is known (fixpoint retry)"
                    .to_string(),
            )
        })?;
        clone_repr(fk)?;
    }
    kinds.push(Kind::Inst(c, Own::Owned));
    Ok(())
}
