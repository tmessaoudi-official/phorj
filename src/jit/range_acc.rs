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

/// One verification attempt at outer trip/counter bound `g`: pass A (collect site growths
/// with accumulator reads unknown), solve the accumulator envelopes, pass B (full intervals +
/// elide marks + i64-fit checks), then an env-STABILITY walk — pass B re-run from pass B's
/// post-body env must reproduce it exactly.
#[allow(clippy::too_many_arguments)] // analysis plumbing
fn verify_with_g(
    func: &crate::chunk::Function,
    reach: &[bool],
    slots: &[Cell],
    counter: usize,
    acc_slots: &[(usize, i64)],
    inners: &[Inner],
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
                    Cell::Coll(lo, hi, len) => AbsVal {
                        iv: None,
                        coll: Some((lo, hi, len)),
                        acc_src: None,
                        growth: None,
                    },
                    Cell::Other => AbsVal::none(),
                }
            })
            .collect()
    };

    // Pass A: accumulator reads are UNKNOWN (poison) — collect per-site EFFECTIVE growth
    // intervals (already multiplied by the enclosing inner trip counts).
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
        inners,
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
            return None;
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
        inners,
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
        inners,
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

/// Linear abstract walk of the outer loop body `[h, e]` with a depth-indexed `AbsVal` stack
/// over the locals env. INNER loops are walked in the same pass: their counters are PINNED to
/// `[0, T]` (resolved at the guard from a const or a `Len` of a known collection), so one
/// linear pass models every iteration; a site inside an inner region records its growth
/// multiplied by the region's trip count. `pass_a` = accumulator reads are UNKNOWN; otherwise
/// full intervals flow and `proven` marks every fit `AddI`/`SubI`/`MulI`, every provable
/// `RemI`-by-pow2, and every `Index` whose index interval sits in `[0, len)` of its
/// collection. Returns the post-body locals env; `None` = out of scope (fail closed).
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
    inners: &[Inner],
    site_growth: &mut Vec<(usize, (i128, i128))>,
    mut proven: Option<&mut Vec<bool>>,
) -> Option<Vec<AbsVal>> {
    let code = &func.chunk.code;
    let mut st: Vec<AbsVal> = Vec::new();
    // Active inner regions: (region end, trip count) — the site multiplier is the product.
    let mut regions: Vec<(usize, i128)> = Vec::new();
    // Inner counters currently pinned: (slot, T) — the read interval is refined per ip
    // (body reads between the passed guard and the increment see [0, T-1]).
    let mut pins: Vec<(usize, i128)> = Vec::new();
    for ip in h..=e {
        if !reach[ip] {
            return None;
        }
        // Entering an inner region: resolve its trip count and pin its counter.
        if let Some(l) = inners.iter().find(|l| l.h == ip) {
            let t: i128 = match code[ip + 1] {
                Op::Const(ci) => match func.chunk.consts.get(ci) {
                    Some(Value::Int(c)) if *c >= 0 => *c as i128,
                    _ => return None,
                },
                Op::GetLocal(cs) => {
                    let cell = if cs < env.len() {
                        *env.get(cs)?
                    } else {
                        *st.get(cs - env.len())?
                    };
                    let (_, _, len) = cell.coll?;
                    len as i128
                }
                _ => return None,
            };
            // The counter's INIT (its value here) must be known non-negative.
            let jcell = if l.counter < env.len() {
                *env.get(l.counter)?
            } else {
                *st.get(l.counter - env.len())?
            };
            if !matches!(jcell.iv, Some((lo, _)) if lo >= 0) {
                return None;
            }
            regions.push((l.e, t));
            pins.push((l.counter, t));
        }
        let pin_of = |s: usize, pins: &[(usize, i128)]| -> Option<(i128, i128)> {
            let (_, t) = pins.iter().find(|(p, _)| *p == s)?;
            let l = inners.iter().find(|l| l.counter == s)?;
            // Between the passed guard and the increment, `j < T` holds (the guard
            // dominates); the guard's own read and the increment result may reach T.
            if ip > l.h + l.guard_len - 1 && ip < l.inc {
                Some((0, (*t - 1).max(0)))
            } else {
                Some((0, *t))
            }
        };
        match &code[ip] {
            Op::GetLocal(s) => {
                let mut v = if let Some(iv) = pin_of(*s, &pins) {
                    AbsVal::int(iv.0, iv.1)
                } else if *s < env.len() {
                    *env.get(*s)?
                } else {
                    *st.get(*s - env.len())?
                };
                if is_acc(*s) {
                    v.growth = Some((0, 0));
                    if pass_a {
                        v.iv = None;
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
                    if v.acc_src != Some(*s) {
                        return None;
                    }
                    let mult: i128 = regions.iter().map(|(_, t)| *t).product();
                    let gr = v.growth?;
                    site_growth.push((*s, (gr.0 * mult, gr.1 * mult)));
                } else if *s == counter || pin_of(*s, &pins).is_some() {
                    // Pinned cells (outer/inner counters) keep their envelope.
                } else if *s < env.len() {
                    *env.get_mut(*s)? = AbsVal {
                        acc_src: None,
                        growth: None,
                        ..v
                    };
                } else {
                    let idx = *s - env.len();
                    *st.get_mut(idx)? = AbsVal {
                        acc_src: None,
                        growth: None,
                        ..v
                    };
                }
            }
            Op::AddI | Op::SubI | Op::MulI => {
                let bv = st.pop()?;
                let av = st.pop()?;
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
                let idx = st.pop()?;
                let coll = st.pop()?;
                // In-bounds elision: an index provably inside [0, len) drops the bounds
                // branch at emit (the value interval is the collection's regardless).
                if let (Some((lo, hi)), Some((_, _, len))) = (idx.iv, coll.coll) {
                    if lo >= 0 && hi < len as i128 {
                        if let Some(p) = proven.as_deref_mut() {
                            p[ip] = true;
                        }
                    }
                }
                st.push(AbsVal {
                    iv: coll.coll.map(|(lo, hi, _)| (lo as i128, hi as i128)),
                    coll: None,
                    acc_src: None,
                    growth: None,
                });
            }
            Op::IterElems => {
                // Identity over a flat-able collection — the coll facts (interval + len)
                // ride along, so the inner guard's `Len` resolves.
                let v = st.pop()?;
                st.push(AbsVal {
                    acc_src: None,
                    growth: None,
                    ..v
                });
            }
            Op::Len => {
                let v = st.pop()?;
                let iv = v.coll.map(|(_, _, len)| (len as i128, len as i128));
                st.push(AbsVal {
                    iv,
                    coll: None,
                    acc_src: None,
                    growth: None,
                });
            }
            Op::Pop => {
                st.pop()?;
            }
            Op::JumpIfFalse(t) => {
                st.pop()?;
                let outer_guard = ip == h + 3 && *t > e;
                let outer_back = ip == e && *t == h;
                let inner_ok = inners
                    .iter()
                    .any(|l| (ip == l.h + l.guard_len - 1 && *t > l.e) || (ip == l.e && *t == l.h));
                if !(outer_guard || outer_back || inner_ok) {
                    return None;
                }
            }
            Op::Jump(t) => {
                let outer_back = ip == e && *t == h;
                let inner_back = inners.iter().any(|l| ip == l.e && *t == l.h);
                if !(outer_back || inner_back) {
                    return None;
                }
            }
            _ => return None,
        }
        // Leaving any region that ends here: unpin its counter, drop its multiplier.
        while let Some(&(re, _)) = regions.last() {
            if re == ip {
                regions.pop();
                if let Some(l) = inners.iter().find(|l| l.e == ip) {
                    pins.retain(|(s, _)| *s != l.counter);
                }
            } else {
                break;
            }
        }
    }
    Some(env)
}
