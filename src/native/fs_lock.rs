//! `Core.FileSystemModule` advisory file LOCKING — the native half of DEC-348.
//!
//! **The mechanism, verified rather than assumed.** `std::fs::File::{lock, try_lock, unlock}` are
//! stable on the pinned toolchain [Verified 2026-07-31: compiled and ran all three under rustc
//! 1.97.1], and on Linux they take a **`flock()`** lock — the same space PHP's `flock()` uses
//! [Verified: `/proc/locks` shows `FLOCK ADVISORY WRITE` for the holding process, and a Rust holder
//! blocks a PHP `LOCK_EX|LOCK_NB` probe while a PHP holder blocks Rust's `try_lock`, reproducibly in
//! both directions]. That bidirectional interop is the whole point of the feature: a phorj program
//! and the PHP it transpiles to must contend for the same lock.
//!
//! **Why an int handle and not a new `Value` variant.** The OS lock lives on an open file
//! description, so something must keep the `File` alive between acquire and release. `Core.Database`
//! does this with `Value::Db` + `Rc<dyn Any>`; that is a heavier mechanism than this needs. Here the
//! native returns an opaque **`int` ticket** into a thread-local slab, and the prelude's `FileLock`
//! carries it — so DEC-348 adds no `Op` and no `Value`, matching DEC-364's discipline.
//!
//! **The lock is released by `FileLock.close()`, which `using` calls on every exit path.** That is
//! the "release guaranteed by construction — no leak path" the ruling asked for, and it is why
//! DEC-348 was sequenced after DEC-364 rather than hand-rolling a `try`/`finally` per call site.
//!
//! **DISCLOSURE — cross-platform (mandated by the DEC-348 ruling).** Everything verified above was
//! verified on **Linux**. Windows is a shipped target, its file-lock semantics may be **mandatory**
//! rather than advisory, and there is **no Windows CI** — so the cross-platform guarantee is
//! `[Unverified]` and says so in `FEATURES.md`, the prelude, and the example.

use super::fs_bodies::{classify, one_path};
use crate::value::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::File;

thread_local! {
    /// Live lock tickets → the open `File` whose file description holds the lock. Dropping the
    /// `File` would release the lock, so the slab existing IS the lock being held.
    static LOCKS: RefCell<HashMap<i64, File>> = RefCell::new(HashMap::new());
    /// Ticket counter. Starts at 1 so **0 can mean "not acquired"** in `lockTryAcquire`'s result
    /// without needing an optional payload across the native boundary.
    static NEXT_TICKET: Cell<i64> = const { Cell::new(1) };
}

/// `0` — the `lockTryAcquire` result meaning *the lock is held by someone else*. Not an error: a
/// failed try is the expected outcome the caller asked about.
pub(super) const NOT_ACQUIRED: i64 = 0;

/// Open (creating if absent) the lock file. Never truncates: taking a lock must not destroy the
/// content the lock exists to protect.
fn open_for_lock(path: &str) -> std::io::Result<File> {
    File::options()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
}

fn store(f: File) -> i64 {
    let ticket = NEXT_TICKET.with(|n| {
        let t = n.get();
        n.set(t + 1);
        t
    });
    LOCKS.with(|m| m.borrow_mut().insert(ticket, f));
    ticket
}

/// `lockAcquire(path)` — block until the exclusive lock is held, then return its ticket. An `Err`
/// here becomes `FileSystemResult.Err`, which the prelude turns into a typed `FileSystemError`
/// (the `wrap` convention shared with every other `Core.Native.FileSystem` body).
pub(super) fn lock_acquire_inner(args: &[Value]) -> Result<Value, String> {
    let path = one_path(args, "withLock")?;
    let f = open_for_lock(path).map_err(|e| classify("withLock", path, &e))?;
    f.lock().map_err(|e| classify("withLock", path, &e))?;
    Ok(Value::Int(store(f)))
}

/// `lockTryAcquire(path)` — take the lock if free, else return [`NOT_ACQUIRED`] (`0`).
///
/// `try_lock` returns `Result<(), TryLockError>` on this toolchain, and the two error cases mean
/// different things: `WouldBlock` is "someone else holds it" (a normal answer, `0`), while `Error(e)`
/// is a real I/O failure that must surface as a typed `FileSystemError`. Collapsing them would report
/// a permissions problem as ordinary contention.
pub(super) fn lock_try_acquire_inner(args: &[Value]) -> Result<Value, String> {
    let path = one_path(args, "tryWithLock")?;
    let f = open_for_lock(path).map_err(|e| classify("tryWithLock", path, &e))?;
    match f.try_lock() {
        Ok(()) => Ok(Value::Int(store(f))),
        Err(std::fs::TryLockError::WouldBlock) => Ok(Value::Int(NOT_ACQUIRED)),
        Err(std::fs::TryLockError::Error(e)) => Err(classify("tryWithLock", path, &e)),
    }
}

/// `lockRelease(ticket)` — unlock and drop the file. **Idempotent**: releasing an unknown or
/// already-released ticket succeeds silently, because `FileLock.close()` must never throw (a
/// `Closable` whose `close` throws would have to be discharged at every `using` site —
/// `E-USING-CLOSE-THROWS`) and `using` may call it after an explicit release.
pub(super) fn lock_release_inner(args: &[Value]) -> Result<Value, String> {
    let ticket = match args {
        [Value::Int(i)] => *i,
        _ => return Err("Core.FileSystemModule.__lockRelease expects (int ticket)".to_string()),
    };
    if let Some(f) = LOCKS.with(|m| m.borrow_mut().remove(&ticket)) {
        // An unlock failure is not actionable and must not fault: dropping `f` closes the descriptor,
        // which releases the flock anyway. Ignoring it here is the OS contract, not a swallowed error.
        let _ = f.unlock();
    }
    Ok(Value::Bool(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        let p = std::env::temp_dir().join(format!("phorj-lock-test-{name}"));
        let _ = std::fs::remove_file(&p);
        p.to_string_lossy().into_owned()
    }

    fn ticket_of(v: &Value) -> i64 {
        match v {
            Value::Int(t) => *t,
            other => panic!("expected an int ticket, got {other:?}"),
        }
    }

    #[test]
    fn acquire_then_release_round_trips() {
        let p = tmp("round-trip");
        let t = ticket_of(&lock_acquire_inner(&[Value::Str(p.clone().into())]).unwrap());
        assert!(
            t > 0,
            "a real ticket must be non-zero (0 means not-acquired)"
        );
        assert_eq!(LOCKS.with(|m| m.borrow().len()), 1, "the lock must be held");
        lock_release_inner(&[Value::Int(t)]).unwrap();
        assert_eq!(
            LOCKS.with(|m| m.borrow().len()),
            0,
            "release must drop the File — that IS the unlock"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn release_is_idempotent_and_tolerates_unknown_tickets() {
        // `FileLock.close()` must never throw, so a double release (explicit + the one `using` runs)
        // and a bogus ticket both have to succeed.
        let p = tmp("idempotent");
        let t = ticket_of(&lock_acquire_inner(&[Value::Str(p.clone().into())]).unwrap());
        lock_release_inner(&[Value::Int(t)]).unwrap();
        lock_release_inner(&[Value::Int(t)]).unwrap();
        lock_release_inner(&[Value::Int(999_999)]).unwrap();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn try_acquire_reports_contention_as_zero_not_as_an_error() {
        // Held by ANOTHER process — same-process `flock` re-locks the same file description rather
        // than blocking, so contention can only be exercised across a process boundary.
        let p = tmp("contended");
        std::fs::write(&p, b"x").unwrap();
        let holder = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("exec flock -x {p} sleep 2"))
            .spawn();
        let Ok(mut holder) = holder else {
            eprintln!("SKIP: `flock(1)` unavailable — cannot hold a cross-process lock");
            return;
        };
        std::thread::sleep(std::time::Duration::from_millis(400));
        let got = lock_try_acquire_inner(&[Value::Str(p.clone().into())]).unwrap();
        let t = ticket_of(&got);
        let _ = holder.kill();
        let _ = holder.wait();
        assert_eq!(
            t, NOT_ACQUIRED,
            "a contended try must answer 0 (not acquired), not raise an error"
        );
        let _ = std::fs::remove_file(&p);
    }
}
