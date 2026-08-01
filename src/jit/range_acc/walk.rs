//! The abstract-interpretation WALK for the task-9 accumulator pass — one pass over the outer loop
//! body, tracking per-slot integer intervals and per-site accumulator growth.
//!
//! Split out of `range_acc.rs` when the conditional-accumulator extension (DEC-425) pushed that file
//! past its Invariant-13 ratchet: the driver in `mod.rs` answers "is this loop in scope, and what is
//! the envelope?", while this file answers "what does one trip through the body do to each slot?".
//! Moved verbatim apart from this header.

use super::*;

/// Linear abstract walk of the outer loop body `[h, e]` with a depth-indexed `AbsVal` stack
/// over the locals env. INNER loops are walked in the same pass: their counters are PINNED to
/// `[0, T]` (resolved at the guard from a const or a `Len` of a known collection), so one
/// linear pass models every iteration; a site inside an inner region records its growth
/// multiplied by the region's trip count. `pass_a` = accumulator reads are UNKNOWN; otherwise
/// full intervals flow and `proven` marks every fit `AddI`/`SubI`/`MulI`, every provable
/// `RemI`-by-pow2, and every `Index` whose index interval sits in `[0, len)` of its
/// collection. Returns the post-body locals env; `None` = out of scope (fail closed).
#[allow(clippy::too_many_arguments)] // analysis plumbing
pub(super) fn walk_body(
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
    // The open CONDITIONAL region: `(join_ip, slots_written_inside)`. A body-level `if` used to be an
    // outright rejection, which is why a CONDITIONAL accumulator — the "tally while scanning" shape,
    // and the single most common one — could never be proven (DEC-425: it left a loop-carried sticky
    // overflow phi on every iteration of `floatloop`, worth 2x).
    //
    // Only ONE region is tracked, not a stack: a nested `if` inside an `if` is rejected below. That is
    // deliberate — this pass is an overflow-elision proof, and an unsound widening here means silently
    // wrong arithmetic, so it takes the shape it can verify and refuses the rest.
    let mut cond: Option<(usize, Vec<usize>)> = None;
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
                // Inside a conditional region this write only MAY happen — remember it so the join
                // below widens the slot to unknown.
                if let Some((_, written)) = cond.as_mut() {
                    if !written.contains(s) {
                        written.push(*s);
                    }
                }
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
            // Two-operand ops whose RESULT carries no integer interval. Float arithmetic and the
            // comparisons are pure and stack-only — they cannot write a slot or divert control — so
            // popping both operands and pushing "unknown" is sound and complete for this pass's
            // purposes. They are listed EXPLICITLY rather than swept up by a catch-all: an op that
            // touched state would be silently mis-modelled here, and this is an overflow-elision
            // proof. `Neg` is deliberately absent — it is a speculated overflow op, not a neutral one.
            Op::DivI
            | Op::Lt
            | Op::Gt
            | Op::Le
            | Op::Ge
            | Op::Eq
            | Op::Ne
            | Op::AddF
            | Op::SubF
            | Op::MulF
            | Op::DivF => {
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
                    // A body-level `if`: a FORWARD branch that lands inside this loop. Conditions,
                    // all load-bearing:
                    //   * forward and within the loop — a backward or escaping target is some other
                    //     control shape this pass has not verified;
                    //   * the operand stack is EMPTY — a statement-level `if`, so the two paths cannot
                    //     disagree about stack depth at the join (an expression `if` could);
                    //   * not already inside one — nested conditionals are refused rather than
                    //     approximated.
                    if !(ip < *t && *t <= e && st.is_empty() && cond.is_none()) {
                        return None;
                    }
                    cond = Some((*t, Vec::new()));
                }
            }
            Op::Jump(t) => {
                let outer_back = ip == e && *t == h;
                let inner_back = inners.iter().any(|l| ip == l.e && *t == l.h);
                // The then-branch's tail jump to the join of the `if` we are inside (the compiler
                // emits one even with no `else`). Any other forward jump is an unverified shape.
                let to_join = matches!(cond, Some((j, _)) if *t == j && ip < *t);
                if !(outer_back || inner_back || to_join) {
                    return None;
                }
            }
            _ => return None,
        }
        // Reaching the JOIN of a conditional region: every slot it MAY have written is now unknown,
        // because the two paths disagree about it. Two exceptions, and both are earned rather than
        // assumed:
        //   * an ACCUMULATOR keeps its envelope interval — that interval is solved from the site
        //     growths with `min(lo, 0)` / `max(hi, 0)` per site, i.e. it ALREADY assumes each site may
        //     or may not have run, which is exactly the conditional case;
        //   * the COUNTER must not be written here at all. Its interval underpins the trip bound, so a
        //     conditional write would invalidate the whole proof — refuse instead of widening.
        if let Some((join, written)) = cond.as_ref() {
            if *join == ip + 1 {
                if written.contains(&counter) {
                    return None;
                }
                for &w in written {
                    if !is_acc(w) {
                        env[w] = AbsVal::none();
                    }
                }
                cond = None;
            }
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
