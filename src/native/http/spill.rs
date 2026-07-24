//! The body spill store (D8c/§7 P1 — bodies above [`super::SPILL_THRESHOLD`] leave the heap).
//! Phorj only ever sees DETERMINISTIC integer handles (0, 1, 2… per execution) — the temp-file
//! path never enters a phorj value (Invariant 10: a generated path in a value would break
//! byte-identity; the PHP twin `__phorj_http_spill` keeps an index-addressed array the same way).
//! Thread-local: one request = one worker thread = one heap (the D8a soundness note), so handles
//! never cross threads. Temp files are cleaned on process exit by the OS tmp reaper — a leaked
//! file per large upload until slice 3's serve loop cleans post-response (KNOWN_ISSUES row).
use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;

thread_local! {
    static SPILLS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// Write `bytes` to a fresh temp file; return its handle. Failures are runtime faults (an
/// unusable temp dir is an ambient-environment error, not a program bug).
pub(super) fn store(bytes: &[u8]) -> Result<i64, String> {
    SPILLS.with(|s| {
        let mut s = s.borrow_mut();
        let idx = s.len();
        let path = std::env::temp_dir().join(format!(
            "phorj-spill-{}-{}-{idx}",
            std::process::id(),
            // Thread id disambiguates concurrent serve workers sharing the pid.
            format!("{:?}", std::thread::current().id()).replace(['(', ')'], "")
        ));
        let mut f =
            std::fs::File::create(&path).map_err(|e| format!("request body spill failed: {e}"))?;
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
