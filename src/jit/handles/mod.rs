//! P-2a/P-2b handle space: the per-run `UbCtx` (repr(C) arena header + slots + canon
//! registry) and the `rt_u_*` slow-path helpers the inline fast paths fall back to.

use super::*;

mod helper_refs;
mod list_builders;
mod maps_ext;
mod sets_ext;
mod strings_ext;
mod symbols;
pub(in crate::jit) use helper_refs::*;
pub(super) use list_builders::*;
pub(super) use maps_ext::*;
pub(super) use sets_ext::*;
pub(super) use strings_ext::*;
pub(super) use symbols::*;

// ===========================================================================================
// P-2a handle space (+ P-2a-inline): the per-run side state + `rt_u_*` helpers that let the
// UNBOXED codegen run string/collection verticals (Concat / list Index / `String.length`)
// natively. The P-2a spike measured helper-call granularity ~2× short of php+JIT, so the hot
// paths are now emitted INLINE in Cranelift IR over a fixed-layout string arena; the helpers
// remain as the slow paths (untagged operands, >22-byte results, non-flat lists). Design:
//
//  - a HANDLE is an `i64` with tag bits (see `UB_TAG_*`):
//      * untagged           — an index into `UbCtx::handles` (a boxed `Value`; consts >22B,
//                             heap concat results). Helper-only.
//      * `SLOT` (bit 62)    — an index into the 64-byte-slot string ARENA (`len:u8` + ≤22 data
//                             + slack so bounded 24-byte over-copies never cross a neighbour).
//                             Readable INLINE: `len = load.u8 buf[idx*64]`.
//      * `SLOT|OWNED` (b60) — same, and freeing recycles the slot (inline free-stack push).
//                             A borrowed slot (a flat-list element, a pinned short const) has
//                             OWNED clear, so an emitted free is a runtime no-op.
//      * `FLAT` (bit 61)    — a list of all-short strings flattened into consecutive arena
//                             slots: bits[40..60) = element count, bits[0..40) = base slot.
//                             `Index` is INLINE: unsigned bounds check + base+idx (zero copy).
//  - the arena header (`buf`, `free_stack`, `free_top`, `bump`, `cap`) leads the `#[repr(C)]`
//    `UbCtx` at FIXED offsets so inline IR can load/store it directly.
//  - every helper is defensive (a bad handle returns `-1` = fault → code 5 "redo on VM"), and
//    NOTHING observable escapes the private `UbCtx` — the fallback re-execution stays sound
//    (arena exhaustion also just redoes on the VM).
//  - the happy paths preserve byte semantics exactly (byte concat, byte `len`, the VM's `Index`
//    bounds — an unsigned `idx < len` compare rejects negatives like `usize::try_from` does).
// ===========================================================================================

/// Handle tag: arena-slot handle (low 40 bits = slot index).
pub(super) const UB_TAG_SLOT: i64 = 1 << 62;
/// Handle tag: flattened string list (bits[40..60) = count, low 40 bits = base slot).
pub(super) const UB_TAG_FLAT: i64 = 1 << 61;
/// Handle tag (with `UB_TAG_SLOT`): freeing recycles the slot.
pub(super) const UB_TAG_OWNED: i64 = 1 << 60;
/// Handle tag: string ACCUMULATOR (low 40 bits = record index) — the php smart_str analog for
/// the `s = s + x` pattern (the strbuild vertical): a JIT-visible `{ptr,len,cap}` record (at
/// `acc_base + idx·24`) over a growable byte buffer, appended to IN PLACE fully inline
/// (cap-checked bounded copy, no call); `rt_u_acc_append` is the slow leg (first-append
/// conversion, capacity growth, non-slot rhs). Deliberately NOT combined with `UB_TAG_OWNED`:
/// the inline release ladders' recycle push keys on OWNED alone (arena-slot semantics) — an
/// ACC handle must take their helper branch, where [`UbCtx::release`] recycles the RECORD but
/// keeps its buffer (capacity reuse across `s = ""` resets — php's trick).
pub(super) const UB_TAG_ACC: i64 = 1 << 59;
/// Handle tag: INT-LIST BUILDER (low 40 bits = record index) — the listappend vertical's
/// analog of `UB_TAG_ACC` for the `xs = List.append(xs, v)` accumulator pattern: the SAME
/// `{ptr,len,cap}` record pool (records are type-agnostic; len/cap in BYTES, elements are
/// consecutive raw i64s), appended fully inline at proven accumulator sites (cap check + one
/// 8-byte store, php's `$xs[] =` analog); `rt_u_list_acc_append` is the slow leg (conversion
/// from a flat/boxed list, capacity growth). Like ACC, deliberately NOT OWNED-tagged — the
/// release ladders route it to the helper, which recycles the record and keeps the buffer.
pub(super) const UB_TAG_ACL: i64 = 1 << 58;
/// Handle tag: MAP BUILDER (low 40 bits = record index) — the mapinsert vertical: a MUTABLE
/// `Map<string,int>` on the SAME record pool, converted from a sealed flat map by the first
/// `m[k] = v` (`Op::SetIndexLocal`) on a uniquely-owned map local. Record-buffer layout
/// (bytes): `[0..8) log2(tsize)`, `[8..16) count`, `[16..16+tsize·16)` PACKED open-addressed
/// bucket table `{canon: u64, value: i64}` (canon 0 = empty, load ≤ 1/2 — the SAME entry shape
/// as the sealed flat-map probe), then `count` rank canons (u64 each, INSERTION order — the
/// order-faithful materialization witness; PHP maps are insertion-ordered, overwrite keeps the
/// original rank). An OVERWRITE hit is fully inline (probe walk + one store into the entry's
/// value word — php's `$m[$k] = $v`); insert / collision-miss / canon-0 keys / conversion /
/// table growth take the ONE `rt_u_map_builder_set` helper. Canon discipline: only BUMP-PINNED
/// slots are ever interned (adopt-or-register — a new runtime key registers a fresh pinned
/// slot), so canon equality ⇔ byte equality holds for the builder exactly as for the seal.
/// Like ACC/ACL, deliberately NOT OWNED-tagged — the release ladders route it to the helper,
/// which recycles the record and keeps its grown buffer (the `m = [...]` reset reuse trick).
pub(super) const UB_TAG_AMB: i64 = 1 << 57;
/// STR-ELEMENT marker for an ACL list-builder record (L2a — set ALONGSIDE `UB_TAG_ACL`):
/// the record's i64s are STRING WORDS the record OWNS (Owned words freed on record release;
/// borrowed-slot/const words no-op), not raw ints. Bit 56 is safe here: a flat word's 20-bit
/// count (bits 40..60) can also cover bit 56, but the FLAT tag dispatches FIRST on every
/// ladder, so an ACL-branch check never sees a flat word.
pub(super) const UB_TAG_ACLS: i64 = 1 << 56;
/// SHARED marker for a MEMO-owned builder record (set alongside `UB_TAG_ACL[|ACLS]` by the map
/// materialization verticals — `maps_ext.rs`): the memo table owns the record, so a consumer
/// release is a NO-OP (the [`UbCtx::release`] gate) and the in-place list-append paths must COPY
/// into a fresh record instead of extending (`rt_u_list_acc_append` / the inline `list_append_acc`
/// exclusion). Bit 55 is below every dispatch tag and above the 40-bit index — never ambiguous.
pub(super) const UB_TAG_SHARED: i64 = 1 << 55;
/// Accumulator record-table capacity. Records are pre-allocated (`acc_base` must never move);
/// live accumulators correspond to compile-time accumulator SITES — a handful at most.
/// Exhaustion falls back to the plain concat path (correct, slower).
pub(super) const UB_ACC_CAP: usize = 16;
/// Slack bytes kept PAST every accumulator buffer's usable cap: the inline append's bounded
/// 3×8-byte over-copy may write up to 24 bytes beyond the appended piece.
const UB_ACC_SLACK: usize = 24;
/// Handle tag: flattened `Map<string,int>` — `SLOT|FLAT` combined (impossible otherwise):
/// bits[40..52) = pair count (≤ 4095), bits[52..57) = log2 of the bucket-table size, low 40 bits
/// = base slot; pair `i` = key slot `base+2i` (hash at byte 24, canon at byte 32), value slot
/// `base+2i+1` (the `i64` value in bytes 0..8). The open-addressed bucket table — PACKED
/// 16-byte `{canon: u64, value: i64}` entries, canon 0 = empty (a real canon is never 0),
/// load factor ≤ 1/2 — fills the arena slots immediately AFTER the `2n` pair slots, so its
/// address derives from `base + 2·count`; a probe hit is two adjacent loads, no indirection.
pub(super) const UB_TAG_FLAT_MAP: i64 = UB_TAG_SLOT | UB_TAG_FLAT;
/// Handle tag: flattened `Set<int>` — the SAME `SLOT|FLAT` bits and count/log2/base FIELD layout as
/// [`UB_TAG_FLAT_MAP`], but the base points DIRECTLY at the open-addressed bucket TABLE (a set has no
/// key/value PAIR region — there are no values). Each bucket is 16 bytes `{occupied: u64, key: i64}`;
/// `occupied` 0 = empty (an int KEY can legitimately be 0, unlike a map's never-0 canon, so occupancy
/// is a SEPARATE word and the probe tests it FIRST). Built by [`rt_u_set_seal`], probed inline by the
/// setcontains vertical. SAFE to share the flat-map bit pattern because `Kind::IntSet` STATICALLY gates
/// every use: the ONLY op that pops a `Kind::IntSet` is `Set.contains` (the setcontains arm); any other
/// IntSet-popping op raises `Unsupported`, so the whole function fails to JIT and runs on the VM — a set
/// handle can NEVER reach map-layout code (and `IntSet` returns are rejected, so it never escapes to a
/// materialize/decode path either).
pub(super) const UB_TAG_FLAT_SET: i64 = UB_TAG_SLOT | UB_TAG_FLAT;
/// The fibonacci-hash multiplier (2^64 / golden ratio, odd) for an int-set bucket: `bucket =
/// (key·MULT) >> (64 − log2)` takes the HIGH bits (well-mixed). Membership correctness does NOT depend
/// on the hash (open addressing + load factor ≤ 1/2 terminates for ANY hash); it only affects spread.
pub(super) const UB_SET_HASH_MULT: i64 = 0x9E37_79B9_7F4A_7C15u64 as i64;
/// Low-bits mask for the slot / base index.
pub(super) const UB_IDX_MASK: i64 = (1 << 40) - 1;
/// Byte offset of a string slot's precomputed FNV-1a hash (`PhStr::cached_hash` — never 0, so 0
/// means "hash unavailable" and the inline map probe falls back to the helper).
pub(super) const UB_SLOT_HASH_OFF: usize = 24;
/// Byte offset of a string slot's CANON word: `interned slot index + 1`, 0 = uncanonical (the
/// inline map probe punts to the helper). Assigned ONLY through the [`UbCtx::interned`] registry
/// (keyed by content, first-registration-wins), so canon equality ⇔ byte equality — the probe
/// compares ONE canon word per bucket instead of hash + three data words.
pub(super) const UB_SLOT_CANON_OFF: usize = 32;
/// Bit positions of the flat-MAP handle's count / log2(table-size) fields (see
/// [`UB_TAG_FLAT_MAP`]; flat LISTS keep their original 20-bit count at bit 40).
pub(super) const UB_MAP_CNT_SHIFT: i64 = 40;
pub(super) const UB_MAP_LOG_SHIFT: i64 = 52;
/// Bytes per arena slot: `len:u8` + up to 22 data bytes + slack, so the inline concat's bounded
/// 3×8-byte over-copies (a copy starting at `dst+1+la`, `la ≤ 22`, ends ≤ `dst+47`) stay inside
/// the 64-byte slot. 64 also keeps every slot cache-line-aligned.
pub(super) const UB_SLOT_SIZE: usize = 64;
/// Arena capacity in slots (256 KiB). Exhaustion is NOT a fault the user sees — the inline alloc
/// branches to code 5 and the whole call redoes on the VM.
pub(super) const UB_SLOT_CAP: usize = 4096;

/// The per-run handle state for unboxed handle ops. Created by [`Compiled::run_unboxed`] iff the
/// compiled graph uses handles; dropped when the run returns (all temps die with it — a fault path
/// leaks nothing into the VM redo). `#[repr(C)]`: the leading five fields are the JIT-visible
/// header, read/written by inline IR at fixed offsets 0/8/16/24/32 — reorder NOTHING above the
/// `--- Rust-only ---` line.
#[repr(C)]
pub(super) struct UbCtx {
    /// offset 0: base of the 64-byte-slot string arena (points into `buf_storage`).
    buf: *mut u8,
    /// offset 8: base of the recycled-slot stack (points into `free_storage`, `cap` entries).
    free_stack: *mut u32,
    /// offset 16: number of live entries on the recycled-slot stack.
    free_top: u64,
    /// offset 24: next never-used slot (grows toward `cap` when the free stack is empty).
    bump: u64,
    /// offset 32: total slots in the arena.
    cap: u64,
    /// offset 40: base of the accumulator record table (`UB_ACC_CAP` × 24-byte [`AccRec`]s —
    /// points into `acc_recs`, pre-allocated so it never moves).
    acc_base: *mut AccRec,
    /// offset 48: base of the map-materialization MEMO table (16 × 3-word entries — see
    /// `maps_ext.rs`; points into `memo_storage`, pre-allocated so it never moves). Probed
    /// INLINE by the `Map.keys`/`values`/`merge` emit arms; 0-words mean "empty".
    memo_base: *mut i64,
    // --- Rust-only (helpers may touch; inline IR must not) ---
    /// Boxed-`Value` handles (untagged): long consts, heap concat results.
    handles: Vec<Value>,
    /// Recycled untagged indices (all `>= n_pinned`).
    free: Vec<usize>,
    /// `handles` prefix holding pinned consts — never freed, never recycled.
    n_pinned: usize,
    /// Owns the arena bytes `buf` points into (Vec heap storage is stable across a struct move).
    #[allow(dead_code)]
    buf_storage: Vec<u8>,
    /// Owns the free-stack entries `free_stack` points into.
    #[allow(dead_code)]
    free_storage: Vec<u32>,
    /// The CANON registry: content → the slot that canonically holds it (pinned consts at
    /// startup; flat-list/map seal slots as they are bump-pinned). First registration wins —
    /// every nonzero slot canon word is `interned[content] + 1`, so canon equality ⇔ byte
    /// equality. Entries reference ONLY never-recycled slots (pinned or bump-pinned).
    interned: std::collections::HashMap<Vec<u8>, u32>,
    /// Owns the accumulator record table `acc_base` points into (fixed `UB_ACC_CAP` length).
    #[allow(dead_code)]
    acc_recs: Vec<AccRec>,
    /// Accumulator byte buffers: `acc_bufs[i]` backs `acc_recs[i]` (always `cap + SLACK` long,
    /// zero-filled — the inline over-copy may touch the slack). KEPT on record release so a
    /// recycled record reuses its grown capacity.
    acc_bufs: Vec<Vec<u8>>,
    /// Recycled accumulator record indices.
    acc_free: Vec<u32>,
    /// Next never-used accumulator record index (grows toward `UB_ACC_CAP`).
    acc_next: u32,
    /// Owns the memo words `memo_base` points into (fixed 120-word length — 40 × 3: map
    /// entries 0..16, string-predicate entries 16..24 (`strings_ext.rs`), set-op entries
    /// 24..32 difference / 32..40 union (`sets_ext.rs`)).
    memo_storage: Vec<i64>,
    /// FULL keys/values memo behind the direct-mapped inline table: map handle → `(keys_h,
    /// values_h)` (0 = that side not built yet). An inline-cache eviction re-installs from
    /// here instead of rebuilding — record/arena growth is bounded by DISTINCT maps.
    memo_kv: std::collections::HashMap<i64, (i64, i64)>,
    /// FULL merge memo behind the inline pair table: `(a, b)` → merged flat-map handle. Same
    /// discipline — an eviction re-installs, NEVER re-seals (the rebuild-per-iteration cliff).
    memo_merge: std::collections::HashMap<(i64, i64), i64>,
    /// FULL string-predicate memo behind the direct-mapped lines (entries 16..24): keys are
    /// PINNED words only (see `strings_ext::word_is_pinned_str`), value = result + 1.
    memo_str: std::collections::HashMap<(i64, i64), i64>,
    /// FULL set-op memo behind entries 24..40: `(a, b, which)` → the sealed result handle.
    memo_setop: std::collections::HashMap<(i64, i64, u8), i64>,
    /// `bump` right after const seeding — the pinned-const arena prefix. Everything at or past
    /// it is per-run state that [`UbCtx::reset_for_run`] reclaims (the ctx-reuse lever).
    const_bump: u64,
}

/// One accumulator record — the JIT-visible `{ptr,len,cap}` triple at `acc_base + idx·24`.
/// `#[repr(C)]`: inline IR reads/writes these at fixed offsets 0/8/16.
#[repr(C)]
pub(super) struct AccRec {
    /// offset 0: base of the byte buffer (points into the matching `acc_bufs` entry).
    ptr: *mut u8,
    /// offset 8: current content length in bytes.
    len: u64,
    /// offset 16: usable capacity (the buffer itself holds `cap + UB_ACC_SLACK` bytes).
    cap: u64,
}

impl UbCtx {
    /// Build a fresh per-run context, seeding the graph's interned consts in the SAME deterministic
    /// order `compile_unboxed` assigned their compile-time handles: a ≤22-byte string const becomes
    /// a pinned arena slot (borrowed `SLOT` handle), anything else a pinned `handles` entry.
    pub(super) fn new(const_values: &[Value]) -> UbCtx {
        let mut buf_storage = vec![0u8; UB_SLOT_CAP * UB_SLOT_SIZE];
        let mut free_storage = vec![0u32; UB_SLOT_CAP];
        let mut handles = Vec::new();
        let mut interned: std::collections::HashMap<Vec<u8>, u32> =
            std::collections::HashMap::new();
        let mut bump = 0u64;
        for v in const_values {
            match v {
                Value::Str(s) if s.len() <= crate::phstr::INLINE_CAP => {
                    let off = bump as usize * UB_SLOT_SIZE;
                    buf_storage[off] = s.len() as u8;
                    buf_storage[off + 1..off + 1 + s.len()].copy_from_slice(s.as_bytes());
                    buf_storage[off + UB_SLOT_HASH_OFF..off + UB_SLOT_HASH_OFF + 8]
                        .copy_from_slice(&s.cached_hash().to_le_bytes());
                    // A pinned const is its own canon (consts are content-deduped at compile).
                    buf_storage[off + UB_SLOT_CANON_OFF..off + UB_SLOT_CANON_OFF + 8]
                        .copy_from_slice(&(bump + 1).to_le_bytes());
                    interned.entry(s.as_bytes().to_vec()).or_insert(bump as u32);
                    bump += 1;
                }
                other => handles.push(other.clone()),
            }
        }
        let n_pinned = handles.len();
        let mut acc_recs: Vec<AccRec> = (0..UB_ACC_CAP)
            .map(|_| AccRec {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            })
            .collect();
        let mut memo_storage = vec![0i64; 120];
        UbCtx {
            buf: buf_storage.as_mut_ptr(),
            free_stack: free_storage.as_mut_ptr(),
            free_top: 0,
            bump,
            cap: UB_SLOT_CAP as u64,
            acc_base: acc_recs.as_mut_ptr(),
            memo_base: memo_storage.as_mut_ptr(),
            handles,
            free: Vec::new(),
            n_pinned,
            buf_storage,
            free_storage,
            interned,
            acc_recs,
            acc_bufs: vec![Vec::new(); UB_ACC_CAP],
            acc_free: Vec::new(),
            acc_next: 0,
            memo_storage,
            memo_kv: std::collections::HashMap::new(),
            memo_merge: std::collections::HashMap::new(),
            memo_str: std::collections::HashMap::new(),
            memo_setop: std::collections::HashMap::new(),
            const_bump: bump,
        }
    }

    /// Reset a CACHED context back to its post-[`UbCtx::new`] state — the ctx-reuse lever: a
    /// many-call graph must not pay the 256 KiB arena build + const seeding per call (measured:
    /// it made JIT'd handle-methods SLOWER than `--no-jit` on the sqlbuild shape). Zeroes ONLY
    /// the region the previous run dirtied, truncates the handle table to the pinned consts,
    /// drops run-registered canons (their slots are reclaimed with the bump), clears both free
    /// lists, and recycles every accumulator record while KEEPING its grown buffer (the reuse
    /// trick, now cross-call). Runs on ENTRY, so a faulted previous run leaves nothing behind.
    pub(super) fn reset_for_run(&mut self) {
        let dirty_from = self.const_bump as usize * UB_SLOT_SIZE;
        let dirty_to = (self.bump as usize * UB_SLOT_SIZE).min(self.buf_storage.len());
        if dirty_to > dirty_from {
            self.buf_storage[dirty_from..dirty_to].fill(0);
        }
        self.bump = self.const_bump;
        self.free_top = 0;
        self.handles.truncate(self.n_pinned);
        self.free.clear();
        let cb = self.const_bump as u32;
        self.interned.retain(|_, slot| *slot < cb);
        self.acc_free.clear();
        for i in 0..self.acc_next {
            self.acc_free.push(i);
            self.acc_recs[i as usize].len = 0;
        }
        // Map-materialization memos: entries key on THIS run's map handles (bump slots the
        // reset just reclaimed) — clearing is CORRECTNESS, not hygiene: a stale hit on a
        // re-sealed base would alias a different map's slots.
        self.memo_storage.fill(0);
        self.memo_kv.clear();
        self.memo_merge.clear();
        self.memo_str.clear();
        self.memo_setop.clear();
    }

    /// The compile-time handles for `const_values`, mirroring [`UbCtx::new`] exactly (same walk,
    /// same order): index `i` → the `iconst` the codegen bakes for that const.
    pub(super) fn const_compile_handles(const_values: &[Value]) -> Vec<i64> {
        let mut out = Vec::with_capacity(const_values.len());
        let mut slot = 0i64;
        let mut table = 0i64;
        for v in const_values {
            match v {
                Value::Str(s) if s.len() <= crate::phstr::INLINE_CAP => {
                    out.push(UB_TAG_SLOT | slot); // borrowed (OWNED clear): pinned, never freed
                    slot += 1;
                }
                _ => {
                    out.push(table);
                    table += 1;
                }
            }
        }
        out
    }

    pub(super) fn alloc(&mut self, v: Value) -> i64 {
        if let Some(i) = self.free.pop() {
            self.handles[i] = v;
            i as i64
        } else {
            self.handles.push(v);
            (self.handles.len() - 1) as i64
        }
    }

    /// Materialize ANY int/str list handle (flat slots, ACL builder record, boxed) into a fresh
    /// `Vec<Value>` — the clone half of PHP value semantics for the GENERAL `List.append`
    /// (`str_elems` = the compile-time element kind; a flat handle does not encode it).
    /// `None` = not a list handle (defensive → code 5, redo on VM).
    fn list_values(&self, h: i64, str_elems: bool) -> Option<Vec<Value>> {
        if h & UB_TAG_FLAT != 0 && h & UB_TAG_SLOT == 0 {
            let n = ((h >> 40) & 0xFFFFF) as usize;
            let base = (h & UB_IDX_MASK) as usize;
            let mut out = Vec::with_capacity(n + 1);
            for i in 0..n {
                let off = (base + i) * UB_SLOT_SIZE;
                if str_elems {
                    let len = self.buf_storage[off] as usize;
                    let s = std::str::from_utf8(&self.buf_storage[off + 1..off + 1 + len]).ok()?;
                    out.push(Value::Str(crate::phstr::PhStr::new(s)));
                } else {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&self.buf_storage[off..off + 8]);
                    out.push(Value::Int(i64::from_le_bytes(b)));
                }
            }
            return Some(out);
        }
        if h & UB_TAG_ACL != 0 {
            // ACL builder records: raw i64s (int semantics) or, with the ACLS marker (L2a),
            // STRING WORDS — the requested element kind must MATCH the record's marker
            // (defensive mismatch → None → code 5).
            let idx = (h & UB_IDX_MASK) as usize;
            if idx >= UB_ACC_CAP || str_elems != (h & UB_TAG_ACLS != 0) {
                return None;
            }
            let n = (self.acc_recs[idx].len / 8) as usize;
            let mut out = Vec::with_capacity(n + 1);
            for i in 0..n {
                let mut b = [0u8; 8];
                b.copy_from_slice(&self.acc_bufs[idx][i * 8..i * 8 + 8]);
                let w = i64::from_le_bytes(b);
                if str_elems {
                    let s: crate::phstr::PhStr = match self.str_bytes(w) {
                        Some(bytes) => std::str::from_utf8(bytes).ok()?.into(),
                        None => match self.handles.get(w as usize) {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return None,
                        },
                    };
                    out.push(Value::Str(s));
                } else {
                    out.push(Value::Int(w));
                }
            }
            return Some(out);
        }
        match self.handles.get(h as usize) {
            Some(Value::List(xs)) => Some(xs.as_ref().clone()),
            _ => None,
        }
    }

    /// Materialize a DYN list (boxed by construction; the one flat form is the empty
    /// literal) into a fresh `Vec<Value>`. `None` = defensive mismatch.
    fn dyn_list_values(&self, h: i64) -> Option<Vec<Value>> {
        if h & UB_TAG_FLAT != 0 && h & UB_TAG_SLOT == 0 {
            if (h >> 40) & 0xFFFFF != 0 {
                return None;
            }
            return Some(Vec::new());
        }
        match self.handles.get(h as usize) {
            Some(Value::List(xs)) => Some(xs.as_ref().clone()),
            _ => None,
        }
    }

    /// Materialize a RETURNED handle word into a real `Value` for the VM (the entry-level
    /// str/list return decode — a raw handle printed as an int was the conformance break this
    /// fixes). `repr`: 2 = str, 3 = str-list, 4 = int-list. `None` = defensive mismatch.
    pub(super) fn materialize(&self, h: i64, repr: i64) -> Option<Value> {
        match repr {
            2 => match self.str_bytes(h) {
                Some(bytes) => std::str::from_utf8(bytes)
                    .ok()
                    .map(|s| Value::Str(crate::phstr::PhStr::new(s))),
                None => match self.handles.get(h as usize) {
                    Some(v @ Value::Str(_)) => Some(v.clone()),
                    _ => None,
                },
            },
            3 => Some(Value::List(std::rc::Rc::new(self.list_values(h, true)?))),
            4 => Some(Value::List(std::rc::Rc::new(self.list_values(h, false)?))),
            5 => Some(Value::List(std::rc::Rc::new(self.dyn_list_values(h)?))),
            _ => None,
        }
    }

    /// Pop a recycled arena slot or bump a fresh one; `None` = arena exhausted (→ redo on VM).
    pub(super) fn alloc_slot(&mut self) -> Option<u64> {
        if self.free_top > 0 {
            self.free_top -= 1;
            Some(u64::from(self.free_storage[self.free_top as usize]))
        } else if self.bump < self.cap {
            let s = self.bump;
            self.bump += 1;
            Some(s)
        } else {
            None
        }
    }

    /// Write `bytes` (≤ 22) into a fresh arena slot with its FNV hash (0 = unavailable) and canon
    /// word (`interned slot + 1`, 0 = uncanonical — pass the registry lookup, NEVER a recyclable
    /// slot's own index); the OWNED `SLOT` handle, or `None` when full. The data tail (bytes
    /// `1+len..=23`) is ZEROED — the invariant the inline map probe whole-word compares rely on
    /// (a recycled slot may carry stale bytes).
    pub(super) fn alloc_slot_bytes(&mut self, bytes: &[u8], hash: u64, canon1: u64) -> Option<i64> {
        let slot = self.alloc_slot()?;
        let off = slot as usize * UB_SLOT_SIZE;
        self.buf_storage[off] = bytes.len() as u8;
        self.buf_storage[off + 1..off + 1 + bytes.len()].copy_from_slice(bytes);
        self.buf_storage[off + 1 + bytes.len()..off + UB_SLOT_HASH_OFF].fill(0);
        self.buf_storage[off + UB_SLOT_HASH_OFF..off + UB_SLOT_HASH_OFF + 8]
            .copy_from_slice(&hash.to_le_bytes());
        self.buf_storage[off + UB_SLOT_CANON_OFF..off + UB_SLOT_CANON_OFF + 8]
            .copy_from_slice(&canon1.to_le_bytes());
        Some(UB_TAG_SLOT | UB_TAG_OWNED | slot as i64)
    }

    /// The canon word (`slot + 1`) for `bytes` if some never-recycled slot canonically holds
    /// that content, else 0.
    pub(super) fn canon1_of(&self, bytes: &[u8]) -> u64 {
        self.interned.get(bytes).map_or(0, |s| u64::from(*s) + 1)
    }

    /// The bytes any STRING handle refers to (slot-tagged or untagged), or `None` on a mismatch.
    /// The slot branch requires `FLAT` CLEAR: `SLOT|FLAT` is a flat MAP handle (P-2b), never a string.
    pub(super) fn str_bytes(&self, h: i64) -> Option<&[u8]> {
        if h & UB_TAG_SLOT != 0 && h & UB_TAG_FLAT == 0 {
            let idx = (h & UB_IDX_MASK) as usize;
            if idx >= self.cap as usize {
                return None;
            }
            let off = idx * UB_SLOT_SIZE;
            let len = self.buf_storage[off] as usize;
            Some(&self.buf_storage[off + 1..off + 1 + len])
        } else if h & UB_TAG_FLAT != 0 {
            None
        } else if h & UB_TAG_ACC != 0 {
            let idx = (h & UB_IDX_MASK) as usize;
            if idx >= UB_ACC_CAP {
                return None;
            }
            let len = self.acc_recs[idx].len as usize;
            Some(&self.acc_bufs[idx][..len])
        } else {
            match self.handles.get(h as usize) {
                Some(Value::Str(s)) => Some(s.as_bytes()),
                _ => None,
            }
        }
    }

    /// In-place append onto a CONSUMED untagged heap-string handle (the fused accumulator
    /// path — see `rt_u_concat`): returns `true` and leaves `h` as the (now longer) result.
    /// The rhs bytes must already be copied OUT of the ctx (this takes `&mut self`).
    pub(super) fn try_append_in_place(&mut self, h: i64, rhs: &[u8]) -> bool {
        if h & (UB_TAG_SLOT | UB_TAG_FLAT | UB_TAG_ACC) != 0 || h < self.n_pinned as i64 {
            // Tagged / pinned-const handles never mutate (a pinned literal's Rc is shared with
            // the chunk consts anyway - get_mut would refuse - but guard explicitly).
            return false;
        }
        match self.handles.get_mut(h as usize) {
            Some(Value::Str(s)) => s.append_in_place(rhs),
            _ => false,
        }
    }

    /// Release any OWNED handle: an owned arena slot recycles onto the free stack; an untagged
    /// temp releases its `handles` entry; borrowed slots / flat lists / pinned entries are no-ops.
    pub(super) fn release(&mut self, h: i64) {
        if h & UB_TAG_SLOT != 0 {
            if h & UB_TAG_OWNED != 0 {
                let idx = (h & UB_IDX_MASK) as u64;
                if idx < self.cap && (self.free_top as usize) < self.free_storage.len() {
                    self.free_storage[self.free_top as usize] = idx as u32;
                    self.free_top += 1;
                }
            }
        } else if h & UB_TAG_FLAT != 0 {
            // Flat-list slots are bump-pinned for the run (built once per call) — no recycling.
        } else if h & (UB_TAG_ACC | UB_TAG_ACL | UB_TAG_AMB) != 0 {
            // A SHARED record is MEMO-owned (maps_ext): live handles to it may exist anywhere,
            // so a consumer release must NOT recycle it — the memo (or the per-run reset) is
            // its sole lifecycle owner.
            if h & UB_TAG_SHARED != 0 {
                return;
            }
            // Recycle the RECORD (string accumulator or list builder — same pool), keep its
            // buffer: a reconverted accumulator/builder (the reset pattern) reuses the grown
            // capacity — php's buffer-reuse trick.
            let idx = (h & UB_IDX_MASK) as u32;
            if (idx as usize) < UB_ACC_CAP {
                if h & UB_TAG_ACLS != 0 {
                    // L2a: a STR list-builder record OWNS its element words — release each
                    // first (borrowed-slot/const words no-op; Owned words free/recycle).
                    let n = (self.acc_recs[idx as usize].len / 8) as usize;
                    for i in 0..n {
                        let mut b8 = [0u8; 8];
                        b8.copy_from_slice(&self.acc_bufs[idx as usize][i * 8..i * 8 + 8]);
                        self.release(i64::from_le_bytes(b8));
                    }
                }
                self.acc_free.push(idx);
            }
        } else {
            let i = h as usize;
            if h >= self.n_pinned as i64 && i < self.handles.len() {
                self.handles[i] = Value::Unit;
                self.free.push(i);
            }
        }
    }

    /// Ensure accumulator `idx`'s usable cap ≥ `need` (doubling growth; the buffer always holds
    /// `cap + UB_ACC_SLACK` zero-filled bytes — the inline over-copy may touch the slack).
    /// Growth preserves content (`Vec::resize` extends) and refreshes the JIT-visible `ptr`.
    fn acc_grow_to(&mut self, idx: usize, need: usize) {
        if (self.acc_recs[idx].cap as usize) < need {
            let new_cap = need.max(self.acc_recs[idx].cap as usize * 2).max(64);
            self.acc_bufs[idx].resize(new_cap + UB_ACC_SLACK, 0);
            self.acc_recs[idx].cap = new_cap as u64;
            self.acc_recs[idx].ptr = self.acc_bufs[idx].as_mut_ptr();
        }
    }

    /// Append `bytes` to accumulator `idx` (capacity must already fit — see [`Self::acc_grow_to`]).
    fn acc_push(&mut self, idx: usize, bytes: &[u8]) {
        let len = self.acc_recs[idx].len as usize;
        self.acc_bufs[idx][len..len + bytes.len()].copy_from_slice(bytes);
        self.acc_recs[idx].len = (len + bytes.len()) as u64;
    }

    /// Take a free accumulator record (recycled first — its buffer keeps the grown capacity),
    /// or `None` when all `UB_ACC_CAP` records are live (fall back to the plain concat path).
    fn acc_take_record(&mut self) -> Option<usize> {
        if let Some(i) = self.acc_free.pop() {
            return Some(i as usize);
        }
        if (self.acc_next as usize) < UB_ACC_CAP {
            let i = self.acc_next;
            self.acc_next += 1;
            return Some(i as usize);
        }
        None
    }
}

/// An UNTAGGED word — a plain `handles` index (no slot/flat/builder/owned tag bits): the
/// boxed-`Value` island where Rc sharing (the VM's own COW discipline) is legal.
#[inline]
fn ub_is_untagged(h: i64) -> bool {
    h >= 0
        && h & (UB_TAG_SLOT | UB_TAG_FLAT | UB_TAG_ACC | UB_TAG_ACL | UB_TAG_AMB | UB_TAG_OWNED)
            == 0
}

/// SAFETY (all `rt_u_*`): `ctx` is the `&mut UbCtx` the current `run_unboxed` call owns, passed as an
/// opaque pointer; the compiled code is single-threaded within the call and never aliases it. Every
/// helper is defensive on the impossible bad-handle case (validated stack discipline) — it returns
/// `-1` (fault → redo on VM) rather than panicking across the `extern "C"` boundary.
pub(super) extern "C" fn rt_u_list_new(ctx: *mut UbCtx, cap: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    ctx.alloc(Value::List(std::rc::Rc::new(Vec::with_capacity(
        cap.max(0) as usize,
    ))))
}

/// Append the string at handle `elem` (any encoding) to the (uniquely-owned, still-building,
/// UNTAGGED) list at `list`. `free_elem != 0` consumes the element handle.
pub(super) extern "C" fn rt_u_list_push(
    ctx: *mut UbCtx,
    list: i64,
    elem: i64,
    free_elem: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let ev = match ctx.str_bytes(elem) {
        // The bytes came from a valid `PhStr` (or an arena slot written from one), so they are
        // valid UTF-8; `PhStr::new` re-copies them into the right representation.
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(s) => Value::Str(crate::phstr::PhStr::new(s)),
            Err(_) => return -1,
        },
        None => match ctx.handles.get(elem as usize) {
            Some(v) => v.clone(),
            None => return -1,
        },
    };
    match ctx.handles.get_mut(list as usize) {
        Some(Value::List(xs)) => match std::rc::Rc::get_mut(xs) {
            Some(v) => v.push(ev),
            None => return -1,
        },
        _ => return -1,
    }
    if free_elem != 0 {
        ctx.release(elem);
    }
    0
}

/// Finalize a just-built list: when EVERY element is a ≤22-byte string, flatten them into
/// consecutive arena slots and return a `FLAT` handle (releasing the boxed list — `Index` then
/// runs fully inline, zero-copy); otherwise return the untagged handle unchanged. `-1` only on a
/// defensive mismatch.
pub(super) extern "C" fn rt_u_list_seal(ctx: *mut UbCtx, list: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    // P-2c: an all-INT list flattens too — each element's raw `i64` in its slot's bytes 0..8
    // (the flat-map VALUE-slot layout); `Index` on a `Kind::IntList` then reads it inline. The
    // handle encoding is the same `FLAT` shape; the compile-time `Kind` picks the read path.
    let ints: Option<Vec<i64>> = match ctx.handles.get(list as usize) {
        Some(Value::List(xs)) if !xs.is_empty() => xs
            .iter()
            .map(|v| match v {
                Value::Int(n) => Some(*n),
                _ => None,
            })
            .collect(),
        _ => None,
    };
    if let Some(vals) = ints {
        let n = vals.len() as i64;
        if n >= 1 << 20 || ctx.bump + n as u64 > ctx.cap {
            return list;
        }
        let base = ctx.bump as i64;
        for v in &vals {
            let off = ctx.bump as usize * UB_SLOT_SIZE;
            ctx.buf_storage[off..off + 8].copy_from_slice(&v.to_le_bytes());
            ctx.bump += 1;
        }
        ctx.release(list);
        return UB_TAG_FLAT | (n << 40) | base;
    }
    let flat: Option<Vec<&[u8]>> = match ctx.handles.get(list as usize) {
        Some(Value::List(xs)) => xs
            .iter()
            .map(|v| match v {
                Value::Str(s) if s.len() <= crate::phstr::INLINE_CAP => Some(s.as_bytes()),
                _ => None,
            })
            .collect(),
        _ => return -1,
    };
    let Some(elems) = flat else {
        return list; // not flattenable — stays a boxed list (helper-path Index)
    };
    let n = elems.len() as i64;
    if n >= 1 << 20 || ctx.bump + n as u64 > ctx.cap {
        return list; // too large to flatten — boxed path still works
    }
    let base = ctx.bump as i64;
    let owned: Vec<(Vec<u8>, u64)> = match ctx.handles.get(list as usize) {
        Some(Value::List(xs)) => xs
            .iter()
            .map(|v| match v {
                Value::Str(s) => (s.as_bytes().to_vec(), s.cached_hash()),
                _ => (Vec::new(), 0),
            })
            .collect(),
        _ => return -1,
    };
    for (bytes, hash) in &owned {
        let slot = ctx.bump as usize;
        let off = slot * UB_SLOT_SIZE;
        ctx.buf_storage[off] = bytes.len() as u8;
        ctx.buf_storage[off + 1..off + 1 + bytes.len()].copy_from_slice(bytes);
        ctx.buf_storage[off + 1 + bytes.len()..off + UB_SLOT_HASH_OFF].fill(0);
        ctx.buf_storage[off + UB_SLOT_HASH_OFF..off + UB_SLOT_HASH_OFF + 8]
            .copy_from_slice(&hash.to_le_bytes());
        // Canonicalize: adopt the registry's slot for this content, or register this one (a flat
        // element is bump-pinned — never recycled — so it may safely enter the registry).
        let canon1 = *ctx.interned.entry(bytes.clone()).or_insert(slot as u32) as u64 + 1;
        ctx.buf_storage[off + UB_SLOT_CANON_OFF..off + UB_SLOT_CANON_OFF + 8]
            .copy_from_slice(&canon1.to_le_bytes());
        ctx.bump += 1;
    }
    ctx.release(list);
    UB_TAG_FLAT | (n << 40) | base
}

/// `list[idx]` — the helper (slow) path for UNTAGGED (boxed) lists; a flat list indexes inline.
/// VM-exact bounds semantics; out-of-range returns `-1` → code 5 → the canonical fault on the VM
/// redo. A short string element lands in an OWNED arena slot (fast for downstream inline ops);
/// anything else becomes an untagged temp. `free_list != 0` consumes the list handle.
pub(super) extern "C" fn rt_u_index(ctx: *mut UbCtx, list: i64, idx: i64, free_list: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if list & (UB_TAG_SLOT | UB_TAG_FLAT) != 0 {
        // Defensive: the codegen sends flat lists down the inline path; mirror it here anyway.
        // A flat LIST has `FLAT` set and `SLOT` clear (`SLOT|FLAT` = a flat MAP — not a list).
        if list & UB_TAG_FLAT != 0 && list & UB_TAG_SLOT == 0 {
            let n = (list >> 40) & 0xFFFFF;
            let base = list & UB_IDX_MASK;
            if (0..n).contains(&idx) {
                return UB_TAG_SLOT | (base + idx); // borrowed slot
            }
        }
        return -1;
    }
    let elem = match ctx.handles.get(list as usize) {
        Some(Value::List(xs)) => match usize::try_from(idx).ok().filter(|i| *i < xs.len()) {
            Some(i) => xs[i].clone(),
            None => return -1,
        },
        _ => return -1,
    };
    if free_list != 0 {
        ctx.release(list);
    }
    match &elem {
        Value::Str(s) if s.len() <= crate::phstr::INLINE_CAP => {
            // An interned content returns its canonical slot BORROWED (zero alloc, probe-ready);
            // an unknown one gets a fresh OWNED slot with canon 0 (recyclable slots must never
            // enter the registry). `None` = arena exhausted → -1 → code 5, redo on VM.
            match ctx.interned.get(s.as_bytes()) {
                Some(slot) => UB_TAG_SLOT | i64::from(*slot),
                None => ctx
                    .alloc_slot_bytes(s.as_bytes(), s.cached_hash(), 0)
                    .unwrap_or(-1),
            }
        }
        _ => ctx.alloc(elem),
    }
}

/// `a + b` (string concat) — the helper (slow) path: any operand encoding, any length. Byte
/// semantics are exactly [`crate::phstr::PhStr::concat`]'s (byte concatenation). A ≤22-byte result
/// lands in an OWNED arena slot (fast for downstream inline ops); longer results go through the
/// single-sourced `PhStr::concat` kernel into an untagged temp. `free_mask` bit0/bit1 consume the
/// operands (OWNED rules apply — a borrowed slot free is a no-op).
pub(super) extern "C" fn rt_u_concat(ctx: *mut UbCtx, a: i64, b: i64, free_mask: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    // FUSED-accumulator fast path (`s = s + x` — the caller marked the lhs CONSUMED): a
    // uniquely-owned untagged heap lhs appends the rhs bytes IN PLACE (amortized O(1)) and the
    // SAME handle is the result — no copy, no new entry. Compile-time ownership proves what
    // PHP proves by refcount. The rhs bytes are copied out first (self-append `s = s + s` and
    // the `&mut` below both need the borrow released); ≤ 64B stays on the stack.
    if free_mask & 1 != 0 && a & (UB_TAG_SLOT | UB_TAG_FLAT) == 0 {
        let mut small = [0u8; 64];
        let mut spill: Vec<u8> = Vec::new();
        let rhs_len = {
            let Some(bb) = ctx.str_bytes(b) else {
                return -1;
            };
            if bb.len() <= small.len() {
                small[..bb.len()].copy_from_slice(bb);
            } else {
                spill = bb.to_vec();
            }
            bb.len()
        };
        let rhs: &[u8] = if rhs_len <= small.len() {
            &small[..rhs_len]
        } else {
            &spill
        };
        if ctx.try_append_in_place(a, rhs) {
            if free_mask & 2 != 0 {
                ctx.release(b);
            }
            return a;
        }
    }
    let (Some(ab), Some(bb)) = (ctx.str_bytes(a), ctx.str_bytes(b)) else {
        return -1;
    };
    let total = ab.len() + bb.len();
    let res = if total <= crate::phstr::INLINE_CAP {
        let mut joined = [0u8; crate::phstr::INLINE_CAP];
        joined[..ab.len()].copy_from_slice(ab);
        joined[ab.len()..total].copy_from_slice(bb);
        let bytes = joined[..total].to_vec();
        // Real hash + canon lookup: a helper-path concat result that reproduces an interned
        // content probes maps fully inline (canon ≠ 0 ⟹ hash is real — the probe's bucket index
        // depends on it; the INLINE concat writes 0/0 and punts to the helper instead).
        let hash = match crate::phstr::fnv1a(&bytes) {
            0 => 1,
            h => h,
        };
        let canon1 = ctx.canon1_of(&bytes);
        match ctx.alloc_slot_bytes(&bytes, hash, canon1) {
            Some(h) => h,
            None => return -1, // arena exhausted → redo on VM
        }
    } else {
        // Both sides are valid UTF-8 by construction; concat of valid UTF-8 is valid UTF-8.
        let (Ok(xs), Ok(ys)) = (std::str::from_utf8(ab), std::str::from_utf8(bb)) else {
            return -1;
        };
        let joined = crate::phstr::PhStr::concat(&crate::phstr::PhStr::new(xs), &{
            crate::phstr::PhStr::new(ys)
        });
        let v = Value::Str(joined);
        // Reborrow dance: `str_bytes` borrows ended above (bytes copied) — safe to mutate now.
        ctx.alloc(v)
    };
    if free_mask & 1 != 0 {
        ctx.release(a);
    }
    if free_mask & 2 != 0 {
        ctx.release(b);
    }
    res
}

/// FUSED accumulator append (`s = s + x`, the strbuild vertical) — the SLOW leg of the inline
/// ACC fast path. Three cases: (1) lhs already an ACC record → capacity growth (doubling) +
/// append; (2) lhs any other string encoding → CONVERT: take a record (a recycled one reuses
/// its grown buffer), copy the lhs bytes in, append the rhs, release the consumed lhs, return
/// `ACC|idx` — every subsequent append then runs fully inline; (3) record table exhausted
/// (> `UB_ACC_CAP` live accumulators) → the plain `rt_u_concat` path (correct, slower). The
/// rhs bytes are copied out first (self-append aliasing + the `&mut` appends). `free_mask`
/// bit0 consumes the lhs (always set at accumulator sites), bit1 the rhs. `-1` = bad handle.
pub(super) extern "C" fn rt_u_acc_append(ctx: *mut UbCtx, a: i64, b: i64, free_mask: i64) -> i64 {
    let ctx_ref = unsafe { &mut *ctx };
    let mut small = [0u8; 64];
    let mut spill: Vec<u8> = Vec::new();
    let rhs_len = {
        let Some(bb) = ctx_ref.str_bytes(b) else {
            return -1;
        };
        if bb.len() <= small.len() {
            small[..bb.len()].copy_from_slice(bb);
        } else {
            spill = bb.to_vec();
        }
        bb.len()
    };
    let rhs: &[u8] = if rhs_len <= small.len() {
        &small[..rhs_len]
    } else {
        &spill
    };
    let res = if a & UB_TAG_ACC != 0 {
        // Growth leg: the inline path found len + rhs_len > cap.
        let idx = (a & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP {
            return -1;
        }
        let need = ctx_ref.acc_recs[idx].len as usize + rhs_len;
        ctx_ref.acc_grow_to(idx, need);
        ctx_ref.acc_push(idx, rhs);
        a
    } else {
        // Conversion leg: the first append onto a non-ACC lhs (fn entry / post-reset).
        let Some(idx) = ctx_ref.acc_take_record() else {
            // Record table exhausted — the plain concat path handles masks itself.
            return rt_u_concat(ctx, a, b, free_mask);
        };
        let lhs_owned: Vec<u8> = match ctx_ref.str_bytes(a) {
            Some(bytes) => bytes.to_vec(),
            None => {
                ctx_ref.acc_free.push(idx as u32); // put the record back
                return -1;
            }
        };
        ctx_ref.acc_grow_to(idx, (lhs_owned.len() + rhs_len).max(64));
        ctx_ref.acc_recs[idx].len = 0;
        ctx_ref.acc_push(idx, &lhs_owned);
        ctx_ref.acc_push(idx, rhs);
        if free_mask & 1 != 0 {
            ctx_ref.release(a);
        }
        UB_TAG_ACC | idx as i64
    };
    if free_mask & 2 != 0 {
        ctx_ref.release(b);
    }
    res
}

/// Render an `Int` operand of a mixed `Concat` as its decimal string — exactly the VM's
/// `as_display` for `Value::Int` (`n.to_string()`) — into a fresh arena slot. An i64 decimal is
/// ≤ 20 bytes, so the result ALWAYS fits inline (`INLINE_CAP` = 22): one slot alloc, no heap.
/// Registered content (real hash + canon) so a rendered key probes maps fully inline. Returns
/// the owned slot handle, or `-1` on arena exhaustion (→ code 5, redo on VM).
pub(super) extern "C" fn rt_u_int_to_str(ctx: *mut UbCtx, v: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    // Zero-alloc decimal render — the exact bytes of the VM's `as_display` (`n.to_string()`):
    // digits written backward into a stack buffer ("-9223372036854775808" is 20 bytes, the max).
    let mut buf = [0u8; 20];
    let mut pos = buf.len();
    let neg = v < 0;
    let mut u = v.unsigned_abs();
    loop {
        pos -= 1;
        buf[pos] = b'0' + (u % 10) as u8;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    if neg {
        pos -= 1;
        buf[pos] = b'-';
    }
    let bytes = &buf[pos..];
    // hash 0 + canon 0 = "punt to the helper" — same marker as the INLINE concat result. A
    // rendered int used as a map key just takes the helper probe (correct, rare); skipping
    // the fnv1a + canon-registry probe here is the hot-path win.
    // arena exhausted → -1 → code 5, redo on VM
    ctx.alloc_slot_bytes(bytes, 0, 0).unwrap_or(-1)
}

/// FUSED mixed-interpolation concat (`Concat(n)`, n ≤ 6, any mix of string handles and raw
/// ints): ONE call renders every `Int` part (zero-alloc stack render — the exact `as_display`
/// bytes) and joins all parts in source order, with NO intermediate slots (the pairwise fold
/// this replaces allocated + copied + freed a slot per merge). `kmask` bit j = part j is a raw
/// `Int` (else a string handle); `fmask` bit j = consume part j's handle (`Int` parts ignore
/// it). Result: an inline-short slot or a heap handle; `-1` = bad handle / arena exhausted
/// (→ code 5, redo on VM).
#[allow(clippy::too_many_arguments)] // fixed extern "C" shape: 6 part registers + masks
pub(super) extern "C" fn rt_u_concat_mix(
    ctx: *mut UbCtx,
    n: i64,
    kmask: i64,
    fmask: i64,
    p0: i64,
    p1: i64,
    p2: i64,
    p3: i64,
    p4: i64,
    p5: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let parts = [p0, p1, p2, p3, p4, p5];
    let n = n as usize;
    debug_assert!(n <= 6);
    // Render/collect every part into ONE stack buffer (source order — the VM's `as_display`
    // walk): 6 parts × (≤22B short string | ≤20B int digits) ≤ 132 < 160, so the hot path is
    // ZERO-alloc. Long (heap) string parts overflow to a heap join — the cold path.
    let mut small = [0u8; 160];
    let mut slen = 0usize;
    let mut overflow: Option<Vec<u8>> = None;
    for (j, &pv) in parts.iter().enumerate().take(n) {
        let mut ibuf = [0u8; 20];
        let piece: &[u8] = if kmask & (1 << j) != 0 {
            // Raw Int → decimal bytes (identical to `Value::Int`'s `as_display`).
            let mut pos = ibuf.len();
            let neg = pv < 0;
            let mut u = pv.unsigned_abs();
            loop {
                pos -= 1;
                ibuf[pos] = b'0' + (u % 10) as u8;
                u /= 10;
                if u == 0 {
                    break;
                }
            }
            if neg {
                pos -= 1;
                ibuf[pos] = b'-';
            }
            &ibuf[pos..]
        } else {
            match ctx.str_bytes(pv) {
                Some(bytes) => bytes,
                None => return -1,
            }
        };
        match &mut overflow {
            Some(v) => v.extend_from_slice(piece),
            None if slen + piece.len() <= small.len() => {
                small[slen..slen + piece.len()].copy_from_slice(piece);
                slen += piece.len();
            }
            None => {
                let mut v = Vec::with_capacity(slen + piece.len() + 64);
                v.extend_from_slice(&small[..slen]);
                v.extend_from_slice(piece);
                overflow = Some(v);
            }
        }
    }
    let joined: &[u8] = match &overflow {
        Some(v) => v,
        None => &small[..slen],
    };
    let res = if joined.len() <= crate::phstr::INLINE_CAP {
        // hash 0 + canon 0 = "punt to the helper" — same marker as the INLINE concat result;
        // skipping the fnv1a + canon-registry probe is the hot-path win (a joined string used
        // as a map key takes the helper probe — correct, rare).
        match ctx.alloc_slot_bytes(joined, 0, 0) {
            Some(h) => h,
            None => return -1, // arena exhausted → redo on VM
        }
    } else {
        // Valid UTF-8 by construction (handle parts are valid, digit bytes are ASCII).
        let Ok(s) = std::str::from_utf8(joined) else {
            return -1;
        };
        let v = Value::Str(crate::phstr::PhStr::new(s));
        ctx.alloc(v)
    };
    for (j, &pv) in parts.iter().enumerate().take(n) {
        if kmask & (1 << j) == 0 && fmask & (1 << j) != 0 {
            ctx.release(pv);
        }
    }
    res
}

/// `Core.String.length` — byte length; the helper (slow) path for untagged handles (a slot handle
/// reads its length inline). `free != 0` consumes the handle. `-1` = defensive bad-handle fault.
/// `Op::Len` helper — a BOXED (untagged) list handle's length (the flat case is fully inline:
/// the count rides the handle bits). `-1` = defensive bad-handle fault (→ code 5, redo on VM).
pub(super) extern "C" fn rt_u_list_len(ctx: *mut UbCtx, h: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    if h & UB_TAG_FLAT != 0 {
        return (h >> 40) & 0xFFFFF;
    }
    if h & UB_TAG_ACL != 0 {
        let idx = (h & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP {
            return -1;
        }
        return (ctx.acc_recs[idx].len / 8) as i64;
    }
    match ctx.handles.get(h as usize) {
        Some(Value::List(xs)) => xs.len() as i64,
        _ => -1,
    }
}

/// Append one `key => value` pair to a still-building map scratch (an UNTAGGED, uniquely-owned
/// `Value::List` accumulating `k1, v1, k2, v2, …` — created by `rt_u_list_new(2n)`). The key is a
/// string handle (any encoding), the value a raw `i64`. `free_key != 0` consumes the key handle.
/// `-1` = defensive bad-handle fault (→ code 5, redo on VM).
pub(super) extern "C" fn rt_u_map_push_pair(
    ctx: *mut UbCtx,
    map: i64,
    key: i64,
    val: i64,
    free_key: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let kv = match ctx.str_bytes(key) {
        // Valid UTF-8 by construction (written from a `PhStr`); `PhStr::new` re-interns.
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(s) => Value::Str(crate::phstr::PhStr::new(s)),
            Err(_) => return -1,
        },
        None => match ctx.handles.get(key as usize) {
            Some(v @ Value::Str(_)) => v.clone(),
            _ => return -1,
        },
    };
    match ctx.handles.get_mut(map as usize) {
        Some(Value::List(xs)) => match std::rc::Rc::get_mut(xs) {
            Some(v) => {
                v.push(kv);
                v.push(Value::Int(val));
            }
            None => return -1,
        },
        _ => return -1,
    }
    if free_key != 0 {
        ctx.release(key);
    }
    0
}

/// Finalize a just-built map scratch (`k1, v1, …` — see [`rt_u_map_push_pair`]): dedup through the
/// canonical [`crate::value::build_map`] kernel (first position, last value — PHP semantics, exactly
/// the VM's `Op::MakeMap`), then flatten iff every key is a ≤22-byte string (values are `Int` by the
/// analyzer's `MakeMap` kind proof): consecutive arena slot PAIRS — pair `i` = key slot `base+2i`
/// (bytes + zero tail + FNV hash at [`UB_SLOT_HASH_OFF`]) and value slot `base+2i+1` (the raw `i64`
/// LE in bytes 0..8) — returning a `SLOT|FLAT` [`UB_TAG_FLAT_MAP`] handle (lookup then runs fully
/// inline). A non-flattenable map becomes a boxed `Value::Map` (untagged handle, helper lookup).
/// `-1` = defensive mismatch / arena exhaustion / kernel fault (→ code 5, redo on VM).
pub(super) extern "C" fn rt_u_map_seal(ctx: *mut UbCtx, map: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let pairs: Vec<(Value, Value)> = match ctx.handles.get(map as usize) {
        Some(Value::List(xs)) if xs.len() % 2 == 0 => xs
            .chunks_exact(2)
            .map(|kv| (kv[0].clone(), kv[1].clone()))
            .collect(),
        _ => return -1,
    };
    let deduped = match crate::value::build_map(pairs) {
        Ok(m) => m,
        Err(_) => return -1, // non-HKey key: checker-unreachable; the VM redo renders the fault
    };
    let flat: Option<Vec<(&crate::phstr::PhStr, i64)>> = deduped
        .iter()
        .map(|(k, v)| match (k, v) {
            (crate::value::HKey::Str(s), Value::Int(n)) if s.len() <= crate::phstr::INLINE_CAP => {
                Some((s, *n))
            }
            _ => None,
        })
        .collect();
    let Some(entries) = flat else {
        // Not flattenable (long key / non-int value — the latter is analyzer-unreachable):
        // boxed map, helper-path lookup through the canonical kernel.
        ctx.release(map);
        return ctx.alloc(Value::Map(std::rc::Rc::new(deduped)));
    };
    // The pair/table WRITER is shared with `Map.merge` — extracted to `maps_ext::seal_flat_entries`
    // (count/arena guards included; `None` → boxed fallback).
    let owned: Vec<(Vec<u8>, u64, i64)> = entries
        .iter()
        .map(|(s, v)| (s.as_bytes().to_vec(), s.cached_hash(), *v))
        .collect();
    ctx.release(map);
    match seal_flat_entries(ctx, &owned) {
        Some(h) => h,
        None => ctx.alloc(Value::Map(std::rc::Rc::new(deduped))),
    }
}

/// `#[repr(C)]` two-`i64` return for [`rt_u_map_get`], matching a Cranelift `returns = [i64, i64]`
/// import signature exactly as the compiled functions' own two-i64 return does (rax:rdx on SysV
/// x86-64, x0:x1 on AArch64). `code` 0 = ok; 5 = redo on VM (missing key → the canonical
/// `"map key not found"` fault, or a defensive mismatch).
#[repr(C)]
pub(super) struct UbMapGetRet {
    value: i64,
    code: i64,
}

/// `m[k]` (string-keyed int map) — the helper (slow) path: a FLAT map probed by hash+bytes (covers
/// hash-0 keys the inline probe punts on), a boxed map through the canonical
/// [`crate::value::map_index`] kernel. `free_mask` bit0 consumes the key, bit1 the map (on success —
/// a code-5 return redoes the whole call on the VM, discarding the ctx).
pub(super) extern "C" fn rt_u_map_get(
    ctx: *mut UbCtx,
    map: i64,
    key: i64,
    free_mask: i64,
) -> UbMapGetRet {
    let ctx = unsafe { &mut *ctx };
    let miss = UbMapGetRet { value: 0, code: 5 };
    let Some(kb) = ctx.str_bytes(key) else {
        return miss;
    };
    if map & UB_TAG_AMB != 0 {
        // A MUTABLE map builder (the mapinsert vertical): canon via the registry (a key that
        // was never interned anywhere cannot be in the builder — genuine miss, the VM redo
        // renders the canonical "map key not found"), then the packed table probe.
        let kb = kb.to_vec();
        let ridx = (map & UB_IDX_MASK) as usize;
        if ridx >= UB_ACC_CAP {
            return miss;
        }
        let Some(&kslot) = ctx.interned.get(&kb) else {
            return miss;
        };
        let canon1 = kslot as u64 + 1;
        let hoff = kslot as usize * UB_SLOT_SIZE + UB_SLOT_HASH_OFF;
        let mut hb = [0u8; 8];
        hb.copy_from_slice(&ctx.buf_storage[hoff..hoff + 8]);
        let hash = u64::from_le_bytes(hb);
        let buf = &ctx.acc_bufs[ridx];
        let mut w8 = [0u8; 8];
        w8.copy_from_slice(&buf[0..8]);
        let tsize = 1usize << u64::from_le_bytes(w8);
        let mut t = (hash as usize) & (tsize - 1);
        loop {
            let eoff = 16 + t * 16;
            let mut cb = [0u8; 8];
            cb.copy_from_slice(&buf[eoff..eoff + 8]);
            let ecanon = u64::from_le_bytes(cb);
            if ecanon == canon1 {
                let mut vb = [0u8; 8];
                vb.copy_from_slice(&buf[eoff + 8..eoff + 16]);
                if free_mask & 1 != 0 {
                    ctx.release(key);
                }
                if free_mask & 2 != 0 {
                    ctx.release(map);
                }
                return UbMapGetRet {
                    value: i64::from_le_bytes(vb),
                    code: 0,
                };
            }
            if ecanon == 0 {
                return miss; // genuine miss — canonical fault on the VM redo
            }
            t = (t + 1) & (tsize - 1);
        }
    }
    let value = if map & UB_TAG_FLAT != 0 {
        let kb = kb.to_vec(); // end the str_bytes borrow before re-borrowing the arena
        let n = (map >> UB_MAP_CNT_SHIFT) & 0xFFF;
        let base = (map & UB_IDX_MASK) as usize;
        let mut found = None;
        for i in 0..n as usize {
            let koff = (base + 2 * i) * UB_SLOT_SIZE;
            let len = ctx.buf_storage[koff] as usize;
            if ctx.buf_storage[koff + 1..koff + 1 + len] == kb[..] {
                let voff = koff + UB_SLOT_SIZE;
                let mut vb = [0u8; 8];
                vb.copy_from_slice(&ctx.buf_storage[voff..voff + 8]);
                found = Some(i64::from_le_bytes(vb));
                break;
            }
        }
        match found {
            Some(v) => v,
            None => return miss, // the VM redo renders the canonical "map key not found"
        }
    } else {
        let Ok(ks) = std::str::from_utf8(kb) else {
            return miss;
        };
        let kv = Value::Str(crate::phstr::PhStr::new(ks));
        match ctx.handles.get(map as usize) {
            Some(Value::Map(m)) => match crate::value::map_index(m, &kv) {
                Ok(Value::Int(v)) => v,
                _ => return miss, // missing key (canonical fault on redo) / non-int (unreachable)
            },
            _ => return miss,
        }
    };
    if free_mask & 1 != 0 {
        ctx.release(key);
    }
    if free_mask & 2 != 0 {
        ctx.release(map);
    }
    UbMapGetRet { value, code: 0 }
}

/// `#[repr(C)]` two-`i64` return for [`rt_u_map_has`], matching a Cranelift `returns = [i64, i64]`
/// import signature (same ABI note as [`UbMapGetRet`]). `present` 1 = key found, 0 = clean absent;
/// `code` 0 = a clean answer (found OR genuinely-absent — a map query never faults on a miss), 5 =
/// redo on VM (DEFENSIVE only — the arm couldn't decide, never a genuine miss).
#[repr(C)]
pub(super) struct UbMapHasRet {
    present: i64,
    code: i64,
}

/// `Map.has(m, k)` (string-keyed map) — the helper (slow) path: a FLAT map scanned by bytes (covers
/// hash-0 keys the inline probe punts on), a boxed map through the native's OWN key-presence kernel
/// (`m.iter().any(|(k,_)| *k == hk)` — byte-identical to `Core.Map.has`). Unlike [`rt_u_map_get`], a
/// genuine miss is a CLEAN `present:0, code:0` (never a code-5 redo): `Map.has` returns `false`, it
/// does not fault. code 5 is DEFENSIVE only (undecidable shapes: a mutable AMB builder — perf-deferred
/// for `has`; an out-of-range handle; a non-utf8 key) — the VM recomputes `Map.has` byte-identically.
/// `free_mask` bit0 consumes the key, bit1 the map (on a clean answer only — a code-5 return redoes
/// the whole call on the VM, discarding the ctx).
pub(super) extern "C" fn rt_u_map_has(
    ctx: *mut UbCtx,
    map: i64,
    key: i64,
    free_mask: i64,
) -> UbMapHasRet {
    let ctx = unsafe { &mut *ctx };
    let redo = UbMapHasRet {
        present: 0,
        code: 5,
    };
    let Some(kb) = ctx.str_bytes(key) else {
        return redo;
    };
    // A MUTABLE map builder (AMB) is perf-DEFERRED for `has` — punt to the VM (byte-identical). The
    // inline AMB packed probe (see `rt_u_map_get`) earns nothing on the `has` bench, so it is not
    // hand-rolled in the audited island (FORK-C, notes.md).
    if map & UB_TAG_AMB != 0 {
        return redo;
    }
    let present = if map & UB_TAG_FLAT != 0 {
        let kb = kb.to_vec(); // end the str_bytes borrow before re-borrowing the arena
        let n = (map >> UB_MAP_CNT_SHIFT) & 0xFFF;
        let base = (map & UB_IDX_MASK) as usize;
        let mut found = false;
        for i in 0..n as usize {
            let koff = (base + 2 * i) * UB_SLOT_SIZE;
            let len = ctx.buf_storage[koff] as usize;
            if ctx.buf_storage[koff + 1..koff + 1 + len] == kb[..] {
                found = true; // present — CLEAN answer (code 0), never a redo
                break;
            }
        }
        found // exhausted scan = genuine absent = CLEAN false (code 0), NOT a redo
    } else {
        let Ok(ks) = std::str::from_utf8(kb) else {
            return redo;
        };
        let kv = Value::Str(crate::phstr::PhStr::new(ks));
        let Some(hk) = crate::value::HKey::from_value(&kv) else {
            return redo;
        };
        match ctx.handles.get(map as usize) {
            // The native's exact key-presence kernel — cannot drift from `Core.Map.has`.
            Some(Value::Map(m)) => m.iter().any(|(k, _)| *k == hk),
            _ => return redo,
        }
    };
    if free_mask & 1 != 0 {
        ctx.release(key);
    }
    if free_mask & 2 != 0 {
        ctx.release(map);
    }
    UbMapHasRet {
        present: present as i64,
        code: 0,
    }
}

/// FUSED map-builder set (`m[k] = v` on a uniquely-owned map local, the mapinsert vertical) —
/// the SLOW leg of the inline AMB overwrite (see [`UB_TAG_AMB`]). (1) map already an AMB
/// record → probe by canon: hit = overwrite (rank unchanged — PHP keeps the original
/// insertion position), hole = INSERT (append rank canon + table entry, count++, doubling
/// rebuild at load > 1/2); (2) map a SEALED flat map → CONVERT: take a record (recycled ones
/// reuse their grown buffer), seed table + ranks from the pair slots (canon/hash were written
/// at seal, order = the seal's insertion order), then apply the set; (3) boxed map / long key
/// / record-table exhaustion → `-1` (code 5, the call redoes on the VM — correct,
/// unspecialized). Canon resolution is registry adopt-or-register: a never-seen key registers
/// a FRESH bump-pinned slot (never a recyclable one), preserving canon ⇔ byte equality.
pub(super) extern "C" fn rt_u_map_builder_set(
    ctx: *mut UbCtx,
    map: i64,
    key: i64,
    val: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let kb: Vec<u8> = match ctx.str_bytes(key) {
        Some(b) => b.to_vec(),
        None => return -1,
    };
    if kb.len() > crate::phstr::INLINE_CAP {
        return -1; // AMB keys are slot-interned (≤ 22 bytes); long keys stay on the VM
    }
    // Canon: adopt the registry entry, else register a fresh bump-pinned key slot.
    let kslot = match ctx.interned.get(&kb) {
        Some(&s) => s as usize,
        None => {
            let Ok(ks) = std::str::from_utf8(&kb) else {
                return -1;
            };
            if ctx.bump + 1 > ctx.cap {
                return -1;
            }
            let kslot = ctx.bump as usize;
            let koff = kslot * UB_SLOT_SIZE;
            let hash = crate::phstr::PhStr::new(ks).cached_hash();
            ctx.buf_storage[koff] = kb.len() as u8;
            ctx.buf_storage[koff + 1..koff + 1 + kb.len()].copy_from_slice(&kb);
            ctx.buf_storage[koff + 1 + kb.len()..koff + UB_SLOT_HASH_OFF].fill(0);
            ctx.buf_storage[koff + UB_SLOT_HASH_OFF..koff + UB_SLOT_HASH_OFF + 8]
                .copy_from_slice(&hash.to_le_bytes());
            let canon1 = (kslot as u64) + 1;
            ctx.buf_storage[koff + UB_SLOT_CANON_OFF..koff + UB_SLOT_CANON_OFF + 8]
                .copy_from_slice(&canon1.to_le_bytes());
            ctx.bump += 1;
            ctx.interned.insert(kb, kslot as u32);
            kslot
        }
    };
    let canon1 = kslot as u64 + 1;
    let hoff = kslot * UB_SLOT_SIZE + UB_SLOT_HASH_OFF;
    let mut hb = [0u8; 8];
    hb.copy_from_slice(&ctx.buf_storage[hoff..hoff + 8]);
    let hash = u64::from_le_bytes(hb);
    // Get-or-convert the builder record.
    let idx = if map & UB_TAG_AMB != 0 {
        let idx = (map & UB_IDX_MASK) as usize;
        if idx >= UB_ACC_CAP {
            return -1;
        }
        idx
    } else if map & UB_TAG_FLAT_MAP == UB_TAG_FLAT_MAP {
        let n = ((map >> UB_MAP_CNT_SHIFT) & 0xFFF) as usize;
        let base = (map & UB_IDX_MASK) as usize;
        let Some(idx) = ctx.acc_take_record() else {
            return -1;
        };
        // Seed sizing keeps load ≤ 1/2 with headroom for the set about to happen.
        let tsize = usize::max(16, (2 * (n + 1)).next_power_of_two());
        ctx.acc_grow_to(idx, 16 + tsize * 16 + (n + 1) * 8);
        let lg = tsize.trailing_zeros() as u64;
        ctx.acc_bufs[idx][0..8].copy_from_slice(&lg.to_le_bytes());
        ctx.acc_bufs[idx][8..16].copy_from_slice(&(n as u64).to_le_bytes());
        ctx.acc_bufs[idx][16..16 + tsize * 16].fill(0);
        for i in 0..n {
            let koff2 = (base + 2 * i) * UB_SLOT_SIZE;
            let mut cb = [0u8; 8];
            cb.copy_from_slice(
                &ctx.buf_storage[koff2 + UB_SLOT_CANON_OFF..koff2 + UB_SLOT_CANON_OFF + 8],
            );
            let c = u64::from_le_bytes(cb);
            let mut hb2 = [0u8; 8];
            hb2.copy_from_slice(
                &ctx.buf_storage[koff2 + UB_SLOT_HASH_OFF..koff2 + UB_SLOT_HASH_OFF + 8],
            );
            let h2 = u64::from_le_bytes(hb2);
            let voff = (base + 2 * i + 1) * UB_SLOT_SIZE;
            let mut vb = [0u8; 8];
            vb.copy_from_slice(&ctx.buf_storage[voff..voff + 8]);
            let v2 = i64::from_le_bytes(vb);
            if c == 0 {
                // Seal always canonizes flat keys — defensive; recycle the record and punt.
                ctx.acc_free.push(idx as u32);
                return -1;
            }
            // Table insert — keys are deduped at seal, every probe finds a hole.
            let mut t = (h2 as usize) & (tsize - 1);
            loop {
                let eoff = 16 + t * 16;
                let mut eb = [0u8; 8];
                eb.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
                if u64::from_le_bytes(eb) == 0 {
                    ctx.acc_bufs[idx][eoff..eoff + 8].copy_from_slice(&c.to_le_bytes());
                    ctx.acc_bufs[idx][eoff + 8..eoff + 16].copy_from_slice(&v2.to_le_bytes());
                    break;
                }
                t = (t + 1) & (tsize - 1);
            }
            let roff = 16 + tsize * 16 + i * 8;
            ctx.acc_bufs[idx][roff..roff + 8].copy_from_slice(&c.to_le_bytes());
        }
        ctx.acc_recs[idx].len = (16 + tsize * 16 + n * 8) as u64;
        // The sealed flat map is bump-pinned — nothing to release.
        idx
    } else {
        return -1; // boxed map — stays on the VM
    };
    let mut w8 = [0u8; 8];
    w8.copy_from_slice(&ctx.acc_bufs[idx][0..8]);
    let mut lg = u64::from_le_bytes(w8);
    let mut tsize = 1usize << lg;
    w8.copy_from_slice(&ctx.acc_bufs[idx][8..16]);
    let count = u64::from_le_bytes(w8) as usize;
    // Probe: overwrite on canon hit (rank unchanged), first hole = insert position.
    let mut t = (hash as usize) & (tsize - 1);
    loop {
        let eoff = 16 + t * 16;
        let mut eb = [0u8; 8];
        eb.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
        let ec = u64::from_le_bytes(eb);
        if ec == canon1 {
            ctx.acc_bufs[idx][eoff + 8..eoff + 16].copy_from_slice(&val.to_le_bytes());
            return UB_TAG_AMB | idx as i64;
        }
        if ec == 0 {
            break;
        }
        t = (t + 1) & (tsize - 1);
    }
    // INSERT — doubling rebuild first if the new count would break load ≤ 1/2.
    if 2 * (count + 1) > tsize {
        let old_tsize = tsize;
        let mut vals: std::collections::HashMap<u64, [u8; 8]> =
            std::collections::HashMap::with_capacity(count);
        for i in 0..old_tsize {
            let eoff = 16 + i * 16;
            let mut eb = [0u8; 8];
            eb.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
            let ec = u64::from_le_bytes(eb);
            if ec != 0 {
                let mut vb = [0u8; 8];
                vb.copy_from_slice(&ctx.acc_bufs[idx][eoff + 8..eoff + 16]);
                vals.insert(ec, vb);
            }
        }
        let ranks: Vec<u64> = (0..count)
            .map(|i| {
                let roff = 16 + old_tsize * 16 + i * 8;
                let mut rb = [0u8; 8];
                rb.copy_from_slice(&ctx.acc_bufs[idx][roff..roff + 8]);
                u64::from_le_bytes(rb)
            })
            .collect();
        tsize *= 2;
        lg += 1;
        ctx.acc_grow_to(idx, 16 + tsize * 16 + (count + 1) * 8);
        ctx.acc_bufs[idx][0..8].copy_from_slice(&lg.to_le_bytes());
        ctx.acc_bufs[idx][16..16 + tsize * 16].fill(0);
        for (i, &rc) in ranks.iter().enumerate() {
            let rslot = (rc - 1) as usize;
            let rhoff = rslot * UB_SLOT_SIZE + UB_SLOT_HASH_OFF;
            let mut rhb = [0u8; 8];
            rhb.copy_from_slice(&ctx.buf_storage[rhoff..rhoff + 8]);
            let rhash = u64::from_le_bytes(rhb);
            let Some(rv) = vals.get(&rc) else {
                ctx.acc_free.push(idx as u32);
                return -1; // defensive — a rank without a table entry
            };
            let mut rt = (rhash as usize) & (tsize - 1);
            loop {
                let eoff = 16 + rt * 16;
                let mut eb = [0u8; 8];
                eb.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
                if u64::from_le_bytes(eb) == 0 {
                    ctx.acc_bufs[idx][eoff..eoff + 8].copy_from_slice(&rc.to_le_bytes());
                    ctx.acc_bufs[idx][eoff + 8..eoff + 16].copy_from_slice(rv);
                    break;
                }
                rt = (rt + 1) & (tsize - 1);
            }
            let roff = 16 + tsize * 16 + i * 8;
            ctx.acc_bufs[idx][roff..roff + 8].copy_from_slice(&rc.to_le_bytes());
        }
        // Re-probe for OUR hole (the key is known-absent — we got here on a miss).
        t = (hash as usize) & (tsize - 1);
        loop {
            let eoff = 16 + t * 16;
            let mut eb = [0u8; 8];
            eb.copy_from_slice(&ctx.acc_bufs[idx][eoff..eoff + 8]);
            if u64::from_le_bytes(eb) == 0 {
                break;
            }
            t = (t + 1) & (tsize - 1);
        }
    } else {
        // Ranks may need one more word (Vec::resize preserves content; the probe position
        // stays valid — the table itself is untouched by the grow).
        ctx.acc_grow_to(idx, 16 + tsize * 16 + (count + 1) * 8);
    }
    let eoff = 16 + t * 16;
    ctx.acc_bufs[idx][eoff..eoff + 8].copy_from_slice(&canon1.to_le_bytes());
    ctx.acc_bufs[idx][eoff + 8..eoff + 16].copy_from_slice(&val.to_le_bytes());
    let roff = 16 + tsize * 16 + count * 8;
    ctx.acc_bufs[idx][roff..roff + 8].copy_from_slice(&canon1.to_le_bytes());
    ctx.acc_bufs[idx][8..16].copy_from_slice(&((count as u64) + 1).to_le_bytes());
    ctx.acc_recs[idx].len = (16 + tsize * 16 + (count + 1) * 8) as u64;
    UB_TAG_AMB | idx as i64
}

/// BUILDER RESEED (map) — the `m = [k => v]` RESET in a builder loop (the mapinsert micro's
/// bounded-memory pattern). Without this, every reset bump-seals 2n pair slots + a table
/// (the arena NEVER recycles bump slots) and a long run exhausts the arena → boxed map →
/// permanent code-5 VM redo — the 1M-iteration cliff. Instead: release the old handle (an
/// AMB record recycles, keeping its grown buffer; a sealed flat map is bump-pinned — no-op),
/// take a record, seed the single pair — ZERO arena growth per reset. Emitted by the
/// `MakeMap(1)` + `SetLocal` peephole (kind-gated to an already-`StrIntMap(Owned)` slot, so
/// the INITIAL binding still seals normally). `-1` = exhaustion/bad key (code 5, VM redo).
pub(super) extern "C" fn rt_u_map_builder_seed(
    ctx: *mut UbCtx,
    old: i64,
    key: i64,
    val: i64,
) -> i64 {
    let cx = unsafe { &mut *ctx };
    cx.release(old);
    let Some(idx) = cx.acc_take_record() else {
        return -1;
    };
    let tsize = 16usize;
    cx.acc_grow_to(idx, 16 + tsize * 16 + 8);
    let lg = tsize.trailing_zeros() as u64;
    cx.acc_bufs[idx][0..8].copy_from_slice(&lg.to_le_bytes());
    cx.acc_bufs[idx][8..16].copy_from_slice(&0u64.to_le_bytes());
    cx.acc_bufs[idx][16..16 + tsize * 16].fill(0);
    cx.acc_recs[idx].len = (16 + tsize * 16) as u64;
    // Delegate the single insert to the canonical set path (canon adopt-or-register).
    rt_u_map_builder_set(ctx, UB_TAG_AMB | idx as i64, key, val)
}

/// The GENERIC pure 2-arg native BRIDGE — converts handle/raw args to `Value`s, calls the
/// REGISTERED native itself (`NativeEval::Pure` — the exact single-sourced VM kernel, so join /
/// contains / splitOnce / drop semantics can never drift), and converts the result back.
/// `meta` bit layout: [0..3) arg-a repr, [3..6) arg-b repr, [6..9) result repr
/// (0 = raw int/bool word, 2 = str handle, 3 = STR-list handle, 4 = INT-list handle),
/// [9] free a if owned, [10] free b if owned. Two-i64 return: `code` 0 = ok, 5 = redo on VM
/// (a native Err — the VM rerun renders the canonical fault — or a defensive mismatch).
pub(super) extern "C" fn rt_u_native2(
    ctx: *mut UbCtx,
    id: i64,
    a: i64,
    b: i64,
    meta: i64,
) -> UbMapGetRet {
    let ctx = unsafe { &mut *ctx };
    let miss = UbMapGetRet { value: 0, code: 5 };
    let to_value = |ctx: &UbCtx, raw: i64, repr: i64| -> Option<Value> {
        match repr {
            0 => Some(Value::Int(raw)),
            2 => match ctx.str_bytes(raw) {
                Some(bytes) => match std::str::from_utf8(bytes) {
                    Ok(s) => Some(Value::Str(crate::phstr::PhStr::new(s))),
                    Err(_) => None,
                },
                None => match ctx.handles.get(raw as usize) {
                    Some(v @ Value::Str(_)) => Some(v.clone()),
                    _ => None,
                },
            },
            3 | 4 if ub_is_untagged(raw) => match ctx.handles.get(raw as usize) {
                // Boxed arg: O(1) Rc bump — the pure native reads the SAME Value the VM
                // would pass (byte-identical by construction, cheaper than materializing).
                Some(v @ Value::List(_)) => Some(v.clone()),
                _ => None,
            },
            3 => Some(Value::List(std::rc::Rc::new(ctx.list_values(raw, true)?))),
            4 => Some(Value::List(std::rc::Rc::new(ctx.list_values(raw, false)?))),
            _ => None,
        }
    };
    let Some(va) = to_value(ctx, a, meta & 7) else {
        return miss;
    };
    let Some(vb) = to_value(ctx, b, (meta >> 3) & 7) else {
        return miss;
    };
    let Some(nf) = crate::native::registry().get(id as usize) else {
        return miss;
    };
    let crate::native::NativeEval::Pure(f) = nf.eval else {
        return miss;
    };
    let mut sink = String::new(); // pure natives never write output (analyze gates on nf.pure)
    let Ok(res) = f(&[va, vb], &mut sink) else {
        return miss; // native fault → the VM redo renders the canonical message
    };
    if meta & (1 << 9) != 0 {
        ctx.release(a);
    }
    if meta & (1 << 10) != 0 {
        ctx.release(b);
    }
    let value = match ((meta >> 6) & 7, res) {
        (0, Value::Int(n)) => n,
        (0, Value::Bool(bv)) => bv as i64,
        (2, v @ Value::Str(_)) => ctx.alloc(v),
        (3 | 4, v @ Value::List(_)) => ctx.alloc(v),
        _ => return miss, // repr mismatch: defensive, VM redo
    };
    UbMapGetRet { value, code: 0 }
}

/// Tag-dispatched `List.append` for DYN elements / DynList receivers (W7): materialize the
/// lhs (`lhs_repr`: 3 = str-list, 4 = int-list, 5 = dyn/boxed-only — a flat-empty word is
/// fine under any repr), convert the (payload, tag) element to a `Value`
/// (tag 0 = int, 1 = float-bits, 2 = bool, 3 = str-handle), push, release the str payload
/// (runtime-bit-gated — a Dyn only ever holds Owned/const words) and the lhs if
/// compile-time-OWNED (`free_list` != 0). Returns a fresh BOXED handle; `-1` = defensive
/// mismatch (code 5, redo on VM).
pub(super) extern "C" fn rt_u_list_append_dyn(
    ctx: *mut UbCtx,
    list: i64,
    payload: i64,
    tag: i64,
    meta: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let lhs_repr = meta & 7;
    let free_list = meta & 8 != 0;
    let mut vals = match lhs_repr {
        3 => match ctx.list_values(list, true) {
            Some(v) => v,
            None => return -1,
        },
        4 => match ctx.list_values(list, false) {
            Some(v) => v,
            None => return -1,
        },
        5 => {
            // Dyn lists are boxed by construction; the one flat form is the EMPTY literal.
            if list & UB_TAG_FLAT != 0 && list & UB_TAG_SLOT == 0 {
                if (list >> 40) & 0xFFFFF != 0 {
                    return -1;
                }
                Vec::new()
            } else {
                match ctx.handles.get(list as usize) {
                    Some(Value::List(xs)) => xs.as_ref().clone(),
                    _ => return -1,
                }
            }
        }
        _ => return -1,
    };
    let ev = match tag {
        0 => Value::Int(payload),
        1 => Value::Float(f64::from_bits(payload as u64)),
        2 => Value::Bool(payload != 0),
        3 => match ctx.str_bytes(payload) {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => Value::Str(crate::phstr::PhStr::new(s)),
                Err(_) => return -1,
            },
            None => match ctx.handles.get(payload as usize) {
                Some(v @ Value::Str(_)) => v.clone(),
                _ => return -1,
            },
        },
        _ => return -1,
    };
    vals.push(ev);
    if tag == 3 {
        ctx.release(payload); // runtime-bit-gated: const/borrowed words no-op
    }
    if free_list {
        ctx.release(list);
    }
    ctx.alloc(Value::List(std::rc::Rc::new(vals)))
}

/// Clone a handle VALUE into a fresh untagged Owned handle (`repr`: 2 = str, 3 = str-list,
/// 4 = int-list) — the Return-of-BORROWED-handle path: a borrowed field/local word returned
/// as-is would leave the caller and the owner both freeing it (double-free); PHP value
/// semantics say the caller gets its own copy. `-1` = defensive mismatch (code 5).
pub(super) extern "C" fn rt_u_clone_value(ctx: *mut UbCtx, h: i64, repr: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    // BOXED fast path (any list repr): an untagged handle holds a `Value::List(Rc<..>)` —
    // phorj value semantics share via Rc with COW at mutation (the VM's own discipline;
    // every JIT in-place mutator is `Rc::get_mut`-guarded fail-closed), so the clone is an
    // O(1) Rc bump instead of a deep materialization. This is what makes the W9a
    // clone-at-boundary cheap on builder-chain shapes (php's refcount equivalence).
    if matches!(repr, 3..=5) && ub_is_untagged(h) {
        if let Some(v @ Value::List(_)) = ctx.handles.get(h as usize) {
            let v = v.clone();
            return ctx.alloc(v);
        }
    }
    let v = match repr {
        2 => match ctx.str_bytes(h) {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => Value::Str(crate::phstr::PhStr::new(s)),
                Err(_) => return -1,
            },
            None => match ctx.handles.get(h as usize) {
                Some(v @ Value::Str(_)) => v.clone(),
                _ => return -1,
            },
        },
        3 => match ctx.list_values(h, true) {
            Some(vals) => Value::List(std::rc::Rc::new(vals)),
            None => return -1,
        },
        4 => match ctx.list_values(h, false) {
            Some(vals) => Value::List(std::rc::Rc::new(vals)),
            None => return -1,
        },
        5 => match ctx.dyn_list_values(h) {
            Some(vals) => Value::List(std::rc::Rc::new(vals)),
            None => return -1,
        },
        _ => return -1,
    };
    ctx.alloc(v)
}

/// The GENERAL (non-accumulator) `List.append` — full PHP value semantics via a clone: any
/// input list form (flat / ACL / boxed) materializes to a fresh Vec, the element is pushed,
/// and a fresh BOXED handle returns; the inputs are untouched unless compile-time-OWNED
/// (`meta` bits: 0 = str element, 1 = free the element, 2 = free the list). This is the
/// widening that lets `ys = List.append(xs, v)` (target ≠ source — NOT an accumulator site)
/// compile instead of declining the whole graph. `-1` = defensive mismatch (code 5).
pub(super) extern "C" fn rt_u_list_append_clone(
    ctx: *mut UbCtx,
    list: i64,
    elem: i64,
    meta: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let str_elem = meta & 1 != 0;
    // Boxed lhs: take an Rc handle up front — after the (optional) lhs release drops the
    // original strong count, `Rc::make_mut` pushes IN PLACE when unique and copies once
    // when genuinely shared (the COW discipline — identical values, fewer copies).
    let boxed_lhs: Option<std::rc::Rc<Vec<Value>>> = if ub_is_untagged(list) {
        match ctx.handles.get(list as usize) {
            Some(Value::List(xs)) => Some(xs.clone()),
            _ => None,
        }
    } else {
        None
    };
    let vals_fallback: Option<Vec<Value>> = if boxed_lhs.is_none() {
        match ctx.list_values(list, str_elem) {
            Some(v) => Some(v),
            None => return -1,
        }
    } else {
        None
    };
    let ev = if str_elem {
        match ctx.str_bytes(elem) {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => Value::Str(crate::phstr::PhStr::new(s)),
                Err(_) => return -1,
            },
            None => match ctx.handles.get(elem as usize) {
                Some(v @ Value::Str(_)) => v.clone(),
                _ => return -1,
            },
        }
    } else {
        Value::Int(elem)
    };
    if meta & 2 != 0 {
        ctx.release(elem);
    }
    if meta & 4 != 0 {
        ctx.release(list);
    }
    if let Some(mut rc) = boxed_lhs {
        std::rc::Rc::make_mut(&mut rc).push(ev);
        return ctx.alloc(Value::List(rc));
    }
    let mut vals = vals_fallback.expect("non-boxed lhs materialized above");
    vals.push(ev);
    ctx.alloc(Value::List(std::rc::Rc::new(vals)))
}

/// Fresh EMPTY int-list builder (the hofpipe vertical: `List.map`'s output list) — take a
/// record (recycled ones reuse their grown buffer), len 0. `-1` = pool exhaustion (code 5).
pub(super) extern "C" fn rt_u_list_builder_new(ctx: *mut UbCtx) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let Some(idx) = ctx.acc_take_record() else {
        return -1;
    };
    ctx.acc_grow_to(idx, 64);
    ctx.acc_recs[idx].len = 0;
    UB_TAG_ACL | idx as i64
}

/// BUILDER RESEED (int list) — the `xs = [v]` RESET twin of [`rt_u_map_builder_seed`]: the
/// listappend micro's reset bump-seals one flat slot per cycle (3906 slots at 1M iterations —
/// 95% of the arena; ~4M would fall off the same cliff). Release the old handle (ACL record
/// recycles with its buffer), take a record, seed one element. `-1` = exhaustion (code 5).
pub(super) extern "C" fn rt_u_list_acc_reseed(ctx: *mut UbCtx, old: i64, v: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    ctx.release(old);
    let Some(idx) = ctx.acc_take_record() else {
        return -1;
    };
    ctx.acc_grow_to(idx, 64);
    ctx.acc_recs[idx].len = 0;
    ctx.acc_push(idx, &v.to_le_bytes());
    UB_TAG_ACL | idx as i64
}

/// Append a raw `i64` element to a still-building (untagged, uniquely-owned) list — the
/// `Kind::IntList` twin of [`rt_u_list_push`]. `-1` = defensive bad-handle fault.
pub(super) extern "C" fn rt_u_list_push_int(ctx: *mut UbCtx, list: i64, val: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    match ctx.handles.get_mut(list as usize) {
        Some(Value::List(xs)) => match std::rc::Rc::get_mut(xs) {
            Some(v) => v.push(Value::Int(val)),
            None => return -1,
        },
        _ => return -1,
    }
    0
}

/// `xs[idx]` on an INT list — the helper (slow) path for untagged (boxed) lists; a flat int list
/// indexes inline. Two-`i64` return like [`rt_u_map_get`] (an int result spans the full range, so
/// no in-band fault sentinel): `code` 0 = ok, 5 = redo on VM (out-of-range → the canonical
/// `"list index out of range"`, or a defensive mismatch). `free != 0` consumes the list handle.
pub(super) extern "C" fn rt_u_index_int(
    ctx: *mut UbCtx,
    list: i64,
    idx: i64,
    free: i64,
) -> UbMapGetRet {
    let ctx = unsafe { &mut *ctx };
    let miss = UbMapGetRet { value: 0, code: 5 };
    if list & UB_TAG_ACL != 0 {
        // An int-list BUILDER read: bounds vs the record's element count, raw i64 load.
        let ridx = (list & UB_IDX_MASK) as usize;
        if ridx >= UB_ACC_CAP {
            return miss;
        }
        let n = (ctx.acc_recs[ridx].len / 8) as i64;
        if !(0..n).contains(&idx) {
            return miss; // the VM redo renders the canonical bounds fault
        }
        let off = idx as usize * 8;
        let mut vb = [0u8; 8];
        vb.copy_from_slice(&ctx.acc_bufs[ridx][off..off + 8]);
        return UbMapGetRet {
            value: i64::from_le_bytes(vb),
            code: 0,
        };
    }
    if list & (UB_TAG_SLOT | UB_TAG_FLAT) != 0 {
        // Defensive mirror of the inline path (a flat INT list: value at slot bytes 0..8).
        if list & UB_TAG_FLAT != 0 && list & UB_TAG_SLOT == 0 {
            let n = (list >> 40) & 0xFFFFF;
            let base = (list & UB_IDX_MASK) as usize;
            if (0..n).contains(&idx) {
                let off = (base + idx as usize) * UB_SLOT_SIZE;
                let mut vb = [0u8; 8];
                vb.copy_from_slice(&ctx.buf_storage[off..off + 8]);
                return UbMapGetRet {
                    value: i64::from_le_bytes(vb),
                    code: 0,
                };
            }
        }
        return miss;
    }
    let value = match ctx.handles.get(list as usize) {
        Some(Value::List(xs)) => match usize::try_from(idx).ok().filter(|i| *i < xs.len()) {
            Some(i) => match xs[i] {
                Value::Int(v) => v,
                _ => return miss, // analyzer-unreachable (kind-proven int list)
            },
            None => return miss, // OOB → canonical fault on the VM redo
        },
        _ => return miss,
    };
    if free != 0 {
        ctx.release(list);
    }
    UbMapGetRet { value, code: 0 }
}

/// Release an owned handle (any encoding — see [`UbCtx::release`]).
pub(super) extern "C" fn rt_u_free(ctx: *mut UbCtx, h: i64) {
    let ctx = unsafe { &mut *ctx };
    ctx.release(h);
}
