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
    let git = std::env::var("PHORJ_GIT").unwrap_or_else(|_| "git".into());
    run_git(&git, &["clone", "--quiet", url], dest, true)?;
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

fn run_git(git: &str, args: &[&str], dest: &Path, is_clone: bool) -> Result<(), String> {
    let mut cmd = Command::new(git);
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
    let out = Command::new(git)
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
}
