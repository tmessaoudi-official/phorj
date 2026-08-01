//! The CANON registry — content-addressed interning of short string keys to their canonical slot.
//!
//! Two operations, both extracted here because each existed twice and each had the SAME defect: they
//! allocated a copy of the key just to PROBE a table that `Vec<u8>: Borrow<[u8]>` probes by slice for
//! free (DEC-433). On `mapinsert` that was one heap allocation per `m[k] = v`, on a key already
//! interned after its first touch — malloc/free was ~3% of the profile, and removing it measured
//! -3.6% instructions and -5..7% wall clock on the phorj leg.
//!
//! The rule both share: **probe borrowed, allocate only to insert.**

use super::*;

impl UbCtx {
    /// The CANON word for `bytes`: adopt the registry's slot if this content is already interned,
    /// else register `slot` as its canonical home. Returns `interned_slot + 1` (0 = uncanonical).
    pub(super) fn canon_for(&mut self, bytes: &[u8], slot: usize) -> u64 {
        match self.interned.get(bytes) {
            Some(&s) => u64::from(s) + 1,
            None => {
                self.interned.insert(bytes.to_vec(), slot as u32);
                slot as u64 + 1
            }
        }
    }

    /// The interned slot for the string handle `key`, registering a fresh bump-pinned key slot if the
    /// content is new. `None` = not representable here (bad handle, over [`crate::phstr::INLINE_CAP`],
    /// non-UTF-8, or the arena is full) — every one of those is a VM-fallback, never a fault.
    pub(super) fn canon_key_slot(&mut self, key: i64) -> Option<usize> {
        // PROBE by BORROWED slice — no allocation on the hot path, which is every touch after the
        // first. The block scopes the shared borrow so the cold arm below can take `&mut self`.
        let probe = {
            let b = self.str_bytes(key)?;
            if b.len() > crate::phstr::INLINE_CAP {
                return None; // AMB keys are slot-interned (<= 22 bytes); long keys stay on the VM
            }
            self.interned.get(b).map(|&s| s as usize)
        };
        if let Some(s) = probe {
            return Some(s);
        }
        // COLD path only: registering needs an owned key and `&mut self`.
        let kb = self.str_bytes(key).map(<[u8]>::to_vec)?;
        let ks = std::str::from_utf8(&kb).ok()?;
        if self.bump + 1 > self.cap {
            return None;
        }
        let kslot = self.bump as usize;
        let koff = kslot * UB_SLOT_SIZE;
        let hash = crate::phstr::PhStr::new(ks).cached_hash();
        self.buf_storage[koff] = kb.len() as u8;
        self.buf_storage[koff + 1..koff + 1 + kb.len()].copy_from_slice(&kb);
        self.buf_storage[koff + 1 + kb.len()..koff + UB_SLOT_HASH_OFF].fill(0);
        self.buf_storage[koff + UB_SLOT_HASH_OFF..koff + UB_SLOT_HASH_OFF + 8]
            .copy_from_slice(&hash.to_le_bytes());
        let canon1 = (kslot as u64) + 1;
        self.buf_storage[koff + UB_SLOT_CANON_OFF..koff + UB_SLOT_CANON_OFF + 8]
            .copy_from_slice(&canon1.to_le_bytes());
        self.bump += 1;
        self.interned.insert(kb, kslot as u32);
        Some(kslot)
    }
}
