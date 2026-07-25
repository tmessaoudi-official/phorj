use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let unique = N.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("phorj_loader_test_{}_{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, contents).unwrap();
        p
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Cohesion split (M-Decomp): the test suite is grouped by topic into sibling files; the shared
// `TempDir` harness lives here and every topic module reaches it (and the loader surface) via
// `use super::*;`.
mod decl_files;
mod imports;
mod loose;
mod member_function_imports;
mod project_structure;
mod public_surface;
mod visibility;
