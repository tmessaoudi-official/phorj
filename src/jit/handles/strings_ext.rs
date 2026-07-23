//! String-op runtime helpers (M-Decomp + the DEC-332 string-scan flips): the unboxed `==`/`!=`
//! string kernel and the DEDICATED scan verticals — `String.contains` (was bridge2: two boxed
//! `Value` materializations per call; now a zero-alloc byte read + the SAME `str::contains`
//! kernel) and the `Core.Validation.isEmail`/`isUrl` predicates (the hand-rolled anchored
//! validators called straight over the arena bytes). Byte-identity: every helper calls the
//! native''s exact kernel — nothing is re-implemented here.

use super::*;

/// String equality for the unboxed `Op::Eq`/`Op::Ne` on two string operands — the shared
/// [`Value::eq_val`] kernel (single-sourced, so `==` on strings can never drift from the VM).
/// Returns 0/1; `-1` = defensive non-string operand (code 5).
pub(in crate::jit) extern "C" fn rt_u_str_eq(ctx: *mut UbCtx, a: i64, b: i64, meta: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let get = |ctx: &UbCtx, h: i64| -> Option<Value> {
        match ctx.str_bytes(h) {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(s) => Some(Value::Str(crate::phstr::PhStr::new(s))),
                Err(_) => None,
            },
            None => match ctx.handles.get(h as usize) {
                Some(v @ Value::Str(_)) => Some(v.clone()),
                _ => None,
            },
        }
    };
    let (Some(va), Some(vb)) = (get(ctx, a), get(ctx, b)) else {
        return -1;
    };
    let eq = va.eq_val(&vb) as i64;
    if meta & 1 != 0 {
        ctx.release(a);
    }
    if meta & 2 != 0 {
        ctx.release(b);
    }
    eq
}

impl UbCtx {
    /// Is `h` a PINNED string word — one whose bytes provably cannot change for the rest of
    /// the run, making it a sound memo key? Two producers qualify: an untagged CONST handle
    /// (`h < n_pinned`, the interned prefix — never truncated mid-run) and a BORROWED arena
    /// slot (`SLOT` set, `OWNED` clear — by construction those words come only from
    /// bump-pinned storage: ≤22-byte string consts, sealed flat-list elements, sealed map key
    /// slots; every recyclable scratch slot carries the `OWNED` bit on its word). An
    /// OWNED/accumulator word can be recycled and refilled with different bytes, so it must
    /// NEVER key a memo.
    fn word_is_pinned_str(&self, h: i64) -> bool {
        (h & UB_TAG_SLOT != 0 && h & UB_TAG_OWNED == 0)
            || (ub_is_untagged(h) && (h as usize) < self.n_pinned)
    }

    /// Install `(a, b) -> r` into the string-predicate memo: the FULL map plus the
    /// direct-mapped inline line (entries 16..24 of the memo table — `{a, b, r + 1}`; the emit
    /// arms probe it in ~8 ops). MUST mirror the inline probe's mixing bit-for-bit.
    fn memo_str_install(&mut self, a: i64, b: i64, r: i64) {
        self.memo_str.insert((a, b), r + 1);
        let e = (16 + ((a ^ (b << 1)).wrapping_mul(UB_SET_HASH_MULT) as u64 >> 61) as usize) * 3;
        self.memo_storage[e] = a;
        self.memo_storage[e + 1] = b;
        self.memo_storage[e + 2] = r + 1;
    }
}

/// `String.contains(hay, needle)` — the dedicated scan vertical: read both operands' bytes
/// straight from the arena / handle table (NO boxed `Value`, no `PhStr` clone — the bridge2
/// path this replaces paid both per call) and run the native's exact kernel
/// (`str::contains`, `src/native/text.rs::text_contains`). A PINNED operand pair memoizes (the
/// scan is a pure function of two immutable byte sequences); an inline-cache eviction
/// re-installs from the full memo, never rescans. Returns 0/1; `-1` = defensive non-string
/// operand (code 5, VM redo). `free_mask` bit 0 = release `hay`, bit 1 = `needle`.
pub(in crate::jit) extern "C" fn rt_u_str_contains(
    ctx: *mut UbCtx,
    hay: i64,
    needle: i64,
    free_mask: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    // Pinned-ness is decided from the RUNTIME words alone — the compile-time free mask may
    // say Owned for a borrowed flat-element word (the kinds are uniform across index legs).
    // A memo hit early-returns past the mask releases: exact, because a hit implies both
    // words are pinned and a pinned word's release is a no-op by construction.
    let pinned = ctx.word_is_pinned_str(hay) && ctx.word_is_pinned_str(needle);
    if pinned {
        if let Some(&r1) = ctx.memo_str.get(&(hay, needle)) {
            ctx.memo_str_install(hay, needle, r1 - 1); // inline-line eviction re-install
            return r1 - 1;
        }
    }
    let r = match (ctx.str_bytes(hay), ctx.str_bytes(needle)) {
        (Some(h), Some(n)) => match (std::str::from_utf8(h), std::str::from_utf8(n)) {
            (Ok(h), Ok(n)) => h.contains(n) as i64,
            _ => -1,
        },
        _ => -1,
    };
    if r >= 0 {
        if pinned {
            ctx.memo_str_install(hay, needle, r);
        }
        if free_mask & 1 != 0 {
            ctx.release(hay);
        }
        if free_mask & 2 != 0 {
            ctx.release(needle);
        }
    }
    r
}

/// `Core.Validation.isEmail` / `isUrl` — the predicate vertical: arena bytes → the exact
/// hand-rolled kernel (`src/native/validate.rs::{is_email, is_url}`, the ones the boxed native
/// wraps). The second arg is the memo pair key `key = -(which + 1)` (which = `-key - 1`; a
/// negative word can never be a handle, so validate entries can't collide with
/// `String.contains` pairs in the shared table — and the emit arm's uniform probe/slow-leg
/// passes the same word both places). A PINNED operand memoizes under `(s, key)`. Returns
/// 0/1; `-1` = defensive non-string operand / bad key (code 5). `free != 0` releases.
pub(in crate::jit) extern "C" fn rt_u_validate(
    ctx: *mut UbCtx,
    s: i64,
    key: i64,
    free: i64,
) -> i64 {
    let ctx = unsafe { &mut *ctx };
    let which = -key - 1;
    if !(0..=1).contains(&which) {
        return -1;
    }
    let pinned = ctx.word_is_pinned_str(s);
    if pinned {
        if let Some(&r1) = ctx.memo_str.get(&(s, key)) {
            ctx.memo_str_install(s, key, r1 - 1);
            return r1 - 1;
        }
    }
    let r = match ctx.str_bytes(s).map(std::str::from_utf8) {
        Some(Ok(t)) => {
            if which == 0 {
                crate::native::validate::is_email(t) as i64
            } else {
                crate::native::validate::is_url(t) as i64
            }
        }
        _ => -1,
    };
    if r >= 0 {
        if pinned {
            ctx.memo_str_install(s, key, r);
        }
        if free != 0 {
            ctx.release(s);
        }
    }
    r
}

/// `String.length` slow leg (the emit arm inlines slot-length reads) — byte length of any
/// string handle. `-1` = defensive non-string operand (code 5). `free != 0` releases.
pub(in crate::jit) extern "C" fn rt_u_str_len(ctx: *mut UbCtx, h: i64, free: i64) -> i64 {
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
