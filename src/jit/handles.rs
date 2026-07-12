//! P-2a/P-2b handle space: the per-run `UbCtx` (repr(C) arena header + slots + canon
//! registry) and the `rt_u_*` slow-path helpers the inline fast paths fall back to.

use super::*;

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
        UbCtx {
            buf: buf_storage.as_mut_ptr(),
            free_stack: free_storage.as_mut_ptr(),
            free_top: 0,
            bump,
            cap: UB_SLOT_CAP as u64,
            acc_base: acc_recs.as_mut_ptr(),
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
        }
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
        } else if h & UB_TAG_ACC != 0 {
            // Recycle the RECORD, keep its buffer: a reconverted accumulator (the `s = ""`
            // reset pattern) reuses the grown capacity — php's buffer-reuse trick.
            let idx = (h & UB_IDX_MASK) as u32;
            if (idx as usize) < UB_ACC_CAP {
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
pub(super) extern "C" fn rt_u_str_len(ctx: *mut UbCtx, h: i64, free: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let n = match ctx.str_bytes(h) {
        Some(bytes) => bytes.len() as i64,
        None => return -1,
    };
    if free != 0 {
        ctx.release(h);
    }
    n
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
    let n = entries.len() as i64;
    // Bucket-table sizing: load factor ≤ 1/2, minimum 4 buckets (an empty map still terminates
    // its probe on an empty bucket). The table lives in the slots right after the 2n pairs.
    // PACKED entries (the mapget vertical): each bucket is 16 bytes `{canon: u64, value: i64}`
    // (canon 0 = empty — a real canon is never 0), so the inline probe's hit is TWO adjacent
    // loads in one cache line (was a 3-deep dependent chain: bucket u32 → pair canon → value).
    let tsize = usize::max(4, (2 * n as usize).next_power_of_two());
    let tslots = (tsize * 16).div_ceil(UB_SLOT_SIZE) as u64; // 16-byte entries / 64-byte slots
    if n >= 1 << 12 || ctx.bump + 2 * n as u64 + tslots > ctx.cap {
        ctx.release(map);
        return ctx.alloc(Value::Map(std::rc::Rc::new(deduped)));
    }
    let base = ctx.bump as i64;
    let owned: Vec<(Vec<u8>, u64, i64)> = entries
        .iter()
        .map(|(s, v)| (s.as_bytes().to_vec(), s.cached_hash(), *v))
        .collect();
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
    ctx.release(map);
    let log2 = tsize.trailing_zeros() as i64;
    UB_TAG_FLAT_MAP | (n << UB_MAP_CNT_SHIFT) | (log2 << UB_MAP_LOG_SHIFT) | base
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

/// The declared import ids of the handle-op helpers (one per `JITModule`, when the graph uses
/// handles); [`UbHelperRefs`] is the same set declared into one function body.
pub(super) struct UbHelperIds {
    pub(super) list_new: FuncId,
    pub(super) list_push: FuncId,
    pub(super) list_seal: FuncId,
    pub(super) index: FuncId,
    pub(super) concat: FuncId,
    pub(super) str_len: FuncId,
    pub(super) free: FuncId,
    pub(super) map_push_pair: FuncId,
    pub(super) map_seal: FuncId,
    pub(super) map_get: FuncId,
    pub(super) list_push_int: FuncId,
    pub(super) index_int: FuncId,
    pub(super) int_to_str: FuncId,
    pub(super) concat_mix: FuncId,
    pub(super) acc_append: FuncId,
}

pub(super) struct UbHelperRefs {
    pub(super) list_new: FuncRef,
    pub(super) list_push: FuncRef,
    pub(super) list_seal: FuncRef,
    pub(super) index: FuncRef,
    pub(super) concat: FuncRef,
    pub(super) str_len: FuncRef,
    pub(super) free: FuncRef,
    pub(super) map_push_pair: FuncRef,
    pub(super) map_seal: FuncRef,
    pub(super) map_get: FuncRef,
    pub(super) list_push_int: FuncRef,
    pub(super) index_int: FuncRef,
    pub(super) int_to_str: FuncRef,
    pub(super) concat_mix: FuncRef,
    pub(super) acc_append: FuncRef,
}
