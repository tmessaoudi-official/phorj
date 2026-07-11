//! `PhStr` — Phorj's string value representation (perf build P-1a, 2026-07-11).
//!
//! Two variants behind one 24-byte value (so `Value` stays 32 bytes):
//!
//! - [`PhStr::Inline`] — runtime-built strings of ≤ 22 bytes live *inside* the value: zero heap
//!   traffic. Concat of two short strings (the dominant dynamic-string workload) allocates nothing.
//! - [`PhStr::Heap`] — literals and long strings share an `Rc<HeapStr>`: cloning is a refcount
//!   bump, and the FNV-1a hash is cached in the handle (the zend_string trick), so a map lookup by
//!   a literal key never re-hashes.
//!
//! Invariant: every `PhStr` holds **valid UTF-8** — constructors only accept `&str`/`String`, and
//! concatenation of valid UTF-8 is valid UTF-8. `as_str` on the inline variant re-checks via
//! `from_utf8` (this crate forbids `unsafe` outside `src/jit/`); the check is O(len ≤ 22) and only
//! on the `&str` escape hatch — the hot kernels (concat, length, eq, ord, hash) are bytes-first
//! and never validate.
//!
//! Semantics guarantees relied on by the value kernels:
//! - equality / ordering are byte-wise, and for valid UTF-8 byte order ≡ code-point order, so
//!   `compare_ord` semantics are unchanged from the previous `String` representation;
//! - `String.length` remains byte length (documented pre-W4-4 semantics);
//! - `Display`/`Debug` render exactly like the underlying `str`.

use std::borrow::Borrow;
use std::cell::Cell;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

/// Max bytes stored inline. 22 + the 1-byte length + the enum tag = 24 bytes total,
/// which keeps `size_of::<Value>()` at 32 (asserted in the tests below).
pub const INLINE_CAP: usize = 22;

/// Shared heap string with a lazily-cached FNV-1a hash (0 = not yet computed; a computed hash of
/// literal 0 is stored as 1 — hash equality is only ever a pre-check confirmed by byte equality,
/// so the rare remap is harmless).
#[derive(Debug)]
pub struct HeapStr {
    hash: Cell<u64>,
    s: String,
}

#[derive(Clone)]
pub enum PhStr {
    Inline { len: u8, buf: [u8; INLINE_CAP] },
    Heap(Rc<HeapStr>),
}

/// FNV-1a over raw bytes — the one hash function both variants (and the map index) use.
#[inline]
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl PhStr {
    /// Empty string (inline, no allocation).
    pub fn empty() -> PhStr {
        PhStr::Inline {
            len: 0,
            buf: [0; INLINE_CAP],
        }
    }

    /// Construct from borrowed text: short → inline (zero alloc beyond the copy), long → heap.
    /// This is the default runtime constructor (`From<&str>`/`From<String>` route here).
    pub fn new(s: &str) -> PhStr {
        if s.len() <= INLINE_CAP {
            let mut buf = [0u8; INLINE_CAP];
            buf[..s.len()].copy_from_slice(s.as_bytes());
            PhStr::Inline {
                len: s.len() as u8,
                buf,
            }
        } else {
            PhStr::heap(s.to_string())
        }
    }

    /// Construct a **literal / interned** string: always heap-shared with the hash pre-computed,
    /// regardless of length. Used by the compiler's const pool (which dedups by content, so every
    /// occurrence of the same literal shares one `Rc` — cloning a literal is a refcount bump and a
    /// map lookup by it reuses the cached hash).
    pub fn literal(s: &str) -> PhStr {
        let h = match fnv1a(s.as_bytes()) {
            0 => 1,
            h => h,
        };
        PhStr::Heap(Rc::new(HeapStr {
            hash: Cell::new(h),
            s: s.to_string(),
        }))
    }

    /// Wrap an owned `String` on the heap without re-copying (hash computed lazily).
    pub fn heap(s: String) -> PhStr {
        PhStr::Heap(Rc::new(HeapStr {
            hash: Cell::new(0),
            s,
        }))
    }

    /// Take ownership of a `String`, choosing inline for short inputs (drops the heap buffer).
    pub fn from_string(s: String) -> PhStr {
        if s.len() <= INLINE_CAP {
            PhStr::new(&s)
        } else {
            PhStr::heap(s)
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            PhStr::Inline { len, buf } => &buf[..*len as usize],
            PhStr::Heap(h) => h.s.as_bytes(),
        }
    }

    /// Borrow as `&str`. Heap: free. Inline: re-validates ≤ 22 bytes (safe-code escape hatch —
    /// unreachable failure by construction invariant).
    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            PhStr::Inline { len, buf } => std::str::from_utf8(&buf[..*len as usize])
                .expect("PhStr invariant: inline bytes are valid UTF-8"),
            PhStr::Heap(h) => &h.s,
        }
    }

    /// Byte length (Phorj `String.length` semantics, pre-W4-4).
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            PhStr::Inline { len, .. } => *len as usize,
            PhStr::Heap(h) => h.s.len(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The cached-or-computed FNV-1a hash of the bytes. Heap caches; inline computes (≤ 22 bytes).
    #[inline]
    pub fn cached_hash(&self) -> u64 {
        match self {
            PhStr::Inline { len, buf } => match fnv1a(&buf[..*len as usize]) {
                0 => 1,
                h => h,
            },
            PhStr::Heap(h) => {
                let cached = h.hash.get();
                if cached != 0 {
                    return cached;
                }
                let computed = match fnv1a(h.s.as_bytes()) {
                    0 => 1,
                    x => x,
                };
                h.hash.set(computed);
                computed
            }
        }
    }

    /// Take the bytes out (uniquely-owned heap strings move without copying).
    pub fn into_bytes(self) -> Vec<u8> {
        String::from(self).into_bytes()
    }

    /// Concatenate two strings with the representation-optimal path: short results stay inline
    /// (zero alloc), long results build one heap buffer. The single-sourced kernel both backends'
    /// `+`/interpolation route through (via `value::concat_display`).
    pub fn concat(a: &PhStr, b: &PhStr) -> PhStr {
        let ab = a.as_bytes();
        let bb = b.as_bytes();
        let total = ab.len() + bb.len();
        if total <= INLINE_CAP {
            let mut buf = [0u8; INLINE_CAP];
            buf[..ab.len()].copy_from_slice(ab);
            buf[ab.len()..total].copy_from_slice(bb);
            PhStr::Inline {
                len: total as u8,
                buf,
            }
        } else {
            let mut s = String::with_capacity(total);
            s.push_str(a.as_str());
            s.push_str(b.as_str());
            PhStr::heap(s)
        }
    }
}

impl Deref for PhStr {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for PhStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for PhStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<std::path::Path> for PhStr {
    fn as_ref(&self) -> &std::path::Path {
        self.as_str().as_ref()
    }
}

impl AsRef<std::ffi::OsStr> for PhStr {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.as_str().as_ref()
    }
}

impl AsRef<[u8]> for PhStr {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl FromIterator<char> for PhStr {
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> PhStr {
        PhStr::from_string(iter.into_iter().collect::<String>())
    }
}

impl<'a> FromIterator<&'a str> for PhStr {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> PhStr {
        PhStr::from_string(iter.into_iter().collect::<String>())
    }
}

impl PartialEq for PhStr {
    #[inline]
    fn eq(&self, other: &PhStr) -> bool {
        // Shared-literal fast path: same Rc ⇒ equal without touching bytes.
        if let (PhStr::Heap(a), PhStr::Heap(b)) = (self, other) {
            if Rc::ptr_eq(a, b) {
                return true;
            }
        }
        self.as_bytes() == other.as_bytes()
    }
}
impl Eq for PhStr {}

impl PartialEq<str> for PhStr {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<&str> for PhStr {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<String> for PhStr {
    fn eq(&self, other: &String) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<PhStr> for str {
    fn eq(&self, other: &PhStr) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<PhStr> for &str {
    fn eq(&self, other: &PhStr) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<PhStr> for String {
    fn eq(&self, other: &PhStr) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialOrd for PhStr {
    fn partial_cmp(&self, other: &PhStr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PhStr {
    #[inline]
    fn cmp(&self, other: &PhStr) -> Ordering {
        // Byte order ≡ code-point order for UTF-8 — identical to the previous `String` ordering.
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl Hash for PhStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Delegate to `str`'s hash so `Borrow<str>` lookups in std maps stay coherent.
        self.as_str().hash(state);
    }
}

impl fmt::Display for PhStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl fmt::Debug for PhStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render exactly like `String`'s Debug (quoted) so dumps/logs are unchanged.
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl From<&str> for PhStr {
    fn from(s: &str) -> PhStr {
        PhStr::new(s)
    }
}
impl From<String> for PhStr {
    fn from(s: String) -> PhStr {
        PhStr::from_string(s)
    }
}
impl From<&String> for PhStr {
    fn from(s: &String) -> PhStr {
        PhStr::new(s)
    }
}
impl From<PhStr> for String {
    fn from(s: PhStr) -> String {
        match s {
            PhStr::Heap(h) => match Rc::try_unwrap(h) {
                Ok(owned) => owned.s,
                Err(shared) => shared.s.clone(),
            },
            inline => inline.as_str().to_string(),
        }
    }
}

impl Default for PhStr {
    fn default() -> PhStr {
        PhStr::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_hold_the_32_byte_value_budget() {
        assert_eq!(std::mem::size_of::<PhStr>(), 24);
        assert_eq!(std::mem::size_of::<crate::value::Value>(), 32);
    }

    #[test]
    fn inline_vs_heap_split_at_cap() {
        assert!(matches!(PhStr::new("x"), PhStr::Inline { .. }));
        assert!(matches!(
            PhStr::new(&"a".repeat(INLINE_CAP)),
            PhStr::Inline { .. }
        ));
        assert!(matches!(
            PhStr::new(&"a".repeat(INLINE_CAP + 1)),
            PhStr::Heap(_)
        ));
        // Literals are always heap (interned + hash-cached), even short ones.
        assert!(matches!(PhStr::literal("a"), PhStr::Heap(_)));
    }

    #[test]
    fn equality_is_value_based_across_variants() {
        let i = PhStr::new("alpha");
        let h = PhStr::literal("alpha");
        assert_eq!(i, h);
        assert_eq!(h, i);
        assert_eq!(i, "alpha");
        assert_ne!(i, PhStr::new("beta"));
    }

    #[test]
    fn concat_stays_inline_when_short_and_heaps_when_long() {
        let a = PhStr::literal("alpha");
        let b = PhStr::literal("beta");
        let s = PhStr::concat(&a, &b);
        assert!(matches!(s, PhStr::Inline { .. }));
        assert_eq!(s.as_str(), "alphabeta");
        assert_eq!(s.len(), 9);

        let long = PhStr::new(&"x".repeat(20));
        let l = PhStr::concat(&long, &long);
        assert!(matches!(l, PhStr::Heap(_)));
        assert_eq!(l.len(), 40);
    }

    #[test]
    fn concat_multibyte_is_utf8_safe() {
        let a = PhStr::new("héllo");
        let b = PhStr::new("wörld");
        let s = PhStr::concat(&a, &b);
        assert_eq!(s.as_str(), "héllowörld");
    }

    #[test]
    fn hash_is_cached_and_consistent_across_variants() {
        let h = PhStr::literal("gamma");
        let i = PhStr::new("gamma");
        assert_eq!(h.cached_hash(), i.cached_hash());
        assert_ne!(h.cached_hash(), 0);
        // Second call returns the cached value (same result, exercised path).
        assert_eq!(h.cached_hash(), h.cached_hash());
    }

    #[test]
    fn ordering_matches_str_ordering() {
        let mut v = [PhStr::new("b"), PhStr::literal("a"), PhStr::new("ab")];
        v.sort();
        let sorted: Vec<&str> = v.iter().map(|s| s.as_str()).collect();
        assert_eq!(sorted, vec!["a", "ab", "b"]);
    }

    #[test]
    fn display_and_debug_match_string_rendering() {
        let s = PhStr::new("he\"y");
        assert_eq!(format!("{s}"), "he\"y");
        assert_eq!(format!("{s:?}"), format!("{:?}", "he\"y"));
    }

    #[test]
    fn deref_gives_str_apis() {
        let s = PhStr::new("hello world");
        assert!(s.starts_with("hello"));
        assert_eq!(s.split(' ').count(), 2);
    }

    #[test]
    fn into_string_round_trips() {
        let s: String = PhStr::literal("round trip").into();
        assert_eq!(s, "round trip");
        let long: String = PhStr::new(&"y".repeat(50)).into();
        assert_eq!(long.len(), 50);
    }
}
