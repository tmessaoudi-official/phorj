//! Task 9 — accumulator overflow-check ELISION (the checked-add price, measured at 1.5M ns of
//! mapget's 11.9M VM leg and the whole intadd gap): a fail-closed INTERVAL analysis over a
//! single counted loop that proves whole families of `AddI`/`SubI`/`MulI`/`RemI` can never
//! overflow, so the emitter drops their `*_overflow` + sticky accumulation (and, when EVERY
//! speculated op is proven, the sticky machinery itself).
//!
//! SOUNDNESS MODEL. All interval arithmetic runs in i128 (never wraps). The loop's trip count
//! and the counter's range are bounded by `G`: when the loop bound is a compile-time const,
//! `G` = that const; when it is a never-written PARAM, the function gains an ENTRY GUARD
//! `param > G → code 5` (the call declines to the VM — correct, just unspecialized; `G` is
//! picked from a ladder `2^31 → 2^24 → 2^20`, largest that verifies). An accumulator's
//! whole-loop interval is `acc0 + G·envelope` where the per-iteration envelope includes 0,
//! so any prefix of ≤ G iterations stays inside it. Every eligibility condition FAILS
//! CLOSED: a miss keeps the checked emission (a perf miss, never a miscompile). Fault
//! behavior is unchanged by construction: an op is elided only where overflow is impossible,
//! so checked and unchecked semantics coincide — and the entry-guard decline redoes the call
//! on the VM, which faults canonically if the real inputs ever would.
//!
//! V1 SCOPE (matches the flip targets intadd/mapget/listindex — anything else keeps checks):
//! single-loop functions; straight-line entry prefix of {Const, GetLocal, SetLocal, MakeList,
//! MakeMap}; branch-free loop body (only the header guard exit and the back-edge); a
//! range-proven `+1` counter with const init ≥ 0; loop bound = const or never-written param.

use super::*;

/// The pass result: extra proven ips (merged into `range_proven_ops`'s vector by the caller)
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
    /// Const-built collection (int list, or string→int map): value/element interval.
    Coll(i64, i64),
    /// Anything else (params, strings, runtime values).
    Other,
}

/// A body-walk abstract stack/env value.
#[derive(Clone, Copy, PartialEq)]
struct AbsVal {
    /// Int value interval (`None` = unknown / not an int).
    iv: Option<(i128, i128)>,
    /// Const-collection element/value interval (a collection handle awaiting `Index`).
    coll: Option<(i64, i64)>,
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

/// Run the task-9 accumulator-elision analysis on one function. `base_proven` is
/// `range_proven_ops`'s result (the counter proof feeds this pass). Returns `None` when the
/// function is out of the v1 scope — the caller keeps the base proofs and full checking.
pub(super) fn accumulator_elision(
    func: &crate::chunk::Function,
    base_proven: &[bool],
) -> Option<AccElision> {
    let code = &func.chunk.code;
    let reach = reachable(code);

    // ---- The single counted loop ---------------------------------------------------------
    let backs: Vec<(usize, usize)> = code
        .iter()
        .enumerate()
        .filter(|&(ip, _)| reach[ip])
        .filter_map(|(ip, op)| match op {
            Op::Jump(t) | Op::JumpIfFalse(t) if *t < ip => Some((ip, *t)),
            _ => None,
        })
        .collect();
    let &[(e, h)] = backs.as_slice() else {
        return None;
    };

    // ---- The proven counter of that loop (exactly one) ------------------------------------
    let counters: Vec<usize> = (h..e)
        .filter(|&k| {
            base_proven[k]
                && matches!(code[k], Op::AddI)
                && k >= 2
                && matches!(code[k - 2], Op::GetLocal(_))
                && matches!(code[k - 1], Op::Const(_))
        })
        .collect();
    let &[ck] = counters.as_slice() else {
        return None;
    };
    let Op::GetLocal(counter) = code[ck - 2] else {
        return None;
    };

    // ---- Header guard bound: const or never-written param ----------------------------------
    // (`range_proven_ops` already verified the `GetLocal(counter); bound; Lt; JumpIfFalse`
    // header shape for the counter proof — re-read the bound operand here.)
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
                // v1: only a never-written PARAM gets a runtime entry guard (its entry
                // value IS the loop bound); a computed local fails closed.
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
                    Some(vs) => Cell::Coll(*vs.iter().min().unwrap(), *vs.iter().max().unwrap()),
                    None => Cell::Other,
                });
            }
            Op::MakeMap(m) => {
                if *m == 0 || slots.len() < 2 * m {
                    return None;
                }
                let pairs = slots.split_off(slots.len() - 2 * m);
                // Values sit at odd positions (k1, v1, k2, v2, …).
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
                    Some(vs) => Cell::Coll(*vs.iter().min().unwrap(), *vs.iter().max().unwrap()),
                    None => Cell::Other,
                });
            }
            _ => return None, // out of the v1 prefix op set — fail closed
        }
    }

    // The counter's env seed is [0, G] — sound only for a const init ≥ 0.
    if !matches!(slots.get(counter), Some(Cell::Int(ci)) if *ci >= 0) {
        return None;
    }

    // ---- Accumulator candidates ------------------------------------------------------------
    // A slot whose EVERY reachable `SetLocal` sits in the body right after an `AddI`, with a
    // prefix-const init. (A body-written slot that is NOT a candidate is caught by the
    // env-stability walk below if its interval grows across iterations.)
    let mut acc_slots: Vec<(usize, i64)> = Vec::new();
    for s in 0..slots.len() {
        if s == counter {
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
        if let Some(mut r) = verify_with_g(func, &reach, &slots, counter, &acc_slots, h, e, g) {
            if let Some(bslot) = guard_slot {
                r.guards.push((bslot, g));
            }
            return Some(r);
        }
    }
    None
}

/// One verification attempt at trip/counter bound `g`: pass A (collect site growths with
/// accumulator reads unknown), solve the accumulator envelopes, pass B (full intervals +
/// elide marks + i64-fit checks), then an env-STABILITY walk — pass B re-run from pass B's
/// post-body env must reproduce it exactly (this catches a hidden growing slot that is not a
/// proven accumulator: its second-walk intervals widen and the comparison fails closed).
#[allow(clippy::too_many_arguments)] // analysis plumbing
fn verify_with_g(
    func: &crate::chunk::Function,
    reach: &[bool],
    slots: &[Cell],
    counter: usize,
    acc_slots: &[(usize, i64)],
    h: usize,
    e: usize,
    g: i64,
) -> Option<AccElision> {
    let code = &func.chunk.code;
    let is_acc = |s: usize| acc_slots.iter().any(|&(a, _)| a == s);
    let base_env = |acc_iv: &dyn Fn(usize) -> Option<(i128, i128)>| -> Vec<AbsVal> {
        (0..slots.len())
            .map(|s| {
                if s == counter {
                    return AbsVal {
                        acc_src: Some(s),
                        ..AbsVal::int(0, g as i128)
                    };
                }
                if let Some(iv) = acc_iv(s) {
                    return AbsVal {
                        iv: Some(iv),
                        coll: None,
                        acc_src: Some(s),
                        growth: None,
                    };
                }
                match slots[s] {
                    Cell::Int(c) => AbsVal::int(c as i128, c as i128),
                    Cell::Coll(lo, hi) => AbsVal {
                        iv: None,
                        coll: Some((lo, hi)),
                        acc_src: None,
                        growth: None,
                    },
                    Cell::Other => AbsVal::none(),
                }
            })
            .collect()
    };

    // Pass A: accumulator reads are UNKNOWN (poison) — collect per-site growth intervals,
    // which must not depend on the accumulator itself.
    let env_a = base_env(&|s| if is_acc(s) { Some((0, 0)) } else { None });
    let mut site_growth: Vec<(usize, (i128, i128))> = Vec::new();
    walk_body(
        func,
        reach,
        h,
        e,
        env_a,
        true,
        &is_acc,
        counter,
        &mut site_growth,
        None,
    )?;

    // Solve the envelopes: acc ∈ acc0 + G·[Σ min(growth.lo, 0), Σ max(growth.hi, 0)].
    let mut acc_iv: Vec<(usize, (i128, i128))> = Vec::new();
    for &(s, init) in acc_slots {
        let sites: Vec<(i128, i128)> = site_growth
            .iter()
            .filter(|(slot, _)| *slot == s)
            .map(|(_, iv)| *iv)
            .collect();
        if sites.is_empty() {
            return None; // a candidate with no proven site growth — fail closed
        }
        let lo: i128 = sites.iter().map(|iv| iv.0.min(0)).sum();
        let hi: i128 = sites.iter().map(|iv| iv.1.max(0)).sum();
        let iv = (init as i128 + g as i128 * lo, init as i128 + g as i128 * hi);
        if !fits_i64(iv) {
            return None;
        }
        acc_iv.push((s, iv));
    }

    // Pass B: full intervals → elide marks + fit checks.
    let lookup = |s: usize| acc_iv.iter().find(|(a, _)| *a == s).map(|(_, iv)| *iv);
    let env_b = base_env(&lookup);
    let mut sink: Vec<(usize, (i128, i128))> = Vec::new();
    let mut proven = vec![false; code.len()];
    let env_after = walk_body(
        func,
        reach,
        h,
        e,
        env_b,
        false,
        &is_acc,
        counter,
        &mut sink,
        Some(&mut proven),
    )?;
    // Every accumulator site must itself be proven (else its elision assumption is void).
    for (ip, op) in code.iter().enumerate() {
        if !(h..=e).contains(&ip) {
            continue;
        }
        if matches!(op, Op::SetLocal(t) if is_acc(*t)) && !proven[ip - 1] {
            return None;
        }
    }
    // Env stability.
    let mut sink2: Vec<(usize, (i128, i128))> = Vec::new();
    let mut proven2 = vec![false; code.len()];
    let env_after2 = walk_body(
        func,
        reach,
        h,
        e,
        env_after.clone(),
        false,
        &is_acc,
        counter,
        &mut sink2,
        Some(&mut proven2),
    )?;
    if env_after != env_after2 || proven != proven2 {
        return None;
    }
    Some(AccElision {
        proven,
        guards: Vec::new(),
    })
}

/// Linear abstract walk of the loop body `[h, e]` with a depth-indexed `AbsVal` stack over
/// the locals env. `pass_a` = accumulator reads are UNKNOWN (poison anything they feed) and
/// only site growths are collected; otherwise full intervals flow and `proven` marks every
/// `AddI`/`SubI`/`MulI` whose i128 result fits i64 and every `RemI` by a positive pow2 const
/// with a provably non-negative dividend. Returns the post-body locals env; `None` = out of
/// the v1 body op set / stack discipline broke (fail closed).
#[allow(clippy::too_many_arguments)] // analysis plumbing
fn walk_body(
    func: &crate::chunk::Function,
    reach: &[bool],
    h: usize,
    e: usize,
    mut env: Vec<AbsVal>,
    pass_a: bool,
    is_acc: &dyn Fn(usize) -> bool,
    counter: usize,
    site_growth: &mut Vec<(usize, (i128, i128))>,
    mut proven: Option<&mut Vec<bool>>,
) -> Option<Vec<AbsVal>> {
    let code = &func.chunk.code;
    let mut st: Vec<AbsVal> = Vec::new();
    for ip in h..=e {
        if !reach[ip] {
            return None;
        }
        match &code[ip] {
            Op::GetLocal(s) => {
                // Locals and temporaries are ONE stack: a slot below the prefix length is a
                // function local (the frozen env); at or above it, a BODY-scoped local that
                // lives in the walk's own operand stack (`st[s - env.len()]`).
                let mut v = if *s < env.len() {
                    *env.get(*s)?
                } else {
                    *st.get(*s - env.len())?
                };
                if is_acc(*s) {
                    v.growth = Some((0, 0)); // a fresh read roots a growth chain
                    if pass_a {
                        v.iv = None; // pass A: the accumulator's value is not yet solved
                    }
                }
                st.push(v);
            }
            Op::Const(ci) => match func.chunk.consts.get(*ci) {
                Some(Value::Int(c)) => st.push(AbsVal::int(*c as i128, *c as i128)),
                _ => st.push(AbsVal::none()),
            },
            Op::SetLocal(s) => {
                let v = st.pop()?;
                if is_acc(*s) {
                    // Every accumulator write must be a bounded self-chain (`acc + …`
                    // rooted at THIS slot) — anything else voids the envelope. Fail closed.
                    if v.acc_src != Some(*s) {
                        return None;
                    }
                    site_growth.push((*s, v.growth?));
                    // The env keeps the SOLVED whole-loop envelope — it covers every
                    // iteration's value by construction, so the write changes nothing.
                } else if *s == counter {
                    // Same: the counter env stays pinned to its [0, G] seed.
                } else if *s < env.len() {
                    *env.get_mut(*s)? = AbsVal {
                        acc_src: None,
                        growth: None,
                        ..v
                    };
                } else {
                    // Body-scoped local: rewrite its cell in the walk's operand stack.
                    let idx = *s - env.len();
                    *st.get_mut(idx)? = AbsVal {
                        acc_src: None,
                        growth: None,
                        ..v
                    };
                }
            }
            Op::Pop => {
                st.pop()?;
            }
            Op::AddI | Op::SubI | Op::MulI => {
                let bv = st.pop()?;
                let av = st.pop()?;
                // An `AddI` whose LEFT operand rides an accumulator chain EXTENDS the chain
                // (`acc + x + y` — growth accumulates); `SubI`/`MulI` break it.
                let (acc_src, growth) = if matches!(code[ip], Op::AddI) && av.acc_src.is_some() {
                    (
                        av.acc_src,
                        av.growth.zip(bv.iv).map(|(gr, b)| (gr.0 + b.0, gr.1 + b.1)),
                    )
                } else {
                    (None, None)
                };
                let iv = match (av.iv, bv.iv) {
                    (Some(a), Some(b)) => {
                        let iv = combine(&code[ip], a, b);
                        if fits_i64(iv) {
                            if let Some(p) = proven.as_deref_mut() {
                                p[ip] = true;
                            }
                        }
                        Some(iv)
                    }
                    _ => None,
                };
                st.push(AbsVal {
                    iv,
                    coll: None,
                    acc_src,
                    growth,
                });
            }
            Op::RemI => {
                let bv = st.pop()?;
                let av = st.pop()?;
                let iv = match bv.iv {
                    Some((c, c2)) if c == c2 && c != 0 => {
                        let cabs = c.unsigned_abs();
                        let pow2 = c > 0 && (c & (c - 1)) == 0;
                        let nonneg = matches!(av.iv, Some((lo, _)) if lo >= 0);
                        // The emit fast path reads the divisor const at ip-1.
                        let const_prev = ip >= 1 && matches!(code[ip - 1], Op::Const(_));
                        if pow2 && nonneg && const_prev {
                            if let Some(p) = proven.as_deref_mut() {
                                p[ip] = true;
                            }
                            Some((0, c - 1))
                        } else {
                            Some((-(cabs as i128 - 1), cabs as i128 - 1))
                        }
                    }
                    _ => None,
                };
                st.push(AbsVal {
                    iv,
                    coll: None,
                    acc_src: None,
                    growth: None,
                });
            }
            Op::DivI | Op::Lt => {
                st.pop()?;
                st.pop()?;
                st.push(AbsVal::none());
            }
            Op::Index => {
                let _idx = st.pop()?;
                let coll = st.pop()?;
                st.push(AbsVal {
                    iv: coll.coll.map(|(lo, hi)| (lo as i128, hi as i128)),
                    coll: None,
                    acc_src: None,
                    growth: None,
                });
            }
            Op::JumpIfFalse(t) => {
                st.pop()?;
                // Only the header's forward exit and a conditional back-edge are in scope.
                if !((ip == h + 3 && *t > e) || (ip == e && *t == h)) {
                    return None;
                }
            }
            Op::Jump(t) => {
                if !(ip == e && *t == h) {
                    return None;
                }
            }
            _ => return None, // out of the v1 body op set — fail closed
        }
    }
    Some(env)
}
