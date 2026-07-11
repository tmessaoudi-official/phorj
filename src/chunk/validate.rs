//! `BytecodeProgram::validate` — the wildcard-free structural validator (Invariant 3).

use super::*;

impl BytecodeProgram {
    /// Check that every index-carrying instruction references something in range, before the VM
    /// executes a single op. An out-of-range `Const`/`Call`/jump is always a *compiler* bug, never
    /// user error — but surfacing it as a clean `Err` (rather than a bare `index out of bounds`
    /// panic, or a silent wrong read) keeps the VM's no-crash contract (EV-7). Slot operands
    /// (`GetLocal`/`SetLocal`) can't be range-checked here — their bound is the runtime locals
    /// window, not anything static — so they stay covered by the VM's `frame_slot` debug-assert.
    ///
    /// P4a added the index-carrying ops `MakeEnum`/`MatchTag` (into `enum_descs`); P4b added
    /// `MakeInstance` (into `class_descs`) and `GetField` (into the `names` pool); P4c adds
    /// `CallMethod` (name into the `names` pool; its function target is resolved at runtime via the
    /// method table, range-checked after the per-op loop). M3 Wave 1 adds `CallNative` (index into
    /// `native::registry()`). Each new index-carrying op extends the
    /// match below in lockstep (see memory `op-variant-match-coupling`). `GetEnumField` carries a
    /// payload index with no static bound (like a local slot) — covered by the VM's runtime guard;
    /// M3 S1.2's `MakeRange(bool)` carries a flag, not an index, so it likewise needs no arm here.
    pub fn validate(&self) -> Result<(), String> {
        let nfns = self.functions.len();
        if self.main >= nfns {
            return Err(format!(
                "invalid bytecode: main index {} out of range ({nfns} functions)",
                self.main
            ));
        }
        let ndescs = self.enum_descs.len();
        let nclasses = self.class_descs.len();
        let nnames = self.names.len();
        let nstatics = self.static_inits.len();
        let nnatives = crate::native::registry().len();
        for (fi, f) in self.functions.iter().enumerate() {
            let code_len = f.chunk.code.len();
            let const_len = f.chunk.consts.len();
            for (ip, op) in f.chunk.code.iter().enumerate() {
                // Exhaustive over `Op` — deliberately NO `_` wildcard. Every variant either
                // carries a pool index that is range-checked here, or is listed in the no-index
                // arm below. A newly added `Op` is therefore a COMPILE ERROR in this match until
                // its bounds intent is declared, closing the old `_ => None` gap that let a new
                // index-carrying op skip its EV-7 check (QW-15 / P1-#16). `exec_op` (src/vm.rs)
                // and `stack_effect` (src/compiler.rs) are already exhaustive; this brings
                // `validate` to the same guarantee. The `.then(|| …)` arms reject on exactly the
                // same condition as the previous guarded arms — rejection behaviour is unchanged.
                let problem = match op {
                    Op::Const(i) => (*i >= const_len)
                        .then(|| format!("const index {i} out of range (pool has {const_len})")),
                    Op::Call(idx) => (*idx >= nfns)
                        .then(|| format!("call target {idx} out of range ({nfns} functions)")),
                    // Green-thread `spawn f(args)` (S4.3): the deferred free-function index.
                    Op::SpawnCall(idx, _) => (*idx >= nfns).then(|| {
                        format!("spawn target {idx} out of range ({nfns} functions)")
                    }),
                    // M-RT super/parent: the parent target is a baked function index.
                    Op::CallParent(idx, _) => (*idx >= nfns).then(|| {
                        format!("parent-call target {idx} out of range ({nfns} functions)")
                    }),
                    Op::CallOverload(sid, _) | Op::CallStaticOverload(sid, _) => {
                        if *sid >= self.overloads.len() {
                            Some(format!(
                                "overload set {sid} out of range ({} sets)",
                                self.overloads.len()
                            ))
                        } else {
                            self.overloads[*sid].iter().find_map(|(_, idx)| {
                                (*idx >= nfns).then(|| {
                                    format!(
                                        "overload target {idx} out of range ({nfns} functions)"
                                    )
                                })
                            })
                        }
                    }
                    Op::MakeEnum(idx) | Op::MatchTag(idx) => (*idx >= ndescs).then(|| {
                        format!("enum descriptor index {idx} out of range ({ndescs} descriptors)")
                    }),
                    Op::MakeInstance(idx) => (*idx >= nclasses).then(|| {
                        format!(
                            "class descriptor index {idx} out of range ({nclasses} descriptors)"
                        )
                    }),
                    Op::GetField(idx) | Op::SetField(idx) | Op::CallMethod(idx, _) => {
                        (*idx >= nnames).then(|| {
                            format!("field-name index {idx} out of range (name pool has {nnames})")
                        })
                    }
                    Op::CallNative(idx, _) => (*idx >= nnatives).then(|| {
                        format!("native index {idx} out of range (registry has {nnatives})")
                    }),
                    Op::GetStatic(idx) | Op::SetStatic(idx) => (*idx >= nstatics).then(|| {
                        format!("static index {idx} out of range ({nstatics} statics)")
                    }),
                    // Absolute targets; `== code_len` is the legal "fall off the end → implicit
                    // return" landing the run loop already handles, so only `>` is invalid.
                    // A handler's catch landing pad is an absolute code index like a jump target.
                    Op::Jump(t) | Op::JumpIfFalse(t) | Op::PushHandler(t) => (*t > code_len)
                        .then(|| format!("jump target {t} out of range (code len {code_len})")),
                    // `MakeClosure` carries a function-table index (must be in range).
                    Op::MakeClosure(idx) => (*idx >= nfns)
                        .then(|| format!("closure target {idx} out of range ({nfns} functions)")),
                    // No pool index to range-check here. These carry either nothing, a count
                    // (`Concat`/`MakeList`/`CallValue` arg counts), a local stack slot
                    // (`GetLocal`/`SetLocal`, bounded by frame sizing, not a pool), or a payload
                    // index a preceding `MatchTag` already proved (`GetEnumField`). Listed
                    // explicitly so the match stays exhaustive.
                    Op::AddI
                    | Op::SubI
                    | Op::MulI
                    | Op::DivI
                    | Op::RemI
                    | Op::AddF
                    | Op::SubF
                    | Op::MulF
                    | Op::DivF
                    | Op::RemF
                    | Op::AddD
                    | Op::SubD
                    | Op::MulD
                    | Op::RemD
                    | Op::DivD
                    | Op::BitAnd
                    | Op::BitOr
                    | Op::BitXor
                    | Op::Shl
                    | Op::Shr
                    | Op::Neg
                    | Op::Not
                    | Op::BitNot
                    | Op::Eq
                    | Op::Ne
                    | Op::Lt
                    | Op::Gt
                    | Op::Le
                    | Op::Ge
                    | Op::Pop
                    | Op::GetLocal(_)
                    | Op::SetLocal(_)
                    | Op::SetIndexLocal(_)
                    | Op::SetPathLocal(_, _)
                    | Op::Concat(_)
                    | Op::MakeList(_)
                    | Op::MakeMap(_)
                    | Op::Index
                    | Op::SetIndex
                    | Op::Len
                    | Op::IterElems
                    | Op::MakeRange(_)
                    | Op::Return
                    | Op::GetEnumField(_)
                    | Op::Fault(_)
                    | Op::CallValue(_)
                    // `Throw`/`PopHandler` carry nothing (like `Fault`/`Return`); `Throw`'s value is
                    // on the stack and `PopHandler` just discards the top handler.
                    | Op::Throw
                    | Op::PopHandler
                    // Carries the class name inline (like `Fault`), not a pool index.
                    | Op::IsInstance(_)
                    // Green-thread ops (M6 W4) carry no pool index — operands are on the stack.
                    | Op::Spawn
                    | Op::ChannelNew
                    | Op::ChannelSend
                    | Op::ChannelRecv
                    | Op::Join => None,
                };
                if let Some(what) = problem {
                    return Err(format!(
                        "invalid bytecode in fn `{}` (#{fi}) at ip {ip}: {what}",
                        f.name
                    ));
                }
            }
        }
        // `Op::CallMethod` resolves its target through the method table at runtime (the function
        // index isn't in the op), so range-check every dispatch target here instead.
        for ((class, method), &idx) in &self.methods {
            if idx >= nfns {
                return Err(format!(
                    "invalid bytecode: method `{class}::{method}` target {idx} out of range ({nfns} functions)"
                ));
            }
        }
        Ok(())
    }
}
