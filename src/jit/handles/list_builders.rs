//! List BUILDER-record appends (M-Decomp from `handles/mod.rs`, Invariant 13): the
//! `rt_u_list_acc_append` / `rt_u_str_list_acc_append` slow legs of the listappend vertical —
//! in-place record growth, flat/boxed/SHARED-record conversion, pool-exhaustion fallbacks.
//! Bodies moved verbatim.

use super::*;

/// FUSED list-builder append (`xs = List.append(xs, v)`, the listappend vertical) — the SLOW
/// leg of the inline ACL fast path. (1) lhs already an ACL record → capacity growth
/// (doubling) + push; (2) lhs a flat/boxed INT list → CONVERT: take a record (recycled ones
/// reuse their grown buffer), copy the elements in as raw i64s, push, release the consumed
/// lhs, return `ACL|idx`; (3) record table exhausted → general clone-append fallback (boxed
/// result, same MOVE semantics — correct bytes at degraded speed, never a code-5 redo
/// storm). Only reachable from proven accumulator sites, so the lhs is always consumed.
pub(in crate::jit) extern "C" fn rt_u_list_acc_append(ctx: *mut UbCtx, a: i64, v: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if a & UB_TAG_ACL != 0 && a & UB_TAG_SHARED == 0 {
        let idx = (a & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP {
            return -1;
        }
        let need = ctx.acc_recs[idx].len as usize + 8;
        ctx.acc_grow_to(idx, need);
        ctx.acc_push(idx, &v.to_le_bytes());
        return a;
    }
    // Conversion leg: collect the current elements (flat arena slots, a SHARED memo-owned
    // record — immutable to consumers, so append COPIES it — or a boxed int list).
    let elems: Vec<i64> = if a & UB_TAG_FLAT != 0 && a & UB_TAG_SLOT == 0 {
        let n = ((a >> 40) & 0xFFFFF) as usize;
        let base = (a & UB_IDX_MASK) as usize;
        (0..n)
            .map(|i| {
                let off = (base + i) * UB_SLOT_SIZE;
                let mut b8 = [0u8; 8];
                b8.copy_from_slice(&ctx.buf_storage[off..off + 8]);
                i64::from_le_bytes(b8)
            })
            .collect()
    } else if a & UB_TAG_ACL != 0 {
        let idx = (a & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP || a & UB_TAG_ACLS != 0 {
            return -1;
        }
        let n = (ctx.acc_recs[idx].len / 8) as usize;
        (0..n)
            .map(|i| {
                let mut b8 = [0u8; 8];
                b8.copy_from_slice(&ctx.acc_bufs[idx][i * 8..i * 8 + 8]);
                i64::from_le_bytes(b8)
            })
            .collect()
    } else {
        match ctx.handles.get(a as usize) {
            Some(Value::List(xs)) => {
                let ints: Option<Vec<i64>> = xs
                    .iter()
                    .map(|e| match e {
                        Value::Int(n) => Some(*n),
                        _ => None,
                    })
                    .collect();
                match ints {
                    Some(v) => v,
                    None => return -1,
                }
            }
            _ => return -1,
        }
    };
    let Some(idx) = ctx.acc_take_record() else {
        // Pool exhausted — fall back to the general clone-append (raw i64 elements are
        // plain words: nothing extra to release).
        return rt_u_list_append_clone(ctx, a, v, 0b100);
    };
    ctx.acc_grow_to(idx, ((elems.len() + 1) * 8).max(64));
    ctx.acc_recs[idx].len = 0;
    for e in &elems {
        ctx.acc_push(idx, &e.to_le_bytes());
    }
    ctx.acc_push(idx, &v.to_le_bytes());
    ctx.release(a);
    UB_TAG_ACL | idx as i64
}

/// L2a: the STR-list twin of [`rt_u_list_acc_append`] — the record stores element WORDS the
/// record then OWNS (the qualify-loop `out = List.append(out, q)` shape pushes q's word,
/// ZERO clones). (1) lhs already a STR record (`ACL|ACLS`) → grow + push the word; an INT
/// record here is analyze/emit drift → `-1`. (2) lhs a FLAT str list → convert: the
/// elements' BORROWED SLOT WORDS go in as-is (the slots are bump-pinned for the run — no
/// copies, and their releases no-op); lhs a BOXED str list → one owned arena word per
/// element (a one-time conversion cost). (3) pool exhaustion → general clone-append
/// fallback (boxed result, never a code-5 redo storm); non-str element → `-1` (code 5,
/// redo on VM). Only reachable from proven accumulator sites, so the lhs is consumed.
pub(in crate::jit) extern "C" fn rt_u_str_list_acc_append(ctx: *mut UbCtx, a: i64, v: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if a & UB_TAG_ACL != 0 && a & UB_TAG_SHARED == 0 {
        if a & UB_TAG_ACLS == 0 {
            return -1;
        }
        let idx = (a & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP {
            return -1;
        }
        let need = ctx.acc_recs[idx].len as usize + 8;
        ctx.acc_grow_to(idx, need);
        ctx.acc_push(idx, &v.to_le_bytes());
        return a;
    }
    let words: Vec<i64> = if a & UB_TAG_FLAT != 0 && a & UB_TAG_SLOT == 0 {
        let n = ((a >> 40) & 0xFFFFF) as usize;
        let base = (a & UB_IDX_MASK) as usize;
        (0..n).map(|i| (base + i) as i64 | UB_TAG_SLOT).collect()
    } else if a & UB_TAG_ACL != 0 {
        // SHARED memo-owned record (Map.keys result): COPY its borrowed slot words — they are
        // bump-pinned, so the copies stay valid and their eventual releases no-op.
        let idx = (a & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP || a & UB_TAG_ACLS == 0 {
            return -1;
        }
        let n = (ctx.acc_recs[idx].len / 8) as usize;
        (0..n)
            .map(|i| {
                let mut b8 = [0u8; 8];
                b8.copy_from_slice(&ctx.acc_bufs[idx][i * 8..i * 8 + 8]);
                i64::from_le_bytes(b8)
            })
            .collect()
    } else {
        let strs: Vec<Value> = match ctx.handles.get(a as usize) {
            Some(Value::List(xs)) => {
                let all: Option<Vec<Value>> = xs
                    .iter()
                    .map(|e| match e {
                        s @ Value::Str(_) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                match all {
                    Some(vs) => vs,
                    None => return -1,
                }
            }
            _ => return -1,
        };
        strs.into_iter().map(|s| ctx.alloc(s)).collect()
    };
    let Some(idx) = ctx.acc_take_record() else {
        // Pool exhausted — fall back to the general clone-append (boxed result, same
        // MOVE semantics: elem word + lhs consumed). Correct bytes at degraded speed,
        // NEVER a code-5 redo storm — mirrors rt_u_acc_append's plain-concat fallback.
        for w in words {
            ctx.release(w); // boxed-lhs materialization made owned words; slot words no-op
        }
        return rt_u_list_append_clone(ctx, a, v, 0b111);
    };
    ctx.acc_grow_to(idx, ((words.len() + 1) * 8).max(64));
    ctx.acc_recs[idx].len = 0;
    for w in &words {
        ctx.acc_push(idx, &w.to_le_bytes());
    }
    ctx.acc_push(idx, &v.to_le_bytes());
    ctx.release(a);
    UB_TAG_ACL | UB_TAG_ACLS | idx as i64
}
