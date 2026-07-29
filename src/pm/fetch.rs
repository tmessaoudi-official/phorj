//! Low-level fetchers (DEC-316): materialize a package's source tree from a local path or a git repo,
//! and compute its integrity hash. Std-only — git is a host-tool exemption (`PHORJ_GIT` overrides),
//! exactly like `bundle::cross.rs` shelling to `curl`. Registry deps resolve to a git URL first
//! (`pm::registry`) then come through [`fetch_git`], so there are only two real fetch mechanisms.

use crate::bundle::sha256::sha256_hex;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A materialized package: where its tree now lives + the git commit it came from (none for a path
/// dep) + the integrity hash of the tree.
pub struct Fetched {
    pub dir: PathBuf,
    pub commit: Option<String>,
    pub hash: String,
}

/// Copy a local directory dependency into `dest` (dev workflow). `src` is resolved relative to the
/// manifest's directory. Excludes `.git` and any nested `vendor/` (deps are re-resolved, never nested).
pub fn fetch_path(manifest_dir: &Path, rel: &str, dest: &Path) -> Result<Fetched, String> {
    let src = manifest_dir.join(rel);
    let src = src
        .canonicalize()
        .map_err(|e| format!("path dependency `{rel}` not found: {e}"))?;
    if !src.is_dir() {
        return Err(format!("path dependency `{rel}` is not a directory"));
    }
    copy_tree(&src, dest)?;
    let hash = tree_hash(dest)?;
    Ok(Fetched {
        dir: dest.to_path_buf(),
        commit: None,
        hash,
    })
}

/// Clone `url` at `git_ref` into `dest`, resolve the exact commit, and strip `.git` (the vendored tree
/// is source only). `url` may be `https://…`, `file://…`, or a bare local path (git handles all three).
pub fn fetch_git(url: &str, git_ref: &str, dest: &Path) -> Result<Fetched, String> {
    validate_git_target(url, git_ref)?;
    let git = std::env::var("PHORJ_GIT").unwrap_or_else(|_| "git".into());
    // `--` ends option parsing so a hostile URL cannot become a flag. (Deliberately NOT used on the
    // `checkout` below: `git checkout -- <x>` means "restore this PATH", not "check out this ref", so
    // the separator would change the verb's meaning. The leading-dash rejection covers the ref.)
    run_git(&git, &["clone", "--quiet", "--", url], dest, true)?;
    run_git(
        &git,
        &["-C", dest_str(dest)?, "checkout", "--quiet", git_ref],
        dest,
        false,
    )?;
    let commit = capture_git(&git, &["-C", dest_str(dest)?, "rev-parse", "HEAD"])?;
    // Strip VCS metadata so the vendored tree is pure source (and hashes stably).
    let _ = std::fs::remove_dir_all(dest.join(".git"));
    let hash = tree_hash(dest)?;
    Ok(Fetched {
        dir: dest.to_path_buf(),
        commit: Some(commit.trim().to_string()),
        hash,
    })
}

/// SHA-256 over the tree's sorted `(relative-path, length, bytes)` stream — order-independent and
/// content-addressing (the `phorj.lock` integrity pin; reuses `bundle::sha256`).
pub fn tree_hash(dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files)?;
    files.sort();
    let mut buf: Vec<u8> = Vec::new();
    for rel in &files {
        buf.extend_from_slice(rel.as_bytes());
        buf.push(0);
        let bytes = std::fs::read(dir.join(rel)).map_err(|e| format!("hash read {rel}: {e}"))?;
        buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(&bytes);
    }
    Ok(sha256_hex(&buf))
}

/// Recursively copy `src` → `dst`, skipping `.git` and nested `vendor/`.
pub fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read_dir {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let name = entry.file_name();
        if name == ".git" || name == "vendor" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let path = entry.path();
        if entry.file_name() == ".git" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| format!("strip prefix: {e}"))?
                .to_string_lossy()
                .replace('\\', "/"); // stable across platforms
            out.push(rel);
        }
    }
    Ok(())
}

fn dest_str(dest: &Path) -> Result<&str, String> {
    dest.to_str()
        .ok_or_else(|| "non-utf8 destination path".to_string())
}

/// Reject the git argument/transport-injection shapes BEFORE any `git` process is spawned (Q28 /
/// DEC-414 — a re-port of the retired `phg vendor` path's verified property **P6**, which the DEC-316
/// package manager did not inherit; see `2026-07-03-unification-audit/raw/A7-security.md:169`).
///
/// The threat is concrete: `url` and `git_ref` come from a `phorj.json` dependency spec, i.e. from
/// whatever repository a user is asked to `phg install`. Git's `ext::` remote helper **runs a shell
/// command**, so `git = "ext::sh -c 'curl … | sh'"` would be arbitrary code execution at install time;
/// a leading `-` on either field turns it into a `git` flag (`--upload-pack=…` is the classic).
///
/// `ext::`/`file::` are the double-colon REMOTE-HELPER forms and are matched case-insensitively.
/// The `file://` transport URL and bare local paths are deliberately still allowed — `fetch_git`
/// documents them as supported, and hermetic tests use them.
pub(crate) fn validate_git_target(url: &str, git_ref: &str) -> Result<(), String> {
    for (what, v) in [("git url", url), ("git ref", git_ref)] {
        if v.starts_with('-') {
            return Err(format!(
                "refusing this {what}: it starts with `-`, which git would read as a command-line \
                 flag rather than a value ({v:?})"
            ));
        }
        if v.is_empty() {
            return Err(format!("refusing an empty {what}"));
        }
    }
    let lower = url.to_ascii_lowercase();
    for helper in ["ext::", "file::"] {
        if lower.starts_with(helper) {
            return Err(format!(
                "refusing this git url: `{helper}` is a git REMOTE HELPER, not a transport — `ext::` \
                 executes a shell command, so a dependency spec could run arbitrary code at install \
                 time ({url:?}). Use an https/ssh/git URL, or a plain local path."
            ));
        }
    }
    Ok(())
}

/// Config + environment hardening applied to EVERY `git` invocation (Q28 / P6): disable the `ext`
/// protocol at the config level as defence in depth behind [`validate_git_target`]'s string check,
/// and scrub the inherited `GIT_*` environment so an ambient `GIT_SSH_COMMAND`, `GIT_CONFIG_*`,
/// `GIT_PROXY_COMMAND`, … cannot redirect or hijack the fetch.
fn harden(cmd: &mut Command) {
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("GIT_") {
            cmd.env_remove(&k);
        }
    }
}

/// The leading args every invocation carries (see [`harden`]): `-c protocol.ext.allow=never`.
const GIT_HARDENING_ARGS: [&str; 2] = ["-c", "protocol.ext.allow=never"];

fn run_git(git: &str, args: &[&str], dest: &Path, is_clone: bool) -> Result<(), String> {
    let mut cmd = Command::new(git);
    harden(&mut cmd);
    cmd.args(GIT_HARDENING_ARGS);
    cmd.args(args);
    if is_clone {
        cmd.arg(dest_str(dest)?);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run `{git}` (needed to fetch git dependencies): {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

fn capture_git(git: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(git);
    harden(&mut cmd);
    let out = cmd
        .args(GIT_HARDENING_ARGS)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run `{git}`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("git output not utf-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("phorj_pm_fetch_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn path_fetch_copies_and_hashes_stably() {
        let src = tmp("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.phg"), b"package Acme.Util;").unwrap();
        std::fs::write(src.join("sub/b.phg"), b"x").unwrap();
        std::fs::create_dir_all(src.join(".git")).unwrap();
        std::fs::write(src.join(".git/HEAD"), b"ref").unwrap();

        let dest = tmp("dest");
        let manifest_dir = src.parent().unwrap().to_path_buf();
        let rel = src.file_name().unwrap().to_str().unwrap();
        let f = fetch_path(&manifest_dir, rel, &dest).unwrap();

        assert!(dest.join("a.phg").exists());
        assert!(dest.join("sub/b.phg").exists());
        assert!(!dest.join(".git").exists()); // .git excluded
        assert!(f.commit.is_none());
        assert_eq!(f.hash.len(), 64); // sha-256 hex

        // Re-hashing the same tree is identical; a content change flips it.
        assert_eq!(tree_hash(&dest).unwrap(), f.hash);
        std::fs::write(dest.join("a.phg"), b"package Acme.Util; // changed").unwrap();
        assert_ne!(tree_hash(&dest).unwrap(), f.hash);

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn path_fetch_rejects_missing() {
        let dest = tmp("nope_dest");
        assert!(fetch_path(Path::new("/nonexistent"), "missing", &dest).is_err());
    }

    // ── Q28 / DEC-414: the re-ported P6 git argument/transport hardening ──────────────────────────
    // These are the shapes a hostile `phorj.json` dependency spec could carry. `ext::` is the sharp
    // one: git's ext remote helper RUNS A SHELL COMMAND, so accepting it is code execution at
    // install time.

    #[test]
    fn git_url_naming_the_ext_remote_helper_is_refused() {
        let err = validate_git_target("ext::sh -c 'echo pwned'", "v1").unwrap_err();
        assert!(err.contains("REMOTE HELPER"), "{err}");
        // …case-insensitively, so `EXT::` cannot slip past.
        assert!(validate_git_target("EXT::sh -c 'x'", "v1").is_err());
    }

    #[test]
    fn git_url_naming_the_file_remote_helper_is_refused() {
        assert!(validate_git_target("file::/etc/passwd", "v1").is_err());
        assert!(validate_git_target("FILE::/tmp/x", "v1").is_err());
    }

    #[test]
    fn a_leading_dash_in_the_url_or_ref_is_refused() {
        // Either would be parsed by git as a FLAG rather than a value.
        let e1 = validate_git_target("--upload-pack=touch /tmp/pwned", "v1").unwrap_err();
        assert!(e1.contains("starts with `-`"), "{e1}");
        let e2 = validate_git_target("https://example.com/r.git", "--upload-pack=x").unwrap_err();
        assert!(e2.contains("starts with `-`"), "{e2}");
    }

    #[test]
    fn empty_url_or_ref_is_refused() {
        assert!(validate_git_target("", "v1").is_err());
        assert!(validate_git_target("https://example.com/r.git", "").is_err());
    }

    #[test]
    fn legitimate_transports_and_local_paths_still_pass() {
        // `file://` (the TRANSPORT) and bare paths are documented as supported and must keep working —
        // only the double-colon HELPER forms are refused.
        for url in [
            "https://example.com/acme/pkg.git",
            "ssh://git@example.com/acme/pkg.git",
            "git://example.com/acme/pkg.git",
            "file:///srv/git/pkg.git",
            "/srv/git/pkg.git",
            "../sibling-repo",
        ] {
            assert!(validate_git_target(url, "v1.2.3").is_ok(), "rejected {url}");
        }
    }

    #[test]
    fn fetch_git_applies_the_guard_before_spawning_git() {
        // Proves the validator is WIRED IN, not merely present: the error must be ours, and it must
        // arrive without git having run (a `git` error would read "git clone failed: …").
        let dest = tmp("guard_dest");
        // `match` rather than `unwrap_err()`: that would force a `Debug` impl on the public
        // `Fetched` struct purely for a test.
        let err = match fetch_git("ext::sh -c 'echo pwned'", "v1", &dest) {
            Ok(_) => panic!("the guard did not fire — a hostile url was accepted"),
            Err(e) => e,
        };
        assert!(err.contains("REMOTE HELPER"), "{err}");
        assert!(
            !err.contains("git clone failed"),
            "git was spawned anyway: {err}"
        );
    }
}
