//! The body spill store (D8c/§7 P1 — bodies above [`super::SPILL_THRESHOLD`] leave the heap).
//! Phorj only ever sees DETERMINISTIC integer handles (0, 1, 2… per execution) — the temp-file
//! path never enters a phorj value (Invariant 10: a generated path in a value would break
//! byte-identity; the PHP twin `__phorj_http_spill` keeps an index-addressed array the same way).
//! Thread-local: one request = one worker thread = one heap (the D8a soundness note), so handles
//! never cross threads. Nothing deletes a spill mid-run: each large upload leaves a file in this
//! process's spill directory until the OS tmp reaper runs (KNOWN_ISSUES row).
use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;

thread_local! {
    static SPILLS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// This process's spill directory: owner-only and created once (`crate::tempdir`), so a spill path is
/// never a guessable name in the shared temp dir that a planted symlink could redirect (panel
/// 2026-10-08). Bodies inside it are created with `create_new`, which never follows a link. A
/// failure is NOT cached: the next spill retries, as the per-file code this replaced did.
fn spill_dir() -> Result<&'static PathBuf, String> {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    if let Some(dir) = DIR.get() {
        return Ok(dir);
    }
    let fresh = crate::tempdir::private_temp_dir("phorj-spill")
        .map_err(|e| format!("request body spill failed: {e}"))?;
    let dir = DIR.get_or_init(|| fresh.clone());
    if *dir != fresh {
        // Another worker initialised it first; ours is still empty, and a failed removal leaves
        // only an empty owner-only directory behind.
        let _ = std::fs::remove_dir(&fresh);
    }
    Ok(dir)
}

/// Write `bytes` to a fresh temp file; return its handle. Failures are runtime faults (an
/// unusable temp dir is an ambient-environment error, not a program bug).
pub(super) fn store(bytes: &[u8]) -> Result<i64, String> {
    let dir = spill_dir()?;
    SPILLS.with(|s| {
        let mut s = s.borrow_mut();
        let idx = s.len();
        // Thread id disambiguates concurrent serve workers sharing the directory.
        let thread = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
        let path = dir.join(format!("{thread}-{idx}"));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("request body spill failed: {e}"))?;
        f.write_all(bytes)
            .map_err(|e| format!("request body spill failed: {e}"))?;
        s.push(path);
        Ok(i64::try_from(idx).expect("spill count fits i64"))
    })
}

/// Read a spilled body back by handle.
pub(super) fn read(handle: i64) -> Result<Vec<u8>, String> {
    SPILLS.with(|s| {
        let s = s.borrow();
        let idx = usize::try_from(handle).map_err(|_| "invalid spill handle".to_string())?;
        let path = s
            .get(idx)
            .ok_or_else(|| "invalid spill handle".to_string())?;
        std::fs::read(path).map_err(|e| format!("request body spill read failed: {e}"))
    })
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn spills_live_in_one_owner_only_directory() {
        use std::os::unix::fs::PermissionsExt;
        let a = super::store(b"first").unwrap();
        let b = super::store(b"second").unwrap();
        assert_eq!(super::read(a).unwrap(), b"first");
        assert_eq!(super::read(b).unwrap(), b"second");
        let dir = super::spill_dir().unwrap();
        assert_ne!(dir, &std::env::temp_dir(), "not the shared temp dir itself");
        let mode = std::fs::metadata(dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "owner-only, got {mode:o}");
        super::SPILLS
            .with(|s| assert!(s.borrow().iter().all(|p| p.parent() == Some(dir.as_path()))));
    }
}
