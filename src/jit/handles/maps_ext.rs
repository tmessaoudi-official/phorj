//! Map MATERIALIZATION helpers — the DEC-332 mapkeys/mapvalues/mapmerge flips. A sealed FLAT
//! map is IMMUTABLE and bump-pinned for the whole run, so `Map.keys` / `Map.values` / `Map.merge`
//! over it are pure functions of the handle word(s): each helper MEMOIZES its result in the
//! JIT-visible memo table (`UbCtx::memo_base`, probed INLINE by the emit arms — a steady-state
//! call never re-enters Rust). Results are builder RECORDS (`ACL[|ACLS]|SHARED`) or fresh sealed
//! flat maps — never per-call bump slots, so a hot loop cannot exhaust the arena. The `SHARED`
//! tag (bit 55) marks a record the MEMO owns: consumer releases no-op (`UbCtx::release` gate) and
//! the in-place append paths copy instead of extending (see `rt_u_list_acc_append`) — a memoized
//! record is immutable to everyone but the memo. Non-flat receivers (untagged boxed maps) take
//! the canonical boxed path un-memoized; a MUTABLE builder map (`AMB`) returns `-1` → code 5, the
//! byte-identical VM redo (disclosed: builder maps don't take these verticals).

use super::*;

/// Direct-mapped memo slot (entry index, 3 words each) for a flat-map handle — entries `0..8`
/// hold `{map, keys_h, values_h}`. FIBONACCI-mixed top-3-bits index (adjacent seal bases must
/// spread — a plain low-bit index made rotating pairs evict each other every iteration, the
/// rebuild-per-iteration arena cliff). Collisions only EVICT (the evicted SHARED record is left
/// pinned — never recycled, its live handles stay valid); correctness never depends on a hit.
/// MUST mirror the inline probe in `emit_unboxed/verticals_map.rs` bit-for-bit.
fn memo_slot(map: i64) -> usize {
    (map.wrapping_mul(UB_SET_HASH_MULT) as u64 >> 61) as usize
}

/// Merge-memo slot — entries `8..16` hold `{a, b, merged_h}` (same mixing discipline).
fn merge_slot(a: i64, b: i64) -> usize {
    8 + ((a ^ (b << 1)).wrapping_mul(UB_SET_HASH_MULT) as u64 >> 61) as usize
}

/// Decode a FLAT map handle into `(pair count, base slot)`.
fn flat_map_parts(map: i64) -> (usize, usize) {
    (
        ((map >> UB_MAP_CNT_SHIFT) & 0xFFF) as usize,
        (map & UB_IDX_MASK) as usize,
    )
}

impl UbCtx {
    /// Read flat-map pair `i` as raw parts: key `(bytes, fnv hash, canon word)` + the value.
    /// Canon words are nonzero by seal construction and canon equality ⇔ byte equality (the
    /// interned-registry invariant) — merge matches keys by ONE canon compare.
    fn flat_pair(&self, base: usize, i: usize) -> (Vec<u8>, u64, u64, i64) {
        let koff = (base + 2 * i) * UB_SLOT_SIZE;
        let len = self.buf_storage[koff] as usize;
        let bytes = self.buf_storage[koff + 1..koff + 1 + len].to_vec();
        let mut h8 = [0u8; 8];
        h8.copy_from_slice(&self.buf_storage[koff + UB_SLOT_HASH_OFF..koff + UB_SLOT_HASH_OFF + 8]);
        let hash = u64::from_le_bytes(h8);
        let mut c8 = [0u8; 8];
        c8.copy_from_slice(
            &self.buf_storage[koff + UB_SLOT_CANON_OFF..koff + UB_SLOT_CANON_OFF + 8],
        );
        let canon = u64::from_le_bytes(c8);
        let voff = koff + UB_SLOT_SIZE;
        let mut v8 = [0u8; 8];
        v8.copy_from_slice(&self.buf_storage[voff..voff + 8]);
        (bytes, hash, canon, i64::from_le_bytes(v8))
    }

    /// The pairs of any NON-builder map handle as boxed values (flat decode / untagged clone),
    /// insertion order — the boxed fallback legs' input. `None` = AMB builder / bad handle.
    fn boxed_map_pairs(&self, h: i64) -> Option<Vec<(crate::value::HKey, Value)>> {
        if h & UB_TAG_FLAT_MAP == UB_TAG_FLAT_MAP {
            let (n, base) = flat_map_parts(h);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let (bytes, _, _, val) = self.flat_pair(base, i);
                let s = std::str::from_utf8(&bytes).ok()?;
                out.push((
                    crate::value::HKey::Str(crate::phstr::PhStr::new(s)),
                    Value::Int(val),
                ));
            }
            return Some(out);
        }
        if h & (UB_TAG_SLOT | UB_TAG_FLAT | UB_TAG_ACC | UB_TAG_ACL | UB_TAG_AMB) != 0 {
            return None;
        }
        match self.handles.get(h as usize) {
            Some(Value::Map(m)) => Some(m.as_ref().clone()),
            _ => None,
        }
    }
}

/// Write `owned` entries (`(key bytes ≤ 22, fnv hash, int value)`, already DEDUPED, insertion
/// order) as a fresh SEALED flat map: `2n` pair slots + the packed open-addressed bucket table
/// (the [`rt_u_map_seal`] layout, extracted so `Map.merge` shares the exact writer). `None` =
/// count/arena guard failed (caller falls back to a boxed map).
pub(in crate::jit) fn seal_flat_entries(
    ctx: &mut UbCtx,
    owned: &[(Vec<u8>, u64, i64)],
) -> Option<i64> {
    let n = owned.len() as i64;
    // Bucket-table sizing: load factor ≤ 1/2, minimum 4 buckets (an empty map still terminates
    // its probe on an empty bucket). The table lives in the slots right after the 2n pairs.
    // PACKED entries: each bucket is 16 bytes `{canon: u64, value: i64}` (canon 0 = empty).
    let tsize = usize::max(4, (2 * n as usize).next_power_of_two());
    let tslots = (tsize * 16).div_ceil(UB_SLOT_SIZE) as u64; // 16-byte entries / 64-byte slots
    if n >= 1 << 12 || ctx.bump + 2 * n as u64 + tslots > ctx.cap {
        return None;
    }
    let base = ctx.bump as i64;
    let mut table = vec![(0u64, 0i64); tsize];
    for (bytes, hash, val) in owned.iter() {
        // Key slot: len + bytes + ZERO tail (the inline probe's whole-word compares) + hash +
        // canon (adopt-or-register — a flat key slot is bump-pinned, registry-safe).
        let kslot = ctx.bump as usize;
        let koff = kslot * UB_SLOT_SIZE;
        ctx.buf_storage[koff] = bytes.len() as u8;
        ctx.buf_storage[koff + 1..koff + 1 + bytes.len()].copy_from_slice(bytes);
        ctx.buf_storage[koff + 1 + bytes.len()..koff + UB_SLOT_HASH_OFF].fill(0);
        ctx.buf_storage[koff + UB_SLOT_HASH_OFF..koff + UB_SLOT_HASH_OFF + 8]
            .copy_from_slice(&hash.to_le_bytes());
        let canon1 = *ctx.interned.entry(bytes.clone()).or_insert(kslot as u32) as u64 + 1;
        ctx.buf_storage[koff + UB_SLOT_CANON_OFF..koff + UB_SLOT_CANON_OFF + 8]
            .copy_from_slice(&canon1.to_le_bytes());
        // Value slot: the raw i64, LE, bytes 0..8 (the rest is never read).
        let voff = koff + UB_SLOT_SIZE;
        ctx.buf_storage[voff..voff + 8].copy_from_slice(&val.to_le_bytes());
        ctx.bump += 2;
        // Open-addressed insert (keys are already deduped — every insert finds a hole; canons
        // are distinct because canon equality ⇔ byte equality, and never 0).
        let mut t = (*hash as usize) & (tsize - 1);
        while table[t].0 != 0 {
            t = (t + 1) & (tsize - 1);
        }
        table[t] = (canon1, *val);
    }
    let toff = ctx.bump as usize * UB_SLOT_SIZE;
    for (i, (c, v)) in table.iter().enumerate() {
        ctx.buf_storage[toff + 16 * i..toff + 16 * i + 8].copy_from_slice(&c.to_le_bytes());
        ctx.buf_storage[toff + 16 * i + 8..toff + 16 * i + 16].copy_from_slice(&v.to_le_bytes());
    }
    ctx.bump += tslots;
    let log2 = tsize.trailing_zeros() as i64;
    Some(UB_TAG_FLAT_MAP | (n << UB_MAP_CNT_SHIFT) | (log2 << UB_MAP_LOG_SHIFT) | base)
}

/// `Map.keys` — a FLAT receiver materializes as a SHARED `ACL|ACLS` record of BORROWED key-slot
/// handles (zero copies — the words point straight at the map's bump-pinned key slots), memoized
/// per map handle; a boxed receiver takes the canonical clone path (untagged list, un-memoized).
/// `-1` = AMB builder / bad handle (→ code 5, byte-identical VM redo).
pub(in crate::jit) extern "C" fn rt_u_map_keys(ctx: *mut UbCtx, map: i64, free_map: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if map & UB_TAG_FLAT_MAP == UB_TAG_FLAT_MAP {
        let e = memo_slot(map) * 3;
        if ctx.memo_storage[e] == map && ctx.memo_storage[e + 1] != 0 {
            return ctx.memo_storage[e + 1];
        }
        // Inline-cache miss ≠ unbuilt: the FULL memo may already hold it (a direct-mapped
        // eviction) — re-install into the cache line, never rebuild.
        if let Some(&(kh, vh)) = ctx.memo_kv.get(&map) {
            if kh != 0 {
                ctx.memo_storage[e] = map;
                ctx.memo_storage[e + 1] = kh;
                ctx.memo_storage[e + 2] = vh;
                return kh;
            }
        }
        let (n, base) = flat_map_parts(map);
        if let Some(idx) = ctx.acc_take_record() {
            ctx.acc_grow_to(idx, (n * 8).max(64));
            ctx.acc_recs[idx].len = 0;
            for i in 0..n {
                let w = UB_TAG_SLOT | (base + 2 * i) as i64; // borrowed: bump-pinned key slot
                ctx.acc_push(idx, &w.to_le_bytes());
            }
            let h = UB_TAG_ACL | UB_TAG_ACLS | UB_TAG_SHARED | idx as i64;
            let vh = ctx.memo_kv.entry(map).or_insert((0, 0));
            vh.0 = h;
            let vh1 = vh.1;
            ctx.memo_storage[e] = map;
            ctx.memo_storage[e + 1] = h;
            ctx.memo_storage[e + 2] = vh1; // the twin's word, if already built
            return h;
        }
        // Record pool exhausted — boxed clone fallback (correct, un-memoized).
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let (bytes, _, _, _) = ctx.flat_pair(base, i);
            match std::str::from_utf8(&bytes) {
                Ok(s) => out.push(Value::Str(crate::phstr::PhStr::new(s))),
                Err(_) => return -1,
            }
        }
        return ctx.alloc(Value::List(std::rc::Rc::new(out)));
    }
    let out: Vec<Value> = match ctx.handles.get(map as usize) {
        Some(Value::Map(m)) if ub_is_untagged(map) => m.iter().map(|(k, _)| k.to_value()).collect(),
        _ => return -1,
    };
    let h = ctx.alloc(Value::List(std::rc::Rc::new(out)));
    if free_map != 0 {
        ctx.release(map);
    }
    h
}

/// `Map.values` — the int twin of [`rt_u_map_keys`]: a SHARED `ACL` record of the raw `i64`
/// values (copied out of the value slots), memoized in the same entry's third word.
pub(in crate::jit) extern "C" fn rt_u_map_values(ctx: *mut UbCtx, map: i64, free_map: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if map & UB_TAG_FLAT_MAP == UB_TAG_FLAT_MAP {
        let e = memo_slot(map) * 3;
        if ctx.memo_storage[e] == map && ctx.memo_storage[e + 2] != 0 {
            return ctx.memo_storage[e + 2];
        }
        if let Some(&(kh, vh)) = ctx.memo_kv.get(&map) {
            if vh != 0 {
                ctx.memo_storage[e] = map;
                ctx.memo_storage[e + 1] = kh;
                ctx.memo_storage[e + 2] = vh;
                return vh;
            }
        }
        let (n, base) = flat_map_parts(map);
        if let Some(idx) = ctx.acc_take_record() {
            ctx.acc_grow_to(idx, (n * 8).max(64));
            ctx.acc_recs[idx].len = 0;
            for i in 0..n {
                let voff = (base + 2 * i + 1) * UB_SLOT_SIZE;
                let v = &ctx.buf_storage[voff..voff + 8].to_vec();
                ctx.acc_push(idx, v);
            }
            let h = UB_TAG_ACL | UB_TAG_SHARED | idx as i64;
            let kv = ctx.memo_kv.entry(map).or_insert((0, 0));
            kv.1 = h;
            let kh = kv.0;
            ctx.memo_storage[e] = map;
            ctx.memo_storage[e + 1] = kh; // the twin's word, if already built
            ctx.memo_storage[e + 2] = h;
            return h;
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let (_, _, _, val) = ctx.flat_pair(base, i);
            out.push(Value::Int(val));
        }
        return ctx.alloc(Value::List(std::rc::Rc::new(out)));
    }
    // AMB builder record (a `Map.map`/`Map.filter` result or a live mapinsert builder): walk the
    // RANKS (insertion order), probe the table per canon for the value. UN-memoized (the record
    // is mutable/recycled — memoizing it would alias a dead buffer); the output is a fresh
    // recycled ACL builder, so a per-iteration `Map.values(mapped)` costs no arena growth.
    if map & UB_TAG_AMB != 0 {
        let idx = (map & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP {
            return -1;
        }
        let mut w8 = [0u8; 8];
        w8.copy_from_slice(&ctx.acc_bufs[idx][0..8]);
        let tsize = 1usize << u64::from_le_bytes(w8);
        w8.copy_from_slice(&ctx.acc_bufs[idx][8..16]);
        let count = u64::from_le_bytes(w8) as usize;
        let mut vals = Vec::with_capacity(count);
        for i in 0..count {
            let roff = 16 + tsize * 16 + i * 8;
            w8.copy_from_slice(&ctx.acc_bufs[idx][roff..roff + 8]);
            let canon = u64::from_le_bytes(w8);
            let hoff = (canon as usize - 1) * UB_SLOT_SIZE + UB_SLOT_HASH_OFF;
            w8.copy_from_slice(&ctx.buf_storage[hoff..hoff + 8]);
            let hash = u64::from_le_bytes(w8);
            let mut t = (hash as usize) & (tsize - 1);
            loop {
                let eoff = 16 + t * 16;
                w8.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
                let ec = u64::from_le_bytes(w8);
                if ec == canon {
                    w8.copy_from_slice(&ctx.acc_bufs[idx][eoff + 8..eoff + 16]);
                    vals.push(i64::from_le_bytes(w8));
                    break;
                }
                if ec == 0 {
                    return -1; // defensive — a rank without a table entry
                }
                t = (t + 1) & (tsize - 1);
            }
        }
        if let Some(out_idx) = ctx.acc_take_record() {
            ctx.acc_grow_to(out_idx, (count * 8).max(64));
            ctx.acc_recs[out_idx].len = 0;
            for v in &vals {
                ctx.acc_push(out_idx, &v.to_le_bytes());
            }
            if free_map != 0 {
                ctx.release(map);
            }
            return UB_TAG_ACL | out_idx as i64;
        }
        let h = ctx.alloc(Value::List(std::rc::Rc::new(
            vals.into_iter().map(Value::Int).collect(),
        )));
        if free_map != 0 {
            ctx.release(map);
        }
        return h;
    }
    let out: Vec<Value> = match ctx.handles.get(map as usize) {
        Some(Value::Map(m)) if ub_is_untagged(map) => m.iter().map(|(_, v)| v.clone()).collect(),
        _ => return -1,
    };
    let h = ctx.alloc(Value::List(std::rc::Rc::new(out)));
    if free_map != 0 {
        ctx.release(map);
    }
    h
}

/// `Map.merge(a, b)` — `a`'s entries with `b`'s merged in (shared key keeps `a`'s position,
/// takes `b`'s value; new keys append — the canonical `map_merge` kernel order). FLAT × FLAT
/// builds a fresh SEALED flat map (memoized per `(a, b)` pair — a rotating-operand loop pays the
/// build once per distinct pair, then two loads); anything boxed merges through the kernel shape
/// into an untagged `Value::Map`. `-1` = AMB / bad handle / non-flattenable (→ code 5, VM redo).
pub(in crate::jit) extern "C" fn rt_u_map_merge(
    ctx: *mut UbCtx,
    a: i64,
    b: i64,
    free_mask: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if a & UB_TAG_FLAT_MAP == UB_TAG_FLAT_MAP && b & UB_TAG_FLAT_MAP == UB_TAG_FLAT_MAP {
        let e = merge_slot(a, b) * 3;
        if ctx.memo_storage[e] == a && ctx.memo_storage[e + 1] == b && ctx.memo_storage[e + 2] != 0
        {
            return ctx.memo_storage[e + 2];
        }
        // Inline-cache miss ≠ unbuilt: colliding pairs would otherwise EVICT each other and
        // re-seal fresh bump slots every iteration (the arena-exhaustion cliff) — the FULL
        // memo makes an eviction cost one lookup, never a rebuild.
        if let Some(&h) = ctx.memo_merge.get(&(a, b)) {
            ctx.memo_storage[e] = a;
            ctx.memo_storage[e + 1] = b;
            ctx.memo_storage[e + 2] = h;
            return h;
        }
        let (an, abase) = flat_map_parts(a);
        let (bn, bbase) = flat_map_parts(b);
        // Merge on CANON words (canon equality ⇔ byte equality — both maps sealed in this ctx).
        let mut merged: Vec<(Vec<u8>, u64, u64, i64)> =
            (0..an).map(|i| ctx.flat_pair(abase, i)).collect();
        for i in 0..bn {
            let (bytes, hash, canon, val) = ctx.flat_pair(bbase, i);
            match merged.iter_mut().find(|(_, _, c, _)| *c == canon) {
                Some(slot) => slot.3 = val,
                None => merged.push((bytes, hash, canon, val)),
            }
        }
        let owned: Vec<(Vec<u8>, u64, i64)> =
            merged.into_iter().map(|(s, h, _, v)| (s, h, v)).collect();
        match seal_flat_entries(ctx, &owned) {
            Some(h) => {
                ctx.memo_merge.insert((a, b), h);
                ctx.memo_storage[e] = a;
                ctx.memo_storage[e + 1] = b;
                ctx.memo_storage[e + 2] = h;
                return h;
            }
            None => return -1, // arena guard — VM redo renders the boxed result
        }
    }
    // Boxed leg: decode both sides (flat or untagged), merge through the kernel shape.
    let Some(ap) = ctx.boxed_map_pairs(a) else {
        return -1;
    };
    let Some(bp) = ctx.boxed_map_pairs(b) else {
        return -1;
    };
    let mut out = ap;
    for (bk, bv) in bp {
        match out.iter_mut().find(|(k, _)| *k == bk) {
            Some(slot) => slot.1 = bv,
            None => out.push((bk, bv)),
        }
    }
    let h = ctx.alloc(Value::Map(std::rc::Rc::new(out)));
    if free_mask & 1 != 0 {
        ctx.release(a);
    }
    if free_mask & 2 != 0 {
        ctx.release(b);
    }
    h
}

/// `Map.map`/`Map.filter` output seed (the DEC-332 mapfilter/mapmap flips): take an AMB builder
/// record sized for `n` entries (the parent flat map's pair count — survivors never exceed it),
/// zero the table, count = 0. Table sizing mirrors [`rt_u_map_builder_set`]'s convert leg
/// (`max(16, (2(n+1)).next_pow2)` — load ≤ 1/2 with one-set headroom), so a later `m[k] = v`
/// onto the result extends it through the SAME layout. A fresh record per loop iteration is
/// RECYCLED by the consumer's release — never sealed, so a hot loop cannot exhaust the arena
/// (the mapmerge bring-up cliff, avoided by construction here). `-1` = record pool exhausted
/// (→ code 5, byte-identical VM redo).
pub(in crate::jit) extern "C" fn rt_u_map_ext_new(ctx: *mut UbCtx, n: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if !(0..1 << 12).contains(&n) {
        return -1;
    }
    let Some(idx) = ctx.acc_take_record() else {
        return -1;
    };
    let tsize = usize::max(16, (2 * (n as usize + 1)).next_power_of_two());
    ctx.acc_grow_to(idx, 16 + tsize * 16 + n as usize * 8);
    let lg = tsize.trailing_zeros() as u64;
    ctx.acc_bufs[idx][0..8].copy_from_slice(&lg.to_le_bytes());
    ctx.acc_bufs[idx][8..16].copy_from_slice(&0u64.to_le_bytes());
    ctx.acc_bufs[idx][16..16 + tsize * 16].fill(0);
    ctx.acc_recs[idx].len = (16 + tsize * 16) as u64;
    UB_TAG_AMB | idx as i64
}

/// Append pair `(parent key slot, val)` to an [`rt_u_map_ext_new`] record: canon + hash are READ
/// from the parent's bump-pinned key slot (the seal already adopt-or-registered them — no
/// interning, no hashing here), rank appended, table hole-inserted. Parent keys are unique, so
/// every probe finds a hole (canon match is defensively an overwrite, mirroring builder-set).
/// The emit loop pushes ≤ `n` survivors into an `n`-sized record — capacity is preflighted, the
/// grow below only ever extends the rank tail. `-1` = bad handle (→ code 5, VM redo).
pub(in crate::jit) extern "C" fn rt_u_map_ext_push(
    ctx: *mut UbCtx,
    amb: i64,
    kslot: i64,
    val: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let idx = (amb & UB_IDX_MASK) as usize;
    if amb & UB_TAG_AMB == 0 || idx >= UB_ACC_CAP {
        return -1;
    }
    let koff = kslot as usize * UB_SLOT_SIZE;
    let mut w8 = [0u8; 8];
    w8.copy_from_slice(&ctx.buf_storage[koff + UB_SLOT_HASH_OFF..koff + UB_SLOT_HASH_OFF + 8]);
    let hash = u64::from_le_bytes(w8);
    w8.copy_from_slice(&ctx.buf_storage[koff + UB_SLOT_CANON_OFF..koff + UB_SLOT_CANON_OFF + 8]);
    let canon1 = u64::from_le_bytes(w8);
    if canon1 == 0 {
        return -1; // seal always canonizes flat keys — defensive
    }
    w8.copy_from_slice(&ctx.acc_bufs[idx][0..8]);
    let tsize = 1usize << u64::from_le_bytes(w8);
    w8.copy_from_slice(&ctx.acc_bufs[idx][8..16]);
    let count = u64::from_le_bytes(w8) as usize;
    let mut t = (hash as usize) & (tsize - 1);
    loop {
        let eoff = 16 + t * 16;
        w8.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
        let ec = u64::from_le_bytes(w8);
        if ec == canon1 {
            ctx.acc_bufs[idx][eoff + 8..eoff + 16].copy_from_slice(&val.to_le_bytes());
            return 0;
        }
        if ec == 0 {
            ctx.acc_bufs[idx][eoff..eoff + 8].copy_from_slice(&canon1.to_le_bytes());
            ctx.acc_bufs[idx][eoff + 8..eoff + 16].copy_from_slice(&val.to_le_bytes());
            break;
        }
        t = (t + 1) & (tsize - 1);
    }
    ctx.acc_grow_to(idx, 16 + tsize * 16 + (count + 1) * 8);
    let roff = 16 + tsize * 16 + count * 8;
    ctx.acc_bufs[idx][roff..roff + 8].copy_from_slice(&canon1.to_le_bytes());
    ctx.acc_bufs[idx][8..16].copy_from_slice(&((count as u64) + 1).to_le_bytes());
    ctx.acc_recs[idx].len = (16 + tsize * 16 + (count + 1) * 8) as u64;
    0
}

/// `Map.size` slow leg — the emit arm inlines FLAT (count bits) and AMB (record count word);
/// this covers the untagged boxed map. `-1` = bad handle (→ code 5, VM redo).
pub(in crate::jit) extern "C" fn rt_u_map_size(ctx: *mut UbCtx, map: i64, free_map: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let n = match ctx.handles.get(map as usize) {
        Some(Value::Map(m)) if ub_is_untagged(map) => m.len() as i64,
        _ => return -1,
    };
    if free_map != 0 {
        ctx.release(map);
    }
    n
}
