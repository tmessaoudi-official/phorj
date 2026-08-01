//! Task 9 — accumulator overflow-check ELISION + in-bounds `Index` elision: a fail-closed
//! INTERVAL analysis over counted loops that proves whole families of `AddI`/`SubI`/`MulI`/
//! `RemI` can never overflow (the emitter drops their `*_overflow` + sticky accumulation —
//! and, when EVERY speculated op is proven, the sticky machinery itself) and that indexed
//! reads sit inside their collection's bounds (the emitter drops the bounds branch).
//!
//! SOUNDNESS MODEL. All interval arithmetic runs in i128 (never wraps). The OUTER loop's trip
//! count and counter are bounded by `G`: a compile-time-const loop bound is exact; a
//! never-written PARAM bound gains an ENTRY GUARD `param > G → code 5` (the call declines to
//! the VM — correct, just unspecialized; `G` is picked from a ladder `2^31 → 2^24 → 2^20`,
//! largest that verifies). v2 admits INNER counted loops (the for-in shape): each inner loop
//! must lead with a `j < T` guard where `T` is a const or the `Len` of a compile-time-known
//! collection, carry the canonical `j = j + 1` increment, and contain no further back-edge;
//! its counter is PINNED to `[0, T]` so one linear pass models every iteration, and an
//! accumulator site inside it multiplies its growth by `T` (per outer iteration). An
//! accumulator's whole-loop interval is `acc0 + G·envelope` where the per-iteration envelope
//! includes 0. Env-STABILITY (the whole walk runs twice and must reproduce its post-state)
//! rejects any hidden growing slot. Every eligibility condition FAILS CLOSED: a miss keeps
//! the checked emission (a perf miss, never a miscompile). Fault behavior is unchanged by
//! construction: ops are elided only where the fault is impossible, and the entry-guard
//! decline redoes the call on the VM, which faults canonically.
//!
//! V1+V2 SCOPE (the flip targets intadd/mapget/listindex + the for-in nest — anything else
//! keeps checks): one outer counted loop (+ non-nested inner counted loops); straight-line
//! entry prefix of {Const, GetLocal, SetLocal, MakeList, MakeMap}; loop bodies restricted to
//! the walked op set with only the recognized guard exits and back-edges.

use super::*;

/// The pass result: extra proven ips (merged into `range_proven_ops`'s vector by the caller —
/// `AddI`/`SubI`/`MulI` = plain wrapping op, `RemI` = band, `Index` = bounds branch dropped)
/// plus the entry guards to emit (`param value > max` ⇒ decline with code 5).
pub(super) struct AccElision {
    pub(super) proven: Vec<bool>,
    pub(super) guards: Vec<(usize, i64)>,
}

/// A prefix-time abstract slot value.
#[derive(Clone, Copy, PartialEq)]
enum Cell {
    /// Compile-time int constant.
    Int(i64),
    /// Const-built collection: value/element interval + element count.
    Coll(i64, i64, i64),
    /// Anything else (params, strings, runtime values).
    Other,
}

/// A body-walk abstract stack/env value.
#[derive(Clone, Copy, PartialEq)]
struct AbsVal {
    /// Int value interval (`None` = unknown / not an int).
    iv: Option<(i128, i128)>,
    /// Const-collection (value interval, element count) — a handle awaiting `Index`/`Len`.
    coll: Option<(i64, i64, i64)>,
    /// The accumulator/counter slot this value was read from (site detection / env pinning).
    acc_src: Option<usize>,
    /// Accumulated per-iteration GROWTH along an `acc + x + y + …` AddI chain rooted at a
    /// `GetLocal(acc)` (meaningful only with `acc_src`); `None` inside a chain = an unbounded
    /// term joined it. Recorded as the site growth when the chain lands in `SetLocal(acc)`.
    growth: Option<(i128, i128)>,
}

impl AbsVal {
    fn none() -> AbsVal {
        AbsVal {
            iv: None,
            coll: None,
            acc_src: None,
            growth: None,
        }
    }
    fn int(lo: i128, hi: i128) -> AbsVal {
        AbsVal {
            iv: Some((lo, hi)),
            coll: None,
            acc_src: None,
            growth: None,
        }
    }
}

/// One recognized INNER counted loop (v2): `[h, e]` with counter slot `counter` pinned to
/// `[0, T]`, `T` resolved at walk time (const bound, or `Len` of a known collection).
struct Inner {
    h: usize,
    e: usize,
    counter: usize,
    /// Ops the guard occupies (`h .. h + guard_len` — 4 for a const bound, 5 for `Len`).
    guard_len: usize,
    /// The canonical increment's `SetLocal` ip — body reads BEFORE it see `[0, T-1]` (the
    /// passed guard refines the pin); the guard's own read and later ips see `[0, T]`.
    inc: usize,
}

fn fits_i64(iv: (i128, i128)) -> bool {
    iv.0 >= i64::MIN as i128 && iv.1 <= i64::MAX as i128
}

/// Interval combine for one binary int op, exact in i128 (inputs are ≤ one op beyond i64
/// ranges, so a single step can never approach i128 bounds).
fn combine(op: &Op, a: (i128, i128), b: (i128, i128)) -> (i128, i128) {
    match op {
        Op::AddI => (a.0 + b.0, a.1 + b.1),
        Op::SubI => (a.0 - b.1, a.1 - b.0),
        _ => {
            let c = [a.0 * b.0, a.0 * b.1, a.1 * b.0, a.1 * b.1];
            (*c.iter().min().unwrap(), *c.iter().max().unwrap())
        }
    }
}

/// Run the task-9 analysis on one function. `base_proven` is `range_proven_ops`'s result
/// (the outer-counter proof feeds this pass). Returns `None` when the function is out of
/// scope — the caller keeps the base proofs and full checking.
pub(super) fn accumulator_elision(
    func: &crate::chunk::Function,
    base_proven: &[bool],
) -> Option<AccElision> {
    let code = &func.chunk.code;
    let reach = reachable(code);

    // ---- Loop structure: ONE outer loop, optionally containing non-nested inner loops ----
    let backs: Vec<(usize, usize)> = code
        .iter()
        .enumerate()
        .filter(|&(ip, _)| reach[ip])
        .filter_map(|(ip, op)| match op {
            Op::Jump(t) | Op::JumpIfFalse(t) if *t < ip => Some((ip, *t)),
            _ => None,
        })
        .collect();
    let (&(e, h), rest) = backs.split_last()?;
    // The outer back-edge must contain every other backward branch; inners must be disjoint
    // from each other (no deeper nesting).
    if rest.iter().any(|&(ei, hi)| !(h < hi && ei < e)) {
        return None;
    }
    for (i, &(ei, _)) in rest.iter().enumerate() {
        for &(ej, hj) in rest.iter().skip(i + 1) {
            if hj <= ei && ei <= ej {
                return None;
            }
        }
    }

    // ---- The OUTER counter (exactly one), proven HERE by shape --------------------------
    // `range_proven_ops`'s not-nested condition rejects a body containing inner back-edges,
    // so v2 re-proves the outer counter directly: the canonical `+1` increment outside every
    // inner region, single writer, named by the header guard. Same soundness argument — the
    // guard `s < V` re-checks every outer iteration (inner regions cannot write `s`: their
    // writes are validated below / caught by the walk).
    let counters: Vec<usize> = (h..e)
        .filter(|&k| {
            matches!(code[k], Op::AddI)
                && k >= 2
                && k + 1 < code.len()
                && matches!(code[k - 1], Op::Const(ci)
                    if matches!(func.chunk.consts.get(ci), Some(Value::Int(1))))
                && matches!((&code[k - 2], &code[k + 1]),
                    (Op::GetLocal(s), Op::SetLocal(t)) if s == t)
                && !rest.iter().any(|&(ei, hi)| hi <= k && k <= ei)
        })
        .filter(|&k| {
            let Op::GetLocal(s) = code[k - 2] else {
                return false;
            };
            // Single writer + the header guard reads this slot.
            let writers = code
                .iter()
                .enumerate()
                .filter(|&(ip, op)| reach[ip] && matches!(op, Op::SetLocal(t) if *t == s))
                .count();
            writers == 1 && matches!(code[h], Op::GetLocal(g) if g == s)
        })
        .collect();
    let &[ck] = counters.as_slice() else {
        return None;
    };
    let Op::GetLocal(counter) = code[ck - 2] else {
        return None;
    };
    // The outer counter's own increment is now proven by this pass too.
    let _ = base_proven;

    // ---- Inner loops: canonical guard + canonical `+1` increment --------------------------
    let mut inners: Vec<Inner> = Vec::new();
    for &(ei, hi) in rest {
        let Op::GetLocal(j) = code[hi] else {
            return None;
        };
        // Guard: `GetLocal(j); Const(T); Lt; JIF(>e)` or `GetLocal(j); GetLocal(c); Len;
        // Lt; JIF(>e)` — T resolves at walk time.
        let guard_len = match (code.get(hi + 1), code.get(hi + 2), code.get(hi + 3)) {
            (Some(Op::Const(_)), Some(Op::Lt), Some(Op::JumpIfFalse(x))) if *x > ei => 4,
            (Some(Op::GetLocal(_)), Some(Op::Len), Some(Op::Lt)) if matches!(code.get(hi + 4), Some(Op::JumpIfFalse(x)) if *x > ei) => {
                5
            }
            _ => return None,
        };
        // Exactly one write to `j` anywhere, inside the region, the canonical increment.
        let writers: Vec<usize> = code
            .iter()
            .enumerate()
            .filter(|&(ip, op)| reach[ip] && matches!(op, Op::SetLocal(t) if *t == j))
            .map(|(ip, _)| ip)
            .collect();
        let &[w] = writers.as_slice() else {
            return None;
        };
        if !(hi..=ei).contains(&w)
            || w < 3
            || !matches!(code[w - 1], Op::AddI)
            || !matches!(code[w - 2], Op::Const(ci)
                if matches!(func.chunk.consts.get(ci), Some(Value::Int(1))))
            || !matches!(code[w - 3], Op::GetLocal(g) if g == j)
        {
            return None;
        }
        inners.push(Inner {
            h: hi,
            e: ei,
            counter: j,
            guard_len,
            inc: w,
        });
    }

    // ---- Header guard bound of the OUTER loop: const or never-written param ---------------
    let mut guard_slot: Option<usize> = None;
    let const_limit: Option<i64> = match code[h + 1] {
        Op::Const(ci) => match func.chunk.consts.get(ci) {
            Some(Value::Int(c)) => Some(*c),
            _ => return None,
        },
        Op::GetLocal(bslot) => {
            let written = code
                .iter()
                .enumerate()
                .any(|(ip, op)| reach[ip] && matches!(op, Op::SetLocal(t) if *t == bslot));
            if written || bslot >= func.arity {
                return None;
            }
            guard_slot = Some(bslot);
            None
        }
        _ => return None,
    };
    if matches!(const_limit, Some(c) if c <= 0) {
        return None; // the body never runs — nothing to elide
    }

    // ---- Entry prefix: straight-line {Const, GetLocal, SetLocal, MakeList, MakeMap} -------
    let mut slots: Vec<Cell> = vec![Cell::Other; func.arity];
    for op in code.iter().take(h) {
        match op {
            Op::Const(ci) => slots.push(match func.chunk.consts.get(*ci) {
                Some(Value::Int(v)) => Cell::Int(*v),
                _ => Cell::Other,
            }),
            Op::GetLocal(s) => slots.push(*slots.get(*s)?),
            Op::SetLocal(s) => {
                let v = slots.pop()?;
                *slots.get_mut(*s)? = v;
            }
            Op::MakeList(m) => {
                if *m == 0 || slots.len() < *m {
                    return None;
                }
                let elems = slots.split_off(slots.len() - m);
                let ints: Option<Vec<i64>> = elems
                    .iter()
                    .map(|c| match c {
                        Cell::Int(v) => Some(*v),
                        _ => None,
                    })
                    .collect();
                slots.push(match ints {
                    Some(vs) => Cell::Coll(
                        *vs.iter().min().unwrap(),
                        *vs.iter().max().unwrap(),
                        *m as i64,
                    ),
                    // A non-int const list (e.g. strings) still has a KNOWN length — its
                    // value interval is unusable but `Len`/for-in trip counts are exact.
                    None => Cell::Coll(0, 0, *m as i64),
                });
            }
            Op::MakeMap(m) => {
                if *m == 0 || slots.len() < 2 * m {
                    return None;
                }
                let pairs = slots.split_off(slots.len() - 2 * m);
                let vals: Option<Vec<i64>> = pairs
                    .iter()
                    .skip(1)
                    .step_by(2)
                    .map(|c| match c {
                        Cell::Int(v) => Some(*v),
                        _ => None,
                    })
                    .collect();
                slots.push(match vals {
                    Some(vs) => Cell::Coll(
                        *vs.iter().min().unwrap(),
                        *vs.iter().max().unwrap(),
                        *m as i64,
                    ),
                    None => Cell::Other,
                });
            }
            _ => return None,
        }
    }

    // The outer counter's env seed is [0, G] — sound only for a const init ≥ 0.
    if !matches!(slots.get(counter), Some(Cell::Int(ci)) if *ci >= 0) {
        return None;
    }

    // ---- Accumulator candidates ------------------------------------------------------------
    let mut acc_slots: Vec<(usize, i64)> = Vec::new();
    for s in 0..slots.len() {
        if s == counter || inners.iter().any(|l| l.counter == s) {
            continue;
        }
        let writers: Vec<usize> = code
            .iter()
            .enumerate()
            .filter(|&(ip, op)| reach[ip] && matches!(op, Op::SetLocal(t) if *t == s))
            .map(|(ip, _)| ip)
            .collect();
        if writers.is_empty() {
            continue;
        }
        let all_acc_shape = writers
            .iter()
            .all(|&w| (h..=e).contains(&w) && w >= 1 && matches!(code[w - 1], Op::AddI));
        if !all_acc_shape {
            continue;
        }
        if let Some(Cell::Int(init)) = slots.get(s).copied() {
            acc_slots.push((s, init));
        }
    }

    // ---- G ladder: the largest bound that verifies -----------------------------------------
    let ladder: Vec<i64> = match const_limit {
        Some(c) => vec![c],
        None => vec![1 << 31, 1 << 24, 1 << 20],
    };
    for g in ladder {
        if let Some(mut r) =
            verify_with_g(func, &reach, &slots, counter, &acc_slots, &inners, h, e, g)
        {
            if let Some(bslot) = guard_slot {
                r.guards.push((bslot, g));
            }
            return Some(r);
        }
    }
    None
}

mod verify;
mod walk;
use verify::verify_with_g;
use walk::walk_body;
