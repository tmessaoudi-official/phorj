//! The task-9 ladder VERIFICATION attempt — one try at a given outer trip/counter bound `G`.
//!
//! Split out of `mod.rs` alongside `walk.rs` when the conditional-accumulator extension (DEC-425)
//! pushed the module past Invariant 13's hard cap. The division of labour: `mod.rs` decides whether a
//! loop is in scope and walks the `G` ladder, `walk.rs` interprets one trip through the body, and this
//! file is the two-pass attempt at a single `G` — collect site growths, solve the accumulator
//! envelopes, re-walk with real intervals, then prove the environment is stable. Moved verbatim apart
//! from this header.

use super::*;

/// One verification attempt at outer trip/counter bound `g`: pass A (collect site growths
/// with accumulator reads unknown), solve the accumulator envelopes, pass B (full intervals +
/// elide marks + i64-fit checks), then an env-STABILITY walk — pass B re-run from pass B's
/// post-body env must reproduce it exactly.
#[allow(clippy::too_many_arguments)] // analysis plumbing
pub(super) fn verify_with_g(
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
