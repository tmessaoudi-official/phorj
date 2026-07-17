//! `Core.SessionModule` (W3 — the SESS parity family, TOP-20 #3): HTTP sessions for `phg serve`.
//!
//! ARCHITECTURE: the session STORE lives native-side in one process-wide `Mutex<HashMap>` (shared
//! across `--workers N` threads — values are plain `String`s, so the store is `Send + Sync` without
//! ever moving an `Rc` value across threads). The prelude `Session` class wraps a session id; the
//! cookie wiring (`Session.start(req)` / `session.apply(resp)`) lives in the prelude on top of the
//! existing `Core.Http` `Request`/`Response` value types, with SECURITY DEFAULTS ON: the cookie is
//! `HttpOnly; SameSite=Lax; Path=/` (PHP requires you to opt in via ini), ids are 128-bit OS-entropy
//! hex, expired ids are silently regenerated (never resurrected), and `regenerate()` (the
//! session-fixation defense) is a first-class method.
//!
//! Sessions expire on IDLE TTL (default 1800 s, touched on every access — PHP's gc_maxlifetime
//! shape). v1 scope (recorded): string values (structured data goes through `Core.Json` — exactly
//! what PHP's serialized `$_SESSION` does under the hood), in-memory store only (a public
//! `SessionStore` contract + file/custom backends is the queued layered-openness v2). All natives
//! are `pure:false` (ambient store + entropy) → spine-quarantined; gated by `tests/session.rs` on
//! both backends. Transpile = `E-TRANSPILE-SESSION` for now (PHP's per-request process model maps
//! differently — a `session_start()` mapping is a recorded future lift).

use crate::native::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Entry {
    data: HashMap<String, String>,
    last_access: Instant,
    ttl: Duration,
}

static STORE: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

/// Run `f` over the live store map (lazily initialized).
fn with_store<T>(f: impl FnOnce(&mut HashMap<String, Entry>) -> T) -> T {
    let mut guard = STORE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(guard.get_or_insert_with(HashMap::new))
}

/// A fresh 128-bit session id as 32 lowercase hex chars, from OS entropy (the CSPRNG used by
/// `Core.Random.secureBytes`). Collisions are cryptographically negligible; the loop is a formality.
fn fresh_sid(store: &HashMap<String, Entry>) -> Result<String, String> {
    for _ in 0..4 {
        let mut buf = [0u8; 16];
        getrandom_fill(&mut buf)?;
        let sid: String = buf.iter().map(|b| format!("{b:02x}")).collect();
        if !store.contains_key(&sid) {
            return Ok(sid);
        }
    }
    Err("Core.SessionModule: could not allocate a fresh session id".into())
}

/// OS entropy without a new dependency: `std::fs::read` from `/dev/urandom` on unix; elsewhere fall
/// back to a hash of time+thread (documented: non-unix hosts should route through `Core.Random`'s
/// entropy source when a port lands — phorj's supported host today is Linux).
fn getrandom_fill(buf: &mut [u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Read as _;
        let mut f = std::fs::File::open("/dev/urandom")
            .map_err(|e| format!("Core.SessionModule: cannot open /dev/urandom: {e}"))?;
        f.read_exact(buf)
            .map_err(|e| format!("Core.SessionModule: cannot read entropy: {e}"))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        for chunk in buf.chunks_mut(8) {
            let mut h = RandomState::new().build_hasher();
            h.write_u128(std::time::UNIX_EPOCH.elapsed().map_or(0, |d| d.as_nanos()));
            let v = h.finish().to_le_bytes();
            for (dst, src) in chunk.iter_mut().zip(v.iter()) {
                *dst = *src;
            }
        }
        Ok(())
    }
}

/// Is `sid` present and not idle-expired? An expired entry is removed en passant (lazy GC — the
/// store also sweeps opportunistically on create).
fn live<'a>(store: &'a mut HashMap<String, Entry>, sid: &str) -> Option<&'a mut Entry> {
    let expired = store
        .get(sid)
        .map(|e| e.last_access.elapsed() > e.ttl)
        .unwrap_or(false);
    if expired {
        store.remove(sid);
        return None;
    }
    store.get_mut(sid).inspect(|_| {})
}

// ── Native bodies ────────────────────────────────────────────────────────────────────────────────────

/// `acquire(candidateSid, ttlSeconds)` → the LIVE sid: reuses `candidateSid` when it names a live
/// session (touching it), else creates a fresh one (sweeping expired entries). `candidateSid == ""`
/// = no cookie arrived.
fn session_open(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (cand, ttl) = match args {
        [Value::Str(c), Value::Int(t)] => (c.as_str(), *t),
        _ => return Err("Core.SessionModule.__acquire expects (string, int)".into()),
    };
    let ttl = Duration::from_secs(u64::try_from(ttl.max(1)).unwrap_or(1800));
    with_store(|store| {
        if !cand.is_empty() {
            if let Some(e) = live(store, cand) {
                e.last_access = Instant::now();
                e.ttl = ttl;
                return Ok(Value::Str(cand.into()));
            }
        }
        // Opportunistic sweep (bounded work: only on create) keeps the store from accumulating
        // dead sessions between requests — the gc_maxlifetime shape without a background thread.
        store.retain(|_, e| e.last_access.elapsed() <= e.ttl);
        let sid = fresh_sid(store)?;
        store.insert(
            sid.clone(),
            Entry {
                data: HashMap::new(),
                last_access: Instant::now(),
                ttl,
            },
        );
        Ok(Value::Str(sid.into()))
    })
}

fn session_get(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (sid, key) = match args {
        [Value::Str(s), Value::Str(k)] => (s.as_str(), k.as_str()),
        _ => return Err("Core.SessionModule.__get expects (string, string)".into()),
    };
    with_store(|store| {
        Ok(match live(store, sid) {
            Some(e) => {
                e.last_access = Instant::now();
                match e.data.get(key) {
                    Some(v) => Value::Str(v.as_str().into()),
                    None => Value::Null,
                }
            }
            None => Value::Null,
        })
    })
}

fn session_set(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (sid, key, val) = match args {
        [Value::Str(s), Value::Str(k), Value::Str(v)] => (s.as_str(), k.as_str(), v.as_str()),
        _ => return Err("Core.SessionModule.__set expects (string, string, string)".into()),
    };
    with_store(|store| {
        if let Some(e) = live(store, sid) {
            e.last_access = Instant::now();
            e.data.insert(key.to_string(), val.to_string());
        }
        Ok(Value::Null)
    })
}

fn session_remove(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let (sid, key) = match args {
        [Value::Str(s), Value::Str(k)] => (s.as_str(), k.as_str()),
        _ => return Err("Core.SessionModule.__remove expects (string, string)".into()),
    };
    with_store(|store| {
        if let Some(e) = live(store, sid) {
            e.last_access = Instant::now();
            e.data.remove(key);
        }
        Ok(Value::Null)
    })
}

/// Sorted key listing (determinism, Invariant 10).
fn session_keys(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let sid = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.SessionModule.__keys expects (string)".into()),
    };
    with_store(|store| {
        let mut keys: Vec<String> = match live(store, sid) {
            Some(e) => e.data.keys().cloned().collect(),
            None => Vec::new(),
        };
        keys.sort();
        Ok(Value::List(std::rc::Rc::new(
            keys.into_iter().map(|k| Value::Str(k.into())).collect(),
        )))
    })
}

fn session_destroy(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let sid = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.SessionModule.__destroy expects (string)".into()),
    };
    with_store(|store| {
        store.remove(sid);
        Ok(Value::Null)
    })
}

/// `regenerate(sid)` → a FRESH id carrying the same data (the session-fixation defense): the old id
/// is dead immediately, so a pre-login id an attacker planted can never become authenticated.
fn session_regenerate(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let sid = match args {
        [Value::Str(s)] => s.as_str(),
        _ => return Err("Core.SessionModule.__regenerate expects (string)".into()),
    };
    with_store(|store| {
        let entry = match store.remove(sid) {
            Some(e) if e.last_access.elapsed() <= e.ttl => e,
            _ => Entry {
                data: HashMap::new(),
                last_access: Instant::now(),
                ttl: Duration::from_secs(1800),
            },
        };
        let fresh = fresh_sid(store)?;
        store.insert(
            fresh.clone(),
            Entry {
                data: entry.data,
                last_access: Instant::now(),
                ttl: entry.ttl,
            },
        );
        Ok(Value::Str(fresh.into()))
    })
}

pub fn session_natives() -> Vec<NativeFn> {
    let entry =
        |name: &'static str,
         params: Vec<Ty>,
         ret: Ty,
         eval: fn(&[Value], &mut String) -> Result<Value, String>| NativeFn {
            module: "Core.Native.Session",
            name,
            params,
            ret,
            pure: false,
            eval: NativeEval::Pure(eval),
            php: |a| a.first().cloned().unwrap_or_else(|| "''".to_string()),
        };
    vec![
        entry(
            "acquire",
            vec![Ty::String, Ty::Int],
            Ty::String,
            session_open,
        ),
        entry(
            "get",
            vec![Ty::String, Ty::String],
            Ty::Optional(Box::new(Ty::String)),
            session_get,
        ),
        entry(
            "set",
            vec![Ty::String, Ty::String, Ty::String],
            Ty::Optional(Box::new(Ty::String)),
            session_set,
        ),
        entry(
            "remove",
            vec![Ty::String, Ty::String],
            Ty::Optional(Box::new(Ty::String)),
            session_remove,
        ),
        entry(
            "keys",
            vec![Ty::String],
            Ty::List(Box::new(Ty::String)),
            session_keys,
        ),
        entry(
            "destroy",
            vec![Ty::String],
            Ty::Optional(Box::new(Ty::String)),
            session_destroy,
        ),
        entry(
            "regenerate",
            vec![Ty::String],
            Ty::String,
            session_regenerate,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value {
        Value::Str(v.into())
    }

    #[test]
    fn open_set_get_regenerate_destroy_round_trip() {
        let mut o = String::new();
        let sid = match session_open(&[s(""), Value::Int(1800)], &mut o).unwrap() {
            Value::Str(x) => x.to_string(),
            other => panic!("{other:?}"),
        };
        assert_eq!(sid.len(), 32, "128-bit hex sid");
        // Reopening with the same candidate reuses it.
        assert!(matches!(
            session_open(&[s(&sid), Value::Int(1800)], &mut o).unwrap(),
            Value::Str(x) if *x == *sid
        ));
        session_set(&[s(&sid), s("user"), s("ada")], &mut o).unwrap();
        assert!(matches!(
            session_get(&[s(&sid), s("user")], &mut o).unwrap(),
            Value::Str(x) if &*x == "ada"
        ));
        // Regenerate: fresh id, same data, old id dead (fixation defense).
        let fresh = match session_regenerate(&[s(&sid)], &mut o).unwrap() {
            Value::Str(x) => x.to_string(),
            other => panic!("{other:?}"),
        };
        assert_ne!(fresh, sid);
        assert!(matches!(
            session_get(&[s(&fresh), s("user")], &mut o).unwrap(),
            Value::Str(x) if &*x == "ada"
        ));
        assert!(matches!(
            session_get(&[s(&sid), s("user")], &mut o).unwrap(),
            Value::Null
        ));
        session_destroy(&[s(&fresh)], &mut o).unwrap();
        assert!(matches!(
            session_get(&[s(&fresh), s("user")], &mut o).unwrap(),
            Value::Null
        ));
    }

    #[test]
    fn expired_sessions_are_not_resurrected() {
        let mut o = String::new();
        // ttl clamps to >= 1s, so force expiry by back-dating the entry directly.
        let sid = match session_open(&[s(""), Value::Int(1)], &mut o).unwrap() {
            Value::Str(x) => x.to_string(),
            other => panic!("{other:?}"),
        };
        session_set(&[s(&sid), s("k"), s("v")], &mut o).unwrap();
        with_store(|store| {
            store.get_mut(&sid).unwrap().last_access = Instant::now() - Duration::from_secs(10);
        });
        // The stale cookie id names an expired session → a FRESH sid with EMPTY data.
        let re = match session_open(&[s(&sid), Value::Int(1800)], &mut o).unwrap() {
            Value::Str(x) => x.to_string(),
            other => panic!("{other:?}"),
        };
        assert_ne!(re, sid, "expired ids are never reused");
        assert!(matches!(
            session_get(&[s(&re), s("k")], &mut o).unwrap(),
            Value::Null
        ));
    }
}
