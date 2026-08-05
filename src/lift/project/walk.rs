//! Directory lift — the WALK: which directories are entered, and what counts as a PHP file.
//!
//! Split from `discover.rs` (Invariant 13) along the cohesion line that matters: that module answers "what
//! did composer DECLARE", this one answers "what does the filesystem actually HOLD". The two questions have
//! different failure modes — a wrong answer there mis-scopes the lift, a wrong answer here fails to
//! terminate.

use std::path::{Path, PathBuf};

/// Directories never walked, whatever `composer.json` says. `vendor/` is the important one: lifting a
/// dependency tree is explicitly NOT what a directory lift does (DEC-439 — vendor is reported, and
/// optionally stubbed, never forked).
const NEVER_WALK: &[&str] = &["vendor", "node_modules", ".git", ".github", "var", "cache"];

/// Directory-nesting bound for both walks — a backstop only. The real cycle defence is [`is_real_dir`]:
/// a depth cap alone does NOT save you, because a symlinked cycle re-walks the whole subtree at every
/// level, so 64 levels is exponential rather than merely deep. [Verified: with the cap alone and
/// `src/up -> ..`, the lift had to be killed at 30s.] 64 is far past any real source layout.
const MAX_WALK_DEPTH: usize = 64;

/// A directory that is NOT a symlink.
///
/// Symlinked directories are skipped rather than followed, which is what makes a cyclic tree
/// (`src/up -> ..`) terminate at all. It also avoids lifting the same file twice under two paths — a
/// symlinked source tree is ordinary in PHP projects (shared library dirs, `vendor` overlays), and the
/// duplicate would then collide on its destination and be renamed for no reason.
pub(super) fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

/// Is this file PHP? By EXTENSION or by CONTENT — the OR is load-bearing in both directions: `bin/console`
/// and Laravel's `artisan` have no extension for a filter to match, while a `<?=`-only short-tag file has no
/// `<?php` for a content check to find. [Verified against six shapes: `artisan`, `console`, `plain.php`
/// detected; a `.txt` and a binary correctly rejected; the short-tag file caught by the extension branch.]
pub(super) fn is_php_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("php") || starts_with_php_open_tag(path)
}

/// Does this file open with `<?php` (allowing a leading `#!` shebang line)? Reads a small prefix only.
fn starts_with_php_open_tag(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let head = &bytes[..bytes.len().min(256)];
    let Ok(text) = std::str::from_utf8(head) else {
        return false;
    };
    // A shebang may precede the open tag (`#!/usr/bin/env php`), so look within the prefix rather than
    // requiring the very first bytes.
    text.contains("<?php")
}

/// Like [`walk`] but collects EVERY file, not just `.php` ones — the caller decides PHP-ness by content.
pub(super) fn walk_any(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?;
    let mut names: Vec<PathBuf> = Vec::new();
    for e in entries {
        names.push(
            e.map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?
                .path(),
        );
    }
    names.sort();
    for path in names {
        if is_real_dir(&path) {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| NEVER_WALK.contains(&n));
            if !skip {
                walk_any(&path, out, depth + 1)?;
            }
        } else if !path.is_dir() {
            // A symlink TO a directory lands here and is correctly not collected as a file.
            out.push(path);
        }
    }
    Ok(())
}

pub(super) fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?;
    // Collected then sorted: `read_dir` order is filesystem-dependent, and a lift that varies with it
    // would produce different reports on different machines for the same input.
    let mut names: Vec<PathBuf> = Vec::new();
    for e in entries {
        names.push(
            e.map_err(|e| format!("cannot read `{}`: {e}", dir.display()))?
                .path(),
        );
    }
    names.sort();
    for path in names {
        if is_real_dir(&path) {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| NEVER_WALK.contains(&n));
            if !skip {
                walk(&path, out, depth + 1)?;
            }
        } else if !path.is_dir() && is_php_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}
