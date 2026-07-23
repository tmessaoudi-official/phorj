//! Set-OP runtime helpers (the DEC-332 setdifference/setunion flips + the relocated
//! `rt_u_set_seal`): a sealed FLAT set is IMMUTABLE and bump-pinned, so `Set.union` /
//! `Set.difference` over two of them are pure functions of the handle pair — each MEMOIZES its
//! result (inline direct-mapped lines at memo entries 24..32 diff / 32..40 union, backed by the
//! FULL `memo_setop` map: an eviction re-installs, never re-seals — the mapmerge discipline).
//! The RESULT is a bucket table with no insertion order: sound because every admitted `IntSet`
//! consumer (`size`/`contains`/these ops) is order-insensitive and set kinds never escape the
//! unboxed graph; order-observing paths run on the VM. Kernel semantics single-sourced:
//! `a`'s keys filtered by membership in `b` (difference) / `a`'s keys then `b`'s new ones
//! (union) — table iteration order does not affect the SET, only the (unobservable) layout.

use super::*;

/// FORK-D — the setcontains vertical's BUILDING helper: seal `Set.of(List<int>)` into an int-keyed
/// packed OPEN-ADDRESSED hash table so `Set.contains` probes in O(1) (the linear-scan vertical could
/// not beat php's C `in_array`). Mirrors [`rt_u_map_seal`] MINUS the key/value pair region — a set has
/// no values, so the base points DIRECTLY at the table (see [`UB_TAG_FLAT_SET`]). Returns a
/// `SLOT|FLAT` [`UB_TAG_FLAT_SET`] handle, or `-1` (→ code 5, whole call redoes on the VM) on a
/// non-int element (analyzer-unreachable), too-many-elements, or arena exhaustion.
///
/// SAFETY (all new `unsafe` is the single `&mut *ctx` deref — same `extern "C"` contract as every
/// sibling helper; keep this argument current if you touch the layout):
///  * The ONLY arena writes are the `tsize` 16-byte buckets into slots `[base, base + tslots)`. The
///    guard `ctx.bump + tslots > ctx.cap` returns BEFORE any write, so `(base + tslots) * 64 ≤
///    cap * 64 = buf_storage.len()` — every written byte is in bounds.
///  * Bucket `i` writes `toff + 16·i .. toff + 16·i + 16`; the max `i = tsize − 1` ends at
///    `16·tsize ≤ tslots·64` (by `div_ceil`), inside the reserved region.
///  * The staging `table` is indexed only by `t & (tsize − 1)`, always `< tsize` — no OOB Vec access;
///    the linear probe terminates because keys are DEDUPED (so `≤ tsize/2` inserts, load factor ≤ 1/2,
///    an empty bucket always exists). Every bucket is written from the staging Vec, so the arena's
///    prior contents are never read (no reliance on zero-init) — exactly the map-seal discipline.
///  * `n` (distinct count) ≤ 4095 fits the 12-bit count field; `log2 ≤ 13` fits the 5-bit field;
///    `base < cap ≤ 4096` fits the 40-bit index. No field overflows the handle word.
pub(in crate::jit) extern "C" fn rt_u_set_seal(ctx: *mut UbCtx, list: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    // Materialize the elements (flat int-list / ACL builder / boxed) uniformly; the owned Vec ends the
    // immutable borrow before any mutable arena write below.
    let Some(vals) = ctx.list_values(list, false) else {
        return -1;
    };
    // Dedup by i64 (set membership is over DISTINCT keys — sizing the table on the distinct count is
    // what makes open-addressing termination, load factor ≤ 1/2, hold). A non-int element is
    // analyzer-unreachable (the vertical only fires on a proven `IntList`); bail defensively → VM redo.
    let mut seen = std::collections::HashSet::with_capacity(vals.len());
    let mut keys: Vec<i64> = Vec::with_capacity(vals.len());
    for v in vals {
        match v {
            Value::Int(k) => {
                if seen.insert(k) {
                    keys.push(k);
                }
            }
            _ => return -1,
        }
    }
    match seal_set_keys(ctx, &keys) {
        Some(h) => {
            ctx.release(list);
            h
        }
        None => {
            // Orphaned on the VM-redo path (the VM re-materializes the list from bytecode), so
            // free it here — symmetric with the success-path release. For a flat list this is a
            // no-op; for a builder/slot-backed list it recycles the record instead of leaking it
            // on this rare (n≥4096 / arena-exhaustion) fallback.
            ctx.release(list);
            -1
        }
    }
}

/// Write DEDUPED int `keys` as a fresh SEALED flat set — the packed open-addressed
/// `{occupied, key}` bucket table (the [`rt_u_set_seal`] layout, extracted so
/// `Set.union`/`Set.difference` share the exact writer). `None` = count/arena guard failed
/// (caller falls back).
pub(in crate::jit) fn seal_set_keys(ctx: &mut UbCtx, keys: &[i64]) -> Option<i64> {
    let n = keys.len() as i64;
    // Table sizing: load factor ≤ 1/2, minimum 4 buckets (an empty set still terminates its probe on
    // an empty bucket). PACKED 16-byte buckets `{occupied: u64, key: i64}` in the slots at `base`.
    let tsize = usize::max(4, (2 * keys.len()).next_power_of_two());
    let tslots = (tsize * 16).div_ceil(UB_SLOT_SIZE) as u64; // 16-byte entries / 64-byte slots
    if n >= 1 << 12 || ctx.bump + tslots > ctx.cap {
        return None; // count / arena guard — the caller falls back (VM redo / boxed)
    }
    let base = ctx.bump as i64;
    let log2 = tsize.trailing_zeros();
    // Stage the table, then write it out whole (map-seal discipline — no reliance on arena zero-state).
    let mut table = vec![(0u64, 0i64); tsize]; // (occupied, key)
    for &key in keys {
        // fibonacci high bits — see UB_SET_HASH_MULT. Identical bits to the inline probe (i64 imul
        // low-64 == u64 wrapping_mul; logical shift == ushr).
        let mut t = (key as u64).wrapping_mul(UB_SET_HASH_MULT as u64) as usize >> (64 - log2);
        while table[t].0 != 0 {
            t = (t + 1) & (tsize - 1);
        }
        table[t] = (1, key);
    }
    let toff = ctx.bump as usize * UB_SLOT_SIZE;
    for (i, (occ, key)) in table.iter().enumerate() {
        ctx.buf_storage[toff + 16 * i..toff + 16 * i + 8].copy_from_slice(&occ.to_le_bytes());
        ctx.buf_storage[toff + 16 * i + 8..toff + 16 * i + 16].copy_from_slice(&key.to_le_bytes());
    }
    ctx.bump += tslots;
    Some(UB_TAG_FLAT_SET | (n << UB_MAP_CNT_SHIFT) | ((log2 as i64) << UB_MAP_LOG_SHIFT) | base)
}

/// Probe a flat set's bucket table for `key` (the `rt_u_set_contains` discipline: Fibonacci
/// top-bits start, linear probe, empty bucket = absent).
fn flat_set_has(ctx: &UbCtx, set: i64, key: i64) -> bool {
    let log2 = ((set >> UB_MAP_LOG_SHIFT) & 0x1F) as u32;
    let tsize = 1usize << log2;
    let base = (set & UB_IDX_MASK) as usize * UB_SLOT_SIZE;
    let mut t = (key as u64).wrapping_mul(UB_SET_HASH_MULT as u64) as usize >> (64 - log2);
    loop {
        let off = base + 16 * t;
        let mut w = [0u8; 8];
        w.copy_from_slice(&ctx.buf_storage[off..off + 8]);
        if u64::from_le_bytes(w) == 0 {
            return false;
        }
        w.copy_from_slice(&ctx.buf_storage[off + 8..off + 16]);
        if i64::from_le_bytes(w) == key {
            return true;
        }
        t = (t + 1) & (tsize - 1);
    }
}

/// A flat set's occupied keys, table order (the SET is order-free — see the module doc).
fn flat_set_keys(ctx: &UbCtx, set: i64) -> Vec<i64> {
    let log2 = ((set >> UB_MAP_LOG_SHIFT) & 0x1F) as u32;
    let tsize = 1usize << log2;
    let base = (set & UB_IDX_MASK) as usize * UB_SLOT_SIZE;
    let mut out = Vec::with_capacity(((set >> UB_MAP_CNT_SHIFT) & 0xFFF) as usize);
    for t in 0..tsize {
        let off = base + 16 * t;
        let mut w = [0u8; 8];
        w.copy_from_slice(&ctx.buf_storage[off..off + 8]);
        if u64::from_le_bytes(w) != 0 {
            w.copy_from_slice(&ctx.buf_storage[off + 8..off + 16]);
            out.push(i64::from_le_bytes(w));
        }
    }
    out
}

/// Is `h` a sealed FLAT set word? (`SLOT|FLAT` both set — see [`UB_TAG_FLAT_SET`].)
fn is_flat_set(h: i64) -> bool {
    h & UB_TAG_FLAT_SET == UB_TAG_FLAT_SET
}

/// The shared memoized set-op body: `which` 0 = difference (`a`'s keys not in `b`), 1 = union
/// (`a`'s keys + `b`'s new keys). FLAT × FLAT probes/installs the full memo + the inline line;
/// anything else returns `-1` (code 5, byte-identical VM redo — boxed sets stay canonical).
fn set_op(ctx: &mut UbCtx, a: i64, b: i64, which: i64) -> i64 {
    if !is_flat_set(a) || !is_flat_set(b) {
        return -1;
    }
    if let Some(&h) = ctx.memo_setop.get(&(a, b, which as u8)) {
        ctx.memo_setop_install(a, b, which, h);
        return h;
    }
    let ak = flat_set_keys(ctx, a);
    let keys: Vec<i64> = if which == 0 {
        ak.into_iter()
            .filter(|&k| !flat_set_has(ctx, b, k))
            .collect()
    } else {
        let mut out = ak;
        for k in flat_set_keys(ctx, b) {
            if !flat_set_has(ctx, a, k) {
                out.push(k);
            }
        }
        out
    };
    match seal_set_keys(ctx, &keys) {
        Some(h) => {
            ctx.memo_setop_install(a, b, which, h);
            h
        }
        None => -1, // arena guard — VM redo renders the boxed result
    }
}

impl UbCtx {
    /// Install `(a, b, which) -> h` into the full memo + the inline direct-mapped line
    /// (entries 24..32 for difference, 32..40 for union — `{a, b, h}`). MUST mirror the inline
    /// probe in `emit_unboxed/verticals_set.rs` bit-for-bit.
    fn memo_setop_install(&mut self, a: i64, b: i64, which: i64, h: i64) {
        self.memo_setop.insert((a, b, which as u8), h);
        let slot = ((a ^ (b << 1)).wrapping_mul(UB_SET_HASH_MULT) as u64 >> 61) as usize;
        let e = (24 + which as usize * 8 + slot) * 3;
        self.memo_storage[e] = a;
        self.memo_storage[e + 1] = b;
        self.memo_storage[e + 2] = h;
    }
}

/// `Set.difference(a, b)` — memoized flat×flat build; `-1` = non-flat operand / arena guard
/// (code 5, VM redo). `free_mask` is accepted for arm symmetry: flat operands' releases no-op.
pub(in crate::jit) extern "C" fn rt_u_set_diff(
    ctx: *mut UbCtx,
    a: i64,
    b: i64,
    _free_mask: i64,
) -> i64 {
    set_op(unsafe { &mut *ctx }, a, b, 0)
}

/// `Set.union(a, b)` — the union twin of [`rt_u_set_diff`].
pub(in crate::jit) extern "C" fn rt_u_set_union(
    ctx: *mut UbCtx,
    a: i64,
    b: i64,
    _free_mask: i64,
) -> i64 {
    set_op(unsafe { &mut *ctx }, a, b, 1)
}
