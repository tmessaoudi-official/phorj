//! L2b `this`-field TAKE analysis for an OWNED-`this` method body (moved out of `analyze/mod.rs`
//! with plan row 4b step 5, which made the row-4b runtime-class ops reachable here).

use super::*;

/// L2b field-TAKE plan for an OWNED-`this` method body: `plan[j] = Some(ip)` means field
/// j's word MOVES out at the `GetField` at `ip` (its LAST static read — max ip; with only
/// forward branches no read of j can execute after it on any path, and a not-taken branch
/// simply leaves the field for the husk release). The take poisons the husk slot
/// (`UB_TAG_SLOT|UB_IDX_MASK`): the teardown release no-ops on it, drift reads fault the
/// helpers to a code-5 redo — never wrong bytes. Whole-method gates:
///  - NO `SetField` (a write through `this` could recreate/observe the void), and no
///    `GetLocal(0); SetLocal(_)` (an alias could re-read the husk);
///  - a field with ANY read inside a backward-jump span is excluded (a loop iteration
///    after the take would read the poison); methods with no backward jump skip this;
///  - fields in the TRANSITIVE this-read set of every method name resolving on our class
///    that this body calls are excluded (`this` may escape as a receiver into them and
///    they read those fields from the SAME husk — over-approximated to all same-class
///    call names, conservative). `this` cannot escape any other way inside the unboxed
///    subset: Inst call args are rejected, borrowed-Inst returns from Inst-param
///    functions are rejected, and MakeInstance accepts no Inst ctor args.
pub(in crate::jit) fn owned_this_taken_fields(
    program: &BytecodeProgram,
    func_idx: usize,
    class: usize,
) -> Vec<Option<usize>> {
    let desc = &program.class_descs[class];
    let nfields = desc.fields.len();
    let none: Vec<Option<usize>> = vec![None; nfields];
    let code = &program.functions[func_idx].chunk.code;
    // Backward-jump spans (loops): any field read inside one is excluded.
    let loop_spans: Vec<(usize, usize)> = code
        .iter()
        .enumerate()
        .filter_map(|(ip, op)| match op {
            Op::Jump(t) | Op::JumpIfFalse(t) if *t <= ip => Some((*t, ip)),
            _ => None,
        })
        .collect();
    let reach = reachable(code);
    let is_leader = leaders(code, &reach, &program.functions[func_idx].chunk.consts);
    let mut last_read: Vec<Option<usize>> = vec![None; nfields];
    let mut looped: Vec<bool> = vec![false; nfields];
    let mut escape_reads: Vec<bool> = vec![false; nfields];
    for (ip, op) in code.iter().enumerate() {
        if !reach[ip] {
            continue;
        }
        match op {
            // Row 4b: a runtime-class op may run a hook that reads any field of `this` — no take.
            Op::SetField(_)
            | Op::GetFieldOrHook(..)
            | Op::SetFieldOrHook(..)
            | Op::CloneInstance => return none,
            Op::GetLocal(0) => match code.get(ip + 1) {
                Some(Op::GetField(nidx)) if !is_leader.get(ip + 1).copied().unwrap_or(true) => {
                    if let Some(j) = desc.fields.iter().position(|f| f == &program.names[*nidx]) {
                        let rip = ip + 1;
                        last_read[j] = Some(last_read[j].map_or(rip, |p: usize| p.max(rip)));
                        if loop_spans.iter().any(|(a, b)| (*a..=*b).contains(&rip)) {
                            looped[j] = true;
                        }
                    }
                }
                Some(Op::SetLocal(_)) => return none, // an alias could re-read the husk
                _ => {}
            },
            Op::CallMethod(nidx, _) => {
                let key = (desc.class.to_string(), program.names[*nidx].clone());
                if let Some(&t) = program.methods.get(&key) {
                    let mut visited = vec![false; program.functions.len()];
                    mark_this_read_fields(program, class, t, &mut visited, &mut escape_reads);
                }
            }
            _ => {}
        }
    }
    (0..nfields)
        .map(|j| {
            if looped[j] || escape_reads[j] {
                None
            } else {
                last_read[j]
            }
        })
        .collect()
}

/// The TRANSITIVE this-read field set of `func` on `class`: every field its body reads via a
/// `GetLocal(0); GetField` pair, plus (recursively) those of every method name it calls that
/// resolves on the same class (a this-receiver call can only target our class; other-receiver
/// same-class calls are included too — over-approximation, conservative).
fn mark_this_read_fields(
    program: &BytecodeProgram,
    class: usize,
    func: usize,
    visited: &mut [bool],
    out: &mut [bool],
) {
    if visited[func] {
        return;
    }
    visited[func] = true;
    let desc = &program.class_descs[class];
    let code = &program.functions[func].chunk.code;
    for (ip, op) in code.iter().enumerate() {
        match op {
            Op::GetLocal(0) => match code.get(ip + 1) {
                Some(Op::GetField(nidx)) => {
                    if let Some(j) = desc.fields.iter().position(|f| f == &program.names[*nidx]) {
                        out[j] = true;
                    }
                }
                Some(Op::SetLocal(_)) => {
                    // An alias inside the callee could read fields invisibly to this scan —
                    // poison the WHOLE set defensively.
                    for o in out.iter_mut() {
                        *o = true;
                    }
                }
                // Any other stray is a further this-receiver escape — the CallMethod
                // recursion below unions every same-class call name's read set, so the
                // fields it could read are already covered.
                _ => {}
            },
            Op::CallMethod(nidx, _) => {
                let key = (desc.class.to_string(), program.names[*nidx].clone());
                if let Some(&t) = program.methods.get(&key) {
                    mark_this_read_fields(program, class, t, visited, out);
                }
            }
            // Row 4b: the runtime class's hook may read any field — poison the whole set.
            Op::GetFieldOrHook(..) | Op::SetFieldOrHook(..) | Op::CloneInstance => {
                for o in out.iter_mut() {
                    *o = true;
                }
            }
            _ => {}
        }
    }
}
