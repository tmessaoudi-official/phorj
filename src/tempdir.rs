//! Private scratch directories. Every temp file phorj itself writes outside tests lives in one of
//! these, never at a fixed
//! `<prefix>-<pid>` name in the shared temp dir: on a multi-user box such a name is guessable, and a
//! symlink planted there in advance redirects the write (or the later read) to a path of the
//! planter's choosing. Callers: the package stage and registry index (`pm`), the `phg build` payload,
//! the serve body spill and `phg benchmark --vs-php`'s transpiled script.

/// A fresh scratch directory under the system temp dir (audit A4; shared since the 2026-10-08 panel).
/// `create_dir`, never `create_dir_all`: an existing path, a symlink another user planted at a
/// guessed name included, is never reused — the name is retried instead. Owner-only on unix.
pub(crate) fn private_temp_dir(prefix: &str) -> Result<std::path::PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    #[cfg(unix)]
    let builder = {
        let mut b = std::fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
        b
    };
    #[cfg(not(unix))]
    let builder = std::fs::DirBuilder::new();
    // The clock only makes names harder to guess; uniqueness and safety come from the exclusive
    // create below, so a clock set before 1970 just yields a guessable name that still never reuses.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for _ in 0..16 {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("{prefix}-{}-{nanos:x}-{seq}", std::process::id()));
        match builder.create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot create a temp dir: {e}")),
        }
    }
    Err(format!(
        "cannot create a temp dir: 16 candidate names under {} already exist",
        std::env::temp_dir().display()
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_temp_dirs_are_fresh_and_owner_only() {
        let a = super::private_temp_dir("phorj-tempdir-test").unwrap();
        let b = super::private_temp_dir("phorj-tempdir-test").unwrap();
        assert_ne!(a, b, "every call gets its own directory");
        assert!(a.is_dir() && b.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&a).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "owner-only, got {mode:o}");
        }
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }
}
