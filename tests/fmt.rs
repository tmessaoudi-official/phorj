//! `phg fmt` CLI integration tests (M-fmt F3) — in-place write, `--check` exit codes, parse-error
//! safety (file untouched, exit 2), and idempotence at the CLI layer.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use phorj::cli;

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let unique = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "phorj_fmt_it_{tag}_{}_{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::write(&p, contents).unwrap();
        p
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const MESSY: &str = "package Main;import Core.Output;\nfunction  main()->void{int  x=1+2 ;Output.printLine(\"{x}\");}\n";

#[test]
fn check_reports_unformatted_then_clean_after_write() {
    let d = TempDir::new("check");
    let f = d.write("a.phg", MESSY);

    // --check on a messy file: exit 1, no write.
    let (report, code) = cli::cmd_fmt(&[f.display().to_string()], true);
    assert_eq!(code, 1, "{report}");
    assert!(report.contains("would reformat"), "{report}");
    assert_eq!(
        std::fs::read_to_string(&f).unwrap(),
        MESSY,
        "check must not write"
    );

    // format in place: exit 0, file rewritten.
    let (report, code) = cli::cmd_fmt(&[f.display().to_string()], false);
    assert_eq!(code, 0, "{report}");
    let formatted = std::fs::read_to_string(&f).unwrap();
    assert_ne!(formatted, MESSY);
    assert!(formatted.contains("int x = 1 + 2;"), "{formatted}");

    // now --check is clean: exit 0.
    let (_r, code) = cli::cmd_fmt(&[f.display().to_string()], true);
    assert_eq!(code, 0);
}

#[test]
fn unparseable_file_is_left_untouched_exit_2() {
    let d = TempDir::new("broken");
    let broken = "package Main;\nfunction (\n";
    let f = d.write("bad.phg", broken);
    let (report, code) = cli::cmd_fmt(&[f.display().to_string()], false);
    assert_eq!(code, 2, "{report}");
    assert!(report.contains("did not parse"), "{report}");
    assert_eq!(
        std::fs::read_to_string(&f).unwrap(),
        broken,
        "a broken file must never be rewritten"
    );
}

#[test]
fn directory_formats_all_phg_recursively() {
    let d = TempDir::new("dir");
    d.write("a.phg", MESSY);
    d.write("b.phg", MESSY);
    let (report, code) = cli::cmd_fmt(&[d.path().display().to_string()], false);
    assert_eq!(code, 0, "{report}");
    assert!(report.contains("2 file(s) formatted"), "{report}");
}

#[test]
fn stdin_path_formats_a_source_string() {
    let out = cli::fmt_source(MESSY).expect("formats");
    assert!(out.contains("function main(): void {"), "{out}");
    // idempotent at the source level.
    assert_eq!(out, cli::fmt_source(&out).unwrap());
}

/// Recursively collect every `*.phg` under `dir`.
fn collect_phg(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_phg(&p, out);
            } else if p.extension().is_some_and(|x| x == "phg") {
                out.push(p);
            }
        }
    }
}

/// F4 dogfood: the formatter must handle **every** real `.phg` in the repo — format without error,
/// be idempotent, and (for a program that runs as a standalone single file) preserve its behavior.
/// This is the meaning-preservation gate on real code; it also guards against any future construct
/// the printer doesn't cover.
#[test]
fn every_repo_phg_formats_idempotently_and_safely() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_phg(&root.join("examples"), &mut files);
    collect_phg(&root.join("selftest"), &mut files);
    files.sort();
    assert!(
        files.len() > 20,
        "expected the repo's example corpus, found {}",
        files.len()
    );

    // The per-file work (two `cmd_treewalk` pipelines, each on a deep-stack worker thread) is CPU-bound
    // and independent per file — and this single test otherwise dominates the whole suite's
    // wall-clock. Fan the corpus out across the cores with std scoped threads (no new dependency).
    // A failing file panics inside its worker; the panic propagates on scope join, so the test still
    // fails with the offending path in the message.
    let n = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);
    let chunk = files.len().div_ceil(n).max(1);
    std::thread::scope(|s| {
        for group in files.chunks(chunk) {
            s.spawn(move || {
                for f in group {
                    assert_phg_fmt_safe(f);
                }
            });
        }
    });
}

/// Format-safety checks for one `.phg`: it formats without error, is idempotent, and (for a
/// standalone-runnable program) preserves behavior. Split out of the corpus sweep above so that
/// sweep can fan out across threads.
fn assert_phg_fmt_safe(f: &Path) {
    let src = std::fs::read_to_string(f).unwrap();
    // Formats without error.
    let once =
        cli::fmt_source(&src).unwrap_or_else(|e| panic!("fmt failed on {}:\n{e}", f.display()));
    // Canonical: every tracked `.phg` is already in width-canonical form, so `fmt(src) == src`
    // (UA-0.8 — the corpus test used to be idempotency-only, letting tracked files silently drift).
    assert_eq!(
        src,
        once,
        "not width-canonical (run `phg format {}`): fmt(src) != src",
        f.display()
    );
    // Idempotent.
    let twice = cli::fmt_source(&once).unwrap();
    assert_eq!(once, twice, "not idempotent: {}", f.display());
    // Meaning preserved for a standalone-runnable program (skip multi-file project parts /
    // impure / fragment files, which don't run as a single source).
    let before = cli::cmd_treewalk(&src);
    if before.is_ok() {
        let after = cli::cmd_treewalk(&once);
        assert_eq!(before, after, "fmt changed behavior of {}", f.display());
    }
}
